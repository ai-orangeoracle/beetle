//! 按 locale 返回给前端的错误/提示文案，统一出口，便于多语言与人话化。
//! User-facing messages by locale; single source for i18n.

use crate::error::Error;

/// 已知 API 错误 key 对应的人话文案（zh / en）。未知 key 回退到 zh 的「操作失败」。
fn message_for_key(key: &str) -> Option<(&'static str, &'static str)> {
    let pair = match key {
        "pairing_required" => ("请先设置配对码", "Please set pairing code first"),
        "pairing_code_wrong" => ("配对码错误", "Wrong pairing code"),
        "operation_failed" => ("操作失败，请重试", "Operation failed, please try again"),
        "invalid_json" => ("请求体不是合法 JSON", "Request body is not valid JSON"),
        "code_must_be_6_digits" => ("配对码须为 6 位数字", "Pairing code must be 6 digits"),
        "pairing_code_already_set" => ("配对码已设置，无法修改", "Pairing code already set"),
        "failed_to_save_code" => ("保存配对码失败", "Failed to save pairing code"),
        "save_failed" => ("保存失败", "Save failed"),
        "content_too_long" => ("内容过长", "Content too long"),
        "invalid_url" => ("无效的 URL", "Invalid URL"),
        "body_read_failed" => ("读取请求体失败", "Failed to read request body"),
        "invalid_utf8" => ("请求体不是合法 UTF-8", "Request body is not valid UTF-8"),
        "body_too_large" => ("请求体过大", "Request body too large"),
        "skill_not_found" => ("未找到该技能", "Skill not found"),
        "missing_name_query" => ("缺少 name 参数", "Missing name parameter"),
        "webhook_disabled" => ("Webhook 未启用", "Webhook is disabled"),
        "invalid_token" => ("Token 无效", "Invalid token"),
        "queue_full" => ("队列已满，请稍后重试", "Queue full, try again later"),
        "missing_name_for_write" => ("缺少技能名称", "Missing skill name for write"),
        "missing_name_or_enabled" => ("缺少 name 或 enabled", "Missing name or enabled"),
        "missing_order_name_content" => ("缺少 order、name+content 或 name+enabled", "Missing order, name+content, or name+enabled"),
        "missing_url" => ("缺少 url", "Missing URL"),
        "missing_name" => ("缺少 name", "Missing name"),
        "url_body_not_utf8" => ("URL 返回内容不是合法 UTF-8", "URL body is not valid UTF-8"),
        "ota_channel_not_configured" => ("渠道未配置", "Update channel not configured"),
        "ota_check_fail" => ("检查更新失败，请稍后重试", "Check for update failed, try again later"),
        "ota_download" => ("网络或下载失败，请检查网络后重试", "Network or download failed, check connection and retry"),
        "ota_validate" => ("固件校验失败，请更换固件来源", "Firmware verification failed, try another source"),
        "ota_write" => ("写入失败，请勿断电并重试", "Write failed, do not power off and retry"),
        _ => return None,
    };
    Some(pair)
}

/// 根据 API 错误 key 返回当前 locale 的人话文案。locale 非 "en" 视为 "zh"。
pub fn from_api_key(key: &str, locale: &str) -> String {
    let (zh, en) = match message_for_key(key) {
        Some(p) => p,
        None => ("操作失败，请重试", "Operation failed, please try again"),
    };
    if locale == "en" {
        en.to_string()
    } else {
        zh.to_string()
    }
}

/// 最大错误详情长度，避免响应体过大；超出截断。
const MAX_ERROR_DETAIL_LEN: usize = 400;

/// 已知 ESP-IDF NVS 错误码的排查提示（4361 = ESP_ERR_NVS_INVALID_STATE）。
fn esp_nvs_hint(e: &Error) -> &'static str {
    if let Error::Esp { code: 4361, .. } = e {
        " (NVS invalid state: power cycle or erase NVS partition to recover)"
    } else {
        ""
    }
}

/// 将 Error 转为可排查的文案：使用错误的 Display，便于排查；过长截断，换行替为空格。
pub fn from_error(e: &Error, _locale: &str) -> String {
    let base = e.to_string();
    let hint = esp_nvs_hint(e);
    let s = format!("{}{}", base, hint);
    let s = s.replace('\n', " ").replace('\r', " ");
    if s.len() <= MAX_ERROR_DETAIL_LEN {
        s
    } else {
        format!("{}...", &s[..MAX_ERROR_DETAIL_LEN.saturating_sub(3)])
    }
}
