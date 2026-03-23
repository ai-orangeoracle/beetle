//! Control STA connect/scan via wpa_cli.

use crate::constants::{WIFI_CONNECT_TIMEOUT_SECS, WIFI_RETRY_BACKOFF_SECS};
use crate::error::{Error, Result};
use crate::platform::state_mount_path;
use crate::platform::wifi::linux_ctrl::net;
use crate::platform::wifi::linux_ctrl::process::{run_checked, write_secure_atomic};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const CMD_TIMEOUT: Duration = Duration::from_secs(8);

fn conf_dir() -> PathBuf {
    state_mount_path().join("wifi/linux")
}

fn wpa_conf_path(iface: &str) -> PathBuf {
    conf_dir().join(format!("wpa_supplicant-{}.conf", iface))
}

/// `wpa_supplicant -P` 写入的 PID 路径（与 [`ensure_daemon`] 一致）。
pub fn supplicant_pid_path(iface: &str) -> PathBuf {
    conf_dir().join(format!("wpa_supplicant-{}.pid", iface))
}

pub fn ensure_daemon(iface: &str) -> Result<()> {
    if run_checked(
        "wpa_cli",
        &["-i", iface, "ping"],
        Duration::from_secs(3),
        "wifi_wpa_cmd",
    )
    .is_ok()
    {
        return Ok(());
    }

    let conf = b"ctrl_interface=/var/run/wpa_supplicant\nupdate_config=1\n";
    let conf_path = wpa_conf_path(iface);
    write_secure_atomic(&conf_path, conf, "wifi_wpa_config")?;
    run_checked(
        "wpa_supplicant",
        &[
            "-B",
            "-i",
            iface,
            "-P",
            supplicant_pid_path(iface).to_string_lossy().as_ref(),
            "-c",
            conf_path.to_string_lossy().as_ref(),
        ],
        Duration::from_secs(10),
        "wifi_wpa_start",
    )?;
    Ok(())
}

pub fn connect_sta(iface: &str, ssid: &str, pass: &str) -> Result<Option<String>> {
    validate_ssid(ssid)?;
    validate_pass(pass)?;
    ensure_daemon(iface)?;

    run_checked(
        "wpa_cli",
        &["-i", iface, "remove_network", "all"],
        CMD_TIMEOUT,
        "wifi_wpa_cmd",
    )?;
    let add = run_checked(
        "wpa_cli",
        &["-i", iface, "add_network"],
        CMD_TIMEOUT,
        "wifi_wpa_cmd",
    )?;
    let net_id = add.stdout.trim();
    if net_id.is_empty() {
        return Err(Error::config(
            "wifi_wpa_cmd",
            "add_network returned empty id",
        ));
    }
    run_checked(
        "wpa_cli",
        &["-i", iface, "set_network", net_id, "ssid", &quote_wpa(ssid)],
        CMD_TIMEOUT,
        "wifi_wpa_cmd",
    )?;
    if pass.is_empty() {
        run_checked(
            "wpa_cli",
            &["-i", iface, "set_network", net_id, "key_mgmt", "NONE"],
            CMD_TIMEOUT,
            "wifi_wpa_cmd",
        )?;
    } else {
        run_checked(
            "wpa_cli",
            &["-i", iface, "set_network", net_id, "psk", &quote_wpa(pass)],
            CMD_TIMEOUT,
            "wifi_wpa_cmd",
        )?;
    }
    run_checked(
        "wpa_cli",
        &["-i", iface, "enable_network", net_id],
        CMD_TIMEOUT,
        "wifi_wpa_cmd",
    )?;
    run_checked(
        "wpa_cli",
        &["-i", iface, "reconnect"],
        CMD_TIMEOUT,
        "wifi_wpa_cmd",
    )?;

    let deadline = std::time::Instant::now() + Duration::from_secs(WIFI_CONNECT_TIMEOUT_SECS);
    loop {
        let status = run_checked(
            "wpa_cli",
            &["-i", iface, "status"],
            Duration::from_secs(3),
            "wifi_wpa_cmd",
        )?;
        if status.stdout.contains("wpa_state=COMPLETED") {
            return net::read_sta_ip(iface);
        }
        if std::time::Instant::now() >= deadline {
            return Err(Error::config(
                "wifi_connect",
                format!("STA timeout after {}s", WIFI_CONNECT_TIMEOUT_SECS),
            ));
        }
        std::thread::sleep(Duration::from_secs(WIFI_RETRY_BACKOFF_SECS[0]));
    }
}

/// 在 `deadline` 前完成一次扫描；各子步骤超时不超过剩余时间，避免总墙钟超过 `WIFI_SCAN_TIMEOUT_SECS`。
pub fn scan_bounded(iface: &str, deadline: Instant) -> Result<Vec<crate::platform::WifiApEntry>> {
    fn remain(deadline: Instant) -> Duration {
        deadline.saturating_duration_since(Instant::now())
    }
    ensure_daemon(iface)?;
    if remain(deadline).is_zero() {
        return Err(Error::config("wifi_scan", "scan timeout"));
    }
    let t_scan = remain(deadline).min(CMD_TIMEOUT);
    run_checked("wpa_cli", &["-i", iface, "scan"], t_scan, "wifi_scan")?;
    let sleep_dur = remain(deadline).min(Duration::from_millis(800));
    if !sleep_dur.is_zero() {
        std::thread::sleep(sleep_dur);
    }
    if remain(deadline).is_zero() {
        return Err(Error::config("wifi_scan", "scan timeout"));
    }
    let t_res = remain(deadline).min(Duration::from_secs(5));
    let out = run_checked(
        "wpa_cli",
        &["-i", iface, "scan_results"],
        t_res,
        "wifi_scan",
    )?;
    parse_scan_results(&out.stdout)
}

fn parse_scan_results(raw: &str) -> Result<Vec<crate::platform::WifiApEntry>> {
    let mut out = Vec::new();
    for line in raw.lines().skip(1) {
        // bssid / freq / signal / flags / ssid
        let mut parts = line.split('\t');
        let _bssid = parts.next();
        let _freq = parts.next();
        let Some(sig) = parts.next() else {
            continue;
        };
        let _flags = parts.next();
        let Some(ssid) = parts.next() else {
            continue;
        };
        if ssid.is_empty() {
            continue;
        }
        let Ok(rssi) = sig.parse::<i16>() else {
            continue;
        };
        let rssi_i8 = rssi.clamp(i8::MIN as i16, i8::MAX as i16) as i8;
        out.push(crate::platform::WifiApEntry {
            ssid: ssid.to_string(),
            rssi: rssi_i8,
        });
    }
    out.sort_by(|a, b| b.rssi.cmp(&a.rssi));
    Ok(out)
}

fn validate_ssid(ssid: &str) -> Result<()> {
    if ssid.is_empty() || ssid.len() > 64 || ssid.contains('\n') || ssid.contains('\r') {
        return Err(Error::config("wifi_connect", "invalid wifi_ssid"));
    }
    Ok(())
}

fn validate_pass(pass: &str) -> Result<()> {
    if pass.len() > 64 || pass.contains('\n') || pass.contains('\r') {
        return Err(Error::config("wifi_connect", "invalid wifi_pass"));
    }
    Ok(())
}

fn quote_wpa(v: &str) -> String {
    let escaped = v.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{}\"", escaped)
}
