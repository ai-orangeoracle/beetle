//! QQ 频道 WSS 入站：取 gateway URL → Hello/Identify → 心跳 → Dispatch(AT_MESSAGE_CREATE) 入队。
//! 仅 ESP 编译；与 HTTP webhook 可并存，由 main 按配置决定是否 spawn。

#![cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]

use crate::bus::PcMsg;
use crate::channels::ChannelHttpClient;
use crate::channels::wss_gateway::{
    connect_esp_wss, run_wss_gateway_loop, WssGatewayDriver, WssRecvAction, WssSessionState,
};
use crate::error::{Error, Result};
use crate::platform::EspHttpClient;

use super::send::{QqTokenRequest, QqTokenResponse, QQ_GET_APP_ACCESS_TOKEN_URL};

const TAG: &str = "qq_ws";
const QQ_GATEWAY_URL: &str = "https://api.sgroup.qq.com/gateway";
const QQ_OP_HELLO: u64 = 10;
const QQ_OP_IDENTIFY: u64 = 2;
const QQ_OP_DISPATCH: u64 = 0;
const QQ_OP_HEARTBEAT: u64 = 1;
const QQ_OP_HEARTBEAT_ACK: u64 = 11;
const QQ_OP_RECONNECT: u64 = 7;
const QQ_OP_INVALID_SESSION: u64 = 9;
const AT_MESSAGE_CREATE: &str = "AT_MESSAGE_CREATE";
const PUBLIC_GUILD_MESSAGES_INTENT: u64 = 1 << 30;

fn get_qq_access_token<H: ChannelHttpClient + ?Sized>(http: &mut H, app_id: &str, client_secret: &str) -> Result<String> {
    let body = QqTokenRequest {
        app_id: app_id.to_string(),
        client_secret: client_secret.to_string(),
    };
    let body_bytes = serde_json::to_vec(&body).map_err(|e| Error::Other {
        source: Box::new(e),
        stage: "qq_ws_token",
    })?;
    let (status, resp_body) = http.http_post(QQ_GET_APP_ACCESS_TOKEN_URL, &body_bytes).map_err(|e| Error::Other {
        source: Box::new(e),
        stage: "qq_ws_token",
    })?;
    if status >= 400 {
        return Err(Error::Http {
            status_code: status,
            stage: "qq_ws_token",
        });
    }
    let r: QqTokenResponse = serde_json::from_slice(resp_body.as_ref()).map_err(|e| Error::Other {
        source: Box::new(e),
        stage: "qq_ws_token",
    })?;
    r.access_token
        .filter(|t| !t.is_empty())
        .ok_or_else(|| Error::Other {
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "qq_ws no access_token",
            )),
            stage: "qq_ws_token",
        })
}

fn get_gateway_url<H: ChannelHttpClient + ?Sized>(http: &mut H, token: &str) -> Result<String> {
    let auth = format!("QQBot {}", token);
    let headers = [("Authorization", auth.as_str())];
    let (status, resp_body) = http
        .http_get_with_headers(QQ_GATEWAY_URL, &headers)
        .map_err(|e| Error::Other {
            source: Box::new(e),
            stage: "qq_ws_gateway",
        })?;
    if status >= 400 {
        return Err(Error::Http {
            status_code: status,
            stage: "qq_ws_gateway",
        });
    }
    #[derive(serde::Deserialize)]
    struct GatewayResp {
        url: Option<String>,
    }
    let r: GatewayResp = serde_json::from_slice(resp_body.as_ref()).map_err(|e| Error::Other {
        source: Box::new(e),
        stage: "qq_ws_gateway",
    })?;
    r.url
        .filter(|u| u.starts_with("wss://") || u.starts_with("ws://"))
        .ok_or_else(|| Error::Other {
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "qq_ws gateway missing url",
            )),
            stage: "qq_ws_gateway",
        })
}

/// QQ WSS 协议驱动：取 token + GET gateway、Hello(op=10)、Identify(op=2)、心跳(op=1)、Dispatch 解析。
struct QqWssDriver {
    app_id: String,
    client_secret: String,
    cached_token: Option<String>,
    last_seq: Option<u64>,
}

impl QqWssDriver {
    fn new(app_id: String, client_secret: String) -> Self {
        Self {
            app_id,
            client_secret,
            cached_token: None,
            last_seq: None,
        }
    }
}

impl WssGatewayDriver for QqWssDriver {
    fn get_url(&mut self, http: &mut dyn ChannelHttpClient) -> Result<String> {
        let token = get_qq_access_token(http, &self.app_id, &self.client_secret)?;
        log::debug!("[{}] token obtained", TAG);
        self.cached_token = Some(token.clone());
        let url = get_gateway_url(http, &token)?;
        log::debug!("[{}] gateway url len={}", TAG, url.len());
        Ok(url)
    }

