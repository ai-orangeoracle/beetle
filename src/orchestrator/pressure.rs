//! 压力等级计算：吸收 resource.rs 的 PressureLevel + ResourceBudget 逻辑。
//! Pressure level computation: absorbs resource.rs PressureLevel + ResourceBudget.

use crate::constants::{
    DEFAULT_MESSAGES_MAX_LEN, DEFAULT_SYSTEM_MAX_LEN, MAX_RESPONSE_BODY_LEN,
};
use std::sync::atomic::Ordering;

use super::state::OrchestratorState;

/// 压力等级：Normal 全量预算，Cautious 缩减，Critical 最低预算并积极丢弃。
/// Pressure level: Normal = full budget, Cautious = reduced, Critical = minimal + aggressive drop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[repr(u8)]
pub enum PressureLevel {
    Normal = 0,
    Cautious = 1,
    Critical = 2,
}

impl PressureLevel {
    pub fn from_byte(b: u8) -> Self {
        match b {
            0 => PressureLevel::Normal,
            1 => PressureLevel::Cautious,
            _ => PressureLevel::Critical,
        }
    }
}

/// 当前压力下各子系统的预算与策略。由 `current_budget()` 返回，无锁只读。
/// Budget and strategy per pressure level. Returned by `current_budget()`, lock-free read-only.
#[derive(Clone, serde::Serialize)]
pub struct ResourceBudget {
    pub level: PressureLevel,
    pub system_prompt_max: usize,
    pub messages_max: usize,
    pub response_body_max: usize,
    pub reconnect_backoff_secs: u64,
    pub llm_hint: &'static str,
}

const NORMAL_INTERNAL_KB: u32 = 70;
const NORMAL_PSRAM_MB: u32 = 4;
const CAUTIOUS_INTERNAL_KB: u32 = 48;
const CAUTIOUS_PSRAM_MB: u32 = 1;

pub fn budget_for_level(level: PressureLevel) -> ResourceBudget {
    match level {
        PressureLevel::Normal => ResourceBudget {
            level: PressureLevel::Normal,
            system_prompt_max: DEFAULT_SYSTEM_MAX_LEN,
            messages_max: DEFAULT_MESSAGES_MAX_LEN,
            response_body_max: MAX_RESPONSE_BODY_LEN,
            reconnect_backoff_secs: 5,
            llm_hint: "[device: ok]",
        },
        PressureLevel::Cautious => ResourceBudget {
            level: PressureLevel::Cautious,
            system_prompt_max: 16 * 1024,
            messages_max: 12 * 1024,
            response_body_max: 256 * 1024,
            reconnect_backoff_secs: 15,
            llm_hint: "[device: memory-constrained — prefer concise replies, avoid heavy tool calls like web_search/fetch_url]",
        },
        PressureLevel::Critical => ResourceBudget {
            level: PressureLevel::Critical,
            system_prompt_max: 8 * 1024,
            messages_max: 6 * 1024,
            response_body_max: 128 * 1024,
            reconnect_backoff_secs: 30,
            llm_hint: "[device: critical — reply in 1-2 sentences only, no tool calls]",
        },
    }
}

/// 队列深度高压阈值：入站 + 出站总深度达 75% 总容量时视为拥塞。
/// Queue congestion threshold: total depth ≥ 75% of combined capacity (2 × DEFAULT_CAPACITY).
const QUEUE_CONGESTION_THRESHOLD: u32 = (crate::constants::DEFAULT_CAPACITY as u32) * 2 * 3 / 4;

/// 根据原子状态计算压力等级，含堆维度 + 连接数维度 + 队列深度维度。
/// Compute pressure level from atomic state, including heap + active connection + queue depth dimensions.
pub fn compute_pressure(state: &OrchestratorState) -> PressureLevel {
    let internal = state.heap_free_internal.load(Ordering::Relaxed);
    let spiram = state.heap_free_spiram.load(Ordering::Relaxed);
    let active_http = state.active_http_count.load(Ordering::Relaxed);
    let internal_kb = internal / 1024;
    let spiram_mb = spiram / (1024 * 1024);
    let queue_total = state.inbound_depth.load(Ordering::Relaxed)
        + state.outbound_depth.load(Ordering::Relaxed);

    // Critical: internal 低于 Cautious 阈值且 PSRAM 也低
    if internal_kb < CAUTIOUS_INTERNAL_KB && (spiram == 0 || spiram_mb < CAUTIOUS_PSRAM_MB) {
        return PressureLevel::Critical;
    }

    // Critical: 队列拥塞 + 堆不足同时出现
    if queue_total >= QUEUE_CONGESTION_THRESHOLD && internal_kb < NORMAL_INTERNAL_KB {
        return PressureLevel::Critical;
    }

    // Cautious: 堆不足 Normal 阈值
    if internal_kb < NORMAL_INTERNAL_KB || (spiram > 0 && spiram_mb < NORMAL_PSRAM_MB) {
        return PressureLevel::Cautious;
    }

    // Cautious: 连接数过高
    if active_http >= crate::constants::MAX_CONCURRENT_HTTP as u32 {
        return PressureLevel::Cautious;
    }

    // Cautious: 队列深度达到拥塞阈值
    if queue_total >= QUEUE_CONGESTION_THRESHOLD {
        return PressureLevel::Cautious;
    }

    PressureLevel::Normal
}
