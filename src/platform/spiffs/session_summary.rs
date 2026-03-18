//! SPIFFS 实现的会话摘要存储。单文件 memory/session_summaries.json，chat 数上界 32。
//! SessionSummaryStore implementation; single JSON file, chat_id -> { summary, last_summary_at_count }.

use crate::constants::SESSION_SUMMARY_MAX_LEN;
use crate::error::{Error, Result};
use crate::memory::{SessionSummaryStore, REL_PATH_SESSION_SUMMARIES};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use super::{read_file, write_file, SPIFFS_BASE};

const MAX_SESSION_SUMMARY_CHATS: usize = 32;

#[derive(Clone, Serialize, Deserialize)]
struct SummaryEntry {
    summary: String,
    last_summary_at_count: usize,
}

fn full_path() -> PathBuf {
    let mut p = PathBuf::from(SPIFFS_BASE);
    p.push(REL_PATH_SESSION_SUMMARIES);
    p
}

fn truncate_summary(s: &str) -> String {
    if s.chars().count() <= SESSION_SUMMARY_MAX_LEN {
        s.to_string()
    } else {
        s.chars().take(SESSION_SUMMARY_MAX_LEN).collect::<String>()
    }
}

pub struct SpiffsSessionSummaryStore;

impl SpiffsSessionSummaryStore {
    pub fn new() -> Self {
        SpiffsSessionSummaryStore
    }
}

impl Default for SpiffsSessionSummaryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionSummaryStore for SpiffsSessionSummaryStore {
    fn get(&self, chat_id: &str) -> Result<Option<String>> {
        let path = full_path();
        let buf = match read_file(&path) {
            Ok(b) => b,
            Err(_) => return Ok(None),
        };
        if buf.len() <= 2 {
            return Ok(None);
        }
        let map: HashMap<String, SummaryEntry> = match serde_json::from_slice(&buf) {
            Ok(m) => m,
            Err(_) => return Ok(None),
        };
        Ok(map.get(chat_id).map(|e| e.summary.clone()))
    }

    fn set(&self, chat_id: &str, summary: &str) -> Result<()> {
        self.set_with_count(chat_id, summary, 0)
    }

    fn set_with_count(&self, chat_id: &str, summary: &str, message_count: usize) -> Result<()> {
        let path = full_path();
        let mut map: HashMap<String, SummaryEntry> = match read_file(&path) {
            Ok(buf) => {
                if buf.len() <= 2 {
                    HashMap::new()
                } else {
                    serde_json::from_slice(&buf).unwrap_or_default()
                }
            }
            Err(_) => HashMap::new(),
        };
        if !map.contains_key(chat_id) && map.len() >= MAX_SESSION_SUMMARY_CHATS {
            let key_to_remove = map.keys().next().cloned();
            if let Some(k) = key_to_remove {
                map.remove(&k);
            }
        }
        map.insert(
            chat_id.to_string(),
            SummaryEntry {
                summary: truncate_summary(summary),
                last_summary_at_count: message_count,
            },
        );
        let json = serde_json::to_vec(&map)
            .map_err(|e| Error::config("session_summary_set", e.to_string()))?;
        write_file(path, &json)
    }

    fn get_with_count(&self, chat_id: &str) -> Result<Option<(String, usize)>> {
        let path = full_path();
        let buf = match read_file(&path) {
            Ok(b) => b,
            Err(_) => return Ok(None),
        };
        if buf.len() <= 2 {
            return Ok(None);
        }
        let map: HashMap<String, SummaryEntry> = match serde_json::from_slice(&buf) {
            Ok(m) => m,
            Err(_) => return Ok(None),
        };
        Ok(map.get(chat_id).map(|e| (e.summary.clone(), e.last_summary_at_count)))
    }
}
