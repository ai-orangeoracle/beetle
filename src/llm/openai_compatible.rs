//! OpenAI Chat Completions 兼容客户端；可对接 OpenAI / OpenRouter / 本地兼容服务。
//! 错误带 stage（llm_request / llm_parse）；HTTP 由 main 注入。
//! Supports both non-streaming (default) and SSE streaming modes.

use crate::config::{AppConfig, LlmSource};
use crate::error::{Error, Result};
use crate::llm::types::{LlmResponse, StopReason, ToolCall, MAX_REQUEST_BODY_LEN};
use crate::llm::{LlmClient, LlmHttpClient, Message, ToolSpec};
use serde::{Deserialize, Serialize};

const TAG: &str = "llm::openai_compat";
const DEFAULT_API_BASE: &str = "https://api.openai.com/v1";
const CHAT_PATH: &str = "/chat/completions";
const DEFAULT_MAX_TOKENS: u32 = 1024;

/// OpenAI 兼容客户端；持 config 只读，HTTP 由 chat 时注入。
pub struct OpenAiCompatibleClient {
    api_base: String,
    model: String,
    api_key: String,
    max_tokens: u32,
    stream: bool,
}

impl OpenAiCompatibleClient {
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

    /// 从单源配置构造，供多源回退使用。
    pub fn from_source(source: &LlmSource, stream: bool) -> Self {
        let api_base = if source.api_url.is_empty() {
            DEFAULT_API_BASE.to_string()
        } else {
            source.api_url.trim_end_matches('/').to_string()
        };
        Self {
            api_base,
            model: source.model.clone(),
            api_key: source.api_key.clone(),
            max_tokens: source.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
            stream,
        }
    }
}

// --- OpenAI Chat Completions 请求/响应 DTO ---

#[derive(Debug, Serialize)]
struct OpenAiRequestMessageRef<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Serialize)]
struct OpenAiFunctionSpecRef<'a> {
    name: &'a str,
    description: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<&'a serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct OpenAiToolRef<'a> {
    #[serde(rename = "type")]
    tool_type: &'static str,
    function: OpenAiFunctionSpecRef<'a>,
}

