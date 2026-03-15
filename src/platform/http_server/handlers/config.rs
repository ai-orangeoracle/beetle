//! 配置 API：GET /api/config、POST /api/config/wifi、POST /api/config/llm、/channels、/system。

use crate::config::{self, AppConfig};
use crate::platform::http_server::common::{to_io, ApiResponse, WifiConfigPayload};
use crate::platform::http_server::user_message;
use serde_json::Value;

use super::HandlerContext;

/// GET /api/config：从 config_store + SPIFFS 实时 load，返回完整配置 JSON（含密钥）+ locale。
pub fn get_body(ctx: &HandlerContext) -> Result<String, std::io::Error> {
    let config = AppConfig::load(
        ctx.config_store.as_ref(),
        Some(ctx.config_file_store.as_ref()),
    );
    let mut j: Value = serde_json::to_value(&config).map_err(|e| to_io(e.to_string()))?;
    j["locale"] = serde_json::Value::String(config::get_locale(ctx.config_store.as_ref()));
    serde_json::to_string_pretty(&j).map_err(|e| to_io(e.to_string()))
}

/// POST /api/config/wifi：body 为 JSON，写 WiFi SSID/密码到 NVS。成功时返回 restart_required 提示需重启生效。
pub fn post_wifi(ctx: &HandlerContext, body: &str) -> Result<ApiResponse, std::io::Error> {
    let locale = config::get_locale(ctx.config_store.as_ref());
    let payload: WifiConfigPayload = match serde_json::from_str(body) {
        Ok(p) => p,
        Err(_) => return Ok(ApiResponse::err_400(&user_message::from_api_key("invalid_json", &locale))),
    };
    match config::save_wifi_to_nvs(
        ctx.config_store.as_ref(),
        &payload.wifi_ssid,
        &payload.wifi_pass,
    ) {
        Ok(()) => Ok(ApiResponse::ok_200_json(r#"{"ok":true,"restart_required":true}"#)),
        Err(e) => Ok(ApiResponse::err_400(&user_message::from_error(&e, &locale))),
    }
}

/// POST /api/config/llm：仅写 LLM 段，body 为 LlmSegment JSON。
pub fn post_llm(ctx: &HandlerContext, body: &str) -> Result<ApiResponse, std::io::Error> {
    let locale = config::get_locale(ctx.config_store.as_ref());
    match config::save_llm_segment(ctx.config_file_store.as_ref(), body) {
        Ok(()) => Ok(ApiResponse::ok_200_json("{\"ok\":true}")),
        Err(e) => Ok(ApiResponse::err_400(&user_message::from_error(&e, &locale))),
    }
}

/// POST /api/config/channels：仅写通道段，body 为 ChannelsSegment JSON。
pub fn post_channels(ctx: &HandlerContext, body: &str) -> Result<ApiResponse, std::io::Error> {
    let locale = config::get_locale(ctx.config_store.as_ref());
    match config::save_channels_segment(ctx.config_file_store.as_ref(), body) {
        Ok(()) => Ok(ApiResponse::ok_200_json("{\"ok\":true}")),
        Err(e) => Ok(ApiResponse::err_400(&user_message::from_error(&e, &locale))),
    }
}

/// POST /api/config/system：仅写系统段（wifi/proxy/session/tg_group/locale），body 为 SystemSegment JSON。
pub fn post_system(ctx: &HandlerContext, body: &str) -> Result<ApiResponse, std::io::Error> {
    let locale = config::get_locale(ctx.config_store.as_ref());
    match config::save_system_segment_to_nvs(ctx.config_store.as_ref(), body) {
        Ok(()) => Ok(ApiResponse::ok_200_json("{\"ok\":true}")),
        Err(e) => Ok(ApiResponse::err_400(&user_message::from_error(&e, &locale))),
    }
}
