//! 运行指标与错误画像：消息吞吐、队列深度、LLM/tool 耗时与错误按 stage 聚合，供基线对比与 health 暴露。
//! Metrics and error profile: throughput, queue depth, LLM/tool timing, errors by stage.

// 32 位目标（xtensa/riscv32）无 AtomicU64，统一用 AtomicU32；快照仍以 u64 暴露。
use std::sync::atomic::{AtomicU32, Ordering};

/// 已知 stage 的错误计数（与 Error::stage() 对齐）；其他 stage 归入 other。
const STAGE_AGENT_ROUTER: &str = "agent_router";
const STAGE_AGENT_CHAT: &str = "agent_chat";
const STAGE_AGENT_CONTEXT: &str = "agent_context";
const STAGE_TOOL_EXECUTE: &str = "tool_execute";
const STAGE_LLM_REQUEST: &str = "llm_request";
const STAGE_LLM_PARSE: &str = "llm_parse";
const STAGE_CHANNEL_DISPATCH: &str = "channel_dispatch";
const STAGE_SESSION_APPEND: &str = "session_append";

static MESSAGES_IN: AtomicU32 = AtomicU32::new(0);
static MESSAGES_OUT: AtomicU32 = AtomicU32::new(0);
static LLM_CALLS: AtomicU32 = AtomicU32::new(0);
static LLM_ERRORS: AtomicU32 = AtomicU32::new(0);
static LLM_LAST_MS: AtomicU32 = AtomicU32::new(0);
static TOOL_CALLS: AtomicU32 = AtomicU32::new(0);
static TOOL_ERRORS: AtomicU32 = AtomicU32::new(0);
static WDT_FEEDS: AtomicU32 = AtomicU32::new(0);
static DISPATCH_SEND_OK: AtomicU32 = AtomicU32::new(0);
static DISPATCH_SEND_FAIL: AtomicU32 = AtomicU32::new(0);
static LAST_ACTIVE_EPOCH_SECS: AtomicU32 = AtomicU32::new(0);

static ERRORS_AGENT_ROUTER: AtomicU32 = AtomicU32::new(0);
static ERRORS_AGENT_CHAT: AtomicU32 = AtomicU32::new(0);
static ERRORS_AGENT_CONTEXT: AtomicU32 = AtomicU32::new(0);
static ERRORS_TOOL_EXECUTE: AtomicU32 = AtomicU32::new(0);
static ERRORS_LLM_REQUEST: AtomicU32 = AtomicU32::new(0);
static ERRORS_LLM_PARSE: AtomicU32 = AtomicU32::new(0);
static ERRORS_CHANNEL_DISPATCH: AtomicU32 = AtomicU32::new(0);
static ERRORS_SESSION_APPEND: AtomicU32 = AtomicU32::new(0);
static ERRORS_OTHER: AtomicU32 = AtomicU32::new(0);

#[inline]
pub fn record_message_in() {
    MESSAGES_IN.fetch_add(1, Ordering::Relaxed);
}

#[inline]
pub fn record_message_out() {
    MESSAGES_OUT.fetch_add(1, Ordering::Relaxed);
    // Update last-active timestamp (epoch seconds).
    let epoch_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .min(u32::MAX as u64) as u32;
    LAST_ACTIVE_EPOCH_SECS.store(epoch_secs, Ordering::Relaxed);
}

#[inline]
pub fn record_llm_call_start() -> std::time::Instant {
    LLM_CALLS.fetch_add(1, Ordering::Relaxed);
    std::time::Instant::now()
}

#[inline]
pub fn record_llm_call_end(start: std::time::Instant) {
    let ms = start.elapsed().as_millis().min(u32::MAX as u128) as u32;
    LLM_LAST_MS.store(ms, Ordering::Relaxed);
}

#[inline]
pub fn record_llm_error() {
    LLM_ERRORS.fetch_add(1, Ordering::Relaxed);
}

#[inline]
pub fn record_tool_call(ok: bool) {
    TOOL_CALLS.fetch_add(1, Ordering::Relaxed);
    if !ok {
        TOOL_ERRORS.fetch_add(1, Ordering::Relaxed);
    }
}

#[inline]
pub fn record_wdt_feed() {
    WDT_FEEDS.fetch_add(1, Ordering::Relaxed);
}

#[inline]
pub fn record_dispatch_send(ok: bool) {
    if ok {
        DISPATCH_SEND_OK.fetch_add(1, Ordering::Relaxed);
    } else {
        DISPATCH_SEND_FAIL.fetch_add(1, Ordering::Relaxed);
    }
}

