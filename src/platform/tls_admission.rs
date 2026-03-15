//! 统一 TLS 准入控制：单并发令牌 + internal 堆/连续块检查，替代固定 128KB 硬拒绝。
//! 所有 HTTP TLS 与 WSS 建连前必须经此准入，错误使用结构化 stage `tls_admission`。

use crate::constants::{
    TLS_ADMISSION_MIN_INTERNAL_BYTES, TLS_ADMISSION_MIN_LARGEST_BLOCK_BYTES,
    TLS_ADMISSION_NO_PSRAM_MIN_BYTES,
};
use crate::error::{Error, Result};
use crate::platform::heap::{
    heap_free_internal, heap_free_spiram, heap_largest_free_block_internal,
};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const TAG: &str = "platform::tls_admission";
const TRY_INTERVAL_MS: u64 = 50;

static PERMIT: Mutex<()> = Mutex::new(());

/// TLS 准入所需 internal 空闲阈值（字节），供上层做与 TLS 同步的低内存判定。
#[inline]
pub const fn tls_min_internal_bytes() -> usize {
    TLS_ADMISSION_MIN_INTERNAL_BYTES
}

/// 持有 TLS 准入令牌的 RAII 守卫，drop 时释放。
pub struct TlsPermitGuard(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

/// 在超时内获取 TLS 准入令牌，获取失败返回 Err(stage: tls_admission)。
/// Mutex 中毒（前持有者 panic）时自动恢复。
pub fn acquire_tls_permit(timeout: Duration) -> Result<TlsPermitGuard> {
    let start = Instant::now();
    loop {
        match PERMIT.try_lock() {
            Ok(guard) => return Ok(TlsPermitGuard(guard)),
            Err(std::sync::TryLockError::Poisoned(pe)) => {
                log::warn!("[{}] TLS permit mutex poisoned, recovering", TAG);
                return Ok(TlsPermitGuard(pe.into_inner()));
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
                std::thread::sleep(Duration::from_millis(TRY_INTERVAL_MS));
            }
        }
    }
}

/// 检查当前 internal 堆是否满足单次 TLS 准入（空闲量 + 最大连续块）。
/// 有 PSRAM 时用较低阈值（mbedTLS 大部分走 SPIRAM），无 PSRAM 时用较高阈值（全 internal）。
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub fn check_internal_heap_for_tls() -> Result<()> {
    let free = heap_free_internal();
    let largest = heap_largest_free_block_internal();
    let spiram = heap_free_spiram();
    let min_free = if spiram > 0 {
        TLS_ADMISSION_MIN_INTERNAL_BYTES
    } else {
        TLS_ADMISSION_NO_PSRAM_MIN_BYTES
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
    if spiram > 0 && largest < TLS_ADMISSION_MIN_LARGEST_BLOCK_BYTES {
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
pub fn check_internal_heap_for_tls() -> Result<()> {
    Ok(())
}

/// 启动时打印一次 TLS 准入基线（internal 空闲、最大连续块、阈值），便于线上复盘。仅 ESP 执行。
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub fn log_baseline() {
    let free = heap_free_internal();
    let largest = heap_largest_free_block_internal();
    let spiram = heap_free_spiram();
    log::info!(
        "[{}] TLS admission baseline: internal_free={} largest_block={} spiram_free={} min_internal={} min_largest={}",
        TAG,
        free,
        largest,
        spiram,
        TLS_ADMISSION_MIN_INTERNAL_BYTES,
        TLS_ADMISSION_MIN_LARGEST_BLOCK_BYTES
    );
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub fn log_baseline() {}
