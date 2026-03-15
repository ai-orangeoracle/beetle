//! 通道抽象与出站分发。仅依赖 bus、error、config；不依赖 agent、llm、tools。
//! Channel sink trait and types; dispatch consumes outbound and sends to sinks.

mod chunk;
mod connectivity;
mod dingtalk;
mod dispatch;
mod feishu;
mod http_client;
mod qq;
mod send;
mod telegram;
mod websocket;
mod wecom;
mod wss_gateway;

pub use connectivity::{check_all, ChannelConnectivityItem};
pub use dingtalk::{flush_dingtalk_sends, run_dingtalk_sender_loop};
pub use dispatch::{run_dispatch, ChannelSinks, MessageSink, QueuedSink};
pub use feishu::{
    event_body_to_pcmsg, flush_feishu_sends, handle_http_event, run_feishu_sender_loop,
    FeishuEventResponse,
};
#[cfg(all(feature = "feishu", any(target_arch = "xtensa", target_arch = "riscv32")))]
pub use feishu::run_feishu_ws_loop;
pub use qq::{
    flush_qq_channel_sends, handle_webhook, run_qq_sender_loop, QqHandlerResult, QqMsgIdCache,
    QQ_WEBHOOK_BODY_MAX,
};
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub use qq::run_qq_ws_loop;
pub use http_client::ChannelHttpClient;
pub use telegram::{
    flush_telegram_sends, get_bot_username, poll_telegram_once, run_telegram_sender_loop,
    send_chat_action, TelegramCommandCtx,
};
pub use websocket::{WebSocketSink, MAX_WS_CONNECTIONS, MAX_WS_MESSAGE_LEN};
pub use wecom::{flush_wecom_sends, run_wecom_sender_loop};

/// 占位 sink：打日志并返回 Ok，供 8.1 验收。
pub struct LogSink {
    pub tag: String,
}

impl LogSink {
    pub fn new(tag: &str) -> Self {
        Self {
            tag: tag.to_string(),
        }
    }
}

impl MessageSink for LogSink {
    fn send(&self, chat_id: &str, content: &str) -> crate::error::Result<()> {
        log::info!(
            "[{}] send chat_id={} content_len={}",
            self.tag,
            chat_id,
            content.len()
        );
        Ok(())
    }
}
