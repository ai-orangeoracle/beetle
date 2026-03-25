//! 状态根文件系统实现（按目标平台分模块）。
//! State root filesystem implementations per target.

use crate::error::{Error, Result};
use std::path::Path;

/// 工具与平台共用的相对路径规范化：trim、去前导 `/`、禁止 `..` 与绝对路径。
pub(crate) fn normalize_state_rel_path(path_arg: &str) -> Result<String> {
    let s = path_arg.trim().trim_start_matches('/');
    if s.contains("..") {
        return Err(Error::config("state_fs", "invalid path"));
    }
    if Path::new(s).is_absolute() {
        return Err(Error::config("state_fs", "invalid path"));
    }
    Ok(s.to_string())
}

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
mod esp32;
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub(crate) use esp32::Esp32StateFs;

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
mod linux;
#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub(crate) use linux::LinuxStateFs;
