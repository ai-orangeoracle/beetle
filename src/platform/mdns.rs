//! mDNS：WiFi 就绪后注册 hostname beetle，同 LAN 访问 http://beetle.local。
//! 仅 ESP 且已启用 espressif/mdns 组件时编译。

#![cfg(all(
    any(target_arch = "xtensa", target_arch = "riscv32"),
    esp_idf_comp_espressif__mdns_enabled
))]

use std::ffi::CString;

use crate::error::{Error, Result};

const TAG: &str = "platform::mdns";
const MDNS_HOSTNAME: &str = "beetle";

/// 初始化 mDNS 并设置 hostname。仅需在 WiFi 就绪后调用一次；失败返回 Err，不保留句柄。
pub fn init() -> Result<()> {
    let err = unsafe { esp_idf_svc::sys::mdns_init() };
    if err != esp_idf_svc::sys::ESP_OK {
        return Err(Error::esp("mdns_init", err));
    }
    let hostname = CString::new(MDNS_HOSTNAME).map_err(|e| Error::Other {
        source: Box::new(e),
        stage: "mdns_hostname",
    })?;
    let set_err = unsafe { esp_idf_svc::sys::mdns_hostname_set(hostname.as_ptr()) };
    if set_err != esp_idf_svc::sys::ESP_OK {
        return Err(Error::esp("mdns_hostname_set", set_err));
    }
    log::info!("[{}] hostname set: {}.local", TAG, MDNS_HOSTNAME);
    Ok(())
}
