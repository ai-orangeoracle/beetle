//! LLM 抽象与实现。核心域不依赖 platform；HTTP 由 main 注入。
//! LLM trait and implementations; HTTP client injected by main.

mod retry;
pub mod sse;
mod types;

pub mod anthropic;
pub mod fallback;
pub mod noop;
pub mod openai_compatible;

pub use anthropic::AnthropicClient;
pub use fallback::FallbackLlmClient;
pub use noop::NoopLlmClient;
pub use openai_compatible::OpenAiCompatibleClient;

pub use types::{
    LlmResponse, Message, StopReason, StreamProgressFn, ToolCall, ToolSpec,
    MAX_MESSAGE_CONTENT_LEN, MAX_REQUEST_BODY_LEN,
};

use crate::config::AppConfig;
use crate::error::Result;
use crate::i18n::Locale;
use std::sync::Arc;

/// 从配置构建单一 worker LLM 客户端。
/// worker 内部使用 Fallback 链。空列表返回 NoopLlmClient。
pub fn build_llm_clients(
    config: &AppConfig,
    resolve_locale: Arc<dyn Fn() -> Locale + Send + Sync>,
) -> Box<dyn LlmClient> {
    const TAG: &str = "beetle";

    let global_stream = config.llm_stream;

    let llm_clients: Vec<Box<dyn LlmClient>> = config
        .llm_sources
        .iter()
        .filter(|s| {
            let has_key = !s.api_key.trim().is_empty();
            let has_model = !s.model.trim().is_empty();
            let has_provider = !s.provider.trim().is_empty();
            let has_url = !s.api_url.trim().is_empty()
                || s.provider == "openai"
                || s.provider == "openai_compatible"
                || s.provider == "gemini"
                || s.provider == "glm"
                || s.provider == "qwen"
                || s.provider == "deepseek"
                || s.provider == "moonshot"
                || s.provider == "ollama";
            has_key && has_model && has_provider && has_url
        })
        .map(|s| -> Box<dyn LlmClient> {
            match s.provider.as_str() {
                "openai" | "openai_compatible" | "gemini" | "glm" | "qwen" | "deepseek"
                | "moonshot" | "ollama" => {
                    Box::new(OpenAiCompatibleClient::from_source(s, global_stream))
                }
                _ => Box::new(AnthropicClient::from_source(s, global_stream)),
            }
        })
        .collect();

    if llm_clients.is_empty() {
        log::warn!(
            "[{}] no valid llm source configured; using NoopLlmClient and skipping external LLM calls",
            TAG
        );
        log::info!(
            "[{}] LLM is in no-op mode: local tools and message processing remain available",
            TAG
        );
        return Box::new(NoopLlmClient::new(Arc::clone(&resolve_locale)));
    }

    Box::new(FallbackLlmClient::new(llm_clients))
}

/// 供 main 注入的 HTTP 客户端抽象；platform::EspHttpClient 在 lib 中实现此 trait。
/// 方法名 do_post 避免与 EspHttpClient::post 重名；headers 含 x-api-key、anthropic-version 等。
pub trait LlmHttpClient {
    fn do_post(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<(u16, crate::platform::ResponseBody)>;

    /// SSE 流式 POST：发送后逐块回调 on_chunk。默认回退到 do_post + 单次 on_chunk。
    fn do_post_streaming(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
        on_chunk: &mut dyn FnMut(&[u8]) -> Result<()>,
    ) -> Result<u16> {
        let (status, resp_body) = self.do_post(url, headers, body)?;
        on_chunk(resp_body.as_ref())?;
        Ok(status)
    }

    /// 重试前调用；失败后连接可能非 initial，实现方可替换为新连接以避免 "connection is not in initial phase"。
    fn reset_connection_for_retry(&mut self) {}
}

/// LLM 客户端 trait；Agent 只依赖此接口。
pub trait LlmClient {
    /// 发起一次 chat；tools 本阶段传 None。HTTP 客户端由调用方注入。
    fn chat(
        &self,
        http: &mut dyn LlmHttpClient,
        system: &str,
        messages: &[Message],
        tools: Option<&[ToolSpec]>,
    ) -> Result<LlmResponse>;

    /// 带流式进度回调的 chat；默认忽略 progress，走普通 chat。
    /// on_progress(delta, accumulated) 在每次收到 text_delta 时调用。
    fn chat_with_progress(
        &self,
        http: &mut dyn LlmHttpClient,
        system: &str,
        messages: &[Message],
        tools: Option<&[ToolSpec]>,
        _on_progress: StreamProgressFn,
    ) -> Result<LlmResponse> {
        self.chat(http, system, messages, tools)
    }
}
