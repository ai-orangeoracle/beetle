//! Linux embedded WiFi implementation (P0 + P1 守护/探测/降级).

use crate::config::AppConfig;
use crate::constants::{
    SOFTAP_DEFAULT_IPV4, SOFTAP_FALLBACK_IPV4, WIFI_LINUX_DAEMON_WATCH_INTERVAL_SECS,
    WIFI_RETRY_BACKOFF_SECS, WIFI_SCAN_TIMEOUT_SECS,
};
use crate::error::{Error, Result};
use crate::platform::wifi::linux_ctrl::{
    capability::{self, PhyCapabilities},
    hostapd, iw_scan, net, process, wpa,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const TAG: &str = "platform::wifi_linux";
const SOFTAP_SSID: &str = "Beetle";

static WIFI_STA_CONNECTED: AtomicBool = AtomicBool::new(false);
static WIFI_STA_IP: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static WIFI_IFACE: OnceLock<Mutex<Option<String>>> = OnceLock::new();

/// GET /api/wifi/scan 返回的单个 AP。
#[derive(Clone, Debug, serde::Serialize)]
pub struct WifiApEntry {
    pub ssid: String,
    pub rssi: i8,
}

/// 向设备请求一次 WiFi 扫描的 trait。
pub trait WifiScan: Send + Sync {
    fn request_scan(&self) -> Result<Vec<WifiApEntry>>;
}

#[derive(Clone)]
pub struct WifiScanHandle {
    iface: String,
    /// 无并发 STA 时不用 `wpa_cli`，改用 `iw dev … scan`。
    scan_via_iw: bool,
}

impl WifiScan for WifiScanHandle {
    fn request_scan(&self) -> Result<Vec<WifiApEntry>> {
        let deadline = Instant::now() + Duration::from_secs(WIFI_SCAN_TIMEOUT_SECS);
        loop {
            let r = if self.scan_via_iw {
                iw_scan::scan_bounded(&self.iface, deadline)
            } else {
                wpa::scan_bounded(&self.iface, deadline)
            };
            match r {
                Ok(list) => return Ok(list),
                Err(e) => {
                    if Instant::now() >= deadline {
                        return Err(e.with_stage("wifi_scan"));
                    }
                    std::thread::sleep(Duration::from_millis(200));
                }
            }
        }
    }
}

pub fn is_wifi_sta_connected() -> bool {
    WIFI_STA_CONNECTED.load(Ordering::Relaxed)
}

pub fn wifi_sta_ip() -> Option<String> {
    WIFI_STA_IP
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|g| g.clone())
}

/// Linux 启动后不阻塞全局启动流程；连接状态由后台与 API 查询。
pub fn wait_for_network_ready() {}

/// 若 iface 上已有 STA 地址落在 `192.168.1.0/24`，则 AP 避让至备用网段，避免与上游路由同网段冲突。
fn choose_ap_ip(iface: &str) -> &'static str {
    match net::read_sta_ip(iface).ok().flatten() {
        Some(ip) if ip.starts_with("192.168.1.") => {
            log::info!(
                "[{}] existing STA in 192.168.1.0/24, AP using fallback {}",
                TAG,
                SOFTAP_FALLBACK_IPV4
            );
            SOFTAP_FALLBACK_IPV4
        }
        _ => SOFTAP_DEFAULT_IPV4,
    }
}

