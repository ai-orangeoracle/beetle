//! 平台抽象 trait：ConfigStore、SkillStorage、PlatformHttpClient、Platform。
//! 核心域与 main 仅依赖这些 trait，便于后续支持多种硬件。

use crate::config::AppConfig;
use crate::error::Result;
use crate::platform::ResponseBody;
use crate::memory::{
    ImportantMessageStore, MemoryStore, PendingRetryStore, RemindAtStore, SessionStore,
    SessionSummaryStore, TaskContinuationStore,
};
use std::sync::Arc;

/// 配置键值存储抽象（如 NVS）。用于 config、pairing、skills 的 NVS 部分。
pub trait ConfigStore: Send + Sync {
    fn read_string(&self, key: &str) -> Result<Option<String>>;
    /// 批量读取；默认逐键 read_string。NVS 实现可覆写为单 handle 多 key 读，减少 open/close 避免 4361。
    fn read_strings(&self, keys: &[&str]) -> Result<Vec<Option<String>>> {
        keys.iter()
            .map(|k| self.read_string(k))
            .collect()
    }
    fn write_string(&self, key: &str, value: &str) -> Result<()>;
    /// 批量写入；默认逐键 write_string，NVS 实现可覆写为单 handle 批量写以避免 4361。
    fn write_strings(&self, pairs: &[(&str, &str)]) -> Result<()> {
        for (k, v) in pairs {
            self.write_string(k, v)?;
        }
        Ok(())
    }
    /// 擦除指定 keys；命名空间由实现方绑定（如 pc_cfg）。
    fn erase_keys(&self, keys: &[&str]) -> Result<()>;
}

/// 技能元数据（顺序、禁用列表）存储抽象。用于 SPIFFS config/skills_meta.json，避免 NVS 高频单键写。
pub trait SkillMetaStore: Send + Sync {
    /// 返回 (order, disabled)。
    fn read_meta(&self) -> Result<(Vec<String>, Vec<String>)>;
    fn write_meta(&self, order: &[String], disabled: &[String]) -> Result<()>;
}

/// Skills 目录下 .md 文件存储抽象。list_names 返回不含 .md 后缀的名称。
pub trait SkillStorage: Send + Sync {
    fn list_names(&self) -> Result<Vec<String>>;
    fn read(&self, name: &str) -> Result<Vec<u8>>;
    fn write(&self, name: &str, content: &[u8]) -> Result<()>;
    fn remove(&self, name: &str) -> Result<()>;
}

