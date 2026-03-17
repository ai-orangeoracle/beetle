//! Telegram 入站 long poll：getUpdates，解析消息入队，命令处理。

use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::bus::{InboundTx, OutboundTx, PcMsg, MAX_CONTENT_LEN};
use crate::channels::ChannelHttpClient;
use crate::error::{Error, Result};
use crate::memory::SessionStore;
use crate::platform::EspHttpClient;

use super::send::set_message_reaction;

const TAG_POLL: &str = "telegram";
const BIND_HINT_EMPTY: &str =
    "Bind: set BEETLE_TG_ALLOWED_CHAT_IDS=<your_chat_id> and rebuild.";
const BIND_HINT_NOT_IN_LIST: &str =
    "Bind: add your chat_id to BEETLE_TG_ALLOWED_CHAT_IDS (comma-separated) and rebuild.";

/// Telegram 控制命令（/activation、/session clear、/status）执行所需的上下文，由 main 传入轮询线程。
pub struct TelegramCommandCtx {
    pub outbound_tx: OutboundTx,
    pub session_store: Arc<dyn SessionStore + Send + Sync>,
    pub wifi_connected: bool,
    pub inbound_depth: Arc<std::sync::atomic::AtomicUsize>,
    pub outbound_depth: Arc<std::sync::atomic::AtomicUsize>,
    pub set_group_activation: Box<dyn Fn(&str) -> Result<()> + Send>,
}

fn map_stage(e: Error, stage: &'static str) -> Error {
    match e {
        Error::Http { status_code, .. } => Error::Http { status_code, stage },
        other => Error::Other {
            source: Box::new(other),
            stage,
        },
    }
}

#[derive(serde::Deserialize)]
struct TelegramUpdates {
    result: Option<Vec<TelegramUpdate>>,
}

#[derive(serde::Deserialize)]
struct TelegramUpdate {
    update_id: i64,
    message: Option<TelegramMessage>,
}

#[derive(serde::Deserialize)]
struct TelegramMessage {
    chat: TelegramChat,
    #[serde(default)]
    message_id: i64,
    text: Option<String>,
    entities: Option<Vec<MessageEntity>>,
}

#[derive(serde::Deserialize)]
struct TelegramChat {
    id: i64,
    #[serde(rename = "type")]
    type_: Option<String>,
}

#[derive(serde::Deserialize)]
struct MessageEntity {
    #[serde(rename = "type")]
    type_: String,
    offset: Option<i32>,
    length: Option<i32>,
}

fn message_mentions_bot(
    text: &str,
    entities: Option<&[MessageEntity]>,
    bot_username: &str,
) -> bool {
    if bot_username.is_empty() {
        return false;
    }
    let mention = format!("@{}", bot_username);
    if text.contains(&mention) {
        return true;
    }
    let Some(entities) = entities else {
        return false;
    };
    for e in entities {
        if e.type_ != "mention" {
            continue;
        }
        let (off, len) = match (e.offset, e.length) {
            (Some(o), Some(l)) if o >= 0 && l > 0 => (o as usize, l as usize),
            _ => continue,
        };
        if let Some(slice) = text.get(off..off.saturating_add(len)) {
            if slice.eq_ignore_ascii_case(&mention) {
                return true;
            }
        }
    }
    false
}

const TELEGRAM_API_BASE: &str = "https://api.telegram.org/bot";

