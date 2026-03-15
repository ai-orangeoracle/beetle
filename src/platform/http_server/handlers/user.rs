//! GET/POST /api/user：配对后读/写 USER 配置（config/USER.md）。

use crate::config;
use crate::memory::MAX_SOUL_USER_LEN;
use crate::platform::http_server::common::ApiResponse;
use crate::platform::http_server::user_message;
use crate::state;

use super::HandlerContext;

/// GET：Ok(content) 写 200 text/plain，Err(msg) 写 500 JSON。
pub fn get_body(ctx: &HandlerContext) -> Result<String, String> {
    ctx.memory_store
        .get_user()
        .map_err(|e| state::sanitize_error_for_log(&e))
}

/// POST：body 为原始请求体；is_json 为 true 时从 {"content":"..."} 取 content。
pub fn post(ctx: &HandlerContext, body: String, is_json: bool) -> ApiResponse {
    let locale = config::get_locale(ctx.config_store.as_ref());
    let content = if is_json {
        serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("content").and_then(|c| c.as_str()).map(String::from))
            .unwrap_or(body)
    } else {
        body
    };
    let content = content.trim();
    if content.len() > MAX_SOUL_USER_LEN {
        return ApiResponse::err_400(&user_message::from_api_key("content_too_long", &locale));
    }
    match ctx.memory_store.set_user(content) {
        Ok(()) => ApiResponse::ok_200_json("{\"ok\":true}"),
        Err(e) => ApiResponse::err_500(&user_message::from_error(&e, &locale)),
    }
}