/// 统一 HTTP 客户端：仅 get/post/post_streaming/reset 方法，LlmHttpClient、ToolContext、ChannelHttpClient 由 lib 层 blanket 转发。
pub trait PlatformHttpClient {
    fn get(&mut self, url: &str, headers: &[(&str, &str)]) -> Result<(u16, ResponseBody)>;
    fn post(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<(u16, ResponseBody)>;
    /// HTTP PATCH; default implementation falls back to POST.
    fn patch(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<(u16, ResponseBody)> {
        self.post(url, headers, body)
    }
    /// SSE 流式 POST：发送请求后逐块回调 on_chunk，不将响应体读入内存。
    /// 默认实现回退到 post()，将完整响应体一次性传给 on_chunk。
    fn post_streaming(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
        on_chunk: &mut dyn FnMut(&[u8]) -> Result<()>,
    ) -> Result<u16> {
        let (status, resp_body) = self.post(url, headers, body)?;
        on_chunk(resp_body.as_ref())?;
        Ok(status)
    }
    fn reset_connection_for_retry(&mut self) {}
}

impl PlatformHttpClient for Box<dyn PlatformHttpClient + '_> {
    fn get(&mut self, url: &str, headers: &[(&str, &str)]) -> Result<(u16, ResponseBody)> {
        (**self).get(url, headers)
    }
    fn post(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<(u16, ResponseBody)> {
        (**self).post(url, headers, body)
    }
    fn post_streaming(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
        on_chunk: &mut dyn FnMut(&[u8]) -> Result<()>,
    ) -> Result<u16> {
        (**self).post_streaming(url, headers, body, on_chunk)
    }
    fn reset_connection_for_retry(&mut self) {
        (**self).reset_connection_for_retry()
    }
    fn patch(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<(u16, ResponseBody)> {
        (**self).patch(url, headers, body)
    }
}

/// 平台能力聚合。main 只依赖当前平台的 Platform 实现。Send + Sync 以便跨线程传入 run_http_server。
pub trait Platform: Send + Sync {
    /// 平台初始化（link_patches、日志、NVS、SPIFFS 等）。main 在构造后首先调用。
    fn init(&self) -> Result<()> {
        Ok(())
    }
    fn init_nvs(&self) -> Result<()>;
    fn init_spiffs(&self) -> Result<()>;
    fn config_store(&self) -> Arc<dyn ConfigStore + Send + Sync>;
    fn connect_wifi(&self, config: &AppConfig) -> Result<()>;
    /// WiFi 扫描句柄（仅 ESP32 在 connect_wifi 成功后为 Some）；用于 GET /api/wifi/scan。
    fn wifi_scan(&self) -> Option<Arc<dyn crate::platform::WifiScan + Send + Sync>> {
        None
    }
    fn memory_store(&self) -> Arc<dyn MemoryStore + Send + Sync>;
    fn session_store(&self) -> Arc<dyn SessionStore + Send + Sync>;
    fn pending_retry_store(&self) -> Arc<dyn PendingRetryStore + Send + Sync>;
    fn task_continuation_store(&self) -> Arc<dyn TaskContinuationStore + Send + Sync>;
    fn important_message_store(&self) -> Arc<dyn ImportantMessageStore + Send + Sync>;
    fn remind_at_store(&self) -> Arc<dyn RemindAtStore + Send + Sync>;
    fn session_summary_store(&self) -> Arc<dyn SessionSummaryStore + Send + Sync>;
    fn skill_storage(&self) -> Arc<dyn SkillStorage + Send + Sync>;
    fn skill_meta_store(&self) -> Arc<dyn SkillMetaStore + Send + Sync>;
    fn create_http_client(&self, config: &AppConfig) -> Result<Box<dyn PlatformHttpClient>>;
    fn spiffs_usage(&self) -> Option<(usize, usize)>;
    fn read_heartbeat_file(&self) -> Result<String>;
    fn fetch_url_to_bytes(&self, url: &str, max_len: usize) -> Result<Vec<u8>>;

    /// 读 SPIFFS 配置文件（相对路径如 config/llm.json）。不存在或非 ESP 返回 Ok(None)。默认 no-op 返回 Ok(None)。
    fn read_config_file(&self, _rel_path: &str) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }

    /// 写 SPIFFS 配置文件。非 ESP 返回 Ok(()) no-op。默认 no-op。
    fn write_config_file(&self, _rel_path: &str, _data: &[u8]) -> Result<()> {
        Ok(())
    }

    /// 删除 SPIFFS 上的配置文件（相对路径如 config/skills_meta.json）。用于 config reset 时清理。默认 no-op。
    fn remove_config_file(&self, _rel_path: &str) -> Result<()> {
        Ok(())
    }

    /// 请求设备重启。ESP 实现调用 esp_restart()；host 默认 no-op。
    fn request_restart(&self) {
        log::warn!("request_restart: not implemented on this platform");
    }

    /// 启动 SNTP 时间同步。WiFi 连接后调用。
    fn init_sntp(&self) {
        log::info!("init_sntp: no-op on this platform");
    }

    /// OTA 固件升级。ESP 实现调用 ota_update_from_url；非 ESP 返回错误。
    fn ota_from_url(&self, _url: &str) -> Result<()> {
        Err(crate::error::Error::config("ota", "OTA not supported on this platform"))
    }
}
