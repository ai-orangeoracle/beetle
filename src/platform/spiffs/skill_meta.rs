//! 技能元数据（顺序、禁用列表）存 SPIFFS config/skills_meta.json，避免 NVS 高频单键写触发 4361。

use crate::error::{Error, Result};
use crate::platform::abstraction::SkillMetaStore;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::{read_file, write_file, SPIFFS_BASE};

const REL_PATH: &str = "config/skills_meta.json";

fn full_path() -> PathBuf {
    let mut p = PathBuf::from(SPIFFS_BASE);
    p.push(REL_PATH);
    p
}

#[derive(Default, Serialize, Deserialize)]
struct Meta {
    #[serde(default)]
    order: Vec<String>,
    #[serde(default)]
    disabled: Vec<String>,
}

/// SPIFFS 实现的 SkillMetaStore；单文件 config/skills_meta.json。
pub struct SpiffsSkillMetaStore;

impl SkillMetaStore for SpiffsSkillMetaStore {
    fn read_meta(&self) -> Result<(Vec<String>, Vec<String>)> {
        let buf = match read_file(full_path()) {
            Ok(b) => b,
            Err(_) => return Ok((Vec::new(), Vec::new())),
        };
        let s = String::from_utf8_lossy(&buf);
        let meta: Meta = serde_json::from_str(&s).unwrap_or_default();
        Ok((meta.order, meta.disabled))
    }

    fn write_meta(&self, order: &[String], disabled: &[String]) -> Result<()> {
        let meta = Meta {
            order: order.to_vec(),
            disabled: disabled.to_vec(),
        };
        let json = serde_json::to_string(&meta).map_err(|e| Error::config("skills_meta", e.to_string()))?;
        write_file(full_path(), json.as_bytes())
    }
}
