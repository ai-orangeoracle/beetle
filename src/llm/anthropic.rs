//! Anthropic Messages API 客户端；依赖注入 HTTP，不依赖 platform。
//! 错误均带 stage（llm_request / llm_parse）；后续健康检查可统计最近一次失败或计数。
//! Anthropic client; HTTP injected, no platform dependency.
//! Supports both non-streaming (default) and SSE streaming modes.

use crate::config::{AppConfig, LlmSource};
use crate::error::{Error, Result};
use crate::llm::types::{
    AnthropicRequest, AnthropicRequestMessage, AnthropicResponse, AnthropicTool,
    StopReason, ToolCall,
};
use crate::llm::{LlmClient, LlmHttpClient, LlmResponse, Message, ToolSpec};
use crate::llm::types::MAX_REQUEST_BODY_LEN;
use serde_json;

const TAG: &str = "llm::anthropic";
const API_BASE: &str = "https://api.anthropic.com/v1/messages";
const DEFAULT_MAX_TOKENS: u32 = 1024;

/// Anthropic Messages API 客户端；持 config 只读，HTTP 由 chat 时注入。
pub struct AnthropicClient {
    model: String,
    api_key: String,
    max_tokens: u32,
    api_base: String,
    stream: bool,
}

impl AnthropicClient {
    pub fn new(config: &AppConfig) -> Self {
        Self::from_source(
            &LlmSource {
                provider: config.model_provider.clone(),
                api_key: config.api_key.clone(),
                model: config.model.clone(),
                api_url: config.api_url.clone(),
                max_tokens: None,
            },
            false,
        )
    }

    /// 从单源配置构造，供多源回退使用。api_url 非空时替代默认 API_BASE。
    pub fn from_source(source: &LlmSource, stream: bool) -> Self {
        let api_base = if source.api_url.trim().is_empty() {
            API_BASE.to_string()
        } else {
            source.api_url.trim_end_matches('/').to_string()
        };
        Self {
            model: source.model.clone(),
            api_key: source.api_key.clone(),
            max_tokens: source.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
            api_base,
            stream,
        }
    }
}

impl LlmClient for AnthropicClient {
    fn chat(
        &self,
        http: &mut dyn LlmHttpClient,
        system: &str,
        messages: &[Message],
        tools: Option<&[ToolSpec]>,
    ) -> Result<LlmResponse> {
        let body = build_request_body(&self.model, self.max_tokens, system, messages, tools, self.stream)?;

        if self.stream {
            crate::llm::retry::with_retry(2, 500, TAG, http, |http| {
                do_request_streaming(http, &self.api_base, &self.api_key, &body, None)
            })
        } else {
            crate::llm::retry::with_retry(2, 500, TAG, http, |http| {
                do_request(http, &self.api_base, &self.api_key, &body)
            })
        }
    }

    fn chat_with_progress(
        &self,
        http: &mut dyn LlmHttpClient,
        system: &str,
        messages: &[Message],
        tools: Option<&[ToolSpec]>,
        on_progress: crate::llm::StreamProgressFn,
    ) -> Result<LlmResponse> {
        if !self.stream {
            return self.chat(http, system, messages, tools);
        }
        let body = build_request_body(&self.model, self.max_tokens, system, messages, tools, true)?;
        // Cannot use retry wrapper with mutable on_progress, so do single attempt.
        do_request_streaming(http, &self.api_base, &self.api_key, &body, Some(on_progress))
    }
}

fn build_request_body(
    model: &str,
    max_tokens: u32,
    system: &str,
    messages: &[Message],
    tools: Option<&[ToolSpec]>,
    stream: bool,
) -> Result<Vec<u8>> {
    let tools_api = tools.and_then(|t| {
        if t.is_empty() {
            None
        } else {
            Some(
                t.iter()
                    .map(|s| AnthropicTool {
                        name: s.name.clone(),
                        description: s.description.clone(),
                        input_schema: s.parameters.clone(),
                    })
                    .collect::<Vec<_>>(),
            )
        }
    });
    let req = AnthropicRequest {
        model: model.to_string(),
        max_tokens,
        system: if system.is_empty() {
            None
        } else {
            Some(system.to_string())
        },
        messages: messages
            .iter()
            .map(|m| AnthropicRequestMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect(),
        tools: tools_api,
        stream: if stream { Some(true) } else { None },
    };
    let body = serde_json::to_vec(&req).map_err(|e| Error::Other {
        source: Box::new(e),
        stage: "llm_parse",
    })?;
    if body.len() > MAX_REQUEST_BODY_LEN {
        return Err(Error::config(
            "llm_request",
            format!("request body exceeds {} bytes", MAX_REQUEST_BODY_LEN),
        ));
    }
    Ok(body)
}

fn do_request(
    http: &mut dyn LlmHttpClient,
    url: &str,
    api_key: &str,
    body: &[u8],
) -> Result<LlmResponse> {
    const ANTHROPIC_VERSION: &str = "2023-06-01";
    let mut cl_buf = [0u8; 20];
    let content_length = crate::util::usize_to_decimal_buf(&mut cl_buf, body.len());
    let headers = [
        ("x-api-key", api_key),
        ("anthropic-version", ANTHROPIC_VERSION),
        ("content-type", "application/json"),
        ("content-length", content_length),
    ];
    let (status, resp_body) = http.do_post(url, &headers, body).map_err(|e| match e {
        Error::Http { status_code, .. } => Error::Http {
            status_code,
            stage: "llm_request",
        },
        _ => Error::Other {
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("{:?}", e),
            )),
            stage: "llm_request",
        },
    })?;

    if status == 429 {
        log::warn!("[{}] rate limited (429)", TAG);
        return Err(Error::Http {
            status_code: 429,
            stage: "llm_request",
        });
    }
    if status >= 400 {
        return Err(Error::Http {
            status_code: status,
            stage: "llm_request",
        });
    }

    let parsed: AnthropicResponse = serde_json::from_slice(resp_body.as_ref()).map_err(|e| Error::Other {
        source: Box::new(e),
        stage: "llm_parse",
    })?;

    Ok(LlmResponse::from_anthropic(parsed))
}

