//! Start/stop hostapd and dnsmasq for AP mode.

use crate::error::Result;
use crate::platform::state_mount_path;
use crate::platform::wifi::linux_ctrl::hostapd_ctrl;
use crate::platform::wifi::linux_ctrl::net;
use crate::platform::wifi::linux_ctrl::process::{run_checked, write_secure_atomic};
use crate::platform::wifi::linux_ctrl::HOSTAPD_CTRL_INTERFACE_DIR;
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

    let hostapd_conf_body = format!(
        "interface={iface}\ndriver=nl80211\nssid={ssid}\nhw_mode=g\nchannel=1\nauth_algs=1\nwpa=0\nctrl_interface={HOSTAPD_CTRL_INTERFACE_DIR}\n",
    );
    let hostapd_conf_file = hostapd_conf_path();
    write_secure_atomic(
        &hostapd_conf_file,
        hostapd_conf_body.as_bytes(),
        "wifi_ap_config",
    )?;

    let dnsmasq_conf_body = format!(
        "interface={iface}\nbind-interfaces\ndhcp-range={net_start},{net_end},255.255.255.0,12h\n",
        net_start = ap_pool_start(ip),
        net_end = ap_pool_end(ip),
    );
    let dnsmasq_conf_file = dnsmasq_conf_path();
    write_secure_atomic(
        &dnsmasq_conf_file,
        dnsmasq_conf_body.as_bytes(),
        "wifi_ap_config",
    )?;

    let hostapd_pid = pidfile("hostapd");
    let dnsmasq_pid = pidfile("dnsmasq");
    let _ = std::fs::remove_file(&hostapd_pid);
    let _ = std::fs::remove_file(&dnsmasq_pid);

    // Use owned `String` argv fragments (not `path().to_string_lossy().as_ref()` on temporaries):
    // dnsmasq 2.90 is strict about argv; unstable pointers produced "junk found in command line".
    let hostapd_pid_s = hostapd_pid.to_string_lossy().into_owned();
    let hostapd_conf_s = hostapd_conf_file.to_string_lossy().into_owned();
    run_checked(
        "hostapd",
        &["-B", "-P", hostapd_pid_s.as_str(), hostapd_conf_s.as_str()],
        CMD_TIMEOUT,
        "wifi_hostapd_start",
    )?;
    // Single-token `--opt=path` avoids any ambiguity with multi-arg parsing on embedded dnsmasq.
    let dnsmasq_cf = format!("--conf-file={}", dnsmasq_conf_file.display());
    let dnsmasq_pf = format!("--pid-file={}", dnsmasq_pid.display());
    run_checked(
        "dnsmasq",
        &[dnsmasq_cf.as_str(), dnsmasq_pf.as_str()],
        CMD_TIMEOUT,
        "wifi_dnsmasq_start",
    )?;
    Ok(())
}

/// 停止 AP 相关进程并清理 PID；`iface` 用于尝试删除 hostapd 控制 socket。
pub fn stop_ap(iface: &str) {
    hostapd_ctrl::try_terminate(iface, Duration::from_secs(3));
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
    let sock = Path::new(HOSTAPD_CTRL_INTERFACE_DIR).join(iface);
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