/// 轮询一次 getUpdates，解析消息并推入 inbound_tx；失败返回 Err 带 stage，调用方退避。
pub fn poll_telegram_once<H: ChannelHttpClient>(
    http: &mut H,
    token: &str,
    offset: Option<i64>,
    inbound_tx: &InboundTx,
    allowed_chat_ids: &[String],
    group_activation: &str,
    bot_username: Option<&str>,
    cmd_ctx: Option<&TelegramCommandCtx>,
) -> Result<Option<i64>> {
    let url = format!(
        "{}{}/getUpdates?timeout=5{}",
        TELEGRAM_API_BASE,
        token,
        offset.map(|o| format!("&offset={}", o)).unwrap_or_default()
    );
    let (status, body) = http
        .http_get(&url)
        .map_err(|e| map_stage(e, "telegram_poll"))?;
    if status >= 400 {
        return Err(Error::Http {
            status_code: status,
            stage: "telegram_poll",
        });
    }
    let updates: TelegramUpdates =
        serde_json::from_slice(body.as_ref()).map_err(|e| Error::Other {
            source: Box::new(e),
            stage: "telegram_parse",
        })?;
    let mut next_offset = offset;
    for u in updates.result.unwrap_or_default() {
        next_offset = Some(u.update_id + 1);
        if let Some(msg) = u.message {
            let chat_id = msg.chat.id.to_string();
            if allowed_chat_ids.is_empty() {
                log::warn!(
                    "[{}] rejected chat_id={} (allowlist empty). {}",
                    TAG_POLL,
                    chat_id,
                    BIND_HINT_EMPTY
                );
                continue;
            }
            if !allowed_chat_ids.iter().any(|id| id == &chat_id) {
                log::warn!(
                    "[{}] rejected chat_id={} (not in allowlist). {} Example: ...{}",
                    TAG_POLL,
                    chat_id,
                    BIND_HINT_NOT_IN_LIST,
                    chat_id
                );
                continue;
            }
            let text = msg.text.unwrap_or_default();
            if text.is_empty() {
                continue;
            }
            let is_group = msg
                .chat
                .type_
                .as_deref()
                .map_or(false, |t| t == "group" || t == "supergroup");
            if is_group && group_activation == "mention" {
                let mentioned = message_mentions_bot(
                    &text,
                    msg.entities.as_deref(),
                    bot_username.unwrap_or(""),
                );
                if !mentioned {
                    continue;
                }
            }
            if let Some(ctx) = cmd_ctx {
                if text.starts_with('/') {
                    let parts: Vec<&str> = text.split_whitespace().collect();
                    let handled = match parts.as_slice() {
                        ["/activation", "mention"] => {
                            if let Err(e) = (ctx.set_group_activation)("mention") {
                                log::warn!("[{}] set_group_activation: {}", TAG_POLL, e);
                            }
                            let _ = PcMsg::new("telegram", &chat_id, "已切换为 mention")
                                .map(|m| ctx.outbound_tx.send(m));
                            true
                        }
                        ["/activation", "always"] => {
                            if let Err(e) = (ctx.set_group_activation)("always") {
                                log::warn!("[{}] set_group_activation: {}", TAG_POLL, e);
                            }
                            let _ = PcMsg::new("telegram", &chat_id, "已切换为 always")
                                .map(|m| ctx.outbound_tx.send(m));
                            true
                        }
                        ["/session", "clear"] => {
                            if let Err(e) = ctx.session_store.clear(&chat_id) {
                                log::warn!("[{}] session clear: {}", TAG_POLL, e);
                            }
                            let _ = PcMsg::new("telegram", &chat_id, "会话已清空")
                                .map(|m| ctx.outbound_tx.send(m));
                            true
                        }
                        ["/status"] => {
                            let wifi = if ctx.wifi_connected {
                                "connected"
                            } else {
                                "disconnected"
                            };
                            let inc = ctx.inbound_depth.load(Ordering::Relaxed);
                            let out = ctx.outbound_depth.load(Ordering::Relaxed);
                            let status =
                                format!("wifi: {}, inbound: {}, outbound: {}", wifi, inc, out);
                            let _ = PcMsg::new("telegram", &chat_id, status)
                                .map(|m| ctx.outbound_tx.send(m));
                            true
                        }
                        _ => false,
                    };
                    if handled {
                        continue;
                    }
                }
            }
            let content = if text.len() > MAX_CONTENT_LEN {
                text.chars().take(MAX_CONTENT_LEN).collect::<String>()
            } else {
                text
            };
            let _ = set_message_reaction(http, token, &chat_id, msg.message_id, "👍");
            let pc = PcMsg {
                channel: "telegram".to_string(),
                chat_id,
                content,
                is_group,
            };
            if inbound_tx.send(pc).is_err() {
                log::warn!("[{}] inbound_tx.send failed (channel closed)", TAG_POLL);
            }
        }
    }
    Ok(next_offset)
}

/// 启动 Telegram 长轮询循环（阻塞，应在独立线程调用）。
/// 内部创建 EspHttpClient、TelegramCommandCtx，执行轮询循环。
pub fn run_telegram_poll_loop(
    token: String,
    allowed_chat_ids: Vec<String>,
    group_activation: String,
    inbound_tx: InboundTx,
    outbound_tx: OutboundTx,
    session_store: Arc<dyn SessionStore + Send + Sync>,
    wifi_connected: bool,
    inbound_depth: Arc<std::sync::atomic::AtomicUsize>,
    outbound_depth: Arc<std::sync::atomic::AtomicUsize>,
    config_store: Arc<dyn crate::platform::ConfigStore>,
) {
    const TAG_TG: &str = "telegram_poll";

    #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
    crate::platform::task_wdt::register_current_task_to_task_wdt();

    let cmd_ctx = TelegramCommandCtx {
        outbound_tx,
        session_store,
        wifi_connected,
        inbound_depth,
        outbound_depth,
        set_group_activation: Box::new(move |v| {
            crate::config::write_tg_group_activation(config_store.as_ref(), v)
        }),
    };

    let mut http = match EspHttpClient::new() {
        Ok(h) => h,
        Err(e) => {
            log::warn!("[{}] EspHttpClient::new failed: {}", TAG_TG, e);
            return;
        }
    };

    let bot_username = match super::send::get_bot_username(&mut http, &token) {
        Ok(Some(u)) => Some(u),
        _ => None,
    };

    let mut offset: Option<i64> = None;
    const POLL_INTERVAL_SECS: u64 = 5;
    const BACKOFF_SECS: u64 = 30;

    loop {
        match poll_telegram_once(
            &mut http,
            &token,
            offset,
            &inbound_tx,
            &allowed_chat_ids,
            &group_activation,
            bot_username.as_deref(),
            Some(&cmd_ctx),
        ) {
            Ok(next) => offset = next,
            Err(e) => {
                log::warn!(
                    "[{}] poll failed: {}, backoff {}s",
                    TAG_TG,
                    e,
                    BACKOFF_SECS
                );
                ChannelHttpClient::reset_connection_for_retry(&mut http);
                std::thread::sleep(std::time::Duration::from_secs(BACKOFF_SECS));
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS));
    }
}
