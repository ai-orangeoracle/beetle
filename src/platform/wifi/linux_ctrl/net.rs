//! Linux network operations for AP/STA.

use crate::error::{Error, Result};
use crate::platform::wifi::linux_ctrl::process::run_checked;
use std::time::Duration;

const CMD_TIMEOUT: Duration = Duration::from_secs(8);

pub fn setup_ap_address(iface: &str, cidr: &str) -> Result<()> {
    let _ = run_checked(
        "ip",
        &["addr", "flush", "dev", iface],
        CMD_TIMEOUT,
        "wifi_ap_ip_flush",
    );
    run_checked(
        "ip",
        &["addr", "add", cidr, "dev", iface],
        CMD_TIMEOUT,
        "wifi_ap_ip_add",
    )?;
    run_checked(
        "ip",
        &["link", "set", iface, "up"],
        CMD_TIMEOUT,
        "wifi_ap_link_up",
    )?;
    Ok(())
}

pub fn read_sta_ip(iface: &str) -> Result<Option<String>> {
    let out = run_checked(
        "ip",
        &["-4", "-o", "addr", "show", "dev", iface],
        CMD_TIMEOUT,
        "wifi_sta_ip",
    )?;
    for line in out.stdout.lines() {
        let Some(idx) = line.find(" inet ") else {
            continue;
        };
        let tail = &line[idx + 6..];
        let Some(first) = tail.split_whitespace().next() else {
            continue;
        };
        let ip = first.split('/').next().unwrap_or_default().trim();
        if !ip.is_empty() {
            return Ok(Some(ip.to_string()));
        }
    }
    Ok(None)
}

pub fn ensure_root_or_cap_net_admin() -> Result<()> {
    // P0: runtime check by probing privileged command.
    run_checked("ip", &["link", "show"], CMD_TIMEOUT, "wifi_permission")
        .map(|_| ())
        .map_err(|e| Error::config("wifi_permission", e.to_string()))
}
