//! 进程内共享状态：最近错误等，供 CLI 与 HTTP /api/health 共用。
//! In-process shared state (e.g. last error) for CLI and HTTP.

use crate::error::Error;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

const LAST_ERRORS_COUNT_CAP: usize = 10;

static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);
static LAST_ERRORS_COUNT: AtomicUsize = AtomicUsize::new(0);
/// 最近一次 memory 加载是否成功（由 build_context 等设置，供 diagnose/health 暴露）。
static MEMORY_LOAD_OK: AtomicBool = AtomicBool::new(false);
/// 最近一次 soul 加载是否成功。
static SOUL_LOAD_OK: AtomicBool = AtomicBool::new(false);

/// 将 Error 转为可安全打印的摘要；直接使用 Error 的 Display 实现。
pub fn sanitize_error_for_log(e: &Error) -> String {
    e.to_string()
}

/// 设置最近错误摘要（供 health 显示；禁止写入密钥）。同时将最近错误条数 +1，暴露时 cap 为 10。
pub fn set_last_error(e: &Error) {
    let msg = sanitize_error_for_log(e);
    if let Ok(mut g) = LAST_ERROR.lock() {
        *g = Some(msg);
    }
    let _ = LAST_ERRORS_COUNT.fetch_add(1, Ordering::Relaxed);
}

/// 返回自启动以来 set_last_error 被调用的次数，上限为 10（轻量可观测，不存完整内容）。
pub fn get_last_errors_count() -> usize {
    LAST_ERRORS_COUNT
        .load(Ordering::Relaxed)
        .min(LAST_ERRORS_COUNT_CAP)
}

/// 读取最近错误摘要。
pub fn get_last_error() -> Option<String> {
    LAST_ERROR.lock().ok().and_then(|g| g.clone())
}

/// 设置最近一次 memory 加载结果（build_context 等调用，供可观测性）。
pub fn set_memory_load_ok(ok: bool) {
    MEMORY_LOAD_OK.store(ok, Ordering::Relaxed);
}

/// 设置最近一次 soul 加载结果。
pub fn set_soul_load_ok(ok: bool) {
    SOUL_LOAD_OK.store(ok, Ordering::Relaxed);
}

/// 最近一次 memory 加载是否成功。
pub fn get_memory_load_ok() -> bool {
    MEMORY_LOAD_OK.load(Ordering::Relaxed)
}

/// 最近一次 soul 加载是否成功。
pub fn get_soul_load_ok() -> bool {
    SOUL_LOAD_OK.load(Ordering::Relaxed)
}
