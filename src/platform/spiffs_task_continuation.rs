//! SPIFFS 实现的多轮延续存储。单文件 memory/task_continuation.json，单设备单任务。
//! TaskContinuationStore implementation; single file for one task state.

use crate::constants::TASK_CONTINUATION_MAX_OUTPUT_LEN;
use crate::error::{Error, Result};
use crate::memory::{TaskContinuationStore, REL_PATH_TASK_CONTINUATION};
use crate::platform::spiffs::{read_file, write_file};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
struct TaskContinuationState {
    chat_id: String,
    round: u32,
    last_output: String,
}

fn full_path() -> PathBuf {
    let mut p = PathBuf::from(crate::platform::spiffs::SPIFFS_BASE);
    p.push(REL_PATH_TASK_CONTINUATION);
    p
}

fn truncate_output_to_max(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut len = 0usize;
    let mut out = String::new();
    for c in s.chars() {
        let n = c.len_utf8();
        if len + n > max_bytes {
            break;
        }
        len += n;
        out.push(c);
    }
    out
}

/// 无状态；单文件，单任务。
pub struct SpiffsTaskContinuationStore;

impl SpiffsTaskContinuationStore {
    pub fn new() -> Self {
        SpiffsTaskContinuationStore
    }
}

impl Default for SpiffsTaskContinuationStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskContinuationStore for SpiffsTaskContinuationStore {
    fn get_task_continuation(&self, chat_id: &str) -> Result<Option<(u32, String)>> {
        let path = full_path();
        let buf = match read_file(&path) {
            Ok(b) => b,
            Err(_) => return Ok(None),
        };
        if buf.len() <= 2 {
            return Ok(None);
        }
        let state: TaskContinuationState = match serde_json::from_slice(&buf) {
            Ok(s) => s,
            Err(_) => return Ok(None),
        };
        if state.chat_id == chat_id {
            Ok(Some((state.round, state.last_output)))
        } else {
            Ok(None)
        }
    }

    fn set_task_continuation(&self, chat_id: &str, round: u32, last_output: &str) -> Result<()> {
        let truncated = truncate_output_to_max(last_output, TASK_CONTINUATION_MAX_OUTPUT_LEN);
        let state = TaskContinuationState {
            chat_id: chat_id.to_string(),
            round,
            last_output: truncated,
        };
        let json = serde_json::to_vec(&state)
            .map_err(|e| Error::config("task_continuation_set", e.to_string()))?;
        write_file(full_path(), &json)
    }

    fn clear_task_continuation(&self, chat_id: &str) -> Result<()> {
        let path = full_path();
        let buf = match read_file(&path) {
            Ok(b) => b,
            Err(_) => return Ok(()),
        };
        if buf.len() <= 2 {
            return Ok(());
        }
        let state: TaskContinuationState = match serde_json::from_slice(&buf) {
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
