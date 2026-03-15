//! 任务看门狗：将当前任务加入 TWDT，使 HTTP 请求与空闲等待时 feed 有效，避免 "task not found"。
//! Task watchdog: add current task to TWDT so feed/reset during HTTP or idle recv_timeout is valid.

/// 将当前任务加入任务看门狗。在运行 agent 循环（会发起长时间 HTTP）的线程中调用一次即可。
/// 若已加入（ESP_ERR_INVALID_STATE）或 TWDT 未启用则忽略返回值。
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub fn register_current_task_to_task_wdt() {
    const ESP_ERR_INVALID_STATE: i32 = 0x103;
    let ret = unsafe { esp_idf_svc::sys::esp_task_wdt_add(core::ptr::null_mut()) };
    if ret != 0 && ret != ESP_ERR_INVALID_STATE {
        log::warn!("[platform::task_wdt] esp_task_wdt_add failed: {}", ret);
    }
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub fn register_current_task_to_task_wdt() {}

/// 喂当前任务看门狗。agent 循环在 recv_timeout 超时后调用，避免长时间等消息时触发 TWDT。
#[cfg(all(
    any(target_arch = "xtensa", target_arch = "riscv32"),
    esp_idf_version_major = "4"
))]
pub fn feed_current_task() {
    unsafe {
        let _ = esp_idf_svc::sys::esp_task_wdt_feed();
    }
}

#[cfg(all(
    any(target_arch = "xtensa", target_arch = "riscv32"),
    not(esp_idf_version_major = "4")
))]
pub fn feed_current_task() {
    unsafe {
        let _ = esp_idf_svc::sys::esp_task_wdt_reset();
    }
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub fn feed_current_task() {}
