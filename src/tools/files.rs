//! files 工具：在 SPIFFS 根下列出或读取文件；只读，路径禁止 `..`，结果截断至 MAX_TOOL_RESULT_LEN。
//! files tool: list or read files under SPIFFS root; read-only, path must not contain '..'.

use crate::error::{Error, Result};
use crate::tools::{parse_tool_args, Tool, ToolContext, MAX_TOOL_RESULT_LEN};
use serde_json::json;
use std::path::Path;

/// 主机上用于测试的模拟 SPIFFS 根目录。
#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
const HOST_SPIFFS_BASE: &str = "./spiffs_data";

fn base_path() -> &'static str {
    #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
    {
        crate::platform::SPIFFS_BASE
    }
    #[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
    {
        HOST_SPIFFS_BASE
    }
}

/// 解析 path 与 mode，返回 (full_path, mode)。path 不得含 `..` 或绝对路径逃逸。
fn resolve_path(path_arg: &str) -> Result<std::path::PathBuf> {
    let path_arg = path_arg.trim().trim_start_matches('/');
    if path_arg.is_empty() {
        return Ok(Path::new(base_path()).to_path_buf());
    }
    if path_arg.contains("..") {
        return Err(Error::config("tool_files", "path must not contain '..'"));
    }
    let base = Path::new(base_path());
    let full = base.join(path_arg);
    // 确保拼接后仍在 base 下（防止 join 产生意外绝对路径）。
    let base_str = base.to_string_lossy();
    let full_str = full.to_string_lossy();
    if !full_str.starts_with(base_str.as_ref()) && full_str != base_str {
        return Err(Error::config("tool_files", "path escapes base"));
    }
    Ok(full)
}

const MAX_LIST_ENTRIES: usize = 256;

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
fn read_storage_file(path: &std::path::Path) -> Result<Vec<u8>> {
    crate::platform::spiffs::read_file(path)
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
fn read_storage_file(path: &std::path::Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|e| Error::io("tool_files", e))
}

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
fn list_storage_dir(path: &std::path::Path) -> Result<Vec<String>> {
    crate::platform::spiffs::list_dir(path)
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
fn list_storage_dir(path: &std::path::Path) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(path).map_err(|e| Error::io("tool_files", e))? {
        let e = e.map_err(|e| Error::io("tool_files", e))?;
        let name = e.file_name();
        if let Some(s) = name.to_str() {
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            out.push(if is_dir { format!("{}/", s) } else { s.to_string() });
            if out.len() >= MAX_LIST_ENTRIES {
                break;
            }
        }
    }
    Ok(out)
}

pub struct FilesTool;

impl Tool for FilesTool {
    fn name(&self) -> &str {
        "files"
    }
    fn description(&self) -> &str {
        "List or read files from storage (SPIFFS). Args: path (string), mode (optional: 'list' or 'read', default 'read'). Read returns content truncated to limit; list returns entry names, max 256."
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path under storage root, e.g. skills/foo.md" },
                "mode": { "type": "string", "description": "list or read (default read)" }
            },
            "required": ["path"]
        })
    }
    fn execute(&self, args: &str, _ctx: &mut dyn ToolContext) -> Result<String> {
        let obj = parse_tool_args(args, "tool_files")?;
        let path_arg = obj
            .get("path")
            .and_then(|x| x.as_str())
            .ok_or_else(|| Error::config("tool_files", "missing path"))?;
        let mode = obj
            .get("mode")
            .and_then(|x| x.as_str())
            .unwrap_or("read")
            .trim()
            .to_lowercase();

        let full = resolve_path(path_arg)?;

        if mode == "list" {
            let entries = list_storage_dir(&full)?;
            let truncated = entries.len() >= MAX_LIST_ENTRIES;
            let out = json!({
                "mode": "list",
                "path": path_arg,
                "entries": entries,
                "truncated": truncated
            });
            return Ok(serde_json::to_string(&out)
                .map_err(|e| Error::config("tool_files", e.to_string()))?);
        }

        if mode != "read" {
            return Err(Error::config("tool_files", "mode must be 'list' or 'read'"));
        }

        let meta = std::fs::metadata(&full).map_err(|e| Error::io("tool_files", e))?;
        if !meta.is_file() {
            return Err(Error::config("tool_files", "path is not a file"));
        }
        let len = meta.len();
        if len > (MAX_TOOL_RESULT_LEN as u64).saturating_mul(2) {
            return Err(Error::config("tool_files", "file too large"));
        }
        let raw = read_storage_file(&full)?;
        let content = std::str::from_utf8(&raw)
            .map_err(|_| Error::config("tool_files", "file is not valid UTF-8"))?
            .to_string();
        let (content, truncated) = if content.len() > MAX_TOOL_RESULT_LEN {
            let mut c = content
                .chars()
                .take(MAX_TOOL_RESULT_LEN)
                .collect::<String>();
            c.push_str("…");
            (c, true)
        } else {
            (content, false)
        };
        let out = json!({
            "mode": "read",
            "path": path_arg,
            "content": content,
            "truncated": truncated
        });
        Ok(serde_json::to_string(&out).map_err(|e| Error::config("tool_files", e.to_string()))?)
    }
}
