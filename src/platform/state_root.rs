//! 状态挂载根路径：ESP 固定 `/spiffs`；host/Linux 由 `BEETLE_STATE_ROOT` 或默认 `./spiffs_data`。
//! State mount root: fixed `/spiffs` on ESP; host uses env or `./spiffs_data`.

use std::path::PathBuf;
use std::sync::OnceLock;

static STATE_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// 返回状态根目录（SPIFFS 挂载点或主机开发目录）。进程内缓存，首次调用时解析环境变量。
/// Returns the state filesystem root (SPIFFS mount or host dev directory). Cached after first call.
pub fn state_mount_path() -> PathBuf {
    STATE_ROOT
        .get_or_init(|| {
            #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
            {
                PathBuf::from("/spiffs")
            }
            #[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
            {
                std::env::var("BEETLE_STATE_ROOT")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("./spiffs_data"))
            }
        })
        .clone()
}
