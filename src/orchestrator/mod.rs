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
use crate::platform::MemorySnapshot;
use std::sync::atomic::AtomicU32;
use std::sync::{Arc, Mutex, OnceLock};
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
/// 上次刷新堆状态的 uptime 秒数（AtomicU32 for xtensa compatibility）。
static LAST_REFRESH_SECS: AtomicU32 = AtomicU32::new(0);
/// refresh_heap_if_stale 使用的启动时刻基准。
static REFRESH_START: OnceLock<std::time::Instant> = OnceLock::new();

/// 装配期注入：`Platform::memory_snapshot` 闭包，由 `run_app` 注册一次。
/// Injected at assembly: `Platform::memory_snapshot` closure, registered once from `run_app`.
static MEMORY_SNAPSHOT_PROVIDER: OnceLock<Arc<dyn Fn() -> MemorySnapshot + Send + Sync>> =
    OnceLock::new();

/// 注册内存快照来源（幂等：仅首次成功）。须在首次调用 [`update_heap_state`] 之前调用。
/// Register memory snapshot source (first call wins). Must run before first [`update_heap_state`].
pub fn register_memory_snapshot_provider(f: Arc<dyn Fn() -> MemorySnapshot + Send + Sync>) {
    if MEMORY_SNAPSHOT_PROVIDER.set(f).is_err() {
        log::error!("[orchestrator] register_memory_snapshot_provider: already registered");
        debug_assert!(
            false,
            "memory snapshot provider must be registered at most once"
        );
    }
}

/// 实时取当前平台内存快照（TLS 准入、堆刷新共用）。未注册时 panic（装配错误）。
pub(crate) fn memory_snapshot_live() -> MemorySnapshot {
    MEMORY_SNAPSHOT_PROVIDER
        .get()
        .expect("memory snapshot provider not registered; call register_memory_snapshot_provider from run_app")()
}

/// 将快照写入 orchestrator 并重算压力等级。
pub(crate) fn apply_memory_snapshot(snap: MemorySnapshot) {
    use std::sync::atomic::Ordering;
    STATE.update_heap(
        snap.heap_free_internal,
        snap.heap_free_spiram,
        snap.heap_largest_block,
    );
    let level = pressure::compute_pressure(&STATE);
    STATE.pressure_level.store(level as u8, Ordering::Relaxed);
}

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
pub fn update_heap_state() {
    apply_memory_snapshot(memory_snapshot_live());
}

/// 若距上次刷新 ≥2s 则重新采样堆状态并返回最新压力等级，否则返回缓存值。
/// Refresh heap state if stale (≥2s since last refresh), otherwise return cached pressure.
const REFRESH_MIN_INTERVAL_SECS: u32 = 2;

pub fn refresh_heap_if_stale() -> PressureLevel {
    let start = REFRESH_START.get_or_init(std::time::Instant::now);
    let now_secs = start.elapsed().as_secs() as u32;
    let last = LAST_REFRESH_SECS.load(std::sync::atomic::Ordering::Relaxed);
    if now_secs.wrapping_sub(last) >= REFRESH_MIN_INTERVAL_SECS {
        LAST_REFRESH_SECS.store(now_secs, std::sync::atomic::Ordering::Relaxed);
        update_heap_state();
    }
    current_pressure()
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

/// 单行资源基线字符串，与 [`snapshot`] 及 `GET /api/resource` 字段一致，供心跳与串口对齐观测。
/// Single-line resource baseline aligned with [`snapshot`] and `GET /api/resource` for heartbeat/serial.
pub fn format_resource_baseline_line() -> String {
    let s = snapshot();
    format!(
        "resource pressure={:?} heap_internal={} heap_spiram={} heap_largest={} active_http={} inbound={} outbound={}",
        s.pressure,
        s.heap_free_internal,
        s.heap_free_spiram,
        s.heap_largest_block_internal,
        s.active_http_count,
        s.inbound_depth,
        s.outbound_depth,
    )
}

/// 请求 HTTP 准入令牌。
/// Request HTTP admission permit.
pub fn request_http_permit(priority: Priority, timeout: Duration) -> Result<HttpPermitGuard> {
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

/// 更新队列深度（由 heartbeat 定期调用）。
/// Update queue depth snapshot (called periodically by heartbeat).
pub fn update_queue_depth(inbound: u32, outbound: u32) {
    STATE
        .inbound_depth
        .store(inbound, std::sync::atomic::Ordering::Relaxed);
    STATE
        .outbound_depth
        .store(outbound, std::sync::atomic::Ordering::Relaxed);
}

/// 更新会话与存储指标（由 heartbeat 定期调用）。
/// Update session & storage metrics (called periodically by heartbeat).
pub fn update_session_storage(session_count: u32, storage_used_kb: u32, storage_total_kb: u32) {
    STATE
        .session_count
        .store(session_count, std::sync::atomic::Ordering::Relaxed);
    STATE
        .storage_used_kb
        .store(storage_used_kb, std::sync::atomic::Ordering::Relaxed);
    STATE
        .storage_total_kb
        .store(storage_total_kb, std::sync::atomic::Ordering::Relaxed);
}

/// LLM 调用门控。
pub fn can_call_llm_pub() -> LlmDecision {
    admission::can_call_llm(&STATE)
}

/// 工具执行门控。
pub fn can_execute_tool_pub(tool_name: &str, requires_network: bool) -> ToolDecision {
    admission::can_execute_tool(&STATE, tool_name, requires_network)
}

/// 出站门禁决策。
/// Outbound admission decision.
pub fn should_accept_outbound_pub(channel: &str) -> AdmissionDecision {
    admission::should_accept_outbound(&STATE, channel)
}

/// 启动时打印 TLS 准入基线。
/// Log TLS admission baseline at startup.
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub fn log_baseline() {
    use crate::constants::{
        TLS_ADMISSION_MIN_INTERNAL_BYTES, TLS_ADMISSION_MIN_LARGEST_BLOCK_BYTES,
    };
    let snap = memory_snapshot_live();
    log::info!(
        "[orchestrator] TLS admission baseline: internal_free={} largest_block={} spiram_free={} min_internal={} min_largest={}",
        snap.heap_free_internal,
        snap.heap_largest_block,
        snap.heap_free_spiram,
        TLS_ADMISSION_MIN_INTERNAL_BYTES,
        TLS_ADMISSION_MIN_LARGEST_BLOCK_BYTES
    );
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub fn log_baseline() {}
