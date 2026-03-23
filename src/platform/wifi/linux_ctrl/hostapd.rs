//! Start/stop hostapd and dnsmasq for AP mode.

use crate::error::Result;
use crate::platform::state_mount_path;
use crate::platform::wifi::linux_ctrl::net;
use crate::platform::wifi::linux_ctrl::process::{run_checked, write_secure_atomic};
use std::path::{Path, PathBuf};
use std::time::Duration;

const CMD_TIMEOUT: Duration = Duration::from_secs(10);

fn config_dir() -> PathBuf {
    state_mount_path().join("wifi/linux")
}

fn hostapd_conf_path() -> PathBuf {
    config_dir().join("hostapd.conf")
}

fn dnsmasq_conf_path() -> PathBuf {
    config_dir().join("dnsmasq.conf")
}

fn pidfile(name: &str) -> PathBuf {
    config_dir().join(format!("{}.pid", name))
}

/// 与 [`start_ap`] 写入位置一致的 PID 路径，供守护线程检查。
pub fn daemon_pid_path(name: &str) -> PathBuf {
    pidfile(name)
}

pub fn start_ap(iface: &str, ssid: &str, ip: &str) -> Result<()> {
    net::setup_ap_address(iface, &format!("{}/24", ip))?;

    let hostapd_conf = format!(
        "interface={iface}\ndriver=nl80211\nssid={ssid}\nhw_mode=g\nchannel=1\nauth_algs=1\nwpa=0\nctrl_interface=/var/run/hostapd\n",
    );
    write_secure_atomic(
        &hostapd_conf_path(),
        hostapd_conf.as_bytes(),
        "wifi_ap_config",
    )?;

    let dnsmasq_conf = format!(
        "interface={iface}\nbind-interfaces\ndhcp-range={net_start},{net_end},255.255.255.0,12h\n",
        net_start = ap_pool_start(ip),
        net_end = ap_pool_end(ip),
    );
    write_secure_atomic(
        &dnsmasq_conf_path(),
        dnsmasq_conf.as_bytes(),
        "wifi_ap_config",
    )?;

    let hostapd_pid = pidfile("hostapd");
    let dnsmasq_pid = pidfile("dnsmasq");
    let _ = std::fs::remove_file(&hostapd_pid);
    let _ = std::fs::remove_file(&dnsmasq_pid);

    run_checked(
        "hostapd",
        &[
            "-B",
            "-P",
            hostapd_pid.to_string_lossy().as_ref(),
            hostapd_conf_path().to_string_lossy().as_ref(),
        ],
        CMD_TIMEOUT,
        "wifi_hostapd_start",
    )?;
    run_checked(
        "dnsmasq",
        &[
            "--conf-file",
            dnsmasq_conf_path().to_string_lossy().as_ref(),
            "--pid-file",
            dnsmasq_pid.to_string_lossy().as_ref(),
        ],
        CMD_TIMEOUT,
        "wifi_dnsmasq_start",
    )?;
    Ok(())
}

/// 停止 AP 相关进程并清理 PID；`iface` 用于尝试删除 hostapd 控制 socket。
pub fn stop_ap(iface: &str) {
    for name in ["dnsmasq", "hostapd"] {
        let pid = pidfile(name);
        if let Ok(raw) = std::fs::read_to_string(&pid) {
            let p = raw.trim();
            if !p.is_empty() {
                let _ = run_checked(
                    "kill",
                    &["-TERM", p],
                    Duration::from_secs(3),
                    "wifi_ap_stop",
                );
            }
        }
        let _ = std::fs::remove_file(pid);
    }
    let sock = Path::new("/var/run/hostapd").join(iface);
    let _ = std::fs::remove_file(sock);
}

fn ap_pool_start(ip: &str) -> String {
    let mut parts: Vec<&str> = ip.split('.').collect();
    if parts.len() == 4 {
        parts[3] = "20";
        return parts.join(".");
    }
    "192.168.1.20".to_string()
}

fn ap_pool_end(ip: &str) -> String {
    let mut parts: Vec<&str> = ip.split('.').collect();
    if parts.len() == 4 {
        parts[3] = "180";
        return parts.join(".");
    }
    "192.168.1.180".to_string()
}