#[derive(Debug, Serialize)]
struct OpenAiRequestRef<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: Vec<OpenAiRequestMessageRef<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAiToolRef<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAiToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolCall {
    id: Option<String>,
    #[serde(rename = "type")]
    _type: Option<String>,
    function: Option<OpenAiToolCallFunction>,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolCallFunction {
    name: Option<String>,
    arguments: Option<String>,
}

fn finish_reason_to_stop_reason(s: Option<&str>) -> StopReason {
    match s {
        Some("stop") => StopReason::EndTurn,
        Some("tool_calls") => StopReason::ToolUse,
        Some("length") => StopReason::MaxTokens,
        _ => StopReason::Other,
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
    let mut req_messages: Vec<OpenAiRequestMessageRef<'_>> =
        Vec::with_capacity(messages.len() + usize::from(!system.is_empty()));
    if !system.is_empty() {
        req_messages.push(OpenAiRequestMessageRef {
            role: "system",
            content: system,
        });
    }
    for m in messages {
        req_messages.push(OpenAiRequestMessageRef {
            role: &m.role,
            content: &m.content,
        });
    }

    let tools_api = tools.and_then(|t| {
        if t.is_empty() {
            None
        } else {
            Some(
                t.iter()
                    .map(|s| OpenAiToolRef {
                        tool_type: "function",
                        function: OpenAiFunctionSpecRef {
                            name: &s.name,
                            description: &s.description,
                            parameters: Some(&s.parameters),
                        },
                    })
                    .collect::<Vec<_>>(),
            )
        }
    });

    let req = OpenAiRequestRef {
        model,
        max_tokens,
        messages: req_messages,
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

impl LlmClient for OpenAiCompatibleClient {
    fn chat(
        &self,
        http: &mut dyn LlmHttpClient,
        system: &str,
        messages: &[Message],
        tools: Option<&[ToolSpec]>,
    ) -> Result<LlmResponse> {
        let body = build_request_body(
            &self.model,
            self.max_tokens,
            system,
            messages,
            tools,
            self.stream,
        )?;
        let url = format!("{}{}", self.api_base, CHAT_PATH);
        if self.stream {
            crate::llm::retry::with_retry(2, 500, TAG, http, |http| {
                do_request_streaming(http, &url, &self.api_key, &body, None)
            })
        } else {
            crate::llm::retry::with_retry(2, 500, TAG, http, |http| {
                do_request(http, &url, &self.api_key, &body)
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
        let url = format!("{}{}", self.api_base, CHAT_PATH);
        // Cannot use retry wrapper with mutable on_progress, so do single attempt.
        do_request_streaming(http, &url, &self.api_key, &body, Some(on_progress))
    }
}

fn do_request(
    http: &mut dyn LlmHttpClient,
    url: &str,
    api_key: &str,
    body: &[u8],
) -> Result<LlmResponse> {
    let mut cl_buf = [0u8; 20];
    let content_length = crate::util::usize_to_decimal_buf(&mut cl_buf, body.len());
    let auth_value = format!("Bearer {}", api_key);
    let mut headers: Vec<(&str, &str)> = vec![
        ("Content-Type", "application/json"),
        ("Content-Length", content_length),
    ];
    if !api_key.is_empty() {
        headers.insert(0, ("Authorization", auth_value.as_str()));
    }
    let (status, resp_body) = http.do_post(url, &headers, body).map_err(|e| match e {
        Error::Http { status_code, .. } => Error::Http {
            status_code,
            stage: "llm_request",
        },
        _ => Error::Other {
            source: Box::new(std::io::Error::other(format!("{:?}", e))),
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

    let choice = {
        let parsed: OpenAiResponse =
            serde_json::from_slice(resp_body.as_ref()).map_err(|e| Error::Other {
                source: Box::new(e),
                stage: "llm_parse",
            })?;
        parsed
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| Error::Other {
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "openai response has no choices",
                )),
                stage: "llm_parse",
            })?
    };
    drop(resp_body);

    let content = choice.message.content.unwrap_or_default();
    let stop_reason = finish_reason_to_stop_reason(choice.finish_reason.as_deref());
    let tool_calls = choice
        .message
        .tool_calls
        .map(|tc_list| {
            tc_list
                .into_iter()
                .filter_map(|tc| {
                    let id = tc.id.unwrap_or_default();
                    let func = tc.function?;
                    let name = func.name.unwrap_or_default();
                    let input = func.arguments.unwrap_or_else(|| "{}".to_string());
                    Some(ToolCall { id, name, input })
                })
                .collect::<Vec<_>>()
        })
        .filter(|v| !v.is_empty());

    Ok(LlmResponse {
        content,
        stop_reason,
        tool_calls: if tool_calls.as_ref().is_none_or(|v| v.is_empty()) {
            None
        } else {
            tool_calls
        },
    })
}

// ---------- SSE streaming ----------

/// OpenAI SSE 流式累加器：逐事件拼接 content / tool_calls。
struct OpenAiStreamAccumulator {
    content: String,
    stop_reason: StopReason,
    /// 按 index 累积 tool_calls（OpenAI streaming delta 中 tool_calls 带 index 字段）。
    tool_calls: Vec<OpenAiToolCallBuilder>,
}

struct OpenAiToolCallBuilder {
    id: String,
    name: String,
    arguments: String,
}

impl OpenAiStreamAccumulator {
    fn new() -> Self {
        Self {
            content: String::new(),
            stop_reason: StopReason::Other,
            tool_calls: Vec::new(),
        }
    }

    /// 处理单条 SSE data 的已解析 JSON chunk；返回 content_delta 供进度回调。
    fn handle_value(&mut self, val: &serde_json::Value) -> Option<String> {
        let mut delta_text: Option<String> = None;

        let choices = val.get("choices").and_then(|v| v.as_array())?;

        for choice in choices {
            // finish_reason
            if let Some(fr) = choice.get("finish_reason").and_then(|v| v.as_str()) {
                self.stop_reason = finish_reason_to_stop_reason(Some(fr));
            }

            let delta = match choice.get("delta") {
                Some(d) => d,
                None => continue,
            };

            // delta.content
            if let Some(text) = delta.get("content").and_then(|v| v.as_str()) {
                self.content.push_str(text);
                delta_text = Some(text.to_string());
            }

            // delta.tool_calls
            if let Some(tc_arr) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                for tc in tc_arr {
                    let index = tc.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    // Guard against malicious index values that could cause OOM.
                    const MAX_TOOL_CALL_INDEX: usize = 128;
                    if index > MAX_TOOL_CALL_INDEX {
                        log::warn!(
                            "[openai_stream] tool_calls index {} exceeds max {}, skipping",
                            index,
                            MAX_TOOL_CALL_INDEX
                        );
                        continue;
                    }
                    // 确保 tool_calls vec 足够长。
                    while self.tool_calls.len() <= index {
                        self.tool_calls.push(OpenAiToolCallBuilder {
                            id: String::new(),
                            name: String::new(),
                            arguments: String::new(),
                        });
                    }
                    let builder = &mut self.tool_calls[index];
                    if let Some(id) = tc.get("id").and_then(|v| v.as_str()) {
                        builder.id = id.to_string();
                    }
                    if let Some(func) = tc.get("function") {
                        if let Some(name) = func.get("name").and_then(|v| v.as_str()) {
                            builder.name.push_str(name);
                        }
                        if let Some(args) = func.get("arguments").and_then(|v| v.as_str()) {
                            builder.arguments.push_str(args);
                        }
                    }
                }
            }
        }
        delta_text
    }

    fn finish(self) -> LlmResponse {
        let tool_calls: Vec<ToolCall> = self
            .tool_calls
            .into_iter()
            .filter(|tc| !tc.name.is_empty())
            .map(|tc| ToolCall {
                id: tc.id,
                name: tc.name,
                input: if tc.arguments.is_empty() {
                    "{}".to_string()
                } else {
                    tc.arguments
                },
            })
            .collect();
        LlmResponse {
            content: self.content,
            stop_reason: self.stop_reason,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
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
    let mut cl_buf = [0u8; 20];
    let content_length = crate::util::usize_to_decimal_buf(&mut cl_buf, body.len());
    let auth_value = format!("Bearer {}", api_key);
    let mut headers: Vec<(&str, &str)> = vec![
        ("Content-Type", "application/json"),
        ("Content-Length", content_length),
    ];
    if !api_key.is_empty() {
        headers.insert(0, ("Authorization", auth_value.as_str()));
    }

    let mut accumulator = OpenAiStreamAccumulator::new();
    let mut sse_reader = crate::llm::sse::SseLineReader::new();
    let mut progress_cb = on_progress;

    let status = http
        .do_post_streaming(url, &headers, body, &mut |chunk| {
            sse_reader.feed(chunk);
            while let Some(event) = sse_reader.next_event() {
                if event.data == "[DONE]" {
                    continue;
                }
                let parsed = match serde_json::from_str::<serde_json::Value>(&event.data) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let delta_text = accumulator.handle_value(&parsed);

                if let (Some(ref delta), Some(ref mut cb)) = (&delta_text, &mut progress_cb) {
                    cb(delta, &accumulator.content);
                }
            }
            Ok(())
        })
        .map_err(|e| match e {
            Error::Http { status_code, .. } => Error::Http {
                status_code,
                stage: "llm_request",
            },
            _ => Error::Other {
                source: Box::new(std::io::Error::other(format!("{:?}", e))),
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
