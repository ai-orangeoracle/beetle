//! CSRF token 生成与验证,防止跨站请求伪造攻击。

use std::sync::Mutex;

static CSRF_TOKEN: Mutex<Option<String>> = Mutex::new(None);

/// 生成新的 CSRF token (16 字节随机数的 hex 字符串)。
pub fn generate_token() -> crate::Result<String> {
    let mut bytes = [0u8; 16];
    #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
    unsafe {
        esp_idf_svc::sys::esp_fill_random(bytes.as_mut_ptr() as *mut _, 16);
    }
    #[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
    {
        getrandom::getrandom(&mut bytes)
            .map_err(|e| crate::Error::config("csrf_entropy", e.to_string()))?;
    }
    Ok(hex_encode(&bytes))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

/// 初始化 CSRF token (启动时调用一次)。
pub fn init() -> crate::Result<()> {
    let token = generate_token()?;
    *CSRF_TOKEN.lock().unwrap_or_else(|p| p.into_inner()) = Some(token);
    log::info!("[csrf] token initialized");
    Ok(())
}

/// 获取当前 CSRF token。
pub fn get_token() -> Option<String> {
    CSRF_TOKEN.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

/// 验证请求的 CSRF token 是否匹配。
pub fn verify_token(token: &str) -> bool {
    match CSRF_TOKEN
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_ref()
    {
        Some(expected) => crate::util::constant_time_eq(token, expected),
        None => false,
    }
}
