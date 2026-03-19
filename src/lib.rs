//! 甲虫 (beetle) - 稳定对外 API。
//! beetle - stable public API.

mod build_info;
mod constants;
mod metrics;
mod util;

pub use build_info::{build_board_id, ota_manifest_url};
/// Re-export PlatformHttpClient at crate root so core modules (agent, tools) can depend on
/// `crate::PlatformHttpClient` without importing `crate::platform` directly.
pub use platform::PlatformHttpClient;
pub mod agent;
pub mod bus;
pub mod config;
pub mod channels;
pub mod doctor;
pub mod error;
pub mod llm;
pub mod memory;
pub mod platform;
pub mod state;
pub mod tools;

#[cfg(feature = "cli")]
pub mod cli;

#[cfg(feature = "ota")]
pub mod ota;

pub mod cron;
pub mod heartbeat;
pub mod orchestrator;
pub mod skills;

pub use agent::{
    build_context, run_agent_loop, AgentLoopConfig, ContextParams, StreamEditor, DEFAULT_MESSAGES_MAX_LEN,
    DEFAULT_SYSTEM_MAX_LEN, SESSION_RECENT_N,
};
pub use bus::{MessageBus, PcMsg, DEFAULT_CAPACITY, MAX_CONTENT_LEN};
pub use channels::{
    feishu_acquire_token, feishu_edit_message, feishu_send_and_get_id, flush_dingtalk_sends,
    flush_feishu_sends, flush_qq_channel_sends, flush_telegram_sends, flush_wecom_sends,
    get_bot_username, poll_telegram_once, run_dingtalk_sender_loop, run_dispatch,
    run_feishu_sender_loop, run_qq_sender_loop, run_telegram_poll_loop,
    run_telegram_sender_loop, run_wecom_sender_loop, send_chat_action, tg_edit_message_text,
    tg_send_and_get_id, ChannelHttpClient, ChannelSinks, LogSink, MessageSink, QueuedSink,
    WebSocketSink,
};
#[cfg(all(feature = "feishu", any(target_arch = "xtensa", target_arch = "riscv32")))]
pub use channels::run_feishu_ws_loop;
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub use channels::run_qq_ws_loop;
pub use config::{
    parse_allowed_chat_ids, LlmSource, AppConfig,
    DeviceEntry, PinConfig, HardwareSegment, save_hardware_segment,
};
pub use error::{Error, Result};
pub use llm::{
    AnthropicClient, FallbackLlmClient, LlmClient, LlmHttpClient, LlmResponse, Message,
    OpenAiCompatibleClient, build_llm_clients,
};
pub use platform::{
    connect_wifi, init_nvs, init_spiffs, spiffs_usage, EspHttpClient, SpiffsMemoryStore,
    SpiffsSessionStore, SPIFFS_BASE,
};
pub use platform::{ConfigStore, Platform, SkillStorage};
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub use platform::Esp32Platform;
pub use tools::{
    CronTool, DeviceControlTool, FetchUrlTool, FilesTool, GetTimeTool, HttpPostTool, KvStoreTool,
    RemindAtTool, Tool, ToolContext, ToolRegistry, UpdateSessionSummaryTool, WebSearchTool,
    build_default_registry,
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