//! SPIFFS 实现的 SessionStore。会话文件为 {sessions_dir}/{short_id}.jsonl，有界 ring。
//! 短 id：chat_id 若不超过 MAX_CHAT_ID_FILENAME_LEN 则直接用，否则用 8 字符哈希，文件首行存 "# chat_id: <真实 id>"。

use crate::error::{Error, Result};
use crate::memory::{
    SessionMessage, SessionStore, MAX_SESSION_ENTRIES, MAX_SESSION_MESSAGE_LEN,
    REL_PATH_SESSIONS_DIR,
};
use serde_json;
use std::path::PathBuf;

use super::{list_dir, read_file, write_file, SPIFFS_BASE};

const TAG: &str = "platform::spiffs::session";

/// 文件名中 chat_id 部分最大长度（不含 .jsonl），超出则用 hash 短名，满足 ESP-IDF 路径/文件名限制。
const MAX_CHAT_ID_FILENAME_LEN: usize = 20;
const SESSION_FILE_EXT: &str = ".jsonl";
const CHAT_ID_HEADER_PREFIX: &str = "# chat_id: ";

fn fnv1a_hash(s: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for b in s.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    h
}

/// 返回 (路径, 是否在文件中写入 chat_id 首行)。长 chat_id 用 8 位 hex 哈希作文件名，首行存真实 id。
fn session_path(chat_id: &str) -> Result<(PathBuf, bool)> {
    if chat_id.is_empty() {
        return Err(Error::config("session_path", "chat_id empty"));
    }
    if !chat_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err(Error::config("session_path", "chat_id contains invalid chars"));
    }
    let mut p = PathBuf::from(SPIFFS_BASE);
    p.push(REL_PATH_SESSIONS_DIR);
    let (filename, write_header) = if chat_id.len() <= MAX_CHAT_ID_FILENAME_LEN {
        (format!("{}{}", chat_id, SESSION_FILE_EXT), false)
    } else {
        let h = fnv1a_hash(chat_id);
        (format!("{:08x}{}", h, SESSION_FILE_EXT), true)
    };
    p.push(&filename);
    if p.as_os_str().len() > 56 {
        return Err(Error::config(
            "session_path",
            format!("path too long ({})", p.as_os_str().len()),
        ));
    }
    Ok((p, write_header))
}

fn parse_jsonl_line(line: &str) -> Option<SessionMessage> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    match serde_json::from_str::<SessionMessage>(line) {
        Ok(m) => Some(m),
        Err(e) => {
            log::warn!("[{}] skip bad line: {}", TAG, e);
            None
        }
    }
}

/// 从文件首行解析 "# chat_id: <id>"，非该格式返回 None。
fn parse_chat_id_header(line: &str) -> Option<String> {
    let line = line.trim();
    if line.starts_with(CHAT_ID_HEADER_PREFIX) {
        let id = line[CHAT_ID_HEADER_PREFIX.len()..].trim();
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }
    None
}

/// 列举 chat_id 数量上界（与 MAX_SESSION_ENTRIES 同量级）。
const MAX_LIST_CHAT_IDS: usize = 128;

/// SessionStore 的 SPIFFS 实现；单会话最多 MAX_SESSION_ENTRIES 条，超限淘汰最旧。
pub struct SpiffsSessionStore;

impl SpiffsSessionStore {
    pub fn new() -> Self {
        SpiffsSessionStore
    }
}

