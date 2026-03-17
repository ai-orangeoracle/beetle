//! analyze_image 工具：通过 LLM 多模态能力分析图片 URL；HTTP 经 ToolContext 注入，不依赖 platform。
//! analyze_image tool: analyzes image URLs via LLM vision; HTTP injected through ToolContext.

use crate::config::AppConfig;
use crate::error::{Error, Result};
use crate::tools::{parse_tool_args, Tool, ToolContext};
use serde_json::json;

const TAG: &str = "tools::analyze_image";
const STAGE: &str = "tool_analyze_image";
const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const OPENAI_DEFAULT_API_BASE: &str = "https://api.openai.com/v1";
const VISION_MAX_TOKENS: u32 = 1024;

pub struct AnalyzeImageTool {
    api_key: String,
    model: String,
    api_base: String,
    provider: String,
}

impl AnalyzeImageTool {
    pub fn new(config: &AppConfig) -> Self {
        let first_source = config.llm_sources.first();
        let provider = first_source
            .map(|s| s.provider.as_str())
            .unwrap_or(config.model_provider.as_str())
            .to_string();
        let api_key = first_source
            .map(|s| s.api_key.as_str())
            .unwrap_or(config.api_key.as_str())
            .to_string();
        let model = first_source
            .map(|s| s.model.as_str())
            .unwrap_or(config.model.as_str())
            .to_string();
        let api_base = first_source
            .map(|s| s.api_url.as_str())
            .unwrap_or(config.api_url.as_str())
            .to_string();
        Self {
            api_key,
            model,
            api_base,
            provider,
        }
    }

    /// 构建 Anthropic Messages API 请求体。
    fn build_anthropic_request(&self, image_url: &str, question: &str) -> serde_json::Value {
        json!({
            "model": self.model,
            "max_tokens": VISION_MAX_TOKENS,
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "image",
                        "source": {
                            "type": "url",
                            "url": image_url
                        }
                    },
                    {
                        "type": "text",
                        "text": question
                    }
                ]
            }]
        })
    }

    /// 构建 OpenAI Chat Completions 请求体。
    fn build_openai_request(&self, image_url: &str, question: &str) -> serde_json::Value {
        json!({
            "model": self.model,
            "max_tokens": VISION_MAX_TOKENS,
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "image_url",
                        "image_url": { "url": image_url }
                    },
                    {
                        "type": "text",
                        "text": question
                    }
                ]
            }]
        })
    }

    /// 从 Anthropic 响应 JSON 提取文本。
    fn extract_anthropic_text(resp: &serde_json::Value) -> Option<String> {
        resp.get("content")?
            .as_array()?
            .iter()
            .filter_map(|block| {
                if block.get("type")?.as_str()? == "text" {
                    block.get("text")?.as_str().map(String::from)
                } else {
                    None
                }
            })
            .next()
    }

    /// 从 OpenAI 响应 JSON 提取文本。
    fn extract_openai_text(resp: &serde_json::Value) -> Option<String> {
        resp.get("choices")?
            .as_array()?
            .first()?
            .get("message")?
            .get("content")?
            .as_str()
            .map(String::from)
    }
}

impl Tool for AnalyzeImageTool {
    fn name(&self) -> &str {
        "analyze_image"
    }

    fn description(&self) -> &str {
        "Analyze an image from a URL using vision AI. Use this when a user sends an image URL and you need to understand its content."
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "image_url": {
                    "type": "string",
                    "description": "The HTTP/HTTPS URL of the image to analyze"
                },
                "question": {
                    "type": "string",
                    "description": "A specific question about the image (default: describe the image in detail)"
                }
            },
            "required": ["image_url"]
        })
    }

    fn execute(&self, args: &str, ctx: &mut dyn ToolContext) -> Result<String> {
        let m = parse_tool_args(args, STAGE)?;

        let image_url = m
            .get("image_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::config(STAGE, "missing or invalid image_url"))?;

        // 校验 URL scheme
        if !image_url.starts_with("http://") && !image_url.starts_with("https://") {
            return Err(Error::config(STAGE, "image_url must start with http:// or https://"));
        }

        let question = m
            .get("question")
            .and_then(|v| v.as_str())
            .unwrap_or("Describe this image in detail");

        if self.api_key.is_empty() {
            return Ok("analyze_image: no API key configured".to_string());
        }

        let is_anthropic = self.provider == "anthropic";

        // 构建请求
        let body = if is_anthropic {
            self.build_anthropic_request(image_url, question)
        } else {
            self.build_openai_request(image_url, question)
        };

        let body_bytes = serde_json::to_vec(&body).map_err(|e| Error::Other {
            source: Box::new(e),
            stage: STAGE,
        })?;

        // 构建 URL 和 headers
        let url = if is_anthropic {
            if self.api_base.is_empty() {
                ANTHROPIC_API_URL.to_string()
            } else {
                self.api_base.trim_end_matches('/').to_string()
            }
        } else {
            let base = if self.api_base.is_empty() {
                OPENAI_DEFAULT_API_BASE
            } else {
                self.api_base.trim_end_matches('/')
            };
            format!("{}/chat/completions", base)
        };

        let auth_value = if is_anthropic {
            self.api_key.clone()
        } else {
            format!("Bearer {}", self.api_key)
        };

        let headers: Vec<(&str, &str)> = if is_anthropic {
            vec![
                ("Content-Type", "application/json"),
                ("x-api-key", &auth_value),
                ("anthropic-version", "2023-06-01"),
            ]
        } else {
            vec![
                ("Content-Type", "application/json"),
                ("Authorization", &auth_value),
            ]
        };

        log::info!(
            "[{}] POST {} provider={} model={} image_url_len={}",
            TAG,
            url,
            self.provider,
            self.model,
            image_url.len()
        );

        let (status, resp_body) = ctx.post_with_headers(&url, &headers, &body_bytes)?;

        if status >= 400 {
            let err_text = String::from_utf8_lossy(resp_body.as_ref());
            log::warn!("[{}] API returned status={}: {}", TAG, status, err_text);
            return Err(Error::Http {
                status_code: status,
                stage: STAGE,
            });
        }

        let parsed: serde_json::Value =
            serde_json::from_slice(resp_body.as_ref()).map_err(|e| Error::Other {
                source: Box::new(e),
                stage: STAGE,
            })?;

        let text = if is_anthropic {
            Self::extract_anthropic_text(&parsed)
        } else {
            Self::extract_openai_text(&parsed)
        };

        match text {
            Some(t) => {
                log::info!("[{}] result len={}", TAG, t.len());
                Ok(t)
            }
            None => {
                log::warn!("[{}] failed to extract text from response", TAG);
                Ok("analyze_image: could not extract text from API response".to_string())
            }
        }
    }
}
