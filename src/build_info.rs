//! 编译期确定的板型 ID 与 OTA manifest URL（option_env! 在编译时求值），与 board_presets.toml / CI 约定一致。

/// 板型 ID，与 board_presets.toml 的 BOARD 一致；CI 构建时设 BOARD=xxx。
#[inline(always)]
pub fn build_board_id() -> &'static str {
    option_env!("BOARD").unwrap_or("esp32-s3-16mb")
}

/// OTA 渠道清单 URL；空则 GET /api/ota/check 视为渠道未配置。CI/Release 构建时设 OTA_MANIFEST_URL。
#[inline(always)]
pub fn ota_manifest_url() -> &'static str {
    option_env!("OTA_MANIFEST_URL").unwrap_or("")
}
