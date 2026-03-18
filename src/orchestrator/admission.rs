//! 四维门禁决策：入站/出站/LLM/工具，基于统一资源快照做全局协调。
//! Four-dimensional admission: inbound/outbound/LLM/tool, coordinated via unified resource snapshot.

use crate::constants::{
    LOW_MEM_DEFER_SLEEP_MS, OUTBOUND_DEFER_DELAY_MS, TLS_ADMISSION_MIN_INTERNAL_BYTES,
    TLS_ADMISSION_MIN_LARGEST_BLOCK_BYTES,
};
use std::sync::atomic::Ordering;

use super::pressure::PressureLevel;
use super::state::OrchestratorState;

/// 入站/出站通用决策。
/// Common admission decision for inbound/outbound.
pub enum AdmissionDecision {
    Accept,
    Defer { delay_ms: u64 },
    Reject { reason: &'static str },
}

/// LLM 调用门控决策。
/// LLM call gating decision.
pub enum LlmDecision {
    Proceed,
    RetryLater { delay_ms: u64 },
    Degrade { reason: &'static str },
}

/// 工具执行门控决策。
/// Tool execution gating decision.
pub enum ToolDecision {
    Allow,
    Deny { reason: &'static str },
}

/// agent loop 收到消息后、处理前调用。
/// Called by agent loop after receiving a message, before processing.
pub fn should_accept_inbound(
    state: &OrchestratorState,
    _channel: &str,
    chat_id: &str,
) -> AdmissionDecision {
    let pressure =
        PressureLevel::from_byte(state.pressure_level.load(Ordering::Relaxed));
    let is_cron = chat_id == "cron";

    match pressure {
        PressureLevel::Critical => {
            if is_cron {
                return AdmissionDecision::Reject {
                    reason: "critical_pressure_background",
                };
            }
            AdmissionDecision::Defer {
                delay_ms: LOW_MEM_DEFER_SLEEP_MS,
            }
        }
        PressureLevel::Cautious => {
            if is_cron {
                return AdmissionDecision::Reject {
                    reason: "cautious_cron_skip",
                };
            }
            AdmissionDecision::Accept
        }
        PressureLevel::Normal => AdmissionDecision::Accept,
    }
}

/// agent 准备调用 LLM 前调用。
/// Called by agent before invoking LLM.
pub fn can_call_llm(state: &OrchestratorState) -> LlmDecision {
    let pressure =
        PressureLevel::from_byte(state.pressure_level.load(Ordering::Relaxed));

    match pressure {
        PressureLevel::Critical => LlmDecision::Degrade {
            reason: "critical_pressure",
        },
        PressureLevel::Cautious => {
            let largest_block = state.heap_largest_block.load(Ordering::Relaxed);
            if largest_block < TLS_ADMISSION_MIN_LARGEST_BLOCK_BYTES as u32 {
                LlmDecision::RetryLater { delay_ms: 3000 }
            } else {
                LlmDecision::Proceed
            }
        }
        PressureLevel::Normal => LlmDecision::Proceed,
    }
}

/// agent 准备执行工具前调用；`requires_network` 由调用方从 ToolRegistry 推导。
/// Called by agent before executing a tool; `requires_network` is derived from ToolRegistry by the caller.
pub fn can_execute_tool(state: &OrchestratorState, _tool_name: &str, requires_network: bool) -> ToolDecision {
    let pressure =
        PressureLevel::from_byte(state.pressure_level.load(Ordering::Relaxed));

    match pressure {
        PressureLevel::Critical => {
            if requires_network {
                ToolDecision::Deny {
                    reason: "critical_no_network_tools",
                }
            } else {
                ToolDecision::Allow
            }
        }
        PressureLevel::Cautious => {
            if requires_network {
                let internal = state.heap_free_internal.load(Ordering::Relaxed);
                if internal < TLS_ADMISSION_MIN_INTERNAL_BYTES as u32 {
                    return ToolDecision::Deny {
                        reason: "cautious_low_heap_for_http_tool",
                    };
                }
            }
            ToolDecision::Allow
        }
        PressureLevel::Normal => ToolDecision::Allow,
    }
}

/// dispatch 发送前调用。出站消息已消耗 LLM 计算资源，优先 Defer 而非 Reject。
/// Called by dispatch before sending. Outbound messages already consumed LLM compute; prefer Defer over Reject.
pub fn should_accept_outbound(state: &OrchestratorState, _channel: &str) -> AdmissionDecision {
    let pressure = PressureLevel::from_byte(state.pressure_level.load(Ordering::Relaxed));
    match pressure {
        PressureLevel::Critical => AdmissionDecision::Defer {
            delay_ms: OUTBOUND_DEFER_DELAY_MS,
        },
        PressureLevel::Cautious | PressureLevel::Normal => AdmissionDecision::Accept,
    }
}
