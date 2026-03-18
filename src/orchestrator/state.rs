//! 原子状态聚合：堆、socket、压力等级、通道健康，全部固定大小 + 原子变量，零堆分配。
//! Atomic state aggregation: heap, socket, pressure, channel health — fixed-size + atomics, zero heap alloc.

use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};

use super::channel_health::ChannelHealthSlot;

/// 通道索引枚举，编译时确定，避免 HashMap + String 的堆分配。
/// Channel index enum, compile-time fixed, avoids HashMap + String heap allocation.
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum ChannelIndex {
    Telegram = 0,
    Feishu = 1,
    DingTalk = 2,
    WeCom = 3,
    QqChannel = 4,
}

pub const MAX_CHANNELS: usize = 5;

/// 通道名 → ChannelIndex 映射（编译时已知的 5 个通道）。
/// Channel name to index mapping (5 channels known at compile time).
pub fn channel_to_index(channel: &str) -> Option<ChannelIndex> {
    match channel {
        "telegram" => Some(ChannelIndex::Telegram),
        "feishu" => Some(ChannelIndex::Feishu),
        "dingtalk" => Some(ChannelIndex::DingTalk),
        "wecom" => Some(ChannelIndex::WeCom),
        "qq_channel" => Some(ChannelIndex::QqChannel),
        _ => None,
    }
}

/// Orchestrator 全局原子状态。零堆分配，仅使用 AtomicU32/AtomicU8（xtensa 兼容）。
/// Global atomic state. Zero heap alloc, only AtomicU32/AtomicU8 (xtensa compatible).
pub struct OrchestratorState {
    // 堆状态（heartbeat 定期更新）
    pub heap_free_internal: AtomicU32,
    pub heap_free_spiram: AtomicU32,
    pub heap_largest_block: AtomicU32,

    // 连接计数（permit acquire/release 时增减）
    pub active_http_count: AtomicU32,

    // 压力等级（由 update_heap_state 计算写入）
    pub pressure_level: AtomicU8,

    // 通道健康（channel_health.rs 管理）—— 固定大小数组，无堆分配
    pub channel_health: [ChannelHealthSlot; MAX_CHANNELS],

    // 队列深度（heartbeat 定期更新）
    pub inbound_depth: AtomicU32,
    pub outbound_depth: AtomicU32,
}

impl Default for OrchestratorState {
    fn default() -> Self {
        Self::new()
    }
}

impl OrchestratorState {
    pub const fn new() -> Self {
        Self {
            heap_free_internal: AtomicU32::new(u32::MAX),
            heap_free_spiram: AtomicU32::new(0),
            heap_largest_block: AtomicU32::new(u32::MAX),
            active_http_count: AtomicU32::new(0),
            pressure_level: AtomicU8::new(0), // PressureLevel::Normal
            channel_health: [
                ChannelHealthSlot::new(),
                ChannelHealthSlot::new(),
                ChannelHealthSlot::new(),
                ChannelHealthSlot::new(),
                ChannelHealthSlot::new(),
            ],
            inbound_depth: AtomicU32::new(0),
            outbound_depth: AtomicU32::new(0),
        }
    }

    /// 更新堆状态（由 heartbeat / update_heap_state 调用）。
    pub fn update_heap(&self, internal: u32, spiram: u32, largest_block: u32) {
        self.heap_free_internal.store(internal, Ordering::Relaxed);
        self.heap_free_spiram.store(spiram, Ordering::Relaxed);
        self.heap_largest_block.store(largest_block, Ordering::Relaxed);
    }
}

/// 全局资源快照（无锁原子读取）。
/// Global resource snapshot (lock-free atomic reads).
#[derive(serde::Serialize)]
pub struct ResourceSnapshot {
    pub pressure: super::pressure::PressureLevel,
    pub heap_free_internal: u32,
    pub heap_free_spiram: u32,
    pub active_http_count: u32,
    pub inbound_depth: u32,
    pub outbound_depth: u32,
    pub budget: super::pressure::ResourceBudget,
}

impl ResourceSnapshot {
    pub fn from_state(state: &OrchestratorState) -> Self {
        let pressure = super::pressure::PressureLevel::from_byte(
            state.pressure_level.load(Ordering::Relaxed),
        );
        Self {
            pressure,
            heap_free_internal: state.heap_free_internal.load(Ordering::Relaxed),
            heap_free_spiram: state.heap_free_spiram.load(Ordering::Relaxed),
            active_http_count: state.active_http_count.load(Ordering::Relaxed),
            inbound_depth: state.inbound_depth.load(Ordering::Relaxed),
            outbound_depth: state.outbound_depth.load(Ordering::Relaxed),
            budget: super::pressure::budget_for_level(pressure),
        }
    }
}
