//! 轻量工具，避免热路径堆分配；敏感信息脱敏供日志安全。
//! Lightweight helpers; secret redaction for safe logging.

/// 按字符边界截断内容至最多 max 个字符；供 agent 与 dispatch 统一使用。
pub fn truncate_content_to_max(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect::<String>()
    }
}

/// 按 UTF-8 字符边界截断至最多 max_bytes 字节；若发生截断则末尾追加 "…"（3 字节）。保证返回值 len() <= max_bytes。
pub fn truncate_to_byte_len(s: &str, max_bytes: usize) -> String {
    const ELLIPSIS: &str = "…";
    let cap = max_bytes.saturating_sub(ELLIPSIS.len());
    let mut len = 0usize;
    let mut out = String::new();
    for c in s.chars() {
        let n = c.len_utf8();
        if len + n > cap {
            break;
        }
        len += n;
        out.push(c);
    }
    if out.len() < s.len() {
        out.push_str(ELLIPSIS);
    }
    out
}

/// URL 查询参数 percent-encode：保留字母数字与 -_.~，其余按 UTF-8 字节编码为 %XX。供 web_search 等使用。
pub fn percent_encode_query(s: &str) -> String {
    fn need_encode(b: u8) -> bool {
        matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~')
    }
    let mut out = String::new();
    for b in s.as_bytes() {
        if need_encode(*b) {
            out.push(*b as char);
        } else if *b == b' ' {
            out.push_str("%20");
        } else {
            out.push('%');
            out.push_str(&hex::encode(std::slice::from_ref(b)));
        }
    }
    out
}

// ---------- 时间/日期（与 cron、remind_at、get_time 共用） ----------

/// 闰年判定。
#[inline]
pub fn is_leap_year(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

/// 自 1970-01-01 起的天数（1970-01-01 为 0）。用于 Unix 秒换算。
pub fn days_from_epoch(year: i32, month: u32, day: u32) -> i64 {
    const MONTH_DAYS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut d = 0i64;
    for y in 1970..year {
        d += if is_leap_year(y) { 366 } else { 365 };
    }
    let mut month_days = MONTH_DAYS;
    if is_leap_year(year) {
        month_days[1] = 29;
    }
    for i in 0..(month as usize).saturating_sub(1) {
        d += month_days[i] as i64;
    }
    d + (day as i64) - 1
}

/// Unix 秒 → (year, month 1-12, day 1-31, hour, min, sec) UTC。
pub fn epoch_to_ymdhms(mut secs: u64) -> (i32, u32, u32, u32, u32, u32) {
    let sec = (secs % 60) as u32;
    secs /= 60;
    let min = (secs % 60) as u32;
    secs /= 60;
    let hour = (secs % 24) as u32;
    secs /= 24;
    let days = secs as i64;
    let mut year = 1970i32;
    let days_in_year = |y: i32| if is_leap_year(y) { 366i64 } else { 365i64 };
    let mut d = days;
    while d >= days_in_year(year) {
        d -= days_in_year(year);
        year += 1;
    }
    let day_of_year = d as u32;
    const MONTH_DAYS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month_days = MONTH_DAYS;
    if is_leap_year(year) {
        month_days[1] = 29;
    }
    let mut m = 0usize;
    let mut acc = 0u32;
    while m < 12 && acc + month_days[m] <= day_of_year {
        acc += month_days[m];
        m += 1;
    }
    let month = (m as u32) + 1;
    let day = day_of_year - acc + 1;
    (year, month, day, hour, min, sec)
}

/// (y,m,d,h,min,sec) UTC → Unix 秒。
pub fn ymdhms_to_epoch(year: i32, month: u32, day: u32, hour: u32, min: u32, sec: u32) -> u64 {
    let d = days_from_epoch(year, month, day);
    (d as u64) * 86400 + (hour as u64) * 3600 + (min as u64) * 60 + (sec as u64)
}

/// 解析 ISO8601 简式（YYYY-MM-DDTHH:MM:SS 或带 Z）或纯数字 Unix 秒字符串。
pub fn parse_iso8601(s: &str) -> Option<u64> {
    let s = s.trim().trim_end_matches('Z');
    if let Ok(n) = s.parse::<u64>() {
        return Some(n);
    }
    let mut parts = s.splitn(2, 'T');
    let date = parts.next()?;
    let time = parts.next()?;
    let mut d = date.split('-');
    let y: i32 = d.next()?.parse().ok()?;
    let m: u32 = d.next()?.parse().ok()?;
    let day: u32 = d.next()?.parse().ok()?;
    let mut t = time.split(':');
    let h: u32 = t.next()?.parse().ok()?;
    let min: u32 = t.next()?.parse().ok()?;
    let sec: u32 = t.next()?.parse().ok()?;
    if m < 1 || m > 12 || day < 1 || day > 31 || h > 23 || min > 59 || sec > 59 {
        return None;
    }
    Some(ymdhms_to_epoch(y, m, day, h, min, sec))
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
/// 当前 Unix 秒。Host 用 SystemTime。
pub fn current_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
/// 当前 Unix 秒。SNTP 同步后 ESP-IDF 自动更新系统时钟，SystemTime 即可取得正确时间。
pub fn current_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 星期几名称。days = Unix 秒 / 86400，1970-01-01 (days=0) 为 Thursday。
pub fn weekday_name(days_since_epoch: u64) -> &'static str {
    const WEEKDAY: [&str; 7] = [
        "Thursday", "Friday", "Saturday", "Sunday", "Monday", "Tuesday", "Wednesday",
    ];
    WEEKDAY[(days_since_epoch % 7) as usize]
}

// ---------- 脱敏 ----------

/// 将 usize 十进制写入缓冲区，返回有效区间的 &str。调用方应传入至少 20 字节（如 `[0u8; 20]`）。
/// 供 content-length 等 header 使用，避免 format! 堆分配。
/// Caller must only write ASCII digits (b'0'+n%10); buf[i..max] is therefore valid UTF-8.
#[inline]
pub fn usize_to_decimal_buf(buf: &mut [u8], n: usize) -> &str {
    let max = buf.len().min(20);
    if max == 0 {
        return std::str::from_utf8(&[]).unwrap();
    }
    if n == 0 {
        buf[0] = b'0';
        return std::str::from_utf8(&buf[..1]).unwrap();
    }
    let mut i = max;
    let mut n = n as u64;
    while n > 0 && i > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    std::str::from_utf8(&buf[i..max]).unwrap()
}