/// AP 已用默认地址而 STA DHCP 落在 `192.168.1.0/24` 时，迁移 AP 至备用地址。
fn migrate_ap_if_subnet_conflict(iface: &str, ap_ip: &str, sta_ip: &Option<String>) -> Result<()> {
    if ap_ip != SOFTAP_DEFAULT_IPV4 {
        return Ok(());
    }
    let Some(sta) = sta_ip else {
        return Ok(());
    };
    if sta.starts_with("192.168.1.") {
        log::warn!(
            "[{}] STA {} on 192.168.1.0/24 conflicts with AP {}; migrating AP to {}",
            TAG,
            sta,
            SOFTAP_DEFAULT_IPV4,
            SOFTAP_FALLBACK_IPV4
        );
        hostapd::stop_ap(iface);
        match hostapd::start_ap(iface, SOFTAP_SSID, SOFTAP_FALLBACK_IPV4) {
            Ok(()) => return Ok(()),
            Err(e) => {
                log::error!(
                    "[{}] migrate AP to {} failed ({}); restoring {} so provisioning stays possible",
                    TAG,
                    SOFTAP_FALLBACK_IPV4,
                    e,
                    SOFTAP_DEFAULT_IPV4
                );
                return hostapd::start_ap(iface, SOFTAP_SSID, SOFTAP_DEFAULT_IPV4);
            }
        }
    }
    Ok(())
}

fn log_phy_caps(caps: &PhyCapabilities) {
    log::info!(
        "[{}] phy: ap={} concurrent_sta_ap={} band_2g={} band_5g={}",
        TAG,
        caps.supports_ap,
        caps.supports_sta_ap_concurrent,
        caps.has_2ghz,
        caps.has_5ghz
    );
}

pub fn connect(config: &AppConfig) -> Result<Option<WifiScanHandle>> {
    net::ensure_root_or_cap_net_admin()?;
    let iface = capability::detect_wifi_iface()?;
    set_iface(&iface);

    let caps = capability::probe_phy(&iface)?;
    log_phy_caps(&caps);
    if !caps.supports_ap {
        return Err(Error::config(
            "wifi_capability_check",
            "nl80211 does not report AP mode; check driver / iw phy",
        ));
    }

    let ap_ip = choose_ap_ip(&iface);
    hostapd::stop_ap(&iface);
    hostapd::start_ap(&iface, SOFTAP_SSID, ap_ip)?;
    clear_sta_state();

    let ap_ip_owned = ap_ip.to_string();
    start_daemon_watch_thread(iface.clone(), ap_ip_owned);

    if config.wifi_ssid.trim().is_empty() {
        log::info!("[{}] AP ready (SSID: {})", TAG, SOFTAP_SSID);
        return Ok(Some(WifiScanHandle {
            iface,
            scan_via_iw: false,
        }));
    }

    let concurrent = caps.supports_sta_ap_concurrent;
    if !concurrent {
        log::warn!(
            "[{}] phy does not advertise managed+AP concurrent combo; SoftAP only — STA connect skipped (use provisioning UI, then reboot if driver allows STA-only)",
            TAG
        );
        clear_sta_state();
        start_sta_probe_thread(iface.clone());
        return Ok(Some(WifiScanHandle {
            iface,
            scan_via_iw: true,
        }));
    }

    match wpa::connect_sta(&iface, config.wifi_ssid.trim(), config.wifi_pass.as_str()) {
        Ok(ip) => {
            if let Err(e) = migrate_ap_if_subnet_conflict(&iface, ap_ip, &ip) {
                log::error!(
                    "[{}] subnet migration failed after restore attempt: {}",
                    TAG,
                    e
                );
                if let Err(e2) = hostapd::start_ap(&iface, SOFTAP_SSID, ap_ip) {
                    log::error!(
                        "[{}] SoftAP emergency recovery failed (user may lose hotspot until reboot): {}",
                        TAG,
                        e2
                    );
                } else {
                    log::info!(
                        "[{}] SoftAP recovered on emergency retry (STA still up)",
                        TAG
                    );
                }
            }
            set_sta_state(ip);
        }
        Err(e) => {
            log::warn!(
                "[{}] STA failed (wrong password or unreachable); SoftAP stays up for provisioning: {}",
                TAG,
                e
            );
            clear_sta_state();
        }
    }
    start_sta_probe_thread(iface.clone());
    Ok(Some(WifiScanHandle {
        iface,
        scan_via_iw: false,
    }))
}

