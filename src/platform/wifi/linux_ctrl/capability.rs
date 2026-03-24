//! Linux WiFi capability detection (`iw`).

use crate::error::{Error, Result};
use crate::platform::wifi::linux_ctrl::process::run_checked;
use std::path::Path;
use std::time::Duration;

fn iface_exists(name: &str) -> bool {
    Path::new("/sys/class/net").join(name).exists()
}

const IW_SHORT: Duration = Duration::from_secs(5);
const IW_LONG: Duration = Duration::from_secs(8);

/// nl80211 能力摘要（用于启动前失败快、降级策略）。
#[derive(Clone, Debug)]
pub struct PhyCapabilities {
    /// 驱动报告支持 AP（含 AP/VLAN）。
    pub supports_ap: bool,
    /// `valid interface combinations` 中同 phy 上可同时存在 managed+AP 类组合（启发式）。
    pub supports_sta_ap_concurrent: bool,
    pub has_2ghz: bool,
    pub has_5ghz: bool,
}

pub fn detect_wifi_iface() -> Result<String> {
    for iface in ["wlan0", "wlan1"] {
        if iface_exists(iface) {
            return Ok(iface.to_string());
        }
    }
    Err(Error::config(
        "wifi_capability_check",
        "no wlan interface found (wlan0/wlan1)",
    ))
}

/// 解析 `wiphy N` → `phyN`。
fn wiphy_name_for_iface(iface: &str) -> Result<String> {
    let o = run_checked(
        "iw",
        &["dev", iface, "info"],
        IW_SHORT,
        "wifi_capability_check",
    )?;
    for line in o.stdout.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("wiphy ") {
            let n = rest.trim();
            if !n.is_empty() {
                return Ok(format!("phy{}", n));
            }
        }
    }
    Err(Error::config(
        "wifi_capability_check",
        "iw dev info: no wiphy line",
    ))
}

fn parse_supported_ap(phy_info: &str) -> bool {
    let mut in_modes = false;
    for line in phy_info.lines() {
        let t = line.trim_start();
        if t.starts_with("Supported interface modes") {
            in_modes = true;
            continue;
        }
        if in_modes {
            if t.starts_with("Band ")
                || t.starts_with("valid interface")
                || t.starts_with("Capabilities")
            {
                break;
            }
            let line_trim = line.trim();
            if line_trim.contains("* AP") || line_trim == "* AP" {
                return true;
            }
        }
    }
    false
}

/// 若 `valid interface combinations` 中任一行同时包含 managed 与 AP，则认为可做 AP+STA（同口并发常见前提）。
fn parse_concurrent_managed_ap(phy_info: &str) -> bool {
    let mut in_combo = false;
    for line in phy_info.lines() {
        let t = line.trim_start();
        if t.starts_with("valid interface combinations") {
            in_combo = true;
            continue;
        }
        if in_combo {
            let tr = line.trim();
            if tr.is_empty() {
                continue;
            }
            if tr.starts_with("Band ")
                || tr.starts_with("Frequencies:")
                || tr.starts_with("Supported commands")
            {
                break;
            }
            if tr.starts_with('*') && tr.contains("managed") && tr.contains("AP") {
                return true;
            }
        }
    }
    false
}

fn parse_bands(phy_info: &str) -> (bool, bool) {
    let mut g2 = false;
    let mut g5 = false;
    let mut in_freq = false;
    for line in phy_info.lines() {
        let t = line.trim();
        if t.starts_with("Band 1:") {
            g2 = true;
        }
        if t.starts_with("Band 2:") || t.starts_with("Band 3:") || t.starts_with("Band 4:") {
            g5 = true;
        }
        if t == "Frequencies:" {
            in_freq = true;
            continue;
        }
        if in_freq {
            if t.starts_with("Band ") {
                in_freq = false;
                continue;
            }
            if let Some(mhz_s) = t.split_whitespace().next() {
                if let Ok(mhz) = mhz_s.trim_end_matches(',').parse::<u32>() {
                    if (2400..=2500).contains(&mhz) {
                        g2 = true;
                    }
                    if mhz >= 4900 {
                        g5 = true;
                    }
                }
            }
        }
    }
    (g2, g5)
}

/// 对当前 `iface` 对应 phy 做 `iw phy … info` 解析。
pub fn probe_phy(iface: &str) -> Result<PhyCapabilities> {
    let phy = wiphy_name_for_iface(iface)?;
    let o = run_checked(
        "iw",
        &["phy", &phy, "info"],
        IW_LONG,
        "wifi_capability_check",
    )?;
    let body = &o.stdout;
    let supports_ap = parse_supported_ap(body);
    let supports_sta_ap_concurrent = parse_concurrent_managed_ap(body);
    let (mut has_2ghz, has_5ghz) = parse_bands(body);
    if !has_2ghz && !has_5ghz {
        has_2ghz = true;
    }
    Ok(PhyCapabilities {
        supports_ap,
        supports_sta_ap_concurrent,
        has_2ghz,
        has_5ghz,
    })
}
