//! ESP32 平台的 Platform 实现。仅在此目标编译。
//! ESP32 implementation of Platform trait.

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
use crate::platform::abstraction::Platform;
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
use crate::platform::{
    fetch_url::fetch_url_to_bytes,
    heartbeat_file::read_heartbeat_file,
    spiffs::{
        read_file, remove_file, spiffs_usage, write_file, SpiffsImportantMessageStore,
        SpiffsMemoryStore, SpiffsPendingRetryStore, SpiffsRemindAtStore, SpiffsSessionStore,
        SpiffsSessionSummaryStore, SpiffsSkillMetaStore, SpiffsSkillStorage,
        SpiffsTaskContinuationStore, SPIFFS_BASE,
    },
    NvsConfigStore,
};
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
use crate::{
    config::AppConfig,
    memory::{
        ImportantMessageStore, MemoryStore, PendingRetryStore, RemindAtStore, SessionStore,
        SessionSummaryStore, TaskContinuationStore,
    },
};
use std::path::PathBuf;
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
use std::sync::{Arc, Mutex};

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
/// ESP32 平台实现。
pub struct Esp32Platform {
    config_store: Arc<NvsConfigStore>,
    skill_storage: Arc<SpiffsSkillStorage>,
    skill_meta_store: Arc<SpiffsSkillMetaStore>,
    memory_store: Arc<SpiffsMemoryStore>,
    session_store: Arc<SpiffsSessionStore>,
    pending_retry_store: Arc<SpiffsPendingRetryStore>,
    task_continuation_store: Arc<SpiffsTaskContinuationStore>,
    important_message_store: Arc<SpiffsImportantMessageStore>,
    remind_at_store: Arc<SpiffsRemindAtStore>,
    session_summary_store: Arc<SpiffsSessionSummaryStore>,
    wifi_scan_handle: Mutex<Option<Arc<dyn crate::platform::WifiScan + Send + Sync>>>,
}

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
impl Esp32Platform {
    pub fn new() -> Self {
        Self {
            config_store: Arc::new(NvsConfigStore),
            skill_storage: Arc::new(SpiffsSkillStorage),
            skill_meta_store: Arc::new(SpiffsSkillMetaStore),
            memory_store: Arc::new(SpiffsMemoryStore::new()),
            session_store: Arc::new(SpiffsSessionStore::new()),
            pending_retry_store: Arc::new(SpiffsPendingRetryStore::new()),
            task_continuation_store: Arc::new(SpiffsTaskContinuationStore::new()),
            important_message_store: Arc::new(SpiffsImportantMessageStore::new()),
            remind_at_store: Arc::new(SpiffsRemindAtStore::new()),
            session_summary_store: Arc::new(SpiffsSessionSummaryStore::new()),
            wifi_scan_handle: Mutex::new(None),
        }
    }
}

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
impl Default for Esp32Platform {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
impl Platform for Esp32Platform {
    fn init_nvs(&self) -> crate::error::Result<()> {
        crate::platform::nvs::init_nvs()
    }

    fn init_spiffs(&self) -> crate::error::Result<()> {
        crate::platform::spiffs::init_spiffs()
    }

    fn config_store(&self) -> Arc<dyn crate::platform::ConfigStore + Send + Sync> {
        Arc::clone(&self.config_store) as Arc<dyn crate::platform::ConfigStore + Send + Sync>
    }

    fn connect_wifi(&self, config: &AppConfig) -> crate::error::Result<()> {
        match crate::platform::connect_wifi(config) {
            Ok(Some(handle)) => {
                let arc_dyn: Arc<dyn crate::platform::WifiScan + Send + Sync> = Arc::new(handle);
                *self
            .wifi_scan_handle
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(arc_dyn);
                Ok(())
            }
            Ok(None) => Ok(()),
            Err(e) => Err(e),
        }
    }

    fn wifi_scan(&self) -> Option<Arc<dyn crate::platform::WifiScan + Send + Sync>> {
        let opt: Option<Arc<dyn crate::platform::WifiScan + Send + Sync>> = self
            .wifi_scan_handle
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        opt
    }

