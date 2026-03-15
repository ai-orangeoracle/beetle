//! web_search 工具：Tavily 优先、Brave 兜底；经 ToolContext 发 HTTP。
//! web_search tool: Tavily first, Brave fallback; uses ToolContext for HTTP.

use crate::config::AppConfig;
use crate::error::{Error, Result};
use crate::tools::{parse_tool_args, Tool, ToolContext};
use crate::util::percent_encode_query;
use serde_json::json;

const TAG: &str = "tools::web_search";
const BRAVE_SEARCH_URL: &str = "https://api.search.brave.com/res/v1/web/search";
const TAVILY_SEARCH_URL: &str = "https://api.tavily.com/search";

pub struct WebSearchTool {
    pub api_key: String,
    pub tavily_key: String,
}

impl WebSearchTool {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            api_key: config.search_key.clone(),
            tavily_key: config.tavily_key.clone(),
        }
    }
}

impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }
    fn description(&self) -> &str {
        "Search the web for a query. Returns a short summary."
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query" }
            },
            "required": ["query"]
        })
    }
    fn execute(&self, args: &str, ctx: &mut dyn ToolContext) -> Result<String> {
        let m = parse_tool_args(args, "tool_web_search")?;
        let query = m
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::config("tool_web_search", "missing or invalid query"))?;

        // 1) 若配置了 Tavily，先请求 Tavily
        if !self.tavily_key.is_empty() {
            let body = json!({ "query": query });
            let body_bytes = serde_json::to_vec(&body).map_err(|e| Error::Other {
                source: Box::new(e),
                stage: "tool_web_search",
            })?;
            let auth = format!("Bearer {}", self.tavily_key);
            let headers: [(&str, &str); 2] = [
                ("Content-Type", "application/json"),
                ("Authorization", &auth),
            ];
            let (status, resp_body) = match ctx.post_with_headers(
                TAVILY_SEARCH_URL,
                &headers,
                &body_bytes,
            ) {
                Ok(r) => r,
                Err(e) => {
                    log::warn!("[{}] Tavily request failed: {:?}", TAG, e);
                    return self.fallback_brave_or_error(query, ctx);
                }
            };
            if (200..300).contains(&status) {
                if let Ok(parsed) = serde_json::from_slice::<serde_json::Value>(resp_body.as_ref()) {
                    let summary = parsed
                        .get("results")
                        .and_then(|r| r.as_array())
                        .map(|arr| {
                            arr.iter()
                                .take(5)
                                .filter_map(|x| x.get("content").and_then(|c| c.as_str()))
                                .collect::<Vec<_>>()
                                .join(" ")
                        })
                        .unwrap_or_else(|| String::from("no results"));
                    log::info!("[{}] Tavily query len={} result len={}", TAG, query.len(), summary.len());
                    return Ok(summary);
                }
            }
            log::warn!("[{}] Tavily status={} or parse failed, fallback", TAG, status);
            return self.fallback_brave_or_error(query, ctx);
        }

        // 2) 未配置 Tavily，仅 Brave
        if !self.api_key.is_empty() {
            return self.do_brave(query, ctx);
        }

        Ok("web_search: no search key configured".to_string())
    }
}

impl WebSearchTool {
    fn fallback_brave_or_error(
        &self,
        query: &str,
        ctx: &mut dyn ToolContext,
    ) -> Result<String> {
        if self.api_key.is_empty() {
            return Ok("web_search: Tavily request failed and no Brave key configured".to_string());
        }
        self.do_brave(query, ctx)
    }

    fn do_brave(&self, query: &str, ctx: &mut dyn ToolContext) -> Result<String> {
        let url = format!("{}?q={}", BRAVE_SEARCH_URL, percent_encode_query(query));
        let headers = [("X-Subscription-Token", self.api_key.as_str())];
        let (status, body) = ctx.get_with_headers(&url, &headers).map_err(|e| match e {
            Error::Http { status_code, .. } => Error::Http {
                status_code,
                stage: "tool_web_search",
            },
            _ => Error::Other {
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("{:?}", e),
                )),
                stage: "tool_web_search",
            },
        })?;
        if status >= 400 {
            return Err(Error::Http {
                status_code: status,
                stage: "tool_web_search",
            });
        }
        let parsed: serde_json::Value = serde_json::from_slice(body.as_ref()).map_err(|e| Error::Other {
            source: Box::new(e),
            stage: "tool_web_search",
        })?;
        let summary = parsed
            .get("web")
            .and_then(|w| w.get("results"))
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .take(3)
                    .filter_map(|x| x.get("description").and_then(|d| d.as_str()))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_else(|| String::from("no results"));
        log::info!("[{}] Brave query len={} result len={}", TAG, query.len(), summary.len());
        Ok(summary)
    }
}