// ---------- SSE streaming ----------

/// Anthropic SSE 流式累加器：逐事件拼接 content / tool_calls，最终产出 LlmResponse。
struct AnthropicStreamAccumulator {
    content: String,
    stop_reason: StopReason,
    /// 当前正在累积的 tool_use 块列表。
    tool_calls: Vec<ToolCallBuilder>,
}

struct ToolCallBuilder {
    id: String,
    name: String,
    input_json: String,
}

impl AnthropicStreamAccumulator {
    fn new() -> Self {
        Self {
            content: String::new(),
            stop_reason: StopReason::Other,
            tool_calls: Vec::new(),
        }
    }

    /// 处理单条 SSE 事件（event type + JSON data）。
    fn handle_event(&mut self, event_type: &str, data: &str) {
        match event_type {
            "content_block_start" => {
                // content_block_start: 新增 text 或 tool_use block。
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(cb) = val.get("content_block") {
                        let block_type = cb.get("type").and_then(|v| v.as_str()).unwrap_or("");
                        if block_type == "tool_use" {
                            let id = cb.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let name = cb.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            self.tool_calls.push(ToolCallBuilder {
                                id,
                                name,
                                input_json: String::new(),
                            });
                        }
                    }
                }
            }
            "content_block_delta" => {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(delta) = val.get("delta") {
                        let delta_type = delta.get("type").and_then(|v| v.as_str()).unwrap_or("");
                        match delta_type {
                            "text_delta" => {
                                if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                                    self.content.push_str(text);
                                }
                            }
                            "input_json_delta" => {
                                if let Some(partial) = delta.get("partial_json").and_then(|v| v.as_str()) {
                                    if let Some(tc) = self.tool_calls.last_mut() {
                                        tc.input_json.push_str(partial);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            "message_delta" => {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(delta) = val.get("delta") {
                        if let Some(sr) = delta.get("stop_reason").and_then(|v| v.as_str()) {
                            self.stop_reason = match sr {
                                "end_turn" => StopReason::EndTurn,
                                "tool_use" => StopReason::ToolUse,
                                "max_tokens" => StopReason::MaxTokens,
                                _ => StopReason::Other,
                            };
                        }
                    }
                }
            }
            _ => {
                // message_start, content_block_stop, message_stop, ping: 忽略。
            }
        }
    }

    /// 流结束，产出最终 LlmResponse。
    fn finish(self) -> LlmResponse {
        let tool_calls: Vec<ToolCall> = self.tool_calls
            .into_iter()
            .map(|tc| ToolCall {
                id: tc.id,
                name: tc.name,
                input: if tc.input_json.is_empty() { "{}".to_string() } else { tc.input_json },
            })
            .collect();
        LlmResponse {
            content: self.content,
            stop_reason: self.stop_reason,
            tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
        }
    }
}

fn do_request_streaming(
    http: &mut dyn LlmHttpClient,
    url: &str,
    api_key: &str,
    body: &[u8],
    on_progress: Option<crate::llm::StreamProgressFn>,
) -> Result<LlmResponse> {
    const ANTHROPIC_VERSION: &str = "2023-06-01";
    let mut cl_buf = [0u8; 20];
    let content_length = crate::util::usize_to_decimal_buf(&mut cl_buf, body.len());
    let headers = [
        ("x-api-key", api_key),
        ("anthropic-version", ANTHROPIC_VERSION),
        ("content-type", "application/json"),
        ("content-length", content_length),
    ];

    let mut accumulator = AnthropicStreamAccumulator::new();
    let mut sse_reader = crate::llm::sse::SseLineReader::new();
    let mut progress_cb = on_progress;

    let status = http.do_post_streaming(url, &headers, body, &mut |chunk| {
        sse_reader.feed(chunk);
        while let Some(event) = sse_reader.next_event() {
            // Check for text_delta before handling, to capture the delta for progress callback.
            let delta_text = if event.event == "content_block_delta" {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&event.data) {
                    val.get("delta")
                        .and_then(|d| d.get("type"))
                        .and_then(|t| t.as_str())
                        .filter(|&t| t == "text_delta")
                        .and_then(|_| val.get("delta").and_then(|d| d.get("text")).and_then(|t| t.as_str()))
                        .map(|s| s.to_string())
                } else {
                    None
                }
            } else {
                None
            };

            accumulator.handle_event(&event.event, &event.data);

            if let (Some(ref delta), Some(ref mut cb)) = (&delta_text, &mut progress_cb) {
                cb(delta, &accumulator.content);
            }
        }
        Ok(())
    }).map_err(|e| match e {
        Error::Http { status_code, .. } => Error::Http {
            status_code,
            stage: "llm_request",
        },
        _ => Error::Other {
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("{:?}", e),
            )),
            stage: "llm_request",
        },
    })?;

    if status == 429 {
        log::warn!("[{}] rate limited (429)", TAG);
        return Err(Error::Http {
            status_code: 429,
            stage: "llm_request",
        });
    }
    if status >= 400 {
        return Err(Error::Http {
            status_code: status,
            stage: "llm_request",
        });
    }

    Ok(accumulator.finish())
}
