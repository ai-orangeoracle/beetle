//! GET /api/csrf_token: 返回当前 CSRF token,供前端获取。

use super::HandlerContext;
use crate::platform::http_server::common::to_io;

pub fn body(_ctx: &HandlerContext) -> Result<String, std::io::Error> {
    let token =
        crate::platform::csrf::get_token().ok_or_else(|| to_io("CSRF token not initialized"))?;
    Ok(format!(r#"{{"csrf_token":"{}"}}"#, token))
}
