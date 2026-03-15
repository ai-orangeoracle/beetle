//! LLM 抽象与实现。核心域不依赖 platform；HTTP 由 main 注入。
//! LLM trait and implementations; HTTP client injected by main.

mod retry;
mod types;
pub mod sse;

pub mod anthropic;
pub mod fallback;
pub mod noop;
pub mod openai_compatible;

pub use anthropic::AnthropicClient;
pub use fallback::FallbackLlmClient;
pub use noop::NoopLlmClient;
pub use openai_compatible::OpenAiCompatibleClient;

pub use types::{
    LlmResponse, Message, StopReason, ToolCall, ToolSpec, MAX_MESSAGE_CONTENT_LEN,
    MAX_REQUEST_BODY_LEN,
};

use crate::error::Result;

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
}
