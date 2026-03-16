//! system_stats 工具：返回设备运行时健康状态（WiFi 连接、SPIFFS 用量）；与 board_info 互补。
//! system_stats tool: runtime health snapshot — WiFi STA status and SPIFFS usage; complementary to board_info.

use crate::error::Result;
use crate::tools::{Tool, ToolContext};
use serde_json::json;

pub struct SystemStatsTool;

impl Tool for SystemStatsTool {
    fn name(&self) -> &str {
        "system_stats"
    }
    fn description(&self) -> &str {
        "Return device runtime health: WiFi STA connection status and SPIFFS storage usage \
         (total/used/free bytes). Use when user asks about network connectivity or storage \
         space. For chip/heap/uptime info use board_info instead."
    }
    fn schema(&self) -> serde_json::Value {
        json!({ "type": "object", "properties": {} })
    }
    fn execute(&self, _args: &str, _ctx: &mut dyn ToolContext) -> Result<String> {
        let wifi_connected = crate::platform::is_wifi_sta_connected();

        let spiffs = crate::platform::spiffs_usage().map(|(total, used)| {
            let free = total.saturating_sub(used);
            json!({
                "total_bytes": total,
                "used_bytes": used,
                "free_bytes": free,
            })
        });

        let out = json!({
            "wifi_sta_connected": wifi_connected,
            "spiffs": spiffs,
        });
        Ok(out.to_string())
    }
}
