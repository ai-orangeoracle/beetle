//! 飞书长连接入站：HTTP 取 wss URL，建 WSS，收 protobuf 帧，解析 EVENT 入队。
//! 仅 ESP + feature feishu 时编译；与 POST /api/feishu/event HTTP 回调并存。
//! 委托 wss_gateway 统一循环，本模块实现 FeishuWssDriver。

#![cfg(all(
    feature = "feishu",
    any(target_arch = "xtensa", target_arch = "riscv32")
))]

use crate::bus::InboundTx;
use crate::channels::ChannelHttpClient;
use crate::channels::wss_gateway::{
    run_wss_gateway_loop, connect_esp_wss, WssGatewayDriver, WssRecvAction, WssSessionState,
};
use crate::error::{Error, Result};
use crate::platform::EspHttpClient;
use prost::Message;

use super::frame::pbbp2;
use super::send::event_body_to_pcmsg;

const TAG: &str = "feishu_ws";
const FEISHU_WS_ENDPOINT: &str = "https://open.feishu.cn/callback/ws/endpoint";
const FRAME_METHOD_CONTROL: i32 = 0;
const FRAME_METHOD_DATA: i32 = 1;
const HEADER_TYPE: &str = "type";
const MESSAGE_TYPE_EVENT: &str = "event";
const FEISHU_HEARTBEAT_MS: u64 = 120_000;

pub fn get_ws_url(
    http: &mut dyn ChannelHttpClient,
    app_id: &str,
    app_secret: &str,
) -> Result<String> {
    #[derive(serde::Serialize)]
    struct Req {
        #[serde(rename = "AppID")]
        app_id: String,
        #[serde(rename = "AppSecret")]
        app_secret: String,
    }
    #[derive(serde::Deserialize)]
    struct Data {
        #[serde(rename = "URL")]
        url: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct Resp {
        data: Option<Data>,
        #[allow(dead_code)]
        code: Option<i32>,
    }
    let body = Req {
        app_id: app_id.to_string(),
        app_secret: app_secret.to_string(),
    };
    let body_bytes = serde_json::to_vec(&body).map_err(|e| Error::Other {
        source: Box::new(e),
        stage: "feishu_ws_endpoint",
    })?;
    let (status, resp_body) = http
        .http_post(FEISHU_WS_ENDPOINT, &body_bytes)
        .map_err(|e| Error::Other {
            source: Box::new(e),
            stage: "feishu_ws_endpoint",
        })?;
    if status >= 400 {
        return Err(Error::Http {
            status_code: status,
            stage: "feishu_ws_endpoint",
        });
    }
    let r: Resp = serde_json::from_slice(resp_body.as_ref()).map_err(|e| Error::Other {
        source: Box::new(e),
        stage: "feishu_ws_endpoint",
    })?;
    let url = r
        .data
        .and_then(|d| d.url)
        .filter(|u| u.starts_with("wss://") || u.starts_with("ws://"));
    url.ok_or_else(|| Error::Other {
        source: Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "feishu endpoint response missing data.URL",
        )),
        stage: "feishu_ws_endpoint",
    })
}

fn encode_control_frame(
    header_type: &str,
    log_id: u64,
    log_id_new: &str,
) -> Result<Vec<u8>> {
    let frame = pbbp2::Frame {
        seq_id: 0,
        log_id,
        service: 0,
        method: FRAME_METHOD_CONTROL,
        headers: vec![pbbp2::Header {
            key: HEADER_TYPE.to_string(),
            value: header_type.to_string(),
        }],
        payload_encoding: String::new(),
        payload_type: String::new(),
        payload: Vec::new(),
        log_id_new: log_id_new.to_string(),
    };
    let mut buf = Vec::with_capacity(64);
    frame.encode(&mut buf).map_err(|e| Error::Other {
        source: Box::new(e),
        stage: "feishu_ws_frame_encode",
    })?;
    Ok(buf)
}

fn build_ping_frame() -> Result<Vec<u8>> {
    log::debug!("[{}] build ping frame", TAG);
    encode_control_frame("ping", 0, "")
}

/// 飞书 WSS 协议驱动：取 URL、无 Hello、pbbp2 解析、pbbp2 ping 心跳。
struct FeishuWssDriver {
    app_id: String,
    app_secret: String,
    allowed_chat_ids: Vec<String>,
}

impl WssGatewayDriver for FeishuWssDriver {
    fn get_url(&mut self, http: &mut dyn ChannelHttpClient) -> Result<String> {
        get_ws_url(http, &self.app_id, &self.app_secret)
    }

    fn on_hello(&mut self, _first_message: &[u8]) -> Result<WssSessionState> {
        Ok(WssSessionState {
            heartbeat_interval_ms: FEISHU_HEARTBEAT_MS,
            identify_payload: None,
        })
    }

    fn on_recv(&mut self, data: &[u8]) -> Result<WssRecvAction> {
        let frame = match pbbp2::Frame::decode(bytes::Bytes::copy_from_slice(data)) {
            Ok(f) => f,
            Err(_) => {
                log::debug!("[{}] pbbp2 decode ignore", TAG);
                return Ok(WssRecvAction::Ignore);
            }
        };
        if frame.method == FRAME_METHOD_CONTROL {
            log::debug!("[{}] recv control frame", TAG);
            return Ok(WssRecvAction::Ignore);
        }
        if frame.method != FRAME_METHOD_DATA {
            log::debug!("[{}] recv method={} ignore", TAG, frame.method);
            return Ok(WssRecvAction::Ignore);
        }
        let type_val = frame
            .headers
            .iter()
            .find(|h| h.key == HEADER_TYPE)
            .map(|h| h.value.as_str())
            .unwrap_or("");
        if type_val != MESSAGE_TYPE_EVENT || frame.payload.is_empty() {
            log::debug!("[{}] recv type={} ignore", TAG, type_val);
            return Ok(WssRecvAction::Ignore);
        }
        let payload_str = match std::str::from_utf8(&frame.payload) {
            Ok(s) => s,
            Err(_) => return Ok(WssRecvAction::Ignore),
        };
        log::info!("[{}] event frame received, payload_len={}", TAG, frame.payload.len());
        let msg = event_body_to_pcmsg(payload_str, &self.allowed_chat_ids);
        let ack = encode_control_frame("reply", frame.log_id, &frame.log_id_new)?;
        Ok(WssRecvAction::DispatchAndAck(msg, ack))
    }

    fn build_heartbeat(&self, _seq: Option<u64>) -> Result<Vec<u8>> {
        build_ping_frame()
    }
}

/// 长连接循环：委托 run_wss_gateway_loop，使用 FeishuWssDriver 与 ESP 连接。
pub fn run_feishu_ws_loop(
    app_id: String,
    app_secret: String,
    allowed_chat_ids: Vec<String>,
    inbound_tx: InboundTx,
) {
    let driver = FeishuWssDriver {
        app_id,
        app_secret,
        allowed_chat_ids,
    };
    let create_http = || EspHttpClient::new();
    let connect = |url: &str| connect_esp_wss(url);
    run_wss_gateway_loop(TAG, driver, inbound_tx, create_http, connect);
}
