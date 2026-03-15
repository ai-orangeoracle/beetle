//! GET /api/wifi/scan：设备侧扫描周边 WiFi，返回 SSID 列表（含 rssi）供配置页下拉选择。

use super::HandlerContext;
use crate::error::Error;

/// 扫描不可用（非 ESP 或 WiFi 未就绪）时返回 503；其他错误返回 500。
#[derive(Debug)]
pub enum WifiScanError {
    Unavailable,
    Other(Error),
}

/// GET /api/wifi/scan：返回 JSON 数组 [{ "ssid": "...", "rssi": -50 }, ...]，按信号强度降序。无需配对码。
pub fn get_body(ctx: &HandlerContext) -> Result<String, WifiScanError> {
    let scanner = match ctx.platform.wifi_scan() {
        Some(s) => s,
        None => return Err(WifiScanError::Unavailable),
    };
    let list = scanner.request_scan().map_err(WifiScanError::Other)?;
    serde_json::to_string(&list).map_err(|e| WifiScanError::Other(Error::Other {
        source: Box::new(e),
        stage: "wifi_scan_serialize",
    }))
}
