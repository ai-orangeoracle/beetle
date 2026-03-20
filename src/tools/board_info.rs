//! board_info 工具：委托 `Platform::board_info_json`，逻辑在 `platform/board_info`。
//! board_info tool: delegates to `Platform::board_info_json`; payload built in `platform/board_info`.

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
        "Return device status: chip, heap, uptime, IDF version, resource pressure, WiFi STA connection, SPIFFS storage (total/used/free). Use when user asks about device status, memory, network, or storage."
    }
    fn schema(&self) -> serde_json::Value {
        json!({ "type": "object", "properties": {} })
    }
    fn execute(&self, _args: &str, _ctx: &mut dyn ToolContext) -> Result<String> {
        self.platform.board_info_json()
    }
}
