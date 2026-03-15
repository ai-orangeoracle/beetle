//! SPIFFS 实现的 SkillStorage；目录固定为 SPIFFS_BASE/skills，文件名为 {name}.md。
//! SkillStorage implementation over SPIFFS; dir = SPIFFS_BASE/skills, files = {name}.md.

use crate::error::Result;
use crate::platform::abstraction::SkillStorage;
use crate::platform::spiffs::{list_dir, read_file, remove_file, write_file, SPIFFS_BASE};
use std::path::PathBuf;

const SKILLS_SUBDIR: &str = "skills";
const MAX_SKILL_COUNT: usize = 64;

fn skills_dir() -> PathBuf {
    let mut p = PathBuf::from(SPIFFS_BASE);
    p.push(SKILLS_SUBDIR);
    p
}

/// SPIFFS 上的 skills 目录存储；list_names 返回不含 .md 的名称。
pub struct SpiffsSkillStorage;

impl SkillStorage for SpiffsSkillStorage {
    fn list_names(&self) -> Result<Vec<String>> {
        let dir = skills_dir();
        let names = list_dir(&dir)?;
        let out: Vec<String> = names
            .into_iter()
            .filter(|n| n.ends_with(".md"))
            .map(|n| n.trim_end_matches(".md").to_string())
            .take(MAX_SKILL_COUNT)
            .collect();
        Ok(out)
    }

    fn read(&self, name: &str) -> Result<Vec<u8>> {
        let mut path = skills_dir();
        path.push(format!("{}.md", name));
        read_file(&path)
    }

    fn write(&self, name: &str, content: &[u8]) -> Result<()> {
        let mut path = skills_dir();
        path.push(format!("{}.md", name));
        write_file(&path, content)
    }

    fn remove(&self, name: &str) -> Result<()> {
        let mut path = skills_dir();
        path.push(format!("{}.md", name));
        remove_file(&path)
    }
}

/// 默认 skill 存储的 Arc，供跨线程使用。步骤 4 过渡；步骤 5 后 main 改用 platform.skill_storage()。
pub fn default_skill_storage_arc() -> std::sync::Arc<dyn SkillStorage + Send + Sync> {
    std::sync::Arc::new(SpiffsSkillStorage)
}
