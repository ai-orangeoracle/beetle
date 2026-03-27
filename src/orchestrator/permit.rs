//! HTTP 准入令牌：优先级 + TLS 单并发 + 堆检查，合并 tls_admission.rs 功能。
//! HTTP admission permit: priority + TLS single-concurrency + heap check, merging tls_admission.rs.

use crate::constants::MAX_CONCURRENT_HTTP;
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
use crate::constants::{
    TLS_ADMISSION_MIN_INTERNAL_BYTES, TLS_ADMISSION_MIN_LARGEST_BLOCK_BYTES,
    TLS_ADMISSION_NO_PSRAM_MIN_BYTES,
};
use crate::error::{Error, Result};
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::time::Duration;
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
use std::time::Instant;

use super::pressure::PressureLevel;
use super::state::OrchestratorState;

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
const TRY_INTERVAL_MS: u64 = 50;

/// HTTP 请求优先级。
/// HTTP request priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low = 0,      // cron、remind_at 等后台任务
    Normal = 1,   // 通道 sender（发消息）
    High = 2,     // agent LLM 请求（用户等待中）
    Critical = 3, // 健康检查、配置 API
}

/// RAII guard：Drop 时递减 active_http_count + 释放 TLS 令牌。
/// RAII guard: decrements active_http_count + releases TLS permit on drop.
pub struct HttpPermitGuard {
    state: &'static OrchestratorState,
    _tls_guard: Option<std::sync::MutexGuard<'static, ()>>,
}

impl Drop for HttpPermitGuard {
    fn drop(&mut self) {
        self.state.active_http_count.fetch_sub(1, Ordering::Relaxed);
    }
}

/// RAII guard：持有期间 `active_agent_tasks` 非零，Drop 时递减。
/// Held for the full lifetime of a single agent task (from admission to reply sent).
/// RAII guard: keeps `active_agent_tasks` > 0 while held, decrements on drop.
pub struct AgentTaskGuard {
    state: &'static OrchestratorState,
}

impl AgentTaskGuard {
    pub(super) fn new(state: &'static OrchestratorState) -> Self {
        state.active_agent_tasks.fetch_add(1, Ordering::Relaxed);
        Self { state }
    }
}

impl Drop for AgentTaskGuard {
    fn drop(&mut self) {
        self.state.active_agent_tasks.fetch_sub(1, Ordering::Relaxed);
    }
}

/// 请求 HTTP 准入令牌。
/// Request HTTP admission permit.
pub fn request_http_permit(
    state: &'static OrchestratorState,
    tls_permit: &'static Mutex<()>,
    priority: Priority,
    timeout: Duration,
) -> Result<HttpPermitGuard> {
    let pressure = PressureLevel::from_byte(state.pressure_level.load(Ordering::Relaxed));

    // 快速路径：Critical 压力下仅放行 Critical/High 优先级
    if pressure == PressureLevel::Critical && priority < Priority::High {
        return Err(Error::Other {
            source: Box::new(std::io::Error::other(
                "critical pressure, low priority rejected",
            )),
            stage: "tls_admission",
        });
    }

    // 检查 active_http_count：若 >= MAX_CONCURRENT_HTTP，低优先级直接拒绝
    let active = state.active_http_count.load(Ordering::Relaxed);
    if active >= MAX_CONCURRENT_HTTP as u32 && priority < Priority::High {
        return Err(Error::Other {
            source: Box::new(std::io::Error::other(format!(
                "max concurrent HTTP reached ({}/{}), low priority rejected",
                active, MAX_CONCURRENT_HTTP
            ))),
            stage: "tls_admission",
        });
    }

    #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
    let tls_guard = {
        // 获取 TLS 单并发令牌（Mutex::try_lock 循环，超时返回错误）
        let start = Instant::now();
        let guard = loop {
            match tls_permit.try_lock() {
                Ok(guard) => break guard,
                Err(std::sync::TryLockError::Poisoned(e)) => {
                    log::warn!("[orchestrator::permit] TLS permit mutex was poisoned, recovering");
                    tls_permit.clear_poison();
                    break e.into_inner();
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    if start.elapsed() >= timeout {
                        return Err(Error::Other {
                            source: Box::new(std::io::Error::new(
                                std::io::ErrorKind::TimedOut,
                                "tls admission permit timeout",
                            )),
                            stage: "tls_admission",
                        });
                    }
                    crate::platform::task_wdt::feed_current_task();
                    std::thread::sleep(Duration::from_millis(TRY_INTERVAL_MS));
                }
            }
        };
        Some(guard)
    };
    #[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
    let tls_guard = {
        let _ = (tls_permit, timeout);
        None
    };

    // 堆检查
    check_internal_heap_for_tls(state)?;

    // 递增 active_http_count，返回 RAII guard
    state.active_http_count.fetch_add(1, Ordering::Relaxed);
    Ok(HttpPermitGuard {
        state,
        _tls_guard: tls_guard,
    })
}

/// 检查 internal 堆是否满足单次 TLS 准入（实时读取堆状态，非原子缓存）。
/// Check if internal heap meets TLS admission requirements (live heap query, not cached).
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
fn check_internal_heap_for_tls(_state: &OrchestratorState) -> Result<()> {
    let snap = super::memory_snapshot_live();
    let free = snap.heap_free_internal;
    let largest = snap.heap_largest_block;
    let spiram = snap.heap_free_spiram;
    let min_free = if spiram > 0 {
        TLS_ADMISSION_MIN_INTERNAL_BYTES as u32
    } else {
        TLS_ADMISSION_NO_PSRAM_MIN_BYTES as u32
    };
    if free < min_free {
        return Err(Error::Other {
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::OutOfMemory,
                format!(
                    "internal heap too low for TLS: free={} min={} spiram={}",
                    free, min_free, spiram
                ),
            )),
            stage: "tls_admission",
        });
    }
    if spiram > 0 && largest < TLS_ADMISSION_MIN_LARGEST_BLOCK_BYTES as u32 {
        return Err(Error::Other {
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::OutOfMemory,
                format!(
                    "internal heap fragmented for TLS: largest={} min={}",
                    largest, TLS_ADMISSION_MIN_LARGEST_BLOCK_BYTES
                ),
            )),
            stage: "tls_admission",
        });
    }
    Ok(())
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
fn check_internal_heap_for_tls(_state: &OrchestratorState) -> Result<()> {
    Ok(())
}