    fn memory_store(&self) -> Arc<dyn MemoryStore + Send + Sync> {
        Arc::clone(&self.memory_store) as Arc<dyn MemoryStore + Send + Sync>
    }

    fn session_store(&self) -> Arc<dyn SessionStore + Send + Sync> {
        Arc::clone(&self.session_store) as Arc<dyn SessionStore + Send + Sync>
    }

    fn pending_retry_store(&self) -> Arc<dyn PendingRetryStore + Send + Sync> {
        Arc::clone(&self.pending_retry_store) as Arc<dyn PendingRetryStore + Send + Sync>
    }

    fn task_continuation_store(&self) -> Arc<dyn TaskContinuationStore + Send + Sync> {
        Arc::clone(&self.task_continuation_store) as Arc<dyn TaskContinuationStore + Send + Sync>
    }

    fn important_message_store(&self) -> Arc<dyn ImportantMessageStore + Send + Sync> {
        Arc::clone(&self.important_message_store) as Arc<dyn ImportantMessageStore + Send + Sync>
    }

    fn remind_at_store(&self) -> Arc<dyn RemindAtStore + Send + Sync> {
        Arc::clone(&self.remind_at_store) as Arc<dyn RemindAtStore + Send + Sync>
    }

    fn session_summary_store(&self) -> Arc<dyn SessionSummaryStore + Send + Sync> {
        Arc::clone(&self.session_summary_store) as Arc<dyn SessionSummaryStore + Send + Sync>
    }

    fn skill_storage(&self) -> Arc<dyn crate::platform::SkillStorage + Send + Sync> {
        Arc::clone(&self.skill_storage) as Arc<dyn crate::platform::SkillStorage + Send + Sync>
    }

    fn skill_meta_store(&self) -> Arc<dyn crate::platform::SkillMetaStore + Send + Sync> {
        Arc::clone(&self.skill_meta_store) as Arc<dyn crate::platform::SkillMetaStore + Send + Sync>
    }

    fn create_http_client(
        &self,
        config: &AppConfig,
    ) -> crate::error::Result<Box<dyn crate::platform::PlatformHttpClient>> {
        if !config.proxy_url.trim().is_empty() {
            Ok(Box::new(crate::platform::EspHttpClient::new_with_config(
                config,
            )?))
        } else {
            Ok(Box::new(crate::platform::EspHttpClient::new()?))
        }
    }

    fn spiffs_usage(&self) -> Option<(usize, usize)> {
        spiffs_usage()
    }

    fn read_heartbeat_file(&self) -> crate::error::Result<String> {
        read_heartbeat_file()
    }

    fn fetch_url_to_bytes(&self, url: &str, max_len: usize) -> crate::error::Result<Vec<u8>> {
        fetch_url_to_bytes(url, max_len)
    }

    fn read_config_file(&self, rel_path: &str) -> crate::error::Result<Option<Vec<u8>>> {
        let mut p = PathBuf::from(SPIFFS_BASE);
        p.push(rel_path);
        match read_file(&p) {
            Ok(b) => Ok(Some(b)),
            Err(_) => Ok(None),
        }
    }

    fn write_config_file(&self, rel_path: &str, data: &[u8]) -> crate::error::Result<()> {
        let mut p = PathBuf::from(SPIFFS_BASE);
        p.push(rel_path);
        write_file(&p, data)
    }

    fn remove_config_file(&self, rel_path: &str) -> crate::error::Result<()> {
        let mut p = PathBuf::from(SPIFFS_BASE);
        p.push(rel_path);
        let _ = remove_file(&p);
        Ok(())
    }

    fn init_mdns(&self) -> crate::error::Result<()> {
        #[cfg(esp_idf_comp_espressif__mdns_enabled)]
        return crate::platform::mdns::init();
        #[cfg(not(esp_idf_comp_espressif__mdns_enabled))]
        Ok(())
    }
}
