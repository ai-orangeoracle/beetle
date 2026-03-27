//! GET /api/channel_connectivity：返回已配置通道的连通性检查结果，供设备页展示。
//! 在 httpd 任务内串行执行最多 6 次外网 HTTP，栈占用较高；http_server 已提高 task stack_size，http_client 读响应用小块缓冲。

use super::HandlerContext;
use crate::channels::check_all;
use crate::config::AppConfig;
use crate::i18n::{locale_from_store, tr, Message};

/// 成功返回 `{ "channels": [ ... ] }` 字符串，失败返回 Err（mod 层写 500，不暴露内部细节）。
/// httpd 线程非 agent 线程，需先注册 TWDT 才能安全 feed（幂等）。
pub fn body(ctx: &HandlerContext) -> Result<String, String> {
    crate::platform::task_wdt::register_current_task_to_task_wdt();
    let loc = locale_from_store(ctx.config_store.as_ref());
    let config = AppConfig::load(
        ctx.config_store.as_ref(),
        Some(ctx.config_file_store.as_ref()),
    );
    let mut client = match ctx.platform.create_http_client(&config) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("[channel_connectivity] create_http_client failed: {}", e);
            return Err(tr(Message::ChannelConnectivityUnavailable, loc));
        }
    };
    let list = check_all(&config, &mut *client, loc);
    let out = serde_json::json!({ "channels": list });
    serde_json::to_string(&out).map_err(|e| e.to_string())
}
