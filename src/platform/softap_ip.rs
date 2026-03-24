//! SoftAP 固定 IP：在 WiFi AP 启动后强制设为 192.168.1.4/24，与 `constants::SOFTAP_*`、文档、配置页一致，连热点后使用该地址访问。
//! 仅 ESP 目标编译。

#![cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]

use crate::error::{Error, Result};

const TAG: &str = "platform::softap_ip";
const SOFTAP_IP: (u8, u8, u8, u8) = (192, 168, 1, 4);
const SOFTAP_NETMASK: (u8, u8, u8, u8) = (255, 255, 255, 0);
const WIFI_AP_DEF: &[u8] = b"WIFI_AP_DEF\0";

/// 将默认 SoftAP 网卡 IP 设为 192.168.1.4，子网掩码 255.255.255.0。须在 `wifi.start()` 之后调用。
pub fn set_softap_ip() -> Result<()> {
    let netif = unsafe {
        esp_idf_svc::sys::esp_netif_get_handle_from_ifkey(WIFI_AP_DEF.as_ptr() as *const _)
    };
    if netif.is_null() {
        return Err(Error::Other {
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "WIFI_AP_DEF netif not found",
            )),
            stage: "softap_ip",
        });
    }

    let err_stop = unsafe { esp_idf_svc::sys::esp_netif_dhcps_stop(netif) };
    if err_stop != esp_idf_svc::sys::ESP_OK {
        return Err(Error::esp("esp_netif_dhcps_stop", err_stop));
    }

    let mut ip_info: esp_idf_svc::sys::esp_netif_ip_info_t = unsafe { std::mem::zeroed() };
    unsafe {
        esp_idf_svc::sys::esp_netif_set_ip4_addr(
            &mut ip_info.ip,
            SOFTAP_IP.0,
            SOFTAP_IP.1,
            SOFTAP_IP.2,
            SOFTAP_IP.3,
        );
        esp_idf_svc::sys::esp_netif_set_ip4_addr(
            &mut ip_info.netmask,
            SOFTAP_NETMASK.0,
            SOFTAP_NETMASK.1,
            SOFTAP_NETMASK.2,
            SOFTAP_NETMASK.3,
        );
        esp_idf_svc::sys::esp_netif_set_ip4_addr(
            &mut ip_info.gw,
            SOFTAP_IP.0,
            SOFTAP_IP.1,
            SOFTAP_IP.2,
            SOFTAP_IP.3,
        );
    }

    let err_set = unsafe { esp_idf_svc::sys::esp_netif_set_ip_info(netif, &ip_info) };
    if err_set != esp_idf_svc::sys::ESP_OK {
        let _ = unsafe { esp_idf_svc::sys::esp_netif_dhcps_start(netif) };
        return Err(Error::esp("esp_netif_set_ip_info", err_set));
    }

    let err_start = unsafe { esp_idf_svc::sys::esp_netif_dhcps_start(netif) };
    if err_start != esp_idf_svc::sys::ESP_OK {
        return Err(Error::esp("esp_netif_dhcps_start", err_start));
    }

    log::info!(
        "[{}] SoftAP IP set to {}.{}.{}.{}",
        TAG,
        SOFTAP_IP.0,
        SOFTAP_IP.1,
        SOFTAP_IP.2,
        SOFTAP_IP.3
    );
    Ok(())
}
