//! 未配置 LLM 时的占位实现；chat 返回固定提示文案。
//! No-op LLM when no source is configured; chat returns a fixed message.

use crate::error::Result;
use crate::llm::{LlmClient, LlmHttpClient, LlmResponse, Message, StopReason, ToolSpec};

pub struct NoopLlmClient;

impl Default for NoopLlmClient {
    fn default() -> Self {
        Self::new()
    }
}

impl NoopLlmClient {
    pub fn new() -> Self {
        Self
    }
}

impl LlmClient for NoopLlmClient {
    fn chat(
        &self,
        _http: &mut dyn LlmHttpClient,
        _system: &str,
        _messages: &[Message],
        _tools: Option<&[ToolSpec]>,
    ) -> Result<LlmResponse> {
        Ok(LlmResponse {
            content: "LLM 未配置或配置无效，请通过 Web UI / 配置 API 设置 llm_sources 或 api_key。"
                .to_string(),
            stop_reason: StopReason::EndTurn,
            tool_calls: None,
        })
    }
}
