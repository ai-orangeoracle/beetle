//! 单调时间抽象：ESP 用 esp_timer_get_time，host 用 std::time::Instant。

/// 系统启动后经过的秒数（单调递增）。
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub fn uptime_secs() -> u64 {
    let us = unsafe { esp_idf_svc::sys::esp_timer_get_time() };
    if us >= 0 { us as u64 / 1_000_000 } else { 0 }
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub fn uptime_secs() -> u64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static BOOT: OnceLock<Instant> = OnceLock::new();
    BOOT.get_or_init(Instant::now).elapsed().as_secs()
}
