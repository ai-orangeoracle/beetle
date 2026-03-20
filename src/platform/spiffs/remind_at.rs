//! SPIFFS 实现的到点提醒存储。单文件 memory/remind_at.json，按 at 排序，条数/context 上界见 constants。

use crate::constants::{REMIND_AT_MAX_CONTEXT_LEN, REMIND_AT_MAX_ENTRIES};
use crate::error::{Error, Result};
use crate::memory::RemindAtStore;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::{read_file, write_file, SPIFFS_BASE};

const REL_PATH_REMIND_AT: &str = "memory/remind_at.json";

#[derive(Clone, Serialize, Deserialize)]
struct RemindEntry {
    channel: String,
    chat_id: String,
    at_unix_secs: u64,
    context: String,
}

fn full_path() -> PathBuf {
    let mut p = PathBuf::from(SPIFFS_BASE);
    p.push(REL_PATH_REMIND_AT);
    p
}

fn truncate_context(s: &str) -> String {
    if s.len() <= REMIND_AT_MAX_CONTEXT_LEN {
        s.to_string()
    } else {
        s.chars()
            .take(REMIND_AT_MAX_CONTEXT_LEN)
            .collect::<String>()
    }
}

/// 单文件，JSON 数组；add 时按 at 排序并保留最多 REMIND_AT_MAX_ENTRIES 条。
pub struct SpiffsRemindAtStore;

impl SpiffsRemindAtStore {
    pub fn new() -> Self {
        SpiffsRemindAtStore
    }
}

impl Default for SpiffsRemindAtStore {
    fn default() -> Self {
        Self::new()
    }
}

impl RemindAtStore for SpiffsRemindAtStore {
    fn add(&self, channel: &str, chat_id: &str, at_unix_secs: u64, context: &str) -> Result<()> {
        let path = full_path();
        let mut list: Vec<RemindEntry> = match read_file(&path) {
            Ok(buf) => {
                if buf.len() <= 2 {
                    vec![]
                } else {
                    serde_json::from_slice(&buf).unwrap_or_default()
                }
            }
            Err(_) => vec![],
        };
        list.push(RemindEntry {
            channel: channel.to_string(),
            chat_id: chat_id.to_string(),
            at_unix_secs,
            context: truncate_context(context),
        });
        list.sort_by_key(|e| e.at_unix_secs);
        if list.len() > REMIND_AT_MAX_ENTRIES {
            list.truncate(REMIND_AT_MAX_ENTRIES);
        }
        let json =
            serde_json::to_vec(&list).map_err(|e| Error::config("remind_at_add", e.to_string()))?;
        write_file(path, &json)
    }

    fn pop_due(&self, now_unix_secs: u64) -> Result<Option<(String, String, String)>> {
        let path = full_path();
        let buf = match read_file(&path) {
            Ok(b) => b,
            Err(_) => return Ok(None),
        };
        if buf.len() <= 2 {
            return Ok(None);
        }
        let mut list: Vec<RemindEntry> = match serde_json::from_slice(&buf) {
            Ok(l) => l,
            Err(_) => return Ok(None),
        };
        let pos = list.iter().position(|e| e.at_unix_secs <= now_unix_secs);
        let Some(idx) = pos else {
            return Ok(None);
        };
        let removed = list.remove(idx);
        let out = (removed.channel, removed.chat_id, removed.context);
        let json =
            serde_json::to_vec(&list).map_err(|e| Error::config("remind_at_pop", e.to_string()))?;
        let _ = write_file(&path, &json);
        Ok(Some(out))
    }
}
