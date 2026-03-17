//! QQ 频道出站：flush、sign/verify、msg_id 缓存、连通性检查。Sink 统一为 dispatch::QueuedSink。

use crate::channels::ChannelHttpClient;
use crate::config::AppConfig;
use crate::error::{Error, Result};
use ed25519_dalek::{Signer, SigningKey};
use hex;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// 单条消息最大字符数，与现有通道对齐。
const QQ_MAX_MESSAGE_LEN: usize = 4096;

/// 被动回复 msg_id 有效时长（秒）。
const QQ_MSG_ID_TTL_SECS: u64 = 300;

/// 将 Bot Secret 字符串重复至 32 字节作为 Ed25519 种子；不足则循环填充。
fn secret_to_seed(secret: &str) -> [u8; 32] {
    let mut seed = [0u8; 32];
    let bytes = secret.as_bytes();
    if bytes.is_empty() {
        return seed;
    }
    for (i, b) in seed.iter_mut().enumerate() {
        *b = bytes[i % bytes.len()];
    }
    seed
}

/// op=13：对 event_ts + plain_token 做 Ed25519 签名，返回 hex 编码的 signature。
pub fn sign_qq_url_verify(secret: &str, event_ts: &str, plain_token: &str) -> Result<String> {
    let seed = secret_to_seed(secret);
    let signing_key = SigningKey::from_bytes(&seed);
    let message = format!("{}{}", event_ts, plain_token);
    let signature = signing_key.sign(message.as_bytes());
    Ok(hex::encode(signature.to_bytes()))
}

/// op=0：校验 X-Signature-Ed25519、X-Signature-Timestamp 与 body 的 Ed25519 验签。
pub fn verify_qq_signature(
    secret: &str,
    timestamp: &str,
    body: &[u8],
    signature_hex: &str,
) -> Result<()> {
    let seed = secret_to_seed(secret);
    let signing_key = SigningKey::from_bytes(&seed);
    let verifying_key = signing_key.verifying_key();
    let sig_bytes: [u8; 64] = hex::decode(signature_hex)
        .map_err(|e| Error::config("qq_verify_hex", e.to_string()))?
        .try_into()
        .map_err(|_| Error::config("qq_verify", "signature length must be 64 bytes"))?;
    let signature = ed25519_dalek::Signature::from_bytes(&sig_bytes);
    let message: Vec<u8> = timestamp.as_bytes().iter().chain(body.iter()).copied().collect();
    verifying_key
        .verify_strict(&message, &signature)
        .map_err(|_| Error::config("qq_verify", "signature verification failed"))?;
    Ok(())
}

/// msg_id 缓存类型：channel_id -> (msg_id, unix_ts)。
pub type QqMsgIdCache = Arc<Mutex<HashMap<String, (String, u64)>>>;

pub const QQ_GET_APP_ACCESS_TOKEN_URL: &str = "https://bots.qq.com/app/getAppAccessToken";
const QQ_MESSAGES_BASE: &str = "https://api.sgroup.qq.com/channels";
const QQ_V2_BASE: &str = "https://api.sgroup.qq.com/v2";

#[derive(serde::Serialize)]
pub struct QqTokenRequest {
    #[serde(rename = "appId")]
    pub app_id: String,
    #[serde(rename = "clientSecret")]
    pub client_secret: String,
}

#[derive(serde::Deserialize)]
pub struct QqTokenResponse {
    pub access_token: Option<String>,
    #[serde(default, deserialize_with = "deserialize_u64_or_string")]
    #[allow(dead_code)]
    pub expires_in: u64,
}

/// QQ API 的 expires_in 可能返回数字或字符串，兼容两种格式。
fn deserialize_u64_or_string<'de, D>(deserializer: D) -> std::result::Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum U64OrString {
        U64(u64),
        Str(String),
    }
    match U64OrString::deserialize(deserializer)? {
        U64OrString::U64(v) => Ok(v),
        U64OrString::Str(s) => s.parse::<u64>().map_err(serde::de::Error::custom),
    }
}

