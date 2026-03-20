//! QQ 入站 HTTP 回调：op=13 验址、op=0 Ed25519 验签，支持 AT_MESSAGE_CREATE / GROUP_AT_MESSAGE_CREATE / C2C_MESSAGE_CREATE 入队。

use super::send::{sign_qq_url_verify, verify_qq_signature, QqMsgIdCache};
use crate::bus::{InboundTx, PcMsg};
use crate::error::{Error, Result};

/// Body 最大字节（拒绝超长请求）。与 http_server 读 body 上限一致，单一数据源。
pub const QQ_WEBHOOK_BODY_MAX: usize = 64 * 1024;

/// 入站处理结果，供 HTTP 层写响应。
pub enum QqHandlerResult {
    /// op=13：需返回 200 且 body {"plain_token":"...","signature":"..."}
    UrlVerification {
        plain_token: String,
        signature: String,
    },
    /// 已处理（含 op=0 或其他 op），返回 200 空 body 或 ACK。
    EventHandled,
}

/// 处理 QQ 回调 body，完成验签/解析/入队；不读 HTTP Header，由调用方传入 timestamp 与 signature。
/// 返回 Ok(result) 时由调用方写 200；Err 时写 401/413 等。
pub fn handle_webhook(
    body: &[u8],
    signature_timestamp: Option<&str>,
    signature_ed25519: Option<&str>,
    _app_id: &str,
    secret: &str,
    inbound_tx: &InboundTx,
    msg_id_cache: QqMsgIdCache,
) -> Result<QqHandlerResult> {
    if body.len() > QQ_WEBHOOK_BODY_MAX {
        return Err(Error::config("qq_webhook", "body too large"));
    }
    let value: serde_json::Value =
        serde_json::from_slice(body).map_err(|e| Error::config("qq_json", e.to_string()))?;
    let op = value.get("op").and_then(|v| v.as_u64()).unwrap_or(99);

    if op == 13 {
        let d = value
            .get("d")
            .and_then(|v| v.as_object())
            .ok_or_else(|| Error::config("qq_op13", "missing d"))?;
        let plain_token = d
            .get("plain_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::config("qq_op13", "missing plain_token"))?
            .to_string();
        let event_ts = d
            .get("event_ts")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let signature = sign_qq_url_verify(secret, &event_ts, &plain_token)?;
        return Ok(QqHandlerResult::UrlVerification {
            plain_token,
            signature,
        });
    }

    if op == 0 {
        let ts = signature_timestamp
            .ok_or_else(|| Error::config("qq_op0", "missing X-Signature-Timestamp"))?;
        let sig = signature_ed25519
            .ok_or_else(|| Error::config("qq_op0", "missing X-Signature-Ed25519"))?;
        verify_qq_signature(secret, ts, body, sig)?;

        let t = value.get("t").and_then(|v| v.as_str()).unwrap_or("");
        let d = value.get("d").and_then(|v| v.as_object());
        if let Some(d) = d {
            let (chat_id, content, msg_id) = match t {
                "AT_MESSAGE_CREATE" => {
                    // 频道消息：chat_id = channel_id
                    let ch = d
                        .get("channel_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let ct = d
                        .get("content")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let mid = d.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());
                    (ch, ct, mid)
                }
                "GROUP_AT_MESSAGE_CREATE" => {
                    // 群聊 @ 消息：chat_id = "group:{group_openid}"
                    let gid = d
                        .get("group_openid")
                        .and_then(|v| v.as_str())
                        .map(|s| format!("group:{}", s));
                    let ct = d
                        .get("content")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let mid = d.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());
                    (gid, ct, mid)
                }
                "C2C_MESSAGE_CREATE" => {
                    // 私聊消息：chat_id = "c2c:{guild_id}"（私信频道 ID，发消息用 /dms/{guild_id}/messages）
                    let gid = d
                        .get("guild_id")
                        .and_then(|v| v.as_str())
                        .map(|s| format!("c2c:{}", s));
                    let ct = d
                        .get("content")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let mid = d.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());
                    (gid, ct, mid)
                }
                _ => (None, None, None),
            };
            if let (Some(id), Some(ch), Some(content)) = (msg_id, chat_id, content) {
                if !ch.is_empty() && !content.is_empty() {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    {
                        let mut cache = msg_id_cache.lock().map_err(|e| Error::Other {
                            source: Box::new(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                e.to_string(),
                            )),
                            stage: "qq_msg_id_cache_lock",
                        })?;
                        cache.insert(ch.clone(), (id, now));
                    }
                    let msg = PcMsg::new("qq_channel", ch, content)?;
                    inbound_tx.send(msg).map_err(|e| Error::Other {
                        source: Box::new(e),
                        stage: "qq_inbound_send",
                    })?;
                }
            }
        }
        return Ok(QqHandlerResult::EventHandled);
    }

    Ok(QqHandlerResult::EventHandled)
}
