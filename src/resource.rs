//! 运行时资源压力分级：根据堆状态计算 Normal/Cautious/Critical，各子系统据此调整预算与策略。
//! 供 agent 上下文、HTTP 缓冲、WSS 退避及 LLM system prompt 注入使用。

use crate::constants::{
    DEFAULT_MESSAGES_MAX_LEN, DEFAULT_SYSTEM_MAX_LEN, MAX_RESPONSE_BODY_LEN,
};
use std::sync::atomic::{AtomicU8, Ordering};

/// 压力等级：Normal 全量预算，Cautious 缩减，Critical 最低预算并积极丢弃。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PressureLevel {
    Normal = 0,
    Cautious = 1,
    Critical = 2,
}

impl PressureLevel {
    fn from_byte(b: u8) -> Self {
        match b {
            0 => PressureLevel::Normal,
            1 => PressureLevel::Cautious,
            _ => PressureLevel::Critical,
        }
    }
}

/// 当前压力下各子系统的预算与策略。由 `current_budget()` 返回，无锁只读。
#[derive(Clone)]
pub struct ResourceBudget {
    pub level: PressureLevel,
    pub system_prompt_max: usize,
    pub messages_max: usize,
    pub response_body_max: usize,
    pub should_accept_inbound: bool,
    pub reconnect_backoff_secs: u64,
    pub llm_hint: &'static str,
}

const NORMAL_INTERNAL_KB: usize = 70;
const NORMAL_PSRAM_MB: usize = 4;
const CAUTIOUS_INTERNAL_KB: usize = 48;
const CAUTIOUS_PSRAM_MB: usize = 1;

fn budget_for_level(level: PressureLevel) -> ResourceBudget {
    match level {
        PressureLevel::Normal => ResourceBudget {
            level: PressureLevel::Normal,
            system_prompt_max: DEFAULT_SYSTEM_MAX_LEN,
            messages_max: DEFAULT_MESSAGES_MAX_LEN,
            response_body_max: MAX_RESPONSE_BODY_LEN,
            should_accept_inbound: true,
            reconnect_backoff_secs: 5,
            llm_hint: "[device: ok]",
        },
        PressureLevel::Cautious => ResourceBudget {
            level: PressureLevel::Cautious,
            system_prompt_max: 16 * 1024,
            messages_max: 12 * 1024,
            response_body_max: 256 * 1024,
            should_accept_inbound: true,
            reconnect_backoff_secs: 15,
            llm_hint: "[device: memory-constrained — prefer concise replies, avoid heavy tool calls like web_search/fetch_url]",
        },
        PressureLevel::Critical => ResourceBudget {
            level: PressureLevel::Critical,
            system_prompt_max: 8 * 1024,
            messages_max: 6 * 1024,
            response_body_max: 128 * 1024,
            should_accept_inbound: false,
            reconnect_backoff_secs: 30,
            llm_hint: "[device: critical — reply in 1-2 sentences only, no tool calls]",
        },
    }
}

static CURRENT_LEVEL: AtomicU8 = AtomicU8::new(0);

/// 根据当前堆状态更新压力等级并写入全局状态。由 heartbeat 定期调用；main 初始化时调一次。
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub fn update() {
    use crate::platform::heap::{heap_free_internal, heap_free_spiram};

    let internal = heap_free_internal();
    let psram = heap_free_spiram();
    let internal_kb = internal / 1024;
    let psram_mb = psram / (1024 * 1024);

    let level = if internal_kb >= NORMAL_INTERNAL_KB && psram_mb >= NORMAL_PSRAM_MB {
        PressureLevel::Normal
    } else if internal_kb >= CAUTIOUS_INTERNAL_KB && psram_mb >= CAUTIOUS_PSRAM_MB {
        PressureLevel::Cautious
    } else {
        PressureLevel::Critical
    };
    CURRENT_LEVEL.store(level as u8, Ordering::Relaxed);
}

/// 非 ESP 目标：不采样堆，保持 Normal。
#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub fn update() {
    CURRENT_LEVEL.store(PressureLevel::Normal as u8, Ordering::Relaxed);
}

/// 返回当前压力对应的预算与策略，无锁只读，可被任意线程/任务频繁调用。
pub fn current_budget() -> ResourceBudget {
    let level = PressureLevel::from_byte(CURRENT_LEVEL.load(Ordering::Relaxed));
    budget_for_level(level)
}
