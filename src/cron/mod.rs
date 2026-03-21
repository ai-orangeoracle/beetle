//! 定时向 bus 推系统消息；错峰退避，失败打日志不 panic。
//! Cron: push system message to bus at interval; backoff on failure, log only.

use crate::bus::{InboundTx, PcMsg};
use std::time::Duration;

const TAG: &str = "cron";
/// 默认轮询间隔（秒）。
pub const DEFAULT_CRON_INTERVAL_SECS: u64 = 60;
/// 发送失败后退避乘数（秒）。
const BACKOFF_SECS: u64 = 5;

/// 在独立线程中循环：每隔 interval_secs 向 inbound_tx 推一条 PcMsg（channel=cron, chat_id=cron）。
/// 发送失败时打日志并退避 BACKOFF_SECS，不 panic。
pub fn run_cron_loop(inbound_tx: InboundTx, interval_secs: u64) {
    std::thread::spawn(move || {
        let interval = Duration::from_secs(interval_secs);
        let mut backoff = 0u64;
        loop {
            std::thread::sleep(interval + Duration::from_secs(backoff));
            let content = "tick".to_string(); // 简短内容，满足 MAX_CONTENT_LEN
            let msg = match PcMsg::new("cron", "cron", content) {
                Ok(m) => m,
                Err(e) => {
                    log::warn!("[{}] PcMsg::new failed: {}", TAG, e);
                    backoff = backoff.saturating_add(BACKOFF_SECS);
                    continue;
                }
            };
            match inbound_tx.send(msg) {
                Ok(()) => {
                    backoff = 0;
                    log::debug!("[{}] cron message pushed", TAG);
                }
                Err(e) => {
                    log::warn!("[{}] inbound_tx.send failed: {}", TAG, e);
                    backoff = backoff.saturating_add(BACKOFF_SECS);
                }
            }
        }
    });
    log::info!("[{}] cron loop started (interval {}s)", TAG, interval_secs);
}
