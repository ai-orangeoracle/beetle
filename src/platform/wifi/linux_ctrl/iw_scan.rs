//! 使用 `iw dev <iface> scan` 解析附近 AP（无 wpa_supplicant 时的降级扫描）。

use crate::error::{Error, Result};
use crate::platform::wifi::linux_ctrl::process::run_checked;
use std::time::{Duration, Instant};

/// 在 `deadline` 前完成一次 `iw` 扫描（与 `wpa::scan_bounded` 墙钟语义对齐）。
pub fn scan_bounded(iface: &str, deadline: Instant) -> Result<Vec<crate::platform::WifiApEntry>> {
    fn remain(deadline: Instant) -> Duration {
        deadline.saturating_duration_since(Instant::now())
    }
    if remain(deadline).is_zero() {
        return Err(Error::config("wifi_scan", "scan timeout"));
    }
    let t = remain(deadline).min(Duration::from_secs(8));
    let out = run_checked(
        "iw",
        &["dev", iface, "scan"],
        t,
        "wifi_scan",
    )?;
    let mut list = parse_iw_scan(&out.stdout)?;
    list.sort_by(|a, b| b.rssi.cmp(&a.rssi));
    Ok(list)
}

fn parse_iw_scan(raw: &str) -> Result<Vec<crate::platform::WifiApEntry>> {
    let mut out = Vec::new();
    let mut cur_ssid: Option<String> = None;
    let mut cur_sig: Option<i8> = None;

    fn flush(
        out: &mut Vec<crate::platform::WifiApEntry>,
        cur_ssid: &mut Option<String>,
        cur_sig: &mut Option<i8>,
    ) {
        if let (Some(ssid), Some(sig)) = (cur_ssid.take(), cur_sig.take()) {
            if !ssid.is_empty() {
                out.push(crate::platform::WifiApEntry { ssid, rssi: sig });
            }
        } else {
            *cur_ssid = None;
            *cur_sig = None;
        }
    }

    for line in raw.lines() {
        if line.starts_with("BSS ") {
            flush(&mut out, &mut cur_ssid, &mut cur_sig);
            continue;
        }
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("signal:") {
            let first = rest.split_whitespace().next();
            if let Some(s) = first {
                if let Ok(v) = s.parse::<f32>() {
                    cur_sig = Some(v.clamp(i8::MIN as f32, i8::MAX as f32) as i8);
                }
            }
        } else if let Some(rest) = t.strip_prefix("SSID:") {
            let ssid = rest.trim();
            if !ssid.is_empty() {
                cur_ssid = Some(ssid.to_string());
            }
        }
    }
    flush(&mut out, &mut cur_ssid, &mut cur_sig);

    if out.is_empty() && !raw.trim().is_empty() {
        log::debug!("[iw_scan] parsed 0 BSS from non-empty iw scan output");
    }
    Ok(out)
}
