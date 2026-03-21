//! 工具抽象与注册。核心域不依赖 platform；HTTP 等由 main 注入 ToolContext。
//! Tool trait and registry; no platform dependency.

mod registry;

pub mod analyze_image;
pub mod board_info;
pub mod cron;
pub mod cron_manage;
pub mod daily_note;
pub mod fetch_url;
pub mod file_write;
pub mod files;
pub mod get_time;
pub mod hardware;
pub mod http_post;
pub mod http_request;
pub mod kv_store;
pub mod memory_manage;
pub mod model_config;
pub mod proxy_config;
pub mod remind_at;
pub mod session_manage;
pub mod system_control;
pub mod update_session_summary;
pub mod web_search;

pub use analyze_image::AnalyzeImageTool;
pub use board_info::BoardInfoTool;
pub use cron::CronTool;
pub use cron_manage::CronManageTool;
pub use daily_note::DailyNoteTool;
pub use fetch_url::FetchUrlTool;
pub use file_write::FileWriteTool;
pub use files::FilesTool;
pub use get_time::GetTimeTool;
pub use hardware::DeviceControlTool;
pub use http_post::HttpPostTool;
pub use http_request::HttpRequestTool;
pub use kv_store::KvStoreTool;
pub use memory_manage::MemoryManageTool;
pub use model_config::ModelConfigTool;
pub use proxy_config::ProxyConfigTool;
pub use registry::{build_default_registry, ToolRegistry};
pub use remind_at::{RemindAtTool, RemindListTool};
pub use session_manage::SessionManageTool;
pub use system_control::SystemControlTool;
pub use update_session_summary::UpdateSessionSummaryTool;
pub use web_search::WebSearchTool;

use crate::error::{Error, Result};
use serde_json::{Map, Value};

/// 将 args 解析为 JSON 对象；供各 tool execute 统一使用，stage 用于错误上下文。
pub fn parse_tool_args(args: &str, stage: &'static str) -> Result<Map<String, Value>> {
    let v: Value = serde_json::from_str(args).map_err(|e| Error::Other {
        source: Box::new(e),
        stage,
    })?;
    v.as_object()
        .cloned()
        .ok_or_else(|| Error::config(stage, "tool args must be a JSON object"))
}

/// 单次 execute 的 args 最大长度（字符）。超限返回 Error::Config。
pub const MAX_TOOL_ARGS_LEN: usize = 8 * 1024;
/// 单次 execute 返回值最大长度（字符）。超限截断或返回 Error::Config。
pub const MAX_TOOL_RESULT_LEN: usize = 16 * 1024;

/// 工具执行时注入的上下文；HTTP 等由 lib 实现（如 EspHttpClient）。
/// 当前会话的 chat_id/channel 供 remind_at 等工具使用；默认 None，agent 循环内用 wrapper 注入。
pub trait ToolContext {
    fn get(&mut self, url: &str) -> Result<(u16, crate::platform::ResponseBody)> {
        self.get_with_headers(url, &[])
    }
    fn get_with_headers(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<(u16, crate::platform::ResponseBody)>;
    /// POST 请求，自定义 headers（须含 Content-Type 等）；供 web_search Tavily 等使用。
    fn post_with_headers(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<(u16, crate::platform::ResponseBody)>;
    /// HTTP PUT 请求；默认回退到 post_with_headers。
    fn put_with_headers(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<(u16, crate::platform::ResponseBody)> {
        self.post_with_headers(url, headers, body)
    }
    /// HTTP DELETE 请求；默认回退到 get_with_headers。
    fn delete_with_headers(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<(u16, crate::platform::ResponseBody)> {
        self.get_with_headers(url, headers)
    }
    /// 当前入站消息的 chat_id；remind_at 等工具写存储时使用。默认 None。
    fn current_chat_id(&self) -> Option<&str> {
        None
    }
    /// 当前入站消息的 channel。默认 None。
    fn current_channel(&self) -> Option<&str> {
        None
    }
}

/// 工具 trait；Agent 按 name 派发，execute 时传入 ctx 以发 HTTP 等。
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> Value;
    fn execute(&self, args: &str, ctx: &mut dyn ToolContext) -> Result<String>;
    /// 该工具是否需要网络（HTTP/TLS）；orchestrator 在高压力时拒绝网络工具。
    /// Whether this tool requires network (HTTP/TLS); orchestrator denies network tools under high pressure.
    fn requires_network(&self) -> bool {
        false
    }
}
