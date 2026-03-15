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
pub mod spiffs_important_message;
pub mod spiffs_memory;
pub mod spiffs_pending_retry;
pub mod spiffs_skill_meta;
pub mod spiffs_skill_storage;
pub mod spiffs_session;
pub mod spiffs_remind_at;
pub mod spiffs_session_summary;
pub mod spiffs_task_continuation;
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
pub use spiffs::{init_spiffs, spiffs_usage, SPIFFS_BASE};
pub use spiffs_memory::SpiffsMemoryStore;
pub use spiffs_session::SpiffsSessionStore;
pub use spiffs_skill_meta::SpiffsSkillMetaStore;
pub use spiffs_skill_storage::{default_skill_storage_arc, SpiffsSkillStorage};
pub use wifi::{connect as connect_wifi, is_wifi_sta_connected, WifiApEntry, WifiScan, WifiScanHandle};