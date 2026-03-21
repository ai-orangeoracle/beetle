//! file_write 工具：向 SPIFFS 写入文件，支持覆写与追加。
//! file_write tool: write files to SPIFFS storage with overwrite or append mode.

use crate::constants::FILE_WRITE_MAX_CONTENT_LEN;
use crate::error::{Error, Result};
use crate::tools::{parse_tool_args, Tool, ToolContext};
use serde_json::json;

/// 受保护路径黑名单：禁止通过工具写入的关键配置文件。
const PROTECTED_PATHS: &[&str] = &[
    "config/llm.json",
    "config/channels.json",
    "config/wifi.json",
    "config/SOUL.md",
    "config/USER.md",
    "memory/MEMORY.md",
];

pub struct FileWriteTool;

impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "file_write"
    }
    fn description(&self) -> &str {
        "Write content to a file in storage (SPIFFS). Supports overwrite and append modes. Protected system files cannot be written. Max content size: 16KB."
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path under storage root, e.g. notes/todo.txt" },
                "content": { "type": "string", "description": "Content to write" },
                "append": { "type": "boolean", "description": "If true, append to existing file (default false, overwrite)" }
            },
            "required": ["path", "content"]
        })
    }
    fn execute(&self, args: &str, _ctx: &mut dyn ToolContext) -> Result<String> {
        let obj = parse_tool_args(args, "tool_file_write")?;
        let path_arg = obj
            .get("path")
            .and_then(|x| x.as_str())
            .ok_or_else(|| Error::config("tool_file_write", "missing path"))?;
        let content = obj
            .get("content")
            .and_then(|x| x.as_str())
            .ok_or_else(|| Error::config("tool_file_write", "missing content"))?;
        let append = obj
            .get("append")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);

        if content.len() > FILE_WRITE_MAX_CONTENT_LEN {
            return Err(Error::config(
                "tool_file_write",
                format!("content exceeds {} bytes", FILE_WRITE_MAX_CONTENT_LEN),
            ));
        }

        // Check protected paths
        let normalized = path_arg.trim().trim_start_matches('/');
        for &protected in PROTECTED_PATHS {
            if normalized == protected {
                return Err(Error::config(
                    "tool_file_write",
                    format!("path '{}' is protected and cannot be written via this tool", protected),
                ));
            }
        }

        let full = super::files::resolve_path(path_arg)?;

        let final_content = if append {
            let existing = std::fs::read_to_string(&full).unwrap_or_default();
            format!("{}{}", existing, content)
        } else {
            content.to_string()
        };

        // Ensure parent directory exists
        if let Some(parent) = full.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| Error::io("tool_file_write", e))?;
            }
        }

        std::fs::write(&full, final_content.as_bytes())
            .map_err(|e| Error::io("tool_file_write", e))?;

        Ok(json!({
            "path": path_arg,
            "ok": true,
            "append": append,
            "bytes_written": content.len()
        })
        .to_string())
    }
}
