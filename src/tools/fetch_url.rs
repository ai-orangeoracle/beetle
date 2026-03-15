//! fetch_url 工具：HTTP GET 拉取 URL，返回 body 截断至 4KB；仅允许 http(s)。
//! fetch_url tool: HTTP GET a URL, return body truncated to 4KB; http(s) only.

use crate::error::{Error, Result};
use crate::tools::{parse_tool_args, Tool, ToolContext};
use serde_json::{json, Value};

const FETCH_BODY_MAX: usize = 4096;

pub struct FetchUrlTool;

impl Tool for FetchUrlTool {
    fn name(&self) -> &str {
        "fetch_url"
    }
    fn description(&self) -> &str {
        "Fetch a URL with HTTP GET and return the response body (text, truncated to 4KB). Only http(s) URLs allowed."
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "URL to fetch (http or https only)" }
            },
            "required": ["url"]
        })
    }
    fn execute(&self, args: &str, ctx: &mut dyn ToolContext) -> Result<String> {
        let m = parse_tool_args(args, "tool_fetch_url")?;
        let url = m
            .get("url")
            .and_then(|u| u.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| Error::config("tool_fetch_url", "invalid or missing url"))?;
        let valid = (url.starts_with("http://") || url.starts_with("https://")) && url.len() > 8;
        if !valid {
            return Err(Error::config("tool_fetch_url", "invalid or missing url"));
        }
        let (status, body) = ctx.get(url)?;
        if !(200..300).contains(&status) {
            return Ok(format!("HTTP status: {}", status));
        }
        let body_slice = body.as_ref();
        let body = if body_slice.len() > FETCH_BODY_MAX {
            &body_slice[..FETCH_BODY_MAX]
        } else {
            body_slice
        };
        Ok(String::from_utf8_lossy(body).into_owned())
    }
}
