//! GET /api/health：仅生成响应体 JSON，配对与写响应在 mod.rs。含 metrics 快照供基线对比。

use super::HandlerContext;
use crate::metrics;
use crate::state;
use std::sync::atomic::Ordering;

/// 生成 health JSON body（含 metrics 快照，无敏感信息）。
pub fn body(ctx: &HandlerContext) -> Result<String, std::io::Error> {
    let wifi = if ctx.wifi_connected {
        "connected"
    } else {
        "disconnected"
    };
    let last_err = state::get_last_error().unwrap_or_else(|| "none".to_string());
    let m = metrics::snapshot();
    let s = format!(
        "{{\"wifi\":\"{}\",\"inbound_depth\":{},\"outbound_depth\":{},\"last_error\":\"{}\",\"metrics\":{{\"msg_in\":{},\"msg_out\":{},\"llm_calls\":{},\"llm_errors\":{},\"llm_last_ms\":{},\"tool_calls\":{},\"tool_errors\":{},\"wdt_feeds\":{},\"dispatch_ok\":{},\"dispatch_fail\":{},\"err_router\":{},\"err_chat\":{},\"err_ctx\":{},\"err_tool\":{},\"err_llm_req\":{},\"err_llm_parse\":{},\"err_dispatch\":{},\"err_session\":{},\"err_other\":{}}}}}",
        wifi,
        ctx.inbound_depth.load(Ordering::Relaxed),
        ctx.outbound_depth.load(Ordering::Relaxed),
        last_err.replace('"', "\\\""),
        m.messages_in,
        m.messages_out,
        m.llm_calls,
        m.llm_errors,
        m.llm_last_ms,
        m.tool_calls,
        m.tool_errors,
        m.wdt_feeds,
        m.dispatch_send_ok,
        m.dispatch_send_fail,
        m.errors_agent_router,
        m.errors_agent_chat,
        m.errors_agent_context,
        m.errors_tool_execute,
        m.errors_llm_request,
        m.errors_llm_parse,
        m.errors_channel_dispatch,
        m.errors_session_append,
        m.errors_other
    );
    Ok(s)
}