fn start_daemon_watch_thread(iface: String, ap_ip: String) {
    let res = std::thread::Builder::new()
        .name("wifi-linux-watch".into())
        .spawn(move || {
            let hostapd_pf = hostapd::daemon_pid_path("hostapd");
            let dnsmasq_pf = hostapd::daemon_pid_path("dnsmasq");
            let wpa_pf = wpa::supplicant_pid_path(&iface);
            loop {
                std::thread::sleep(Duration::from_secs(WIFI_LINUX_DAEMON_WATCH_INTERVAL_SECS));
                let need_ap = !pid_file_alive(&hostapd_pf) || !pid_file_alive(&dnsmasq_pf);
                if need_ap {
                    log::warn!(
                        "[{}] AP stack pid missing or dead; restarting hostapd+dnsmasq",
                        TAG
                    );
                    hostapd::stop_ap(&iface);
                    if let Err(e) = hostapd::start_ap(&iface, SOFTAP_SSID, &ap_ip) {
                        log::error!("[{}] AP stack restart failed: {}", TAG, e);
                    }
                    continue;
                }
                if wpa_pf.exists() {
                    if let Some(pid) = process::read_pid_file(wpa_pf.as_path()) {
                        if !process::is_pid_alive(pid) {
                            log::warn!("[{}] wpa_supplicant not running; re-ensure", TAG);
                            if let Err(e) = wpa::ensure_daemon(&iface) {
                                log::error!("[{}] wpa_supplicant restart failed: {}", TAG, e);
                            }
                        }
                    }
                }
            }
        });
    if let Err(e) = res {
        log::error!("[{}] daemon watch thread spawn failed: {}", TAG, e);
    }
}

fn pid_file_alive(path: &Path) -> bool {
    match process::read_pid_file(path) {
        Some(pid) => process::is_pid_alive(pid),
        None => false,
    }
}

fn start_sta_probe_thread(iface: String) {
    let res = std::thread::Builder::new()
        .name("wifi-linux-probe".into())
        .spawn(move || {
            for attempt in 0..3u32 {
                let iface_cl = iface.clone();
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    probe_loop(&iface_cl);
                }));
                if r.is_err() {
                    log::warn!("[{}] probe thread panic, restart {}/3", TAG, attempt + 1);
                    if attempt == 2 {
                        log::error!("[{}] probe thread aborted after 3 panics", TAG);
                        return;
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        });
    if let Err(e) = res {
        log::error!("[{}] probe thread spawn failed: {}", TAG, e);
    }
}

fn probe_loop(iface: &str) {
    loop {
        let ip = net::read_sta_ip(iface).ok().flatten();
        match ip {
            Some(v) => {
                WIFI_STA_CONNECTED.store(true, Ordering::Relaxed);
                if let Ok(mut g) = WIFI_STA_IP.get_or_init(|| Mutex::new(None)).lock() {
                    *g = Some(v);
                }
            }
            None => {
                WIFI_STA_CONNECTED.store(false, Ordering::Relaxed);
                if let Ok(mut g) = WIFI_STA_IP.get_or_init(|| Mutex::new(None)).lock() {
                    *g = None;
                }
            }
        }
        std::thread::sleep(Duration::from_secs(WIFI_RETRY_BACKOFF_SECS[0]));
    }
}

fn set_sta_state(ip: Option<String>) {
    if let Ok(mut g) = WIFI_STA_IP.get_or_init(|| Mutex::new(None)).lock() {
        *g = ip.clone();
    }
    WIFI_STA_CONNECTED.store(ip.is_some(), Ordering::Relaxed);
}

fn clear_sta_state() {
    WIFI_STA_CONNECTED.store(false, Ordering::Relaxed);
    if let Ok(mut g) = WIFI_STA_IP.get_or_init(|| Mutex::new(None)).lock() {
        *g = None;
    }
}

fn set_iface(iface: &str) {
    if let Ok(mut g) = WIFI_IFACE.get_or_init(|| Mutex::new(None)).lock() {
        *g = Some(iface.to_string());
    }
}
