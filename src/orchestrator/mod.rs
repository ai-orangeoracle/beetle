//! 统一资源编排器：单一权威状态中心，所有资源决策基于统一快照。
//! Unified resource orchestrator: single authority for all resource decisions.
//!
//! 零堆分配、零锁（除 TLS 单并发 Mutex）、xtensa 兼容（仅 AtomicU32/AtomicU8）。
//! Zero heap alloc, zero locks (except TLS single-concurrency Mutex), xtensa compatible.

pub mod admission;
pub mod channel_health;
pub mod permit;
pub mod pressure;
pub mod state;

use crate::error::Result;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

pub use admission::{AdmissionDecision, LlmDecision, ToolDecision};
pub use channel_health::is_channel_healthy;
pub use permit::{HttpPermitGuard, Priority};
pub use pressure::{PressureLevel, ResourceBudget};
pub use state::ResourceSnapshot;

/// 全局单例 orchestrator 状态。
/// Global singleton orchestrator state.
static STATE: state::OrchestratorState = state::OrchestratorState::new();
static TLS_PERMIT: Mutex<()> = Mutex::new(());
static INITIALIZED: OnceLock<()> = OnceLock::new();

/// main 启动时调用一次，初始化 orchestrator（幂等）。
/// Called once by main at startup (idempotent).
pub fn init() {
    INITIALIZED.get_or_init(|| {
        update_heap_state();
        log::info!(
            "[orchestrator] initialized, pressure={:?}",
            current_pressure()
        );
    });
}

/// 更新堆状态并重算压力等级。由 heartbeat 定期调用。
/// Update heap state and recompute pressure level. Called periodically by heartbeat.
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub fn update_heap_state() {
    use crate::platform::heap::{
        heap_free_internal, heap_free_spiram, heap_largest_free_block_internal,
    };
    use std::sync::atomic::Ordering;

    let internal = heap_free_internal() as u32;
    let spiram = heap_free_spiram() as u32;
    let largest = heap_largest_free_block_internal() as u32;
    STATE.update_heap(internal, spiram, largest);
    let level = pressure::compute_pressure(&STATE);
    STATE
        .pressure_level
        .store(level as u8, Ordering::Relaxed);
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub fn update_heap_state() {
    use std::sync::atomic::Ordering;
    STATE.update_heap(u32::MAX, 0, u32::MAX);
    STATE
        .pressure_level
        .store(PressureLevel::Normal as u8, Ordering::Relaxed);
}

/// 返回当前压力等级。
pub fn current_pressure() -> PressureLevel {
    PressureLevel::from_byte(
        STATE
            .pressure_level
            .load(std::sync::atomic::Ordering::Relaxed),
    )
}

/// 返回当前压力对应的预算与策略，无锁只读。
/// Return budget for current pressure level, lock-free read-only.
pub fn current_budget() -> ResourceBudget {
    pressure::budget_for_level(current_pressure())
}

/// 返回全局资源快照（无锁原子读取）。
/// Return global resource snapshot (lock-free atomic reads).
pub fn snapshot() -> ResourceSnapshot {
    state::ResourceSnapshot::from_state(&STATE)
}

/// 请求 HTTP 准入令牌。
/// Request HTTP admission permit.
pub fn request_http_permit(
    priority: Priority,
    timeout: Duration,
) -> Result<HttpPermitGuard> {
    permit::request_http_permit(&STATE, &TLS_PERMIT, priority, timeout)
}

/// 记录通道发送结果（成功/失败）。
/// Record channel send result.
pub fn record_channel_result_pub(channel: &str, success: bool) {
    channel_health::record_channel_result(&STATE, channel, success);
}

/// 通道是否健康。
/// Whether channel is healthy.
pub fn is_channel_healthy_pub(channel: &str) -> bool {
    channel_health::is_channel_healthy(&STATE, channel)
}

/// 入站准入决策。
pub fn should_accept_inbound_pub(channel: &str, chat_id: &str) -> AdmissionDecision {
    admission::should_accept_inbound(&STATE, channel, chat_id)
}

/// LLM 调用门控。
pub fn can_call_llm_pub() -> LlmDecision {
    admission::can_call_llm(&STATE)
}

/// 工具执行门控。
pub fn can_execute_tool_pub(tool_name: &str) -> ToolDecision {
    admission::can_execute_tool(&STATE, tool_name)
}

/// 启动时打印 TLS 准入基线。
/// Log TLS admission baseline at startup.
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub fn log_baseline() {
    use crate::constants::{
        TLS_ADMISSION_MIN_INTERNAL_BYTES, TLS_ADMISSION_MIN_LARGEST_BLOCK_BYTES,
    };
    use crate::platform::heap::{
        heap_free_internal, heap_free_spiram, heap_largest_free_block_internal,
    };
    let free = heap_free_internal();
    let largest = heap_largest_free_block_internal();
    let spiram = heap_free_spiram();
    log::info!(
        "[orchestrator] TLS admission baseline: internal_free={} largest_block={} spiram_free={} min_internal={} min_largest={}",
        free,
        largest,
        spiram,
        TLS_ADMISSION_MIN_INTERNAL_BYTES,
        TLS_ADMISSION_MIN_LARGEST_BLOCK_BYTES
    );
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub fn log_baseline() {}
