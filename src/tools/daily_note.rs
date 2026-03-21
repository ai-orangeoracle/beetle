//! daily_note 工具：独立的每日笔记读写入口，使 LLM 更容易识别高频日记操作。
//! daily_note tool: standalone daily note operations for easier LLM recognition.

use crate::constants::DAILY_NOTE_MAX_LIST;
use crate::error::{Error, Result};
use crate::memory::MemoryStore;
use crate::tools::{parse_tool_args, Tool, ToolContext};
use serde_json::json;
use std::sync::Arc;

pub struct DailyNoteTool {
    store: Arc<dyn MemoryStore + Send + Sync>,
}

impl DailyNoteTool {
    pub fn new(store: Arc<dyn MemoryStore + Send + Sync>) -> Self {
        Self { store }
    }
}

impl Tool for DailyNoteTool {
    fn name(&self) -> &str {
        "daily_note"
    }
    fn description(&self) -> &str {
        "Read, write or list daily notes. Op: write (with optional append=true), read, list. Name format: YYYY-MM-DD.md"
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "op": { "type": "string", "description": "Operation: write|read|list" },
                "name": { "type": "string", "description": "Note name, e.g. 2025-03-10.md (required for read/write)" },
                "content": { "type": "string", "description": "Content to write (required for write)" },
                "append": { "type": "boolean", "description": "If true, append to existing note instead of overwrite (default false)" },
                "recent_n": { "type": "integer", "description": "Max notes to list (default 10, max 30)" }
            },
            "required": ["op"]
        })
    }
    fn execute(&self, args: &str, _ctx: &mut dyn ToolContext) -> Result<String> {
        let obj = parse_tool_args(args, "tool_daily_note")?;
        let op = obj
            .get("op")
            .and_then(|x| x.as_str())
            .ok_or_else(|| Error::config("tool_daily_note", "missing op"))?;

        match op {
            "list" => {
                let recent_n = obj
                    .get("recent_n")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(10) as usize;
                let recent_n = recent_n.min(DAILY_NOTE_MAX_LIST);
                let names = self.store.list_daily_note_names(recent_n)?;
                Ok(json!({"op": "list", "names": names}).to_string())
            }
            "read" => {
                let name = obj
                    .get("name")
                    .and_then(|x| x.as_str())
                    .ok_or_else(|| Error::config("tool_daily_note", "missing name"))?;
                validate_note_name(name)?;
                let content = self.store.get_daily_note(name)?;
                Ok(json!({"op": "read", "name": name, "content": content}).to_string())
            }
            "write" => {
                let name = obj
                    .get("name")
                    .and_then(|x| x.as_str())
                    .ok_or_else(|| Error::config("tool_daily_note", "missing name"))?;
                validate_note_name(name)?;
                let content = obj
                    .get("content")
                    .and_then(|x| x.as_str())
                    .ok_or_else(|| Error::config("tool_daily_note", "missing content"))?;
                let append = obj
                    .get("append")
                    .and_then(|x| x.as_bool())
                    .unwrap_or(false);

                let final_content = if append {
                    let existing = self.store.get_daily_note(name).unwrap_or_default();
                    if existing.is_empty() {
                        content.to_string()
                    } else {
                        format!("{}\n{}", existing, content)
                    }
                } else {
                    content.to_string()
                };
                self.store.write_daily_note(name, &final_content)?;
                Ok(json!({"op": "write", "name": name, "ok": true, "append": append}).to_string())
            }
            _ => Err(Error::config(
                "tool_daily_note",
                format!("unknown op: {}", op),
            )),
        }
    }
}

fn validate_note_name(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > 64 {
        return Err(Error::config("tool_daily_note", "invalid name length"));
    }
    if !name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.')
    {
        return Err(Error::config(
            "tool_daily_note",
            "name must match [a-zA-Z0-9_\\-.]",
        ));
    }
    Ok(())
}
