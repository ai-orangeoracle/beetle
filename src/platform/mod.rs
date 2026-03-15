//! 平台抽象：仅此处依赖 esp-idf-svc/硬件。核心域不依赖本模块。
//! Platform: only place that depends on esp-idf-svc/hardware.

pub mod abstraction;
pub mod response_body;
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub mod task_wdt;
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub mod tls_admission;
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub mod esp32;
pub mod fetch_url;
pub mod heap;
pub mod heartbeat_file;
pub mod http_client;
pub mod http_server;
pub mod nvs;
pub mod pairing;
pub mod response;
pub mod spiffs;
pub mod wifi;
#[cfg(all(
    any(target_arch = "xtensa", target_arch = "riscv32"),
    esp_idf_comp_espressif__mdns_enabled
))]
pub mod mdns;

pub use abstraction::{ConfigStore, Platform, PlatformHttpClient, SkillMetaStore, SkillStorage};
pub use response_body::ResponseBody;
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub use esp32::Esp32Platform;
pub use fetch_url::fetch_url_to_bytes;
pub use heartbeat_file::read_heartbeat_file;
pub use http_client::EspHttpClient;
pub use nvs::{
    default_config_store, default_config_store_arc, erase_namespace,
    init_nvs, read_string, write_string, NvsConfigStore,
};
pub use spiffs::{
    default_skill_storage_arc, init_spiffs, spiffs_usage, SpiffsMemoryStore, SpiffsSessionStore,
    SpiffsSkillMetaStore, SpiffsSkillStorage, SPIFFS_BASE,
};
pub use wifi::{connect as connect_wifi, is_wifi_sta_connected, WifiApEntry, WifiScan, WifiScanHandle};