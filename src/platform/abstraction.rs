//! 平台抽象 trait：ConfigStore、SkillStorage、PlatformHttpClient、Platform。
//! 核心域与 main 仅依赖这些 trait，便于后续支持多种硬件。

use crate::config::AppConfig;
use crate::display::{DisplayCommand, DisplayConfig};
use crate::error::Result;
use crate::memory::{
    ImportantMessageStore, MemoryStore, PendingRetryStore, RemindAtStore, SessionStore,
    SessionSummaryStore, TaskContinuationStore,
};
use crate::platform::ResponseBody;
use std::sync::Arc;

/// 配置键值存储抽象（如 NVS）。用于 config、pairing、skills 的 NVS 部分。
pub trait ConfigStore: Send + Sync {
    fn read_string(&self, key: &str) -> Result<Option<String>>;
    /// 批量读取；默认逐键 read_string。NVS 实现可覆写为单 handle 多 key 读，减少 open/close 避免 4361。
    fn read_strings(&self, keys: &[&str]) -> Result<Vec<Option<String>>> {
        keys.iter().map(|k| self.read_string(k)).collect()
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
    /// HTTP PUT; default implementation falls back to POST.
    fn put(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<(u16, ResponseBody)> {
        self.post(url, headers, body)
    }
    /// HTTP DELETE; default implementation falls back to GET.
    fn delete(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<(u16, ResponseBody)> {
        self.get(url, headers)
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
    fn put(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<(u16, ResponseBody)> {
        (**self).put(url, headers, body)
    }
    fn delete(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<(u16, ResponseBody)> {
        (**self).delete(url, headers)
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
    /// 当前 STA IPv4 地址（例如 192.168.1.42）；不可用时返回 None。
    fn wifi_sta_ip(&self) -> Option<String> {
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

    /// 板级状态 JSON（芯片、堆、运行时间、压力、WiFi、SPIFFS）。默认实现委托 `platform/board_info`；新平台可覆写。
    fn board_info_json(&self) -> Result<String> {
        Ok(crate::platform::board_info::board_info_json_string())
    }

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
        Err(crate::error::Error::config(
            "ota",
            "OTA not supported on this platform",
        ))
    }

    /// 初始化显示器硬件。默认 no-op（非显示平台）。
    fn init_display(&self, _config: &DisplayConfig) -> Result<()> {
        Ok(())
    }

    /// 显示器是否可用。默认 false。
    fn display_available(&self) -> bool {
        false
    }

    /// 执行显示指令。默认 no-op。
    fn display_command(&self, _cmd: DisplayCommand) -> Result<()> {
        Ok(())
    }

    /// 设置显示器背光开关。on=true 开启，on=false 关闭。默认 no-op。
    /// Set display backlight on/off. Default no-op.
    fn set_display_backlight(&self, _on: bool) -> Result<()> {
        Ok(())
    }

    /// 背光控制是否可用（需有 BL 引脚且显示器已初始化）。默认 false。
    /// Whether backlight control is available. Default false.
    fn display_backlight_available(&self) -> bool {
        false
    }

    /// 设置显示器背光亮度（0-100%）。PWM 调光；默认 no-op。
    /// Set display backlight brightness (0-100%). Default no-op.
    fn set_display_backlight_brightness(&self, _percent: u8) -> Result<()> {
        Ok(())
    }

    /// 背光渐变（阻塞，在调用线程执行）。默认 no-op。
    /// Fade display backlight from `from`% to `to`% over `duration_ms`. Blocking. Default no-op.
    fn fade_display_backlight(&self, _from: u8, _to: u8, _duration_ms: u32) -> Result<()> {
        Ok(())
    }

    /// I2C 读取：从指定地址的寄存器读取数据。默认返回不支持错误。
    /// I2C read: read data from register at given address. Default returns unsupported error.
    fn i2c_read(&self, _addr: u8, _register: u8, _len: usize) -> Result<Vec<u8>> {
        Err(crate::error::Error::config(
            "i2c_read",
            "I2C not supported on this platform",
        ))
    }

    /// I2C 写入：向指定地址的寄存器写入数据。默认返回不支持错误。
    /// I2C write: write data to register at given address. Default returns unsupported error.
    fn i2c_write(&self, _addr: u8, _register: u8, _data: &[u8]) -> Result<()> {
        Err(crate::error::Error::config(
            "i2c_write",
            "I2C not supported on this platform",
        ))
    }
}