    fn expects_hello(&self) -> bool {
        true
    }

    fn on_hello(&mut self, first_message: &[u8]) -> Result<WssSessionState> {
        let value: serde_json::Value = serde_json::from_slice(first_message).map_err(|e| Error::Other {
            source: Box::new(e),
            stage: "qq_ws_hello",
        })?;
        let op = value.get("op").and_then(|v| v.as_u64()).unwrap_or(0);
        if op != QQ_OP_HELLO {
            return Ok(WssSessionState {
                heartbeat_interval_ms: 45_000,
                identify_payload: None,
            });
        }
        let interval = value
            .get("d")
            .and_then(|d| d.get("heartbeat_interval"))
            .and_then(|v| v.as_u64())
            .unwrap_or(45_000);
        let identify_payload = self
            .cached_token
            .as_ref()
            .and_then(|token| {
                let d = serde_json::json!({
                    "token": format!("QQBot {}", token),
                    "intents": PUBLIC_GUILD_MESSAGES_INTENT,
                    "shard": [0u64, 1u64],
                    "properties": { "$os": "linux", "$browser": "my_library", "$device": "my_library" }
                });
                serde_json::to_vec(&serde_json::json!({ "op": QQ_OP_IDENTIFY, "d": d }))
                    .map_err(|e| log::error!("[{}] identify serialize failed: {}", TAG, e))
                    .ok()
            })
            .filter(|v| !v.is_empty());
        log::info!("[{}] hello ok, heartbeat_interval_ms={}", TAG, interval);
        Ok(WssSessionState {
            heartbeat_interval_ms: interval,
            identify_payload,
        })
    }

    fn on_recv(&mut self, data: &[u8]) -> Result<WssRecvAction> {
        let value: serde_json::Value = match serde_json::from_slice(data) {
            Ok(v) => v,
            Err(_) => return Ok(WssRecvAction::Ignore),
        };
        let op = value.get("op").and_then(|v| v.as_u64()).unwrap_or(99);
        let s = value.get("s").and_then(|v| v.as_u64());
        if let Some(seq) = s {
            self.last_seq = Some(seq);
        }
        log::debug!("[{}] recv op={} s={:?}", TAG, op, s);
        match op {
            QQ_OP_DISPATCH => {
                let t = value.get("t").and_then(|v| v.as_str()).unwrap_or("");
                log::debug!("[{}] dispatch t={}", TAG, t);
                if t == AT_MESSAGE_CREATE {
                    let d = value.get("d").and_then(|v| v.as_object());
                    if let Some(d) = d {
                        let channel_id = d.get("channel_id").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let content = d.get("content").and_then(|v| v.as_str()).map(|s| s.to_string());
                        if let (Some(ch), Some(content)) = (channel_id, content) {
                            if !ch.is_empty() && !content.is_empty() {
                                if let Ok(msg) = PcMsg::new("qq_channel", ch, content) {
                                    return Ok(WssRecvAction::Dispatch(Some(msg)));
                                }
                            }
                        }
                    }
                }
                Ok(WssRecvAction::Dispatch(None))
            }
            QQ_OP_HEARTBEAT_ACK => Ok(WssRecvAction::SendHeartbeat(self.last_seq.unwrap_or(0))),
            QQ_OP_RECONNECT => {
                log::info!("[{}] server requested reconnect", TAG);
                Ok(WssRecvAction::Disconnect)
            }
            QQ_OP_INVALID_SESSION => {
                log::warn!("[{}] invalid session", TAG);
                Ok(WssRecvAction::Disconnect)
            }
            _ => Ok(WssRecvAction::Ignore),
        }
    }

    fn build_heartbeat(&self, seq: Option<u64>) -> Result<Vec<u8>> {
        let d = seq.unwrap_or(0);
        log::debug!("[{}] build_heartbeat seq={}", TAG, d);
        serde_json::to_vec(&serde_json::json!({ "op": QQ_OP_HEARTBEAT, "d": d })).map_err(|e| Error::Other {
            source: Box::new(e),
            stage: "qq_ws_heartbeat",
        })
    }
}

/// 长连接循环：委托 run_wss_gateway_loop，使用 QqWssDriver 与 ESP 连接。
pub fn run_qq_ws_loop(app_id: String, client_secret: String, inbound_tx: crate::bus::InboundTx) {
    let driver = QqWssDriver::new(app_id, client_secret);
    let create_http = || EspHttpClient::new();
    let connect = |url: &str| connect_esp_wss(url);
    run_wss_gateway_loop(TAG, driver, inbound_tx, create_http, connect);
}