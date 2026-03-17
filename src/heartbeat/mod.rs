//! 周期打日志（版本、运行时长、可选 heap），供外部监控存活；可读 HEARTBEAT.md 待办并注入入站。
//! Heartbeat: periodic log (version, uptime, optional heap) for liveness monitoring.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const TAG: &str = "heartbeat";
const TASK_THROTTLE_SECS: u64 = 30;

/// HEARTBEAT.md 中是否存在未勾选任务行（`- [ ]`）。空行、`#` 标题、`- [x]` 忽略。
pub fn has_pending_tasks(content: &str) -> bool {
    first_pending_task(content).is_some()
}

/// 返回第一个未完成任务行去掉 `- [ ]` 后的 trim 文本；无则 None。
pub fn first_pending_task(content: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.contains("- [ ]") {
            let t = line.split("- [ ]").nth(1).map(|s| s.trim()).unwrap_or("");
            return Some(t.to_string());
        }
    }
    None
}

/// 启动时间，由 run_heartbeat_loop 在启动时设置。
static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
/// 待办注入限频：同一内容 30s 内不重复注入。(content, last_inject_time)
static LAST_TASK_INJECT: OnceLock<Mutex<(String, Option<Instant>)>> = OnceLock::new();

/// 周期（秒）打一条日志：版本、运行时长、可选 heap；可被外部脚本/串口抓取判断存活。
pub fn run_heartbeat_loop(version: &'static str, interval_secs: u64) {
    START.get_or_init(Instant::now);
    let v = version;
    std::thread::spawn(move || {
        let interval = std::time::Duration::from_secs(interval_secs);
        loop {
            std::thread::sleep(interval);
            crate::orchestrator::update_heap_state();
            let uptime_secs = START.get().map(|s| s.elapsed().as_secs()).unwrap_or(0);
            #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
            {
                let internal_free =
                    unsafe { esp_idf_svc::sys::heap_caps_get_free_size(esp_idf_svc::sys::MALLOC_CAP_INTERNAL) };
                let spiram_free =
                    unsafe { esp_idf_svc::sys::heap_caps_get_free_size(esp_idf_svc::sys::MALLOC_CAP_SPIRAM) };
                let total_free = unsafe { esp_idf_svc::sys::esp_get_free_heap_size() };
                log::info!(
                    "[{}] HEARTBEAT version={} uptime_secs={} heap_internal={} heap_spiram={} heap_total={}",
                    TAG, v, uptime_secs, internal_free, spiram_free, total_free
                );
            }
            #[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
            log::info!("[{}] HEARTBEAT version={} uptime_secs={}", TAG, v, uptime_secs);
        }
    });
    log::info!("[{}] heartbeat loop started (interval {}s)", TAG, interval_secs);
}

/// 周期打日志并在有待办时向 inbound 注入一条 PcMsg；同一待办 30s 内不重复注入。
pub fn run_heartbeat_loop_with_tasks(
    version: &'static str,
    interval_secs: u64,
    inbound_tx: crate::bus::InboundTx,
    read_heartbeat: impl Fn() -> String + Send + 'static,
) {
    START.get_or_init(Instant::now);
    let interval = Duration::from_secs(interval_secs);
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(interval);
            crate::orchestrator::update_heap_state();
            let uptime_secs = START.get().map(|s| s.elapsed().as_secs()).unwrap_or(0);
            #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
            {
                let internal_free =
                    unsafe { esp_idf_svc::sys::heap_caps_get_free_size(esp_idf_svc::sys::MALLOC_CAP_INTERNAL) };
                let spiram_free =
                    unsafe { esp_idf_svc::sys::heap_caps_get_free_size(esp_idf_svc::sys::MALLOC_CAP_SPIRAM) };
                let total_free = unsafe { esp_idf_svc::sys::esp_get_free_heap_size() };
                log::info!(
                    "[{}] HEARTBEAT version={} uptime_secs={} heap_internal={} heap_spiram={} heap_total={}",
                    TAG, version, uptime_secs, internal_free, spiram_free, total_free
                );
            }
            #[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
            log::info!("[{}] HEARTBEAT version={} uptime_secs={}", TAG, version, uptime_secs);
            let baseline = crate::metrics::snapshot().to_baseline_log_line();
            log::info!("[{}] {}", TAG, baseline);

            let content = read_heartbeat();
            if !has_pending_tasks(&content) {
                continue;
            }
            let Some(task_content) = first_pending_task(&content) else {
                continue;
            };
            let should_inject = {
                let guard = LAST_TASK_INJECT.get_or_init(|| Mutex::new((String::new(), None)));
                let mut g = guard.lock().expect("heartbeat LAST_TASK_INJECT lock");
                let (last_content, last_time) = (&g.0, g.1);
                let same = last_content == &task_content;
                let within =
                    last_time.map(|t| t.elapsed() < Duration::from_secs(TASK_THROTTLE_SECS)).unwrap_or(false);
                if same && within {
                    false
                } else {
                    *g = (task_content.clone(), Some(Instant::now()));
                    true
                }
            };
            if !should_inject {
                continue;
            }
            let msg = match crate::bus::PcMsg::new(
                "heartbeat",
                "heartbeat",
                "请根据 HEARTBEAT.md 中的待办事项执行并更新文件。",
            ) {
                Ok(m) => m,
                Err(e) => {
                    log::warn!("[{}] PcMsg::new failed: {}", TAG, e);
                    continue;
                }
            };
            if inbound_tx.send(msg).is_err() {
                log::warn!("[{}] inbound_tx.send failed (channel closed?)", TAG);
            }
        }
    });
    log::info!("[{}] heartbeat loop with tasks started (interval {}s)", TAG, interval_secs);
}
