//! SPIFFS 挂载与路径约定。提供读/写/列目录最小 API，写操作有大小约束。
//! SPIFFS mount and path convention. Min API: read/write/list; write has size limit.
//! ESP-IDF VFS/SPIFFS 多线程并发会引发 fd 错用或死锁，故所有通过本模块的 SPIFFS 访问在 ESP 下由 SPIFFS_MUTEX 串行化。

use crate::error::{Error, Result};
use std::ffi::CString;
use std::io::{Read, Write};
use std::path::Path;
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
use std::sync::{Mutex, OnceLock};

/// SPIFFS 挂载点。
pub const SPIFFS_BASE: &str = "/spiffs";

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
static SPIFFS_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
fn lock_spiffs() -> std::sync::MutexGuard<'static, ()> {
    SPIFFS_MUTEX
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// 单次写入最大字节数，避免写满分区。
const MAX_WRITE_SIZE: usize = 256 * 1024;

/// 挂载 SPIFFS。partition_label=None 表示默认 "storage"；format_if_mount_failed=true。
pub fn init_spiffs() -> Result<()> {
    let base = CString::new(SPIFFS_BASE).map_err(|e| Error::config("spiffs", e.to_string()))?;
    let conf = esp_idf_svc::sys::esp_vfs_spiffs_conf_t {
        base_path: base.as_ptr(),
        partition_label: std::ptr::null(),
        max_files: 10,
        format_if_mount_failed: true,
    };
    let err = unsafe { esp_idf_svc::sys::esp_vfs_spiffs_register(&conf) };
    if err != 0 {
        return Err(Error::esp("spiffs_register", err));
    }
    let mut total: usize = 0;
    let mut used: usize = 0;
    unsafe { esp_idf_svc::sys::esp_spiffs_info(std::ptr::null(), &mut total, &mut used) };
    log::info!(
        "[platform::spiffs] mounted base={} total={} used={}",
        SPIFFS_BASE,
        total,
        used
    );
    Ok(())
}

/// 返回 SPIFFS 总字节数与已用字节数；用于启动自检或运维。失败返回 None（如未挂载）。
pub fn spiffs_usage() -> Option<(usize, usize)> {
    #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
    let _guard = lock_spiffs();
    let mut total: usize = 0;
    let mut used: usize = 0;
    let ret = unsafe { esp_idf_svc::sys::esp_spiffs_info(std::ptr::null(), &mut total, &mut used) };
    if ret == 0 {
        Some((total, used))
    } else {
        None
    }
}

/// 读整个文件到 Vec。路径相对于 SPIFFS_BASE，或绝对如 /spiffs/config/SOUL.md。
/// 有 metadata 时预分配 capacity，减少 read_to_end 的多次 realloc。
pub fn read_file(path: impl AsRef<Path>) -> Result<Vec<u8>> {
    #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
    let _guard = lock_spiffs();
    let p = path.as_ref();
    let path_str = p
        .to_str()
        .ok_or_else(|| Error::config("spiffs_read", "invalid path"))?;
    let mut f = std::fs::File::open(path_str).map_err(|e| Error::io("spiffs_read", e))?;
    let capacity = f
        .metadata()
        .ok()
        .and_then(|m| m.len().try_into().ok())
        .map(|len: usize| len.min(MAX_WRITE_SIZE))
        .unwrap_or(0);
    let mut buf = if capacity > 0 {
        Vec::with_capacity(capacity)
    } else {
        Vec::new()
    };
    f.read_to_end(&mut buf).map_err(|e| Error::io("spiffs_read", e))?;
    Ok(buf)
}

/// 写字节到文件。超过 MAX_WRITE_SIZE 返回错误。
pub fn write_file(path: impl AsRef<Path>, data: &[u8]) -> Result<()> {
    #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
    let _guard = lock_spiffs();
    if data.len() > MAX_WRITE_SIZE {
        return Err(Error::config(
            "spiffs_write",
            format!("write size {} exceeds limit {}", data.len(), MAX_WRITE_SIZE),
        ));
    }
    let p = path.as_ref();
    let path_str = p
        .to_str()
        .ok_or_else(|| Error::config("spiffs_write", "invalid path"))?;
    let mut f = std::fs::File::create(path_str).map_err(|e| Error::io("spiffs_write", e))?;
    f.write_all(data).map_err(|e| Error::io("spiffs_write", e))?;
    Ok(())
}

/// 删除文件。仅删除文件，不删目录。用于技能删除等。
pub fn remove_file(path: impl AsRef<Path>) -> Result<()> {
    #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
    let _guard = lock_spiffs();
    let p = path.as_ref();
    let path_str = p
        .to_str()
        .ok_or_else(|| Error::config("spiffs_remove", "invalid path"))?;
    std::fs::remove_file(path_str).map_err(|e| Error::io("spiffs_remove", e))?;
    Ok(())
}

/// 列目录条目（仅一层）。路径如 /spiffs/config。
pub fn list_dir(path: impl AsRef<Path>) -> Result<Vec<String>> {
    #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
    let _guard = lock_spiffs();
    let p = path.as_ref();
    let path_str = p
        .to_str()
        .ok_or_else(|| Error::config("spiffs_list", "invalid path"))?;
    let mut names = Vec::new();
    for e in std::fs::read_dir(path_str).map_err(|e| Error::io("spiffs_list", e))? {
        let e = e.map_err(|e| Error::io("spiffs_list", e))?;
        if let Some(s) = e.file_name().to_str() {
            names.push(s.to_string());
        }
    }
    Ok(names)
}