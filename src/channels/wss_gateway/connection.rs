//! WSS 传输抽象：发二进制帧、带超时收事件。实现由 platform 或 cfg 模块提供。
//! Transport abstraction for WSS: send binary frame, receive events with timeout.

use crate::error::Result;
use std::time::Duration;

/// 与 ESP `esp_websocket_client` 默认 buffer 对齐（仅嵌入式构建使用）。
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub(crate) const DEFAULT_WSS_BUFFER_SIZE: usize = 4096;
/// 应用层 `send_binary` 最大字节数（ESP 与客户端 buffer 对齐；主机/Linux 需容纳 QQ Identify 等较长 JSON）。
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub(crate) const MAX_WSS_SEND_PAYLOAD_BYTES: usize = DEFAULT_WSS_BUFFER_SIZE - 32;
#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub(crate) const MAX_WSS_SEND_PAYLOAD_BYTES: usize = 64 * 1024;

/// 单次收到的 WSS 事件。
#[derive(Debug, Clone)]
pub enum WssEvent {
    Binary(Vec<u8>),
    Disconnected,
    Closed,
}

/// 带超时收一条事件：有数据返回 Some(ev)，超时返回 None；连接断开等错误返回 Err。
pub trait WssConnection {
    fn send_binary(&mut self, data: &[u8]) -> Result<()>;
    /// 阻塞最多 timeout，返回收到的事件或 None 表示超时。
    fn recv_timeout(&mut self, timeout: Duration) -> Result<Option<WssEvent>>;
}
