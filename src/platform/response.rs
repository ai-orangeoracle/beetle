//! HTTP 响应公共处理：2xx 检查与 body 截断，供 fetch_url 与 HandlerContext 复用。
//! Shared 2xx check + body truncation for GET responses.

use crate::error::{Error, Result};

/// 若 status 在 200..300 则截断 body 至 max_len 并返回，否则返回 Err。
pub fn check_2xx_and_truncate(
    stage: &'static str,
    status: u16,
    mut body: Vec<u8>,
    max_len: usize,
) -> Result<Vec<u8>> {
    if !(200..300).contains(&status) {
        return Err(Error::http(stage, status));
    }
    if body.len() > max_len {
        body.truncate(max_len);
    }
    Ok(body)
}
