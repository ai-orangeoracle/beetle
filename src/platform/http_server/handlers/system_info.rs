//! GET /api/system_info：供系统信息页展示用，返回 product_name、system_status、current_time、firmware_version、locale。

use super::HandlerContext;
use crate::config;
use crate::platform::http_server::common::to_io;
use crate::state;
use std::sync::atomic::Ordering;

fn current_time_str() -> String {
    #[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(d) => d.as_secs(),
            Err(_) => return "—".to_string(),
        };
        let t = secs % 86400;
        let h = (t / 3600) as u32;
        let m = (t % 3600 / 60) as u32;
        let s = (t % 60) as u32;
        let d = secs / 86400;
        let (y, mo, day) = days_to_ymd(d);
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
            y, mo, day, h, m, s
        )
    }
    #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
    {
        "—".to_string()
    }
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
fn days_to_ymd(days: u64) -> (u32, u32, u32) {
    let mut d = days;
    let mut y = 1970u32;
    let is_leap = |y: u32| (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400);
    let days_in_year = |y: u32| if is_leap(y) { 366 } else { 365 };
    while d >= days_in_year(y) as u64 {
        d -= days_in_year(y) as u64;
        y += 1;
    }
    let mon_days = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut mon_days = mon_days;
    if is_leap(y) {
        mon_days[1] = 29;
    }
    let mut m = 0usize;
    let mut acc = 0u64;
    while m < 12 && acc + mon_days[m] <= d {
        acc += mon_days[m];
        m += 1;
    }
    let (mo, day) = if m >= 12 {
        let dec_start = acc - mon_days[11];
        (12u32, ((d - dec_start + 1) as u32).min(mon_days[11] as u32))
    } else {
        let day_raw = (d - acc + 1) as u32;
        (m as u32 + 1, day_raw.min(mon_days[m] as u32))
    };
    (y, mo, day)
}

/// 生成 system_info JSON：product_name, system_status, current_time, firmware_version。
pub fn body(ctx: &HandlerContext) -> Result<String, std::io::Error> {
    let memory_loaded = ctx.memory_store.get_memory().is_ok();
    let soul_loaded = ctx.memory_store.get_soul().is_ok();
    let storage_ok = memory_loaded || soul_loaded;
    let last_error = state::get_last_error();
    let inc = ctx.inbound_depth.load(Ordering::Relaxed);
    let out = ctx.outbound_depth.load(Ordering::Relaxed);
    let sta_up = crate::platform::is_wifi_sta_connected();
    let system_status = if sta_up && storage_ok && last_error.is_none() && inc <= 6 && out <= 6 {
        "正常"
    } else if !sta_up {
        "WiFi 未连接"
    } else if !storage_ok {
        "存储异常"
    } else if last_error.is_some() {
        "通道异常"
    } else {
        "运行中"
    };
    let product_name = "beetle";
    let current_time = current_time_str();
    let firmware_version = ctx.version.as_ref();
    let ota_available = cfg!(feature = "ota");
    let locale = config::get_locale(ctx.config_store.as_ref());
    let json = serde_json::json!({
        "product_name": product_name,
        "system_status": system_status,
        "current_time": current_time,
        "firmware_version": firmware_version,
        "board_id": ctx.board_id.as_ref(),
        "ota_available": ota_available,
        "locale": locale,
    });
    serde_json::to_string(&json).map_err(to_io)
}