/// 连通性检查：供 GET /api/channel_connectivity 使用。
pub fn check_connectivity<H: ChannelHttpClient + ?Sized>(
    config: &AppConfig,
    http: &mut H,
) -> super::super::connectivity::ChannelConnectivityItem {
    use super::super::connectivity;
    let configured = !config.qq_channel_app_id.trim().is_empty() && !config.qq_channel_secret.trim().is_empty();
    let (ok, message) = if !configured {
        (false, None)
    } else {
        let body = QqTokenRequest {
            app_id: config.qq_channel_app_id.trim().to_string(),
            client_secret: config.qq_channel_secret.trim().to_string(),
        };
        let body_bytes = match serde_json::to_vec(&body) {
            Ok(b) => b,
            Err(e) => return connectivity::item("qq_channel", configured, false, Some(e.to_string())),
        };
        let (status, resp_body) = match http.http_post(QQ_GET_APP_ACCESS_TOKEN_URL, &body_bytes) {
            Ok(r) => r,
            Err(e) => return connectivity::item("qq_channel", configured, false, Some(e.to_string())),
        };
        if status >= 400 {
            return connectivity::item("qq_channel", configured, false, Some(format!("getAppAccessToken status {}", status)));
        }
        let r: QqTokenResponse = match serde_json::from_slice(resp_body.as_ref()) {
            Ok(x) => x,
            Err(e) => return connectivity::item("qq_channel", configured, false, Some(e.to_string())),
        };
        match r.access_token {
            Some(t) if !t.is_empty() => (true, None),
            _ => (false, Some("no access_token".into())),
        }
    };
    connectivity::item("qq_channel", configured, ok, message)
}

fn acquire_qq_token<H: ChannelHttpClient>(
    http: &mut H,
    app_id: &str,
    secret: &str,
) -> Option<String> {
    const TAG: &str = "qq_send";
    let body = QqTokenRequest {
        app_id: app_id.to_string(),
        client_secret: secret.to_string(),
    };
    let body_bytes = match serde_json::to_vec(&body) {
        Ok(b) => b,
        Err(e) => {
            log::warn!("[{}] token json: {}", TAG, e);
            return None;
        }
    };
    let (status, resp_body) = match http.http_post(QQ_GET_APP_ACCESS_TOKEN_URL, &body_bytes) {
        Ok(r) => r,
        Err(e) => {
            log::warn!("[{}] getAppAccessToken failed: {}", TAG, e);
            return None;
        }
    };
    if status >= 400 {
        log::warn!("[{}] getAppAccessToken status={}", TAG, status);
        return None;
    }
    let token_resp: QqTokenResponse = match serde_json::from_slice(resp_body.as_ref()) {
        Ok(t) => t,
        Err(e) => {
            log::warn!("[{}] token parse: {}", TAG, e);
            return None;
        }
    };
    match token_resp.access_token {
        Some(t) if !t.is_empty() => Some(t),
        _ => {
            log::warn!("[{}] no access_token in response", TAG);
            None
        }
    }
}

/// 根据 chat_id 前缀确定 API 端点：
/// - "group:{group_openid}" → /v2/groups/{group_openid}/messages（群聊）
/// - "c2c:{user_openid}"   → /v2/users/{user_openid}/messages（C2C 单聊）
/// - 其他                   → /channels/{channel_id}/messages（频道）
fn build_qq_message_url(chat_id: &str) -> String {
    if let Some(group_openid) = chat_id.strip_prefix("group:") {
        format!("{}/groups/{}/messages", QQ_V2_BASE, group_openid)
    } else if let Some(user_openid) = chat_id.strip_prefix("c2c:") {
        format!("{}/users/{}/messages", QQ_V2_BASE, user_openid)
    } else {
        format!("{}/{}/messages", QQ_MESSAGES_BASE, chat_id)
    }
}

/// 群聊和私聊（v2 API）需要 msg_type 字段；频道 API 不需要。
fn is_v2_chat(chat_id: &str) -> bool {
    chat_id.starts_with("group:") || chat_id.starts_with("c2c:")
}

