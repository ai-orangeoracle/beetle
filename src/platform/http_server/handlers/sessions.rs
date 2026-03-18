//! GET /api/sessions：仅生成响应体或错误信息，配对与写响应在 mod.rs。
//! GET /api/sessions?chat_id=xxx：返回指定会话最近消息。
//! DELETE /api/sessions?chat_id=xxx：删除指定会话。

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

/// 返回指定 chat_id 的最近 N 条消息 JSON。
pub fn detail(ctx: &HandlerContext, chat_id: &str) -> Result<String, String> {
    let messages = ctx
        .session_store
        .load_recent(chat_id, 50)
        .map_err(|e| state::sanitize_error_for_log(&e))?;
    serde_json::to_string(&messages).map_err(|e| e.to_string())
}

/// 删除指定 chat_id 的会话。
pub fn delete(ctx: &HandlerContext, chat_id: &str) -> Result<String, String> {
    ctx.session_store
        .delete(chat_id)
        .map_err(|e| state::sanitize_error_for_log(&e))?;
    Ok(r#"{"ok":true}"#.to_string())
}
