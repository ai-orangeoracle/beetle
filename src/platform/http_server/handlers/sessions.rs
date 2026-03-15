//! GET /api/sessions：仅生成响应体或错误信息，配对与写响应在 mod.rs。

use super::HandlerContext;
use crate::state;

/// 成功返回 JSON 数组字符串，失败返回错误信息（mod 层写 500）。
pub fn body(ctx: &HandlerContext) -> Result<String, String> {
    let ids = ctx
        .session_store
        .list_chat_ids()
        .map_err(|e| state::sanitize_error_for_log(&e))?;
    serde_json::to_string(&ids).map_err(|e| e.to_string())
}
