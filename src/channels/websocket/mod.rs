//! WebSocket 网关：出站经 MessageSink（本阶段占位打日志）；入站与 WS 服务端可后续接入。
//! 单条消息大小遵守 bus::MAX_CONTENT_LEN；连接数上界由调用方或 config 约定。

use crate::bus::MAX_CONTENT_LEN;
use crate::channels::dispatch::MessageSink;
use crate::error::Result;

/// 单连接/会话 ID 与消息大小上界（与 config 或既有约定一致即可）。
pub const MAX_WS_MESSAGE_LEN: usize = MAX_CONTENT_LEN;
/// 最大连接数占位；实际由服务端或 config 限制。
pub const MAX_WS_CONNECTIONS: usize = 4;

/// 出站：本阶段仅打日志；后续可接入真实 WS 写回或队列 + flush。
pub struct WebSocketSink {
    tag: String,
}

impl WebSocketSink {
    pub fn new(tag: &str) -> Self {
        Self {
            tag: tag.to_string(),
        }
    }
}

impl MessageSink for WebSocketSink {
    fn send(&self, chat_id: &str, content: &str) -> Result<()> {
        let len = content.len().min(MAX_WS_MESSAGE_LEN);
        log::info!(
            "[{}] send chat_id={} content_len={}",
            self.tag,
            chat_id,
            len
        );
        Ok(())
    }
}
