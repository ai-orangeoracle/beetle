//! SNTP 时间同步：WiFi STA 连接后调用 init_sntp() 启动后台同步。
//! ESP-IDF 5.x 使用 esp_netif_sntp API；同步成功后 gettimeofday / SystemTime 自动更新。
//! SNTP time sync: call init_sntp() after WiFi STA is connected.

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
use esp_idf_svc::sys;

const TAG: &str = "platform::sntp";

/// 启动 SNTP 后台同步（非阻塞）；WiFi 连接后调用一次即可。
/// ESP-IDF 5.x 使用 esp_sntp_setoperatingmode + esp_sntp_setservername + esp_sntp_init。
/// 同步成功后系统时钟自动更新，std::time::SystemTime 即为正确 UTC 时间。
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub fn init_sntp() {
    unsafe {
        // 停止已有实例（幂等）。
        sys::esp_sntp_stop();

        sys::esp_sntp_setoperatingmode(sys::esp_sntp_operatingmode_t_ESP_SNTP_OPMODE_POLL);

        // 设置 NTP 服务器（最多 CONFIG_LWIP_SNTP_MAX_SERVERS 个，默认 1）。
        let server = b"pool.ntp.org\0";
        sys::esp_sntp_setservername(0, server.as_ptr() as *const _);

        sys::esp_sntp_init();
    }
    log::info!(
        "[{}] SNTP started (pool.ntp.org), time will sync in background",
        TAG
    );
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub fn init_sntp() {
    log::info!("[{}] SNTP no-op on host", TAG);
}
