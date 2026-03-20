//! 板级状态 JSON（芯片、堆、运行时间、压力、WiFi、SPIFFS）。供 `Platform::board_info_json` 与工具层复用。
//! Board status JSON (chip, heap, uptime, pressure, WiFi, SPIFFS). Shared by `Platform::board_info_json` and tools.

use serde_json::json;

// 从编译目标推断芯片型号（esp_chip_info 未在 esp-idf-sys bindings 中暴露，避免依赖）。
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
        #[cfg(target_arch = "xtensa")]
        let fallback = ("ESP32-S3", 2u32);
        #[cfg(target_arch = "riscv32")]
        let fallback = ("ESP32-C3", 1u32);
        fallback
    };
    (model, 0u32, cores)
}

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
fn collect_esp() -> String {
    use crate::platform::heap::{heap_free_internal, heap_free_spiram};

    let (chip_model, chip_revision, cores) = chip_model_from_target();

    let heap_internal = heap_free_internal();
    let psram_free = heap_free_spiram();
    let heap_total = heap_internal.saturating_add(psram_free);

    let heap_min_free = crate::platform::heap::heap_min_free_internal() as u64;

    let uptime_secs = crate::platform::time::uptime_secs();

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
        // Keep heap_free for backward compatibility; it represents internal + PSRAM total.
        "heap_free": heap_total,
        "heap_free_total": heap_total,
        "heap_free_internal": heap_internal,
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
        "heap_free_total": 0,
        "heap_free_internal": 0,
        "heap_min_free": 0,
        "psram_free": 0,
        "uptime_secs": crate::platform::time::uptime_secs(),
        "idf_version": "n/a",
        "pressure_level": format!("{:?}", budget.level),
        "hint": budget.llm_hint,
        "wifi_sta_connected": wifi_sta_connected,
        "spiffs": spiffs,
    });
    out.to_string()
}

/// 按当前编译目标生成板级 JSON 字符串（ESP 与 host 分支）。
pub fn board_info_json_string() -> String {
    #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
    {
        collect_esp()
    }
    #[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
    {
        collect_host()
    }
}
