//! OpenAI Chat Completions 兼容客户端；可对接 OpenAI / OpenRouter / 本地兼容服务。
//! 错误带 stage（llm_request / llm_parse）；HTTP 由 main 注入。

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
}

impl OpenAiCompatibleClient {
    pub fn new(config: &AppConfig) -> Self {
        Self::from_source(&LlmSource {
            provider: config.model_provider.clone(),
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            api_url: config.api_url.clone(),
        })
    }

    /// 从单源配置构造，供多源回退使用。
    pub fn from_source(source: &LlmSource) -> Self {
        let api_base = if source.api_url.is_empty() {
            DEFAULT_API_BASE.to_string()
        } else {
            source.api_url.trim_end_matches('/').to_string()
        };
        Self {
            api_base,
            model: source.model.clone(),
            api_key: source.api_key.clone(),
            max_tokens: DEFAULT_MAX_TOKENS,
        }
    }
}

// --- OpenAI Chat Completions 请求/响应 DTO ---

#[derive(Debug, Serialize)]
struct OpenAiRequestMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct OpenAiFunctionSpec {
    name: String,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct OpenAiTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAiFunctionSpec,
}

#[derive(Debug, Serialize)]
struct OpenAiRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<OpenAiRequestMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAiTool>>,
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

impl LlmClient for OpenAiCompatibleClient {
    fn chat(
        &self,
        http: &mut dyn LlmHttpClient,
        system: &str,
        messages: &[Message],
        tools: Option<&[ToolSpec]>,
    ) -> Result<LlmResponse> {
        let mut req_messages: Vec<OpenAiRequestMessage> = Vec::new();
        if !system.is_empty() {
            req_messages.push(OpenAiRequestMessage {
                role: "system".to_string(),
                content: system.to_string(),
            });
        }
        for m in messages {
            req_messages.push(OpenAiRequestMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            });
        }

        let tools_api = tools.and_then(|t| {
            if t.is_empty() {
                None
            } else {
                Some(
                    t.iter()
                        .map(|s| OpenAiTool {
                            tool_type: "function".to_string(),
                            function: OpenAiFunctionSpec {
                                name: s.name.clone(),
                                description: s.description.clone(),
                                parameters: Some(s.parameters.clone()),
                            },
                        })
                        .collect::<Vec<_>>(),
                )
            }
        });

        let req = OpenAiRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            messages: req_messages,
            tools: tools_api,
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

        let url = format!("{}{}", self.api_base, CHAT_PATH);
        crate::llm::retry::with_retry(2, 500, TAG, http, |http| {
            do_request(http, &url, &self.api_key, &body)
        })
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
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("{:?}", e),
            )),
            stage: "llm_request",
        },
    })?;

    if status == 429 {
        log::warn!("[{}] rate limited (429), backing off", TAG);
        std::thread::sleep(std::time::Duration::from_secs(5));
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

    let parsed: OpenAiResponse = serde_json::from_slice(resp_body.as_ref()).map_err(|e| Error::Other {
        source: Box::new(e),
        stage: "llm_parse",
    })?;

    let choice = parsed
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| Error::Other {
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "openai response has no choices",
            )),
            stage: "llm_parse",
        })?;

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
        tool_calls: if tool_calls.as_ref().map_or(true, |v| v.is_empty()) {
            None
        } else {
            tool_calls
        },
    })
}
