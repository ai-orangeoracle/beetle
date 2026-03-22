//! 平台抽象：仅此处依赖 esp-idf-svc/硬件。核心域不依赖本模块。
//! Platform: only place that depends on esp-idf-svc/hardware.

pub mod abstraction;
pub mod board_info;
pub mod csrf;
pub mod display_driver;
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub mod esp32;
pub mod fetch_url;
pub mod hardware_drivers;
pub mod heap;
pub mod heartbeat_file;
pub mod http_client;
pub mod http_server;
pub mod nvs;
pub mod pairing;
pub mod response;
pub mod response_body;
pub mod sntp;
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub mod softap_ip;
pub mod spiffs;
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub mod task_wdt;
pub mod time;
pub mod wifi;

pub use abstraction::{ConfigStore, Platform, PlatformHttpClient, SkillMetaStore, SkillStorage};
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub use esp32::Esp32Platform;
pub use fetch_url::fetch_url_with_client;
pub use heartbeat_file::read_heartbeat_file;
pub use http_client::EspHttpClient;
pub use nvs::{
    default_config_store, default_config_store_arc, erase_namespace, init_nvs, read_string,
    write_string, NvsConfigStore,
};
pub use response_body::ResponseBody;
pub use sntp::init_sntp;
pub use spiffs::{
    default_skill_storage_arc, init_spiffs, spiffs_usage, SpiffsMemoryStore, SpiffsSessionStore,
    SpiffsSkillMetaStore, SpiffsSkillStorage, SPIFFS_BASE,
};
pub use wifi::{
    connect as connect_wifi, is_wifi_sta_connected, wait_for_network_ready, WifiApEntry, WifiScan,
    WifiScanHandle,
};
