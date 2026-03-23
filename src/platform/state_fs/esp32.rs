//! ESP32：状态文件委托 `platform::spiffs`（含 SPIFFS 互斥）。
//! ESP32: state files delegate to `platform::spiffs` (including SPIFFS mutex).

use crate::error::{Error, Result};
use crate::platform::abstraction::StateFs;
use crate::platform::spiffs::{self, SPIFFS_BASE, MAX_WRITE_SIZE};
use std::path::{Path, PathBuf};

/// 零大小类型；SPIFFS 串行化在 `spiffs::*` 内完成。
#[derive(Debug, Default)]
pub struct Esp32StateFs;

fn validate_rel(rel_path: &str) -> Result<()> {
    if rel_path.contains("..") {
        return Err(Error::config("state_fs", "invalid path"));
    }
    if Path::new(rel_path).is_absolute() || rel_path.starts_with('/') {
        return Err(Error::config("state_fs", "invalid path"));
    }
    Ok(())
}

fn abs_path(rel_path: &str) -> Result<PathBuf> {
    validate_rel(rel_path)?;
    Ok(PathBuf::from(SPIFFS_BASE).join(rel_path))
}

fn map_read_result(r: Result<Vec<u8>, Error>) -> Result<Option<Vec<u8>>> {
    match r {
        Ok(b) => Ok(Some(b)),
        Err(e) => match &e {
            Error::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
            _ => Err(e),
        },
    }
}

impl StateFs for Esp32StateFs {
    fn read(&self, rel_path: &str) -> Result<Option<Vec<u8>>> {
        let path = abs_path(rel_path)?;
        map_read_result(spiffs::read_file(&path))
    }

    fn write(&self, rel_path: &str, data: &[u8]) -> Result<()> {
        if data.len() > MAX_WRITE_SIZE {
            return Err(Error::config(
                "state_fs",
                format!(
                    "write size {} exceeds limit {}",
                    data.len(),
                    MAX_WRITE_SIZE
                ),
            ));
        }
        let path = abs_path(rel_path)?;
        spiffs::write_file(&path, data)
    }

    fn remove(&self, rel_path: &str) -> Result<()> {
        let path = abs_path(rel_path)?;
        match spiffs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) => match &e {
                Error::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
                _ => Err(e),
            },
        }
    }

    fn list_dir(&self, rel_path: &str) -> Result<Vec<String>> {
        let path = abs_path(rel_path)?;
        spiffs::list_dir(&path)
    }
}
