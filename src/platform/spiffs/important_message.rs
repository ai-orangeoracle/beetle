//! SPIFFS 实现的重要消息偏移存储。单文件 memory/important_message.json，单 chat 单 offset。
//! ImportantMessageStore implementation; single file, one chat's offset at a time.

use crate::error::{Error, Result};
use crate::memory::{ImportantMessageStore, REL_PATH_IMPORTANT_MESSAGE};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::{read_file, write_file, SPIFFS_BASE};

fn full_path() -> PathBuf {
    let mut p = PathBuf::from(SPIFFS_BASE);
    p.push(REL_PATH_IMPORTANT_MESSAGE);
    p
}

/// 无状态；单文件。
pub struct SpiffsImportantMessageStore;

impl SpiffsImportantMessageStore {
    pub fn new() -> Self {
        SpiffsImportantMessageStore
    }
}

impl Default for SpiffsImportantMessageStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ImportantMessageStore for SpiffsImportantMessageStore {
    fn set_important_offset_from_end(&self, chat_id: &str, offset_from_end: u32) -> Result<()> {
        let state = ImportantMessageState {
            chat_id: chat_id.to_string(),
            offset_from_end,
        };
        let json = serde_json::to_vec(&state)
            .map_err(|e| Error::config("important_message_set", e.to_string()))?;
        write_file(full_path(), &json)
    }

    fn get_important_offset(&self, chat_id: &str) -> Result<Option<u32>> {
        let path = full_path();
        let buf = match read_file(&path) {
            Ok(b) => b,
            Err(_) => return Ok(None),
        };
        if buf.len() <= 2 {
            return Ok(None);
        }
        let state: ImportantMessageState = match serde_json::from_slice(&buf) {
            Ok(s) => s,
            Err(_) => return Ok(None),
        };
        if state.chat_id == chat_id {
            Ok(Some(state.offset_from_end))
        } else {
            Ok(None)
        }
    }

    fn clear_important(&self, chat_id: &str) -> Result<()> {
        let path = full_path();
        let buf = match read_file(&path) {
            Ok(b) => b,
            Err(_) => return Ok(()),
        };
        if buf.len() <= 2 {
            return Ok(());
        }
        let state: ImportantMessageState = match serde_json::from_slice(&buf) {
            Ok(s) => s,
            Err(_) => return Ok(()),
        };
        if state.chat_id == chat_id {
            write_file(path, b"{}")
        } else {
            Ok(())
        }
    }
}

#[derive(Serialize, Deserialize)]
struct ImportantMessageState {
    chat_id: String,
    offset_from_end: u32,
}
