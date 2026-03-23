//! 甲壳虫 (beetle) - 稳定对外 API。
//! beetle - stable public API.

mod build_info;
pub mod constants;
pub mod metrics;
pub mod util;

pub use build_info::{build_board_id, ota_manifest_url};
/// Re-export PlatformHttpClient at crate root so core modules (agent, tools) can depend on
/// `crate::PlatformHttpClient` without importing `crate::platform` directly.
pub use platform::PlatformHttpClient;
pub mod agent;
pub mod bus;
pub mod channels;
pub mod config;
pub mod doctor;
pub mod display;
pub mod error;
pub mod llm;
pub mod memory;
pub mod platform;
pub mod state;
pub mod tools;

#[cfg(feature = "cli")]
pub mod cli;

#[cfg(all(feature = "ota", any(target_arch = "xtensa", target_arch = "riscv32")))]
pub mod ota;

pub mod cron;
pub mod heartbeat;
pub mod orchestrator;
pub mod skills;

pub use agent::{
    build_context, run_agent_loop, AgentLoopConfig, ContextParams, StreamEditor,
    DEFAULT_MESSAGES_MAX_LEN, DEFAULT_SYSTEM_MAX_LEN, SESSION_RECENT_N,
};
pub use bus::{MessageBus, PcMsg, DEFAULT_CAPACITY, MAX_CONTENT_LEN};
#[cfg(feature = "feishu")]
pub use channels::run_feishu_ws_loop;
pub use channels::run_qq_ws_loop;
pub use channels::{
    feishu_acquire_token, feishu_edit_message, feishu_send_and_get_id, flush_dingtalk_sends,
    flush_feishu_sends, flush_qq_channel_sends, flush_telegram_sends, flush_wecom_sends,
    get_bot_username, poll_telegram_once, run_dingtalk_sender_loop, run_dispatch,
    run_feishu_sender_loop, run_qq_sender_loop, run_telegram_poll_loop, run_telegram_sender_loop,
    run_wecom_sender_loop, send_chat_action, tg_edit_message_text, tg_send_and_get_id,
    ChannelHttpClient, ChannelSinks, LogSink, MessageSink, QueuedSink, WebSocketSink,
};
pub use config::{
    parse_allowed_chat_ids, save_hardware_segment, AppConfig, DeviceEntry, HardwareSegment,
    I2cBusConfig, I2cDeviceEntry, LlmSource, PinConfig,
};
pub use display::{
    default_disabled_display_config, validate_display_config_core, DisplayBus, DisplayChannelStatus,
    DisplayColorOrder, DisplayCommand, DisplayConfig, DisplayDriver, DisplayPressureLevel,
    DisplaySystemState,
};
pub use error::{Error, Result};
pub use llm::{
    build_llm_clients, AnthropicClient, FallbackLlmClient, LlmClient, LlmHttpClient, LlmResponse,
    Message, OpenAiCompatibleClient,
};
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub use platform::{
    connect_wifi, init_nvs, init_spiffs, spiffs_usage, Esp32Platform, EspHttpClient,
    SpiffsMemoryStore, SpiffsSessionStore, SPIFFS_BASE,
};
pub use platform::{ConfigStore, Platform, SkillStorage, StateFs};
pub use tools::{
    build_default_registry, CronManageTool, DeviceControlTool,
    FileWriteTool, FilesTool, GetTimeTool, HttpRequestTool, I2cDeviceTool, KvStoreTool,
    MemoryManageTool, ModelConfigTool, NetworkScanTool, ProxyConfigTool, RemindAtTool,
    SensorWatchTool, SessionManageTool, SystemControlTool, Tool, ToolContext, ToolRegistry,
    UpdateSessionSummaryTool, WebSearchTool,
};

/// 任何 PlatformHttpClient 均可作为 LlmHttpClient、ToolContext、ChannelHttpClient 使用。
impl<T: platform::PlatformHttpClient> llm::LlmHttpClient for T {
    fn do_post(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<(u16, platform::ResponseBody)> {
        platform::PlatformHttpClient::post(self, url, headers, body)
    }
    fn do_post_streaming(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
        on_chunk: &mut dyn FnMut(&[u8]) -> Result<()>,
    ) -> Result<u16> {
        platform::PlatformHttpClient::post_streaming(self, url, headers, body, on_chunk)
    }
    fn reset_connection_for_retry(&mut self) {
        platform::PlatformHttpClient::reset_connection_for_retry(self);
    }
}

impl<T: platform::PlatformHttpClient> tools::ToolContext for T {
    fn get_with_headers(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<(u16, platform::ResponseBody)> {
        platform::PlatformHttpClient::get(self, url, headers)
    }
    fn post_with_headers(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<(u16, platform::ResponseBody)> {
        platform::PlatformHttpClient::post(self, url, headers, body)
    }
    fn put_with_headers(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<(u16, platform::ResponseBody)> {
        platform::PlatformHttpClient::put(self, url, headers, body)
    }
    fn delete_with_headers(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<(u16, platform::ResponseBody)> {
        platform::PlatformHttpClient::delete(self, url, headers)
    }
}

impl<T: platform::PlatformHttpClient> channels::ChannelHttpClient for T {
    fn http_get(&mut self, url: &str) -> Result<(u16, platform::ResponseBody)> {
        platform::PlatformHttpClient::get(self, url, &[])
    }
    fn http_get_with_headers(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<(u16, platform::ResponseBody)> {
        platform::PlatformHttpClient::get(self, url, headers)
    }
    fn http_post(&mut self, url: &str, body: &[u8]) -> Result<(u16, platform::ResponseBody)> {
        platform::PlatformHttpClient::post(self, url, &[], body)
    }
    fn http_post_with_headers(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<(u16, platform::ResponseBody)> {
        platform::PlatformHttpClient::post(self, url, headers, body)
    }
    fn http_patch_with_headers(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<(u16, platform::ResponseBody)> {
        platform::PlatformHttpClient::patch(self, url, headers, body)
    }
    fn reset_connection_for_retry(&mut self) {
        platform::PlatformHttpClient::reset_connection_for_retry(self);
    }
}

impl channels::ChannelHttpClient for dyn platform::PlatformHttpClient + '_ {
    fn http_get(&mut self, url: &str) -> Result<(u16, platform::ResponseBody)> {
        platform::PlatformHttpClient::get(self, url, &[])
    }
    fn http_get_with_headers(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<(u16, platform::ResponseBody)> {
        platform::PlatformHttpClient::get(self, url, headers)
    }
    fn http_post(&mut self, url: &str, body: &[u8]) -> Result<(u16, platform::ResponseBody)> {
        platform::PlatformHttpClient::post(self, url, &[], body)
    }
    fn http_post_with_headers(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<(u16, platform::ResponseBody)> {
        platform::PlatformHttpClient::post(self, url, headers, body)
    }
    fn http_patch_with_headers(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<(u16, platform::ResponseBody)> {
        platform::PlatformHttpClient::patch(self, url, headers, body)
    }
    fn reset_connection_for_retry(&mut self) {
        platform::PlatformHttpClient::reset_connection_for_retry(self);
    }
}
