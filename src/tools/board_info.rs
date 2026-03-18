//! board_info 工具：返回设备状态 JSON（芯片、堆、运行时间、IDF 版本、压力、WiFi、SPIFFS）；ESP 用 platform/heap 与 esp_idf_svc::sys，host 返回占位。
//! board_info tool: device status JSON — chip, heap, uptime, IDF version, pressure, WiFi, SPIFFS; ESP uses platform/heap and esp_idf_svc::sys, host placeholder.

use crate::error::Result;
use crate::tools::{Tool, ToolContext};
use serde_json::json;

pub struct BoardInfoTool;

// 从编译目标推断芯片型号（esp_chip_info 未在 esp-idf-sys bindings 中暴露，避免依赖）。
// 若 TARGET 未包含 esp32（如构建环境未传），则按 target_arch 回退，避免显示 ESP32-unknown。
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
fn chip_model_from_target() -> (&'static str, u32, u32) {
    let target = option_env!("TARGET").unwrap_or("");
    let (model, cores) = if target.contains("esp32s3") {
        ("ESP32-S3", 2u32)
    } else if target.contains("esp32s2") {
        ("ESP32-S2", 1u32)
    } else if target.contains("esp32c3") {
        ("ESP32-C3", 1u32)
    } else if target.contains("esp32c6") {
        ("ESP32-C6", 1u32)
    } else if target.contains("esp32h2") {
        ("ESP32-H2", 1u32)
    } else if target.contains("esp32c2") {
        ("ESP32-C2", 1u32)
    } else if target.contains("esp32") {
        ("ESP32", 2u32)
    } else {
        // TARGET 未设置或不含 esp32 时按架构回退（常见于非常规构建）
        #[cfg(target_arch = "xtensa")]
        let fallback = ("ESP32-S3", 2u32);
        #[cfg(target_arch = "riscv32")]
        let fallback = ("ESP32-C3", 1u32);
        fallback
    };
    (model, 0u32, cores) // revision 运行时不可靠则填 0
}

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
fn collect_esp() -> String {
    use crate::platform::heap::{heap_free_internal, heap_free_spiram};

    // 芯片型号从编译目标推断；勿用 esp_idf_svc::sys::esp_chip_info（bindings 未暴露）
    let (chip_model, chip_revision, cores) = chip_model_from_target();

    let heap_internal = heap_free_internal();
    let psram_free = heap_free_spiram();
    let heap_free = heap_internal.saturating_add(psram_free);

    let heap_min_free = unsafe {
        esp_idf_svc::sys::heap_caps_get_minimum_free_size(esp_idf_svc::sys::MALLOC_CAP_INTERNAL)
            as u64
    };

    let uptime_secs = unsafe {
        let us = esp_idf_svc::sys::esp_timer_get_time();
        if us >= 0 {
            (us as u64) / 1_000_000
        } else {
            0
        }
    };

    // 编译时由 build.rs 从 IDF_PATH/version.txt 注入；未设置 IDF_PATH 时为 "unknown"
    let idf_version = option_env!("IDF_VERSION").unwrap_or("unknown");
    let budget = crate::orchestrator::current_budget();
    let wifi_sta_connected = crate::platform::is_wifi_sta_connected();
    let spiffs = crate::platform::spiffs_usage().map(|(total, used)| {
        let free = total.saturating_sub(used);
        json!({
            "total_bytes": total,
            "used_bytes": used,
            "free_bytes": free,
        })
    });

    let out = json!({
        "chip_model": chip_model,
        "chip_revision": chip_revision,
        "cores": cores,
        "heap_free": heap_free,
        "heap_min_free": heap_min_free,
        "psram_free": psram_free,
        "uptime_secs": uptime_secs,
        "idf_version": idf_version,
        "pressure_level": format!("{:?}", budget.level),
        "hint": budget.llm_hint,
        "wifi_sta_connected": wifi_sta_connected,
        "spiffs": spiffs,
    });
    out.to_string()
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
fn collect_host() -> String {
    let budget = crate::orchestrator::current_budget();
    let wifi_sta_connected = crate::platform::is_wifi_sta_connected();
    let spiffs = crate::platform::spiffs_usage().map(|(total, used)| {
        let free = total.saturating_sub(used);
        json!({
            "total_bytes": total,
            "used_bytes": used,
            "free_bytes": free,
        })
    });
    let out = json!({
        "chip_model": "host",
        "chip_revision": 0,
        "cores": 0,
        "heap_free": 0,
        "heap_min_free": 0,
        "psram_free": 0,
        "uptime_secs": 0,
        "idf_version": "n/a",
        "pressure_level": format!("{:?}", budget.level),
        "hint": budget.llm_hint,
        "wifi_sta_connected": wifi_sta_connected,
        "spiffs": spiffs,
    });
    out.to_string()
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
        let result = {
            #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
            {
                collect_esp()
            }
            #[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
            {
                collect_host()
            }
        };
        Ok(result)
    }
}
