//! 消息总线：入站/出站 channel，固定容量，背压由 `SyncSender::send` 阻塞实现。
//! Message bus: inbound/outbound channels, fixed capacity; backpressure = blocking send when full.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::Arc;

pub use crate::constants::{DEFAULT_CAPACITY, MAX_CONTENT_LEN};
pub use crate::util::{truncate_content_to_max, truncate_to_byte_len};

/// 总线消息。入队前需校验 `content.len() <= MAX_CONTENT_LEN`。可序列化供 pending_retry 持久化。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PcMsg {
    pub channel: String,
    pub chat_id: String,
    pub content: String,
    /// 是否来自群组（group/supergroup）；用于 system 注入与 SILENT 约定。
    pub is_group: bool,
}

impl PcMsg {
    /// 构造并校验 content 长度，超限返回 `Error::Config`。出站消息 is_group 恒为 false。
    pub fn new(
        channel: impl Into<String>,
        chat_id: impl Into<String>,
        content: impl Into<String>,
    ) -> Result<Self> {
        Self::new_inbound(channel, chat_id, content, false)
    }

    /// 入站消息用；与 `new` 相同但可指定 is_group（群聊/话题群为 true）。
    pub fn new_inbound(
        channel: impl Into<String>,
        chat_id: impl Into<String>,
        content: impl Into<String>,
        is_group: bool,
    ) -> Result<Self> {
        let content = content.into();
        if content.len() > MAX_CONTENT_LEN {
            return Err(Error::config(
                "PcMsg::new_inbound",
                format!(
                    "content length {} exceeds max {}",
                    content.len(),
                    MAX_CONTENT_LEN
                ),
            ));
        }
        Ok(PcMsg {
            channel: channel.into(),
            chat_id: chat_id.into(),
            content,
            is_group,
        })
    }
}

/// 带深度计数的发送端，send/try_send 成功时递增，供 health 查询。
pub struct TrackedSender<T> {
    inner: SyncSender<T>,
    depth: Arc<AtomicUsize>,
}

impl<T> TrackedSender<T> {
    /// 仅当 send 成功时递增深度。
    pub fn send(&self, t: T) -> std::result::Result<(), mpsc::SendError<T>> {
        let result = self.inner.send(t);
        if result.is_ok() {
            self.depth.fetch_add(1, Ordering::Relaxed);
        }
        result
    }

    /// 非阻塞发送；队列满时返回 Err(TrySendError::Full(t))。
    pub fn try_send(&self, t: T) -> std::result::Result<(), mpsc::TrySendError<T>> {
        let result = self.inner.try_send(t);
        if result.is_ok() {
            self.depth.fetch_add(1, Ordering::Relaxed);
        }
        result
    }
}

impl<T> Clone for TrackedSender<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            depth: Arc::clone(&self.depth),
        }
    }
}

/// 带深度计数的接收端，recv/recv_timeout 成功时递减。
pub struct TrackedReceiver<T> {
    inner: Receiver<T>,
    depth: Arc<AtomicUsize>,
}

impl<T> TrackedReceiver<T> {
    pub fn recv(&self) -> std::result::Result<T, mpsc::RecvError> {
        let result = self.inner.recv();
        if result.is_ok() {
            self.depth.fetch_sub(1, Ordering::Relaxed);
        }
        result
    }

    /// 带超时的接收；超时返回 Err(RecvTimeoutError::Timeout)。
    pub fn recv_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> std::result::Result<T, mpsc::RecvTimeoutError> {
        let result = self.inner.recv_timeout(timeout);
        if result.is_ok() {
            self.depth.fetch_sub(1, Ordering::Relaxed);
        }
        result
    }
}

pub type InboundTx = TrackedSender<PcMsg>;
pub type OutboundTx = TrackedSender<PcMsg>;
pub type InboundRx = TrackedReceiver<PcMsg>;
pub type OutboundRx = TrackedReceiver<PcMsg>;

/// 消息总线：main 唯一创建；通道侧持 `inbound_tx` 推入站，dispatch 持 `outbound_rx` 取出站。
/// 背压：队满时 `send()` 阻塞，直至有空间。深度由 Arc<AtomicUsize> 暴露供 health 使用。
pub struct MessageBus {
    pub inbound_tx: InboundTx,
    pub outbound_tx: OutboundTx,
    pub inbound_depth: Arc<AtomicUsize>,
    pub outbound_depth: Arc<AtomicUsize>,
}

impl MessageBus {
    /// 创建入站/出站 channel，容量均为 `capacity`。返回 (bus, inbound_rx, outbound_rx)。
    pub fn new(capacity: usize) -> (Self, InboundRx, OutboundRx) {
        let (inbound_tx, inbound_rx) = mpsc::sync_channel(capacity);
        let (outbound_tx, outbound_rx) = mpsc::sync_channel(capacity);
        let inbound_depth = Arc::new(AtomicUsize::new(0));
        let outbound_depth = Arc::new(AtomicUsize::new(0));
        let inbound_depth_rx = Arc::clone(&inbound_depth);
        let outbound_depth_rx = Arc::clone(&outbound_depth);
        (
            MessageBus {
                inbound_tx: TrackedSender {
                    inner: inbound_tx,
                    depth: Arc::clone(&inbound_depth),
                },
                outbound_tx: TrackedSender {
                    inner: outbound_tx,
                    depth: Arc::clone(&outbound_depth),
                },
                inbound_depth,
                outbound_depth,
            },
            TrackedReceiver {
                inner: inbound_rx,
                depth: inbound_depth_rx,
            },
            TrackedReceiver {
                inner: outbound_rx,
                depth: outbound_depth_rx,
            },
        )
    }
}
