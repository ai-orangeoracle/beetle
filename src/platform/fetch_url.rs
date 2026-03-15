//! 同步 GET URL 返回 body；供 skills/import 等使用。仅 ESP 目标实现。
//! Synchronous GET URL to bytes; for skills/import etc. ESP target only.

use crate::error::Result;
use crate::platform::response::check_2xx_and_truncate;

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub fn fetch_url_to_bytes(url: &str, max_len: usize) -> Result<Vec<u8>> {
    let mut client = crate::platform::EspHttpClient::new()?;
    let (status, mut body) = client.get(url)?;
    check_2xx_and_truncate("fetch_url", status, body.into_vec(), max_len)
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub fn fetch_url_to_bytes(_url: &str, _max_len: usize) -> Result<Vec<u8>> {
    Err(crate::error::Error::config(
        "fetch_url",
        "fetch_url_to_bytes not available on host",
    ))
}