/// 按 stage 记录错误，用于故障画像 TopN；已知 stage 用常量匹配，其余归入 other。
pub fn record_error_by_stage(stage: &str) {
    let c = match stage {
        STAGE_AGENT_ROUTER => &ERRORS_AGENT_ROUTER,
        STAGE_AGENT_CHAT => &ERRORS_AGENT_CHAT,
        STAGE_AGENT_CONTEXT => &ERRORS_AGENT_CONTEXT,
        STAGE_TOOL_EXECUTE => &ERRORS_TOOL_EXECUTE,
        STAGE_LLM_REQUEST => &ERRORS_LLM_REQUEST,
        STAGE_LLM_PARSE => &ERRORS_LLM_PARSE,
        STAGE_CHANNEL_DISPATCH => &ERRORS_CHANNEL_DISPATCH,
        STAGE_SESSION_APPEND => &ERRORS_SESSION_APPEND,
        _ => &ERRORS_OTHER,
    };
    c.fetch_add(1, Ordering::Relaxed);
}

/// 快照：用于 health API 与结构化基线日志（无敏感信息）。内部用 u32 存储，以 u64 暴露。
pub fn snapshot() -> MetricsSnapshot {
    MetricsSnapshot {
        messages_in: MESSAGES_IN.load(Ordering::Relaxed) as u64,
        messages_out: MESSAGES_OUT.load(Ordering::Relaxed) as u64,
        llm_calls: LLM_CALLS.load(Ordering::Relaxed) as u64,
        llm_errors: LLM_ERRORS.load(Ordering::Relaxed) as u64,
        llm_last_ms: LLM_LAST_MS.load(Ordering::Relaxed) as u64,
        tool_calls: TOOL_CALLS.load(Ordering::Relaxed) as u64,
        tool_errors: TOOL_ERRORS.load(Ordering::Relaxed) as u64,
        wdt_feeds: WDT_FEEDS.load(Ordering::Relaxed) as u64,
        dispatch_send_ok: DISPATCH_SEND_OK.load(Ordering::Relaxed) as u64,
        dispatch_send_fail: DISPATCH_SEND_FAIL.load(Ordering::Relaxed) as u64,
        errors_agent_router: ERRORS_AGENT_ROUTER.load(Ordering::Relaxed) as u64,
        errors_agent_chat: ERRORS_AGENT_CHAT.load(Ordering::Relaxed) as u64,
        errors_agent_context: ERRORS_AGENT_CONTEXT.load(Ordering::Relaxed) as u64,
        errors_tool_execute: ERRORS_TOOL_EXECUTE.load(Ordering::Relaxed) as u64,
        errors_llm_request: ERRORS_LLM_REQUEST.load(Ordering::Relaxed) as u64,
        errors_llm_parse: ERRORS_LLM_PARSE.load(Ordering::Relaxed) as u64,
        errors_channel_dispatch: ERRORS_CHANNEL_DISPATCH.load(Ordering::Relaxed) as u64,
        errors_session_append: ERRORS_SESSION_APPEND.load(Ordering::Relaxed) as u64,
        errors_other: ERRORS_OTHER.load(Ordering::Relaxed) as u64,
        last_active_epoch_secs: LAST_ACTIVE_EPOCH_SECS.load(Ordering::Relaxed) as u64,
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct MetricsSnapshot {
    pub messages_in: u64,
    pub messages_out: u64,
    pub llm_calls: u64,
    pub llm_errors: u64,
    pub llm_last_ms: u64,
    pub tool_calls: u64,
    pub tool_errors: u64,
    pub wdt_feeds: u64,
    pub dispatch_send_ok: u64,
    pub dispatch_send_fail: u64,
    pub errors_agent_router: u64,
    pub errors_agent_chat: u64,
    pub errors_agent_context: u64,
    pub errors_tool_execute: u64,
    pub errors_llm_request: u64,
    pub errors_llm_parse: u64,
    pub errors_channel_dispatch: u64,
    pub errors_session_append: u64,
    pub errors_other: u64,
    pub last_active_epoch_secs: u64,
}

impl MetricsSnapshot {
    /// 结构化单行日志，便于基线对比（key=value，无敏感信息）。
    pub fn to_baseline_log_line(&self) -> String {
        use std::fmt::Write;
        // Pre-allocate: typical line ~220 bytes.
        let mut buf = String::with_capacity(256);
        let _ = write!(
            buf,
            "metrics msg_in={} msg_out={} llm_calls={} llm_err={} llm_last_ms={} tool_calls={} tool_err={} wdt_feeds={} dispatch_ok={} dispatch_fail={} err_router={} err_chat={} err_ctx={} err_tool={} err_llm_req={} err_llm_parse={} err_dispatch={} err_session={} err_other={} last_active_epoch={}",
            self.messages_in,
            self.messages_out,
            self.llm_calls,
            self.llm_errors,
            self.llm_last_ms,
            self.tool_calls,
            self.tool_errors,
            self.wdt_feeds,
            self.dispatch_send_ok,
            self.dispatch_send_fail,
            self.errors_agent_router,
            self.errors_agent_chat,
            self.errors_agent_context,
            self.errors_tool_execute,
            self.errors_llm_request,
            self.errors_llm_parse,
            self.errors_channel_dispatch,
            self.errors_session_append,
            self.errors_other,
            self.last_active_epoch_secs
        );
        buf
    }
}
