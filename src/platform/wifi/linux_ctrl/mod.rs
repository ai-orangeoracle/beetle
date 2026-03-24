//! Linux WiFi control helpers (P0): capability, command runner, AP/STA operations.

/// 与 `wpa_supplicant` 最小配置中 `ctrl_interface=` 一致。
/// Matches `ctrl_interface=` in minimal `wpa_supplicant` config.
pub const WPA_CTRL_INTERFACE_DIR: &str = "/var/run/wpa_supplicant";
/// 与 [`hostapd::start_ap`] 写入的 `ctrl_interface=` 一致。
/// Matches `ctrl_interface=` written by [`hostapd::start_ap`].
pub const HOSTAPD_CTRL_INTERFACE_DIR: &str = "/var/run/hostapd";

pub mod capability;
mod ctrl_iface;
pub mod hostapd;
pub mod hostapd_ctrl;
pub mod iw_scan;
pub mod net;
mod net_rt;
pub mod process;
pub mod wpa;
pub mod wpa_ctrl;
