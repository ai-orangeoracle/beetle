//! board_info 工具：委托 `Platform::board_info_json`，载荷由 `platform/board_info` 按目标（ESP32 / Linux / 其它 OS 名）组装。
//! board_info tool: delegates to `Platform::board_info_json`; payload per target (ESP32 / Linux / other OS per `std::env::consts::OS`).

use crate::error::Result;
use crate::platform::Platform;
use crate::tools::{Tool, ToolContext};
use serde_json::json;
use std::sync::Arc;

pub struct BoardInfoTool {
    platform: Arc<dyn Platform>,
}

impl BoardInfoTool {
    pub fn new(platform: Arc<dyn Platform>) -> Self {
        Self { platform }
    }
}

impl Tool for BoardInfoTool {
    fn name(&self) -> &str {
        "board_info"
    }
    fn description(&self) -> &str {
        "Return device/system status JSON. ESP: chip, heap, IDF, SPIFFS. Linux: platform \"linux\" plus cpu_model, cpu_cores, mem_*, distro_pretty/distro_id, kernel_release, hostname, arch, storage, os (/proc/version), uptime, pressure, WiFi STA. Non-Linux: platform is the OS name (e.g. macos, windows) with fewer fields. Use for device status, distro, CPU/RAM, disk."
    }
    fn schema(&self) -> serde_json::Value {
        json!({ "type": "object", "properties": {} })
    }
    fn execute(&self, _args: &str, _ctx: &mut dyn ToolContext) -> Result<String> {
        self.platform.board_info_json()
    }
}