fn send_one_qq<H: ChannelHttpClient>(
    http: &mut H,
    token: &str,
    chat_id: &str,
    content: &str,
    msg_id: Option<&str>,
) {
    const TAG: &str = "qq_send";
    let url = build_qq_message_url(chat_id);
    let v2 = is_v2_chat(chat_id);
    let chunks = crate::channels::chunk::chunk_str_by_char_count(content, QQ_MAX_MESSAGE_LEN);
    for (i, chunk) in chunks.iter().enumerate() {
        let mut body_obj = serde_json::json!({ "content": chunk });
        if v2 {
            body_obj["msg_type"] = serde_json::json!(0); // 0 = 文本
            // v2 API（群聊/C2C）需要 msg_seq 去重；每个分片递增
            body_obj["msg_seq"] = serde_json::json!(i + 1);
        }
        if i == 0 {
            if let Some(id) = msg_id {
                body_obj["msg_id"] = serde_json::json!(id);
            }
        }
        let body_bytes = match serde_json::to_vec(&body_obj) {
            Ok(b) => b,
            Err(e) => {
                log::warn!("[{}] message json: {}", TAG, e);
                continue;
            }
        };
        let auth_header = format!("QQBot {}", token);
        let mut cl_buf = [0u8; 20];
        let content_length = crate::util::usize_to_decimal_buf(&mut cl_buf, body_bytes.len());
        let headers = [
            ("Authorization", auth_header.as_str()),
            ("content-type", "application/json"),
            ("content-length", content_length),
        ];
        match crate::channels::send::send_post_with_headers(
            TAG, http, &url, &headers, &body_bytes,
        ) {
            Ok((status, ref body)) if status >= 400 => {
                let preview = String::from_utf8_lossy(
                    &body.as_ref()[..body.as_ref().len().min(256)],
                );
                log::warn!("[{}] send status={} body={}", TAG, status, preview);
            }
            Err(ref e) => {
                log::warn!("[{}] send error: {}", TAG, e);
            }
            _ => {}
        }
    }
}

fn pop_msg_id(cache: &QqMsgIdCache, chat_id: &str) -> Option<String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    cache.lock().ok().and_then(|mut c| {
        let entry = c.get(chat_id).map(|(id, ts)| (id.clone(), *ts));
        if let Some((id_clone, ts)) = entry {
            if now.saturating_sub(ts) <= QQ_MSG_ID_TTL_SECS {
                c.remove(chat_id);
                return Some(id_clone);
            }
        }
        None
    })
}

/// 从 rx 取出待发送（一次性 drain）。
pub fn flush_qq_channel_sends<H: ChannelHttpClient>(
    rx: &std::sync::mpsc::Receiver<(String, String)>,
    app_id: &str,
    secret: &str,
    cache: QqMsgIdCache,
    http: &mut H,
) {
    if app_id.is_empty() || secret.is_empty() {
        return;
    }
    let token = match acquire_qq_token(http, app_id, secret) {
        Some(t) => t,
        None => return,
    };
    while let Ok((chat_id, content)) = rx.try_recv() {
        let msg_id = pop_msg_id(&cache, &chat_id);
        send_one_qq(http, &token, &chat_id, &content, msg_id.as_deref());
    }
}

/// 持续运行的 QQ 频道发送循环；按需创建 HTTP 客户端，发完即释放。
pub fn run_qq_sender_loop<H, F>(
    rx: std::sync::mpsc::Receiver<(String, String)>,
    app_id: &str,
    secret: &str,
    cache: QqMsgIdCache,
    mut create_http: F,
) where
    H: ChannelHttpClient,
    F: FnMut() -> crate::error::Result<H>,
{
    const TAG: &str = "qq_sender";
    if app_id.is_empty() || secret.is_empty() {
        return;
    }
    #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
    crate::platform::task_wdt::register_current_task_to_task_wdt();
    log::info!("[{}] sender loop started", TAG);

    let recv_timeout = std::time::Duration::from_secs(30);
    loop {
        let (chat_id, content) = match rx.recv_timeout(recv_timeout) {
            Ok(item) => item,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                crate::platform::task_wdt::feed_current_task();
                continue;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                log::info!("[{}] rx disconnected, exiting", TAG);
                break;
            }
        };
        crate::platform::task_wdt::feed_current_task();
        let mut sent = false;
        for retry in 0..3u8 {
            if retry > 0 {
                std::thread::sleep(std::time::Duration::from_secs(2));
                crate::platform::task_wdt::feed_current_task();
            }
            let mut http = match create_http() {
                Ok(h) => h,
                Err(e) => {
                    log::warn!("[{}] create http failed (attempt {}): {}", TAG, retry + 1, e);
                    continue;
                }
            };
            let token = match acquire_qq_token(&mut http, app_id, secret) {
                Some(t) => t,
                None => continue,
            };
            let msg_id = pop_msg_id(&cache, &chat_id);
            send_one_qq(&mut http, &token, &chat_id, &content, msg_id.as_deref());
            while let Ok((cid, cnt)) = rx.try_recv() {
                let mid = pop_msg_id(&cache, &cid);
                send_one_qq(&mut http, &token, &cid, &cnt, mid.as_deref());
            }
            sent = true;
            break;
        }
        if !sent {
            log::error!("[{}] message dropped after 3 retries, chat_id={}", TAG, chat_id);
        }
    }
}
