//! 定时向 bus 推系统消息；错峰退避，失败打日志不 panic。
//! Cron: push system message to bus at interval; backoff on failure, log only.
//! 同时检查持久化 cron 任务并在到期时注入消息。

use crate::bus::{InboundTx, PcMsg};
use crate::memory::MemoryStore;
use std::sync::Arc;
use std::time::Duration;

const TAG: &str = "cron";
/// 默认轮询间隔（秒）。
pub const DEFAULT_CRON_INTERVAL_SECS: u64 = 60;
/// 发送失败后退避乘数（秒）。
const BACKOFF_SECS: u64 = 5;

/// 在独立线程中循环：每隔 interval_secs 向 inbound_tx 推一条 PcMsg（channel=cron, chat_id=cron）。
/// 同时检查持久化 cron 任务，到期的任务生成消息推入 inbound_tx。
/// 发送失败时打日志并退避 BACKOFF_SECS，不 panic。
pub fn run_cron_loop(
    inbound_tx: InboundTx,
    interval_secs: u64,
    memory_store: Option<Arc<dyn MemoryStore + Send + Sync>>,
) {
    crate::util::spawn_guarded("cron", move || {
        let interval = Duration::from_secs(interval_secs);
        let mut backoff = 0u64;
        loop {
            std::thread::sleep(interval + Duration::from_secs(backoff));

            // 1. Push standard cron tick
            let msg = match PcMsg::new("cron", "cron", "tick") {
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

            // 2. Check persisted cron tasks
            if let Some(ref store) = memory_store {
                fire_persisted_tasks(store.as_ref(), &inbound_tx);
            }
        }
    });
    log::info!("[{}] cron loop started (interval {}s)", TAG, interval_secs);
}

/// Check persisted cron tasks and fire any that match the current minute.
fn fire_persisted_tasks(store: &dyn MemoryStore, inbound_tx: &InboundTx) {
    let tasks = crate::tools::cron_manage::load_persisted_cron_tasks(store);
    if tasks.is_empty() {
        return;
    }

    let now_secs = crate::util::current_unix_secs();
    let (_y, mo, d, h, min, _s) = crate::util::epoch_to_ymdhms(now_secs);
    let dow_actual = ((now_secs / 86400) as u32 + 4) % 7; // 0=Sunday

    for task in &tasks {
        if !task.enabled {
            continue;
        }
        if let Ok(matches) = cron_matches(&task.expr, min, h, d, mo, dow_actual) {
            if matches {
                let content = format!("定时任务 [{}]: {}", task.id, task.action);
                match PcMsg::new(&task.channel, &task.chat_id, content) {
                    Ok(msg) => {
                        if let Err(e) = inbound_tx.send(msg) {
                            log::warn!("[{}] failed to fire cron task {}: {}", TAG, task.id, e);
                        } else {
                            log::info!("[{}] fired cron task {}", TAG, task.id);
                        }
                    }
                    Err(e) => {
                        log::warn!("[{}] PcMsg::new for task {} failed: {}", TAG, task.id, e);
                    }
                }
            }
        }
    }
}

/// Check if a cron expression matches the given time components.
fn cron_matches(
    expr: &str,
    minute: u32,
    hour: u32,
    dom: u32,
    month: u32,
    dow: u32,
) -> crate::error::Result<bool> {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() != 5 {
        return Ok(false);
    }
    let minutes = crate::tools::cron::parse_cron_field(parts[0], 0, 59)?;
    let hours = crate::tools::cron::parse_cron_field(parts[1], 0, 23)?;
    let doms = crate::tools::cron::parse_cron_field(parts[2], 1, 31)?;
    let months = crate::tools::cron::parse_cron_field(parts[3], 1, 12)?;
    let dows = crate::tools::cron::parse_cron_field(parts[4], 0, 6)?;

    Ok(minutes.contains(&minute)
        && hours.contains(&hour)
        && (doms.contains(&dom) || dows.contains(&dow))
        && months.contains(&month))
}