impl SessionStore for SpiffsSessionStore {
    fn append(&self, chat_id: &str, role: &str, content: &str) -> Result<()> {
        let msg = SessionMessage {
            role: role.to_string(),
            content: content.to_string(),
        };
        let line = serde_json::to_string(&msg).map_err(|e| Error::config("session_append", e.to_string()))?;
        if line.len() > MAX_SESSION_MESSAGE_LEN {
            return Err(Error::config(
                "session_append",
                format!("message serialized len {} exceeds {}", line.len(), MAX_SESSION_MESSAGE_LEN),
            ));
        }

        let (path, write_header) = session_path(chat_id)?;
        let mut messages: Vec<SessionMessage> = Vec::with_capacity(MAX_SESSION_ENTRIES);
        if let Ok(buf) = read_file(&path) {
            let mut first = true;
            for line in buf.split(|&b| b == b'\n') {
                if line.is_empty() {
                    continue;
                }
                if let Ok(s) = std::str::from_utf8(line) {
                    if first && parse_chat_id_header(s).is_some() {
                        first = false;
                        continue;
                    }
                    first = false;
                    if let Some(m) = parse_jsonl_line(s) {
                        messages.push(m);
                    }
                }
            }
        }

        messages.push(msg);
        if messages.len() > MAX_SESSION_ENTRIES {
            messages.drain(0..(messages.len() - MAX_SESSION_ENTRIES));
        }

        let cap = messages
            .len()
            .saturating_mul(MAX_SESSION_MESSAGE_LEN.saturating_add(1))
            .saturating_add(if write_header {
                CHAT_ID_HEADER_PREFIX.len() + chat_id.len() + 2
            } else {
                0
            });
        let mut body = String::with_capacity(cap);
        if write_header {
            body.push_str(CHAT_ID_HEADER_PREFIX);
            body.push_str(chat_id);
            body.push('\n');
        }
        for (i, m) in messages.iter().enumerate() {
            if i > 0 {
                body.push('\n');
            }
            let line = serde_json::to_string(m).unwrap_or_default();
            body.push_str(&line);
        }
        write_file(&path, body.as_bytes())
    }

    fn load_recent(&self, chat_id: &str, n: usize) -> Result<Vec<SessionMessage>> {
        let (path, _) = session_path(chat_id)?;
        let cap = n.min(MAX_SESSION_ENTRIES);
        let mut recent: Vec<SessionMessage> = Vec::with_capacity(cap);
        if let Ok(buf) = read_file(&path) {
            for line in buf.split(|&b| b == b'\n') {
                if line.is_empty() {
                    continue;
                }
                if let Ok(s) = std::str::from_utf8(line) {
                    if parse_chat_id_header(s).is_some() {
                        continue;
                    }
                    if let Some(m) = parse_jsonl_line(s) {
                        recent.push(m);
                        if recent.len() > cap {
                            recent.remove(0);
                        }
                    }
                }
            }
        }
        Ok(recent)
    }

    fn clear(&self, chat_id: &str) -> Result<()> {
        let (path, write_header) = session_path(chat_id)?;
        if write_header {
            let mut empty = String::from(CHAT_ID_HEADER_PREFIX);
            empty.push_str(chat_id);
            empty.push('\n');
            write_file(&path, empty.as_bytes())
        } else {
            write_file(&path, b"")
        }
    }

    fn list_chat_ids(&self) -> Result<Vec<String>> {
        let mut p = PathBuf::from(SPIFFS_BASE);
        p.push(REL_PATH_SESSIONS_DIR);
        let names = match list_dir(&p) {
            Ok(n) => n,
            Err(e) => {
                log::warn!("[{}] list_dir {:?} failed: {}", TAG, p, e);
                return Ok(Vec::new());
            }
        };
        let mut out: Vec<String> = Vec::with_capacity(MAX_LIST_CHAT_IDS.min(names.len()));
        for name in names {
            if !name.ends_with(SESSION_FILE_EXT) {
                continue;
            }
            let stem = name.trim_end_matches(SESSION_FILE_EXT);
            // 短 chat_id 直接作文件名，无首行 header，无需读文件；仅 8 位 hex 哈希名需读首行取真实 chat_id
            let chat_id = if stem.len() == 8 && stem.chars().all(|c| c.is_ascii_hexdigit()) {
                p.push(&name);
                let id = if let Ok(buf) = read_file(&p) {
                    let first_line = buf
                        .split(|&b| b == b'\n')
                        .next()
                        .and_then(|line| std::str::from_utf8(line).ok());
                    first_line
                        .and_then(parse_chat_id_header)
                        .unwrap_or_else(|| stem.to_string())
                } else {
                    stem.to_string()
                };
                p.pop();
                id
            } else {
                stem.to_string()
            };
            if !chat_id.is_empty() {
                out.push(chat_id);
                if out.len() >= MAX_LIST_CHAT_IDS {
                    break;
                }
            }
        }
        Ok(out)
    }
}
