//! http_post 工具：HTTP POST 指定 URL，携带自定义 body 与 Content-Type，返回响应 body（截断至 4KB）。
//! http_post tool: POST a URL with body and Content-Type, return response body truncated to 4KB.

use crate::error::{Error, Result};
use crate::tools::{parse_tool_args, Tool, ToolContext};
use serde_json::{json, Value};

const RESP_BODY_MAX: usize = 4096;

pub struct HttpPostTool;

impl Tool for HttpPostTool {
    fn name(&self) -> &str {
        "http_post"
    }
    fn description(&self) -> &str {
        "Send an HTTP POST request to a URL with a body and return the response body (truncated to \
         4KB). Use to trigger webhooks, push data to REST APIs (e.g. HomeAssistant, IFTTT, n8n), \
         or call any external service. Only http(s) URLs allowed."
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "Target URL (must start with http:// or https://)"
                },
                "body": {
                    "type": "string",
                    "description": "Request body as a UTF-8 string (e.g. JSON payload)"
                },
                "content_type": {
                    "type": "string",
                    "description": "Content-Type header value (default: application/json)"
                }
            },
            "required": ["url", "body"]
        })
    }
    fn requires_network(&self) -> bool {
        true
    }
    fn execute(&self, args: &str, ctx: &mut dyn ToolContext) -> Result<String> {
        let m = parse_tool_args(args, "tool_http_post")?;

        let url = m
            .get("url")
            .and_then(|u| u.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| Error::config("tool_http_post", "invalid or missing url"))?;

        if !(url.starts_with("http://") || url.starts_with("https://")) || url.len() <= 8 {
            return Err(Error::config(
                "tool_http_post",
                "url must start with http:// or https://",
            ));
        }
        if crate::util::is_private_url(url) {
            return Err(Error::config(
                "tool_http_post",
                "access to private/internal addresses is not allowed",
            ));
        }

        let body = m
            .get("body")
            .and_then(|b| b.as_str())
            .unwrap_or("");

        let content_type = m
            .get("content_type")
            .and_then(|c| c.as_str())
            .unwrap_or("application/json");

        let headers = [("Content-Type", content_type)];
        let (status, resp_body) = ctx.post_with_headers(url, &headers, body.as_bytes())?;

        if !(200..300).contains(&status) {
            return Ok(format!("HTTP status: {}", status));
        }

        let body_slice = resp_body.as_ref();
        let out = if body_slice.len() > RESP_BODY_MAX {
            &body_slice[..RESP_BODY_MAX]
        } else {
            body_slice
        };
        Ok(String::from_utf8_lossy(out).into_owned())
    }
}
