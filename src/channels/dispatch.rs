//! 出站分发：从 outbound_rx 取 PcMsg，按 channel 调用对应 MessageSink；按通道熔断，避免单通道拖垮全局。
//! Outbound dispatch: recv from outbound_rx, send via MessageSink; per-channel circuit breaker.

use crate::bus::{OutboundRx, MAX_CONTENT_LEN};
use crate::util::truncate_content_to_max;
use crate::constants::{CHANNEL_FAIL_COOLDOWN_SECS, CHANNEL_FAIL_THRESHOLD};
use crate::error::Result;
use crate::metrics;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 出站发送抽象；各通道实现此 trait，由 main 注册到 ChannelSinks。
pub trait MessageSink: Send + Sync {
    fn send(&self, chat_id: &str, content: &str) -> Result<()>;
}

/// 队列型 Sink：将 (chat_id, content) 送入 channel，由 main 的 flush_*_sends 消费。各通道仅 stage 不同。
pub struct QueuedSink {
    tx: std::sync::mpsc::SyncSender<(String, String)>,
    stage: &'static str,
}

impl QueuedSink {
    pub fn new(tx: std::sync::mpsc::SyncSender<(String, String)>, stage: &'static str) -> Self {
        Self { tx, stage }
    }
}

impl MessageSink for QueuedSink {
    fn send(&self, chat_id: &str, content: &str) -> Result<()> {
        let content = truncate_content_to_max(content, MAX_CONTENT_LEN);
        self.tx.try_send((chat_id.to_string(), content)).map_err(|e| crate::error::Error::Other {
            source: Box::new(e),
            stage: self.stage,
        })
    }
}

/// channel 名称 → sink 映射；由 main 构造并传入 run_dispatch。
pub struct ChannelSinks {
    map: HashMap<String, Box<dyn MessageSink>>,
}

impl ChannelSinks {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn register(&mut self, channel: impl Into<String>, sink: Box<dyn MessageSink>) {
        self.map.insert(channel.into(), sink);
    }

    fn get(&self, channel: &str) -> Option<&dyn MessageSink> {
        self.map.get(channel).map(|b| b.as_ref())
    }
}

impl Default for ChannelSinks {
    fn default() -> Self {
        Self::new()
    }
}

/// 可选重试次数（含首次）；重试间隔（毫秒），避免连续锤击失败通道。
const SEND_RETRY: u32 = 2;
const SEND_RETRY_DELAY_MS: u64 = 500;

static CHANNEL_FAIL_STATE: OnceLock<Mutex<HashMap<String, (u32, Instant)>>> = OnceLock::new();

fn channel_fail_state() -> &'static Mutex<HashMap<String, (u32, Instant)>> {
    CHANNEL_FAIL_STATE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn is_channel_in_cooldown(channel: &str) -> bool {
    let guard = match channel_fail_state().lock() {
        Ok(g) => g,
        Err(_) => return false,
    };
    if let Some((count, last)) = guard.get(channel) {
        *count >= CHANNEL_FAIL_THRESHOLD
            && last.elapsed() < Duration::from_secs(CHANNEL_FAIL_COOLDOWN_SECS)
    } else {
        false
    }
}

fn record_channel_fail(channel: &str) {
    let now = Instant::now();
    if let Ok(mut guard) = channel_fail_state().lock() {
        let entry = guard.entry(channel.to_string()).or_insert((0, now));
        entry.0 = entry.0.saturating_add(1);
        entry.1 = now;
    }
}

fn record_channel_ok(channel: &str) {
    if let Ok(mut guard) = channel_fail_state().lock() {
        guard.remove(channel);
    }
}

/// 熔断冷却期暂存的消息上限，防止无限积累。
const COOLDOWN_BUFFER_MAX: usize = 16;

/// 循环接收出站消息，按 msg.channel 查找 sink 并调用 send；失败打日志并重试；
/// 单通道熔断冷却期内暂存消息，冷却结束后重放。
pub fn run_dispatch(outbound_rx: OutboundRx, sinks: Arc<ChannelSinks>) {
    const TAG: &str = "channel_dispatch";
    let mut cooldown_buffer: Vec<crate::bus::PcMsg> = Vec::new();

    while let Ok(msg) = outbound_rx.recv() {
        let content = truncate_content_to_max(&msg.content, MAX_CONTENT_LEN);
        if content.trim() == "SILENT" || msg.channel == "cron" {
            continue;
        }

        // Replay buffered messages whose channel is out of cooldown
        let mut i = 0;
        while i < cooldown_buffer.len() {
            if !is_channel_in_cooldown(&cooldown_buffer[i].channel) {
                let buffered = cooldown_buffer.swap_remove(i);
                let bc = truncate_content_to_max(&buffered.content, MAX_CONTENT_LEN);
                if let Some(sink) = sinks.get(&buffered.channel) {
                    if sink.send(&buffered.chat_id, &bc).is_ok() {
                        record_channel_ok(&buffered.channel);
                        metrics::record_dispatch_send(true);
                    }
                }
            } else {
                i += 1;
            }
        }

        if is_channel_in_cooldown(&msg.channel) {
            if cooldown_buffer.len() < COOLDOWN_BUFFER_MAX {
                cooldown_buffer.push(msg);
            } else {
                log::warn!(
                    "[{}] channel={} cooldown buffer full, dropping oldest",
                    TAG,
                    msg.channel
                );
                cooldown_buffer.remove(0);
                cooldown_buffer.push(msg);
            }
            continue;
        }
        if let Some(sink) = sinks.get(&msg.channel) {
            let mut last_err = None;
            for attempt in 0..SEND_RETRY {
                if attempt > 0 {
                    std::thread::sleep(Duration::from_millis(SEND_RETRY_DELAY_MS));
                }
                match sink.send(&msg.chat_id, &content) {
                    Ok(()) => {
                        last_err = None;
                        record_channel_ok(&msg.channel);
                        metrics::record_dispatch_send(true);
                        break;
                    }
                    Err(e) => {
                        last_err = Some(e);
                    }
                }
            }
            if let Some(e) = last_err {
                record_channel_fail(&msg.channel);
                metrics::record_dispatch_send(false);
                metrics::record_error_by_stage("channel_dispatch");
                log::warn!("[{}] channel={} send failed after retries: {}", TAG, msg.channel, e);
            }
        } else {
            log::warn!("[{}] no sink for channel={}", TAG, msg.channel);
        }
    }
}
