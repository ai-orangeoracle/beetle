//! 配对码：store 仅存 pairing_code（6 位数字）；未设置时仅白名单可访问，已设置后 GET 类放行、写操作需带码校验。
//! Pairing: single key "pairing_code"; when not set only whitelist; when set, GETs pass, writes require code.

use crate::error::Result;
use crate::platform::ConfigStore;

const NVS_KEY_PAIRING_CODE: &str = "pairing_code";
const CODE_LEN: usize = 6;

fn constant_time_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

fn valid_code_format(code: &str) -> bool {
    code.len() == CODE_LEN && code.chars().all(|c| c.is_ascii_digit())
}

/// 是否已设置配对码（store 中存在有效 6 位码）。
pub fn code_set(store: &dyn ConfigStore) -> bool {
    match store.read_string(NVS_KEY_PAIRING_CODE) {
        Ok(Some(s)) => valid_code_format(s.trim()),
        _ => false,
    }
}

/// 仅当未设置时写入用户提供的 6 位码；校验格式，常量时间不暴露存储内容。返回 Ok(true) 表示已写入。
pub fn set_code(store: &dyn ConfigStore, code: &str) -> Result<bool> {
    if code_set(store) {
        return Ok(false);
    }
    let code = code.trim();
    if !valid_code_format(code) {
        return Ok(false);
    }
    store.write_string(NVS_KEY_PAIRING_CODE, code)?;
    log::info!("[pairing] code set (6 digits)");
    Ok(true)
}

/// 校验传入码与 store 中一致（常量时间）；未设置时返回 false。
pub fn verify_code(store: &dyn ConfigStore, code: &str) -> bool {
    let stored = match store.read_string(NVS_KEY_PAIRING_CODE) {
        Ok(Some(s)) => s,
        _ => return false,
    };
    let stored = stored.trim();
    if !valid_code_format(stored) || !valid_code_format(code) {
        return false;
    }
    constant_time_eq(code, stored)
}

/// 清除配对码（恢复出厂后调用），使设备回到未激活状态。
pub fn clear_code(store: &dyn ConfigStore) -> Result<()> {
    store.erase_keys(&[NVS_KEY_PAIRING_CODE])
}
