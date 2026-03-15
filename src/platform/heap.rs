//! ESP 堆查询与 PSRAM 大块分配：供 OOM 前检查、HTTP 响应体与可观测性复用。
//! Heap query and PSRAM allocation for ESP.

use crate::constants::{MIN_FREE_HEAP_FOR_AGENT_ROUND, MIN_FREE_INTERNAL_WHEN_PSRAM};

/// 返回当前内部堆空闲字节数。仅 ESP 目标有效；非 ESP 返回 u32::MAX（视为充足）。
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub fn heap_free_internal() -> usize {
    unsafe {
        esp_idf_svc::sys::heap_caps_get_free_size(esp_idf_svc::sys::MALLOC_CAP_INTERNAL) as usize
    }
}

/// 返回当前 PSRAM 空闲字节数。仅 ESP 目标有效；无 PSRAM 时返回 0。
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub fn heap_free_spiram() -> usize {
    unsafe {
        esp_idf_svc::sys::heap_caps_get_free_size(esp_idf_svc::sys::MALLOC_CAP_SPIRAM) as usize
    }
}

/// 返回 internal 堆当前最大连续空闲块（字节）。仅 ESP 有效；非 ESP 返回 usize::MAX。供 TLS 准入碎片化判定。
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub fn heap_largest_free_block_internal() -> usize {
    unsafe {
        esp_idf_svc::sys::heap_caps_get_largest_free_block(esp_idf_svc::sys::MALLOC_CAP_INTERNAL)
            as usize
    }
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub fn heap_free_internal() -> usize {
    usize::MAX
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub fn heap_free_spiram() -> usize {
    0
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub fn heap_largest_free_block_internal() -> usize {
    usize::MAX
}

/// 无 PSRAM 且内部堆低于阈值时返回 true，调用方应拒绝或降级大分配（当前仅 S3 支持，恒有 PSRAM，此分支作防御保留）。
/// 阈值与 MIN_FREE_HEAP_FOR_AGENT_ROUND 一致，避免在无法容纳大响应体时仍尝试读入。
#[inline]
pub fn is_low_memory_no_spiram() -> bool {
    #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
    {
        heap_free_internal() < MIN_FREE_HEAP_FOR_AGENT_ROUND && heap_free_spiram() == 0
    }
    #[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
    {
        false
    }
}

/// 当前堆是否足以跑一轮 agent。有 PSRAM（S3）时要求 internal >= MIN_FREE_INTERNAL_WHEN_PSRAM（双 TLS 预留）；
/// 无 PSRAM 时要求 internal >= MIN_FREE_HEAP_FOR_AGENT_ROUND（96K，当前未使用）。非 ESP 返回 true。
#[inline]
pub fn is_heap_ok_for_agent_round() -> bool {
    #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
    {
        if heap_free_spiram() > 0 {
            heap_free_internal() >= MIN_FREE_INTERNAL_WHEN_PSRAM
        } else {
            heap_free_internal() >= MIN_FREE_HEAP_FOR_AGENT_ROUND
        }
    }
    #[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
    {
        true
    }
}

/// S3 上从 PSRAM 分配大块缓冲区；无 PSRAM 或失败返回 None。调用方负责 heap_caps_free。
#[cfg(target_arch = "xtensa")]
pub fn alloc_spiram_buffer(size: usize) -> Option<*mut u8> {
    let ptr = unsafe {
        esp_idf_svc::sys::heap_caps_malloc(size, esp_idf_svc::sys::MALLOC_CAP_SPIRAM)
    };
    if ptr.is_null() {
        None
    } else {
        Some(ptr as *mut u8)
    }
}

#[cfg(target_arch = "xtensa")]
pub unsafe fn free_spiram_buffer(ptr: *mut u8) {
    if !ptr.is_null() {
        esp_idf_svc::sys::heap_caps_free(ptr as *mut core::ffi::c_void);
    }
}

#[cfg(not(target_arch = "xtensa"))]
pub fn alloc_spiram_buffer(_size: usize) -> Option<*mut u8> {
    None
}

#[cfg(not(target_arch = "xtensa"))]
pub unsafe fn free_spiram_buffer(_ptr: *mut u8) {}
