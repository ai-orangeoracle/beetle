//! Agent ReAct 循环：入站一条 → context → chat（含 tool_use 多轮）→ 会话持久化 → 出站一条。
//! 仅依赖 trait；HTTP/Tool 由 main 注入同一实现（如 EspHttpClient）。

use crate::agent::context::build_context;
use crate::bus::{InboundTx, OutboundTx, PcMsg, MAX_CONTENT_LEN};
use crate::constants::{
    AGENT_MARKER_MARK_IMPORTANT, AGENT_MARKER_SIGNAL_COMFORT, AGENT_MARKER_STOP,
    AGENT_RETRY_BASE_MS, AGENT_RETRY_MAX_MS, INBOUND_RECV_TIMEOUT_SECS,
    MAX_TOOL_RESULTS_USER_MESSAGE_LEN, TASK_CONTINUATION_CONTINUE_THRESHOLD_LEN,
};
use crate::error::Result;
use crate::llm::{LlmClient, LlmHttpClient, Message, StopReason, ToolSpec};
use crate::memory::{
    EmotionSignalStore, ImportantMessageStore, MemoryStore, PendingRetryStore, SessionStore,
    SessionSummaryStore, TaskContinuationStore,
};
use crate::metrics;
use crate::orchestrator::admission::{AdmissionDecision, LlmDecision, ToolDecision};
use crate::state;
use crate::tools::ToolContext;
use crate::util::{truncate_content_to_max, truncate_to_byte_len};
use crate::PlatformHttpClient;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};
/// 最大 ReAct 轮数（含首轮 chat），防止无限 tool 循环。
const MAX_REACT_ROUNDS: usize = 10;

/// 路由用短 system；回复约定：REPLY: <内容> 或 WORKER。
const ROUTER_SYSTEM: &str =
    "You are a router. Reply with exactly one line: either 'REPLY: <your direct reply>' or 'WORKER'.";

/// 低内存时发给用户的固定人话（出站），并重新入队当前消息。
const LOW_MEMORY_USER_MESSAGE: &str = "设备内存紧张，请稍后再试。";
/// 同一 chat_id 的 "low memory, defer" 日志最少间隔，避免刷屏。
const LOW_MEM_DEFER_LOG_INTERVAL: Duration = Duration::from_secs(60);

/// 在 run_worker_path 内包装 http，注入当前 msg 的 chat_id/channel，供 remind_at 等工具使用。
struct AgentToolCtx<'a, H: PlatformHttpClient> {
    http: &'a mut H,
    chat_id: String,
    channel: String,
}

impl<H: PlatformHttpClient> LlmHttpClient for AgentToolCtx<'_, H> {
    fn do_post(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<(u16, crate::platform::ResponseBody)> {
        crate::platform::PlatformHttpClient::post(self.http, url, headers, body)
    }
}

impl<H: PlatformHttpClient> ToolContext for AgentToolCtx<'_, H> {
    fn get_with_headers(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<(u16, crate::platform::ResponseBody)> {
        crate::platform::PlatformHttpClient::get(self.http, url, headers)
    }
    fn post_with_headers(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<(u16, crate::platform::ResponseBody)> {
        crate::platform::PlatformHttpClient::post(self.http, url, headers, body)
    }
    fn current_chat_id(&self) -> Option<&str> {
        Some(self.chat_id.as_str())
    }
    fn current_channel(&self) -> Option<&str> {
        Some(self.channel.as_str())
    }
}

/// run_worker_path 返回：正常内容或用户要求停止时的确认文案。
pub enum WorkerOutcome {
    Content(String),
    Interrupt(String),
}

/// 流式编辑器：LLM 流式输出期间，发送占位消息并逐步编辑内容。
/// 实现方内部自行创建/管理 HTTP 连接，不占用 agent 的 LLM HTTP 连接。
pub trait StreamEditor {
    /// 发送初始占位消息，返回 message_id（用于后续编辑）。
    fn send_initial(&self, chat_id: &str, content: &str) -> Result<Option<String>>;
    /// 编辑已发送的消息。
    fn edit(&self, chat_id: &str, message_id: &str, content: &str) -> Result<()>;
}

/// Agent 循环的存储与运行参数，由 main 构建并传入 run_agent_loop，减少参数数量。
pub struct AgentLoopConfig<'a> {
    pub memory_store: &'a dyn MemoryStore,
    pub session_store: &'a dyn SessionStore,
    pub session_summary_store: &'a dyn SessionSummaryStore,
    pub tool_specs: &'a [ToolSpec],
    pub get_skill_descriptions: &'a dyn Fn() -> String,
    pub session_max_messages: usize,
    pub tg_group_activation: &'a str,
    pub task_continuation: &'a dyn TaskContinuationStore,
    pub task_continuation_max_rounds: u32,
    pub important_message_store: &'a dyn ImportantMessageStore,
    pub emotion_signal_store: &'a dyn EmotionSignalStore,
    pub pending_retry: &'a dyn PendingRetryStore,
    /// 全局 LLM 流式模式；true 时 agent 使用 chat_with_progress 回调。
    pub llm_stream: bool,
    /// 流式编辑器；llm_stream 开且通道支持编辑时由 main 传入。
    pub stream_editor: Option<&'a dyn StreamEditor>,
}

/// 从 inbound_rx 取一条 PcMsg，构建 context，多轮 chat（含 tool 执行），写会话并发送一条出站。
/// router_llm：若为 Some 则先调路由，解析 REPLY/WORKER 再决定是否调 worker_llm；None 则仅用 worker_llm。
/// worker_llm：执行完整 context + tools 的客户端（可为 FallbackLlmClient）。
pub fn run_agent_loop<H: PlatformHttpClient>(
    http: &mut H,
    router_llm: Option<&dyn LlmClient>,
    worker_llm: &dyn LlmClient,
    registry: &crate::tools::ToolRegistry,
    config: &AgentLoopConfig<'_>,
    inbound_tx: InboundTx,
    inbound_rx: crate::bus::InboundRx,
    outbound_tx: OutboundTx,
    mut typing_notifier: Option<&mut dyn FnMut(&str, &str, &mut H)>,
) -> Result<()> {
    let tool_descriptions = registry.format_descriptions_for_system_prompt(8 * 1024);

    // Track repeated LLM failure for same request body, avoid infinite retry.
    // Key: u64 hash of (channel, chat_id, content) — avoids per-message format! String alloc.
    // Value: (failure count, last failure time) — entries expire after 5 minutes.
    let mut llm_failure_count: HashMap<u64, (u8, Instant)> = HashMap::new();
    // Throttle "low memory, defer" log per chat_id to avoid log spam.
    let mut low_mem_defer_log: Option<(String, Instant)> = None;

    if let Ok(Some(m)) = config.pending_retry.load_pending_retry() {
        let _ = config.pending_retry.clear_pending_retry();
        let _ = inbound_tx.send(m);
    }

    let recv_timeout = Duration::from_secs(INBOUND_RECV_TIMEOUT_SECS);
    loop {
        let msg = match inbound_rx.recv_timeout(recv_timeout) {
            Ok(m) => m,
            Err(RecvTimeoutError::Timeout) => {
                crate::platform::task_wdt::feed_current_task();
                metrics::record_wdt_feed();
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => break,
        };
        metrics::record_message_in();
        crate::platform::task_wdt::feed_current_task();
        let msg_key = {
            let mut hasher = DefaultHasher::new();
            msg.channel.hash(&mut hasher);
            msg.chat_id.hash(&mut hasher);
            msg.content.hash(&mut hasher);
            hasher.finish()
        };
        let now_for_key = Instant::now();
        if llm_failure_count
            .get(&msg_key)
            .map(|(count, ts)| {
                *count >= 3 && now_for_key.duration_since(*ts) < Duration::from_secs(300)
            })
            .unwrap_or(false)
        {
            let out = PcMsg {
                channel: msg.channel.clone(),
                chat_id: msg.chat_id.clone(),
                content: "节点正在维护，请稍后...".to_string(),
                is_group: false,
            };
            metrics::record_message_out();
            let _ = outbound_tx.try_send(out);
            continue;
        }

        // Refresh heap state if stale before admission check.
        crate::orchestrator::refresh_heap_if_stale();
        match crate::orchestrator::should_accept_inbound_pub(&msg.channel, &msg.chat_id) {
            AdmissionDecision::Accept => {}
            AdmissionDecision::Defer { delay_ms } => {
                let defer_out = PcMsg {
                    channel: msg.channel.clone(),
                    chat_id: msg.chat_id.clone(),
                    content: LOW_MEMORY_USER_MESSAGE.to_string(),
                    is_group: false,
                };
                metrics::record_message_out();
                let _ = outbound_tx.try_send(defer_out);
                let chat_id = msg.chat_id.clone();
                match inbound_tx.try_send(msg) {
                    Ok(()) => {
                        let now = Instant::now();
                        let should_log = low_mem_defer_log
                            .as_ref()
                            .map(|(id, t)| {
                                id != &chat_id || t.elapsed() >= LOW_MEM_DEFER_LOG_INTERVAL
                            })
                            .unwrap_or(true);
                        if should_log {
                            log::warn!("[agent] admission defer chat_id={}", chat_id);
                            low_mem_defer_log = Some((chat_id, now));
                        }
                    }
                    Err(std::sync::mpsc::TrySendError::Full(m)) => {
                        let _ = config.pending_retry.save_pending_retry(&m);
                        log::warn!(
                            "[agent] admission defer, pending_retry saved chat_id={}",
                            m.chat_id
                        );
                    }
                    Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                        log::error!("[agent] inbound_tx disconnected");
                    }
                }
                std::thread::sleep(Duration::from_millis(delay_ms));
                crate::platform::task_wdt::feed_current_task();
                metrics::record_wdt_feed();
                continue;
            }
            AdmissionDecision::Reject { reason } => {
                let now = Instant::now();
                let should_log = low_mem_defer_log
                    .as_ref()
                    .map(|(id, t)| id != reason || t.elapsed() >= LOW_MEM_DEFER_LOG_INTERVAL)
                    .unwrap_or(true);
                if should_log {
                    log::warn!("[agent] inbound rejected: {}", reason);
                    low_mem_defer_log = Some((reason.to_string(), now));
                }
                continue;
            }
        }
        if let Some(ref mut f) = typing_notifier {
            f(&msg.channel, &msg.chat_id, http);
        }

        // LLM 门控：Critical 压力下降级，Cautious 堆不足时延迟重试
        crate::orchestrator::refresh_heap_if_stale();
        match crate::orchestrator::can_call_llm_pub() {
            LlmDecision::Proceed => {}
            LlmDecision::RetryLater { delay_ms } => {
                let _ = inbound_tx.try_send(msg);
                std::thread::sleep(Duration::from_millis(delay_ms));
                crate::platform::task_wdt::feed_current_task();
                continue;
            }
            LlmDecision::Degrade { reason } => {
                log::info!("[agent] LLM degraded: {}", reason);
                let out = PcMsg {
                    channel: msg.channel.clone(),
                    chat_id: msg.chat_id.clone(),
                    content: LOW_MEMORY_USER_MESSAGE.to_string(),
                    is_group: false,
                };
                metrics::record_message_out();
                let _ = outbound_tx.try_send(out);
                continue;
            }
        }

        let final_content = match router_llm {
            Some(router) => {
                let router_messages = [Message {
                    role: "user".to_string(),
                    content: msg.content.clone(),
                }];
                let t0 = metrics::record_llm_call_start();
                match router.chat(http, ROUTER_SYSTEM, &router_messages, None) {
                    Ok(resp) => {
                        metrics::record_llm_call_end(t0);
                        crate::platform::task_wdt::feed_current_task();
                        let line = resp.content.trim();
                        if line.starts_with("REPLY: ") {
                            Ok((
                                WorkerOutcome::Content(line["REPLY: ".len()..].trim().to_string()),
                                None,
                                false,
                            ))
                        } else {
                            run_worker_path(
                                http,
                                worker_llm,
                                &msg,
                                registry,
                                config,
                                &tool_descriptions,
                            )
                        }
                    }
                    Err(e) => {
                        metrics::record_llm_call_end(t0);
                        crate::platform::task_wdt::feed_current_task();
                        metrics::record_llm_error();
                        metrics::record_error_by_stage("agent_router");
                        Err(e.with_stage("agent_router"))
                    }
                }
            }
            None => run_worker_path(http, worker_llm, &msg, registry, config, &tool_descriptions),
        };

        let (outcome, consumed_round, streamed) = match final_content {
            Ok(ok) => ok,
            Err(e) => {
                crate::platform::task_wdt::feed_current_task();
                metrics::record_error_by_stage(e.stage());
                log::warn!("[agent] chat loop failed: {}", e);
                state::set_last_error(&e);

                let (counter, _) = llm_failure_count
                    .entry(msg_key)
                    .or_insert((0, Instant::now()));
                *counter = counter.saturating_add(1);

                if *counter < 3 {
                    match inbound_tx.try_send(msg.clone()) {
                        Ok(()) => {}
                        Err(std::sync::mpsc::TrySendError::Full(_)) => {
                            let _ = config.pending_retry.save_pending_retry(&msg);
                            log::warn!(
                                "[agent] llm retry: inbound full, pending_retry saved chat_id={}",
                                msg.chat_id
                            );
                        }
                        Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                            log::error!("[agent] inbound_tx disconnected during llm retry");
                        }
                    }
                    let delay_ms = (AGENT_RETRY_BASE_MS * (1 << (*counter as u64).min(4)))
                        .min(AGENT_RETRY_MAX_MS);
                    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                    continue;
                }

                let reply = PcMsg {
                    channel: msg.channel.clone(),
                    chat_id: msg.chat_id.clone(),
                    content: "节点正在维护，请稍后...".to_string(),
                    is_group: false,
                };
                metrics::record_message_out();
                let _ = outbound_tx.try_send(reply);
                continue;
            }
        };
        let (mut reply_content, is_interrupt) = match &outcome {
            WorkerOutcome::Interrupt(confirm) => (
                truncate_content_to_max(confirm, MAX_CONTENT_LEN).into_owned(),
                true,
            ),
            WorkerOutcome::Content(s) => (
                truncate_content_to_max(s, MAX_CONTENT_LEN).into_owned(),
                false,
            ),
        };
        let mark_important = !is_interrupt && reply_content.contains(AGENT_MARKER_MARK_IMPORTANT);
        if !is_interrupt {
            if mark_important {
                reply_content = reply_content
                    .replace(AGENT_MARKER_MARK_IMPORTANT, "")
                    .trim()
                    .to_string();
                reply_content =
                    truncate_content_to_max(&reply_content, MAX_CONTENT_LEN).into_owned();
            }
            if reply_content.contains(AGENT_MARKER_SIGNAL_COMFORT) {
                reply_content = reply_content
                    .replace(AGENT_MARKER_SIGNAL_COMFORT, "")
                    .trim()
                    .to_string();
                reply_content =
                    truncate_content_to_max(&reply_content, MAX_CONTENT_LEN).into_owned();
                let _ = config.emotion_signal_store.set(&msg.chat_id, "comfort");
            }
        }

        if !is_interrupt && config.task_continuation_max_rounds > 0 {
            match consumed_round {
                Some(round) => {
                    if round < config.task_continuation_max_rounds
                        && (reply_content.contains("[CONTINUE]")
                            || reply_content.len() > TASK_CONTINUATION_CONTINUE_THRESHOLD_LEN)
                    {
                        let _ = config.task_continuation.set_task_continuation(
                            &msg.chat_id,
                            round + 1,
                            &reply_content,
                        );
                    } else {
                        let _ = config
                            .task_continuation
                            .clear_task_continuation(&msg.chat_id);
                    }
                }
                None => {
                    let _ = config
                        .task_continuation
                        .clear_task_continuation(&msg.chat_id);
                }
            }
        }

        // SILENT 或 cron 空回复不写 session，直接跳过。
        if reply_content.trim() == "SILENT" || (msg.channel == "cron" && reply_content.is_empty()) {
            llm_failure_count.remove(&msg_key);
            if llm_failure_count.len() > 64 {
                let now = Instant::now();
                llm_failure_count.retain(|_, (count, ts)| {
                    *count > 0 && now.duration_since(*ts) < Duration::from_secs(300)
                });
            }
            continue;
        }

        if let Err(e) = config
            .session_store
            .append(&msg.chat_id, "user", &msg.content)
        {
            log::warn!("[agent_session] append user failed: {}", e);
            metrics::record_error_by_stage("session_append");
        }
        if let Err(e) = config
            .session_store
            .append(&msg.chat_id, "assistant", &reply_content)
        {
            log::warn!("[agent_session] append assistant failed: {}", e);
            metrics::record_error_by_stage("session_append");
        }
        if mark_important {
            let _ = config
                .important_message_store
                .set_important_offset_from_end(&msg.chat_id, 1);
        }
        llm_failure_count.remove(&msg_key);
        if llm_failure_count.len() > 64 {
            let now = Instant::now();
            llm_failure_count.retain(|_, (count, ts)| {
                *count > 0 && now.duration_since(*ts) < Duration::from_secs(300)
            });
        }

        // 流式编辑已发送到通道时，跳过 outbound_tx 避免重复发送。
        if !streamed {
            let out = PcMsg {
                channel: msg.channel.clone(),
                chat_id: msg.chat_id.clone(),
                content: reply_content,
                is_group: false,
            };
            metrics::record_message_out();
            crate::platform::task_wdt::feed_current_task();
            if let Err(e) = outbound_tx.try_send(out) {
                log::warn!("[agent] outbound queue full or disconnected: {}", e);
            }
        } else {
            metrics::record_message_out();
            crate::platform::task_wdt::feed_current_task();
        }
    }
    Ok(())
}

/// 完整 context + worker LLM + ReAct 循环，返回 (WorkerOutcome, consumed_round, streamed)。不写 session，由调用方写。
/// streamed=true 表示已通过流式编辑发送到通道，调用方应跳过 outbound_tx。
fn run_worker_path<H: PlatformHttpClient>(
    http: &mut H,
    worker_llm: &dyn LlmClient,
    msg: &crate::bus::PcMsg,
    registry: &crate::tools::ToolRegistry,
    config: &AgentLoopConfig<'_>,
    tool_descriptions: &str,
) -> Result<(WorkerOutcome, Option<u32>, bool)> {
    let mut tool_ctx = AgentToolCtx {
        http,
        chat_id: msg.chat_id.clone(),
        channel: msg.channel.clone(),
    };
    let (suffix, consumed_round) =
        match config.task_continuation.get_task_continuation(&msg.chat_id) {
            Ok(Some((r, out))) => {
                let _ = config
                    .task_continuation
                    .clear_task_continuation(&msg.chat_id);
                let s = format!(
                    "上一轮产出（第{}轮）：\n{}\n\n本轮请在此基础上继续。",
                    r, out
                );
                (Some(s), Some(r))
            }
            _ => (None, None),
        };
    let skill_descriptions = (config.get_skill_descriptions)();
    let emotion_signal_suffix = config
        .emotion_signal_store
        .get_then_clear(&msg.chat_id)
        .ok()
        .flatten()
        .and_then(|s| {
            if s == "comfort" {
                Some("用户可能需安慰，回复时可适当照顾情绪。")
            } else {
                None
            }
        });
    let summary_with_count = config
        .session_summary_store
        .get_with_count(&msg.chat_id)
        .ok()
        .flatten();
    let summary_text = summary_with_count.as_ref().map(|(s, _)| s.as_str());
    let summary_last_count = summary_with_count.as_ref().map(|(_, c)| *c).unwrap_or(0);
    let budget = crate::orchestrator::current_budget();
    let (system, mut messages) = build_context(&super::ContextParams {
        msg,
        memory: config.memory_store,
        session: config.session_store,
        important_message_store: config.important_message_store,
        tool_descriptions,
        skill_descriptions: &skill_descriptions,
        system_max_len: budget.system_prompt_max,
        messages_max_len: budget.messages_max,
        session_max_messages: config.session_max_messages,
        group_activation: config.tg_group_activation,
        system_continuation_suffix: suffix.as_deref(),
        emotion_signal_suffix,
        summary_text,
        summary_last_count,
    })
    .map_err(|e| e.with_stage("agent_context"))?;

    let mut final_content = String::new();
    // 流式编辑状态（跨 ReAct 轮次共享）。
    let editor = if config.llm_stream {
        config.stream_editor
    } else {
        None
    };
    let mut stream_msg_id: Option<String> = None;
    let mut last_edit_time = Instant::now();
    let mut stream_edit_disabled = false; // send_initial 失败后禁用流式编辑
    let mut stream_edit_fail_count: u8 = 0; // edit 连续失败计数
    const EDIT_THROTTLE_MS: u64 = 500;
    const MAX_EDIT_FAILURES: u8 = 3;

    for _round in 0..MAX_REACT_ROUNDS {
        // Inter-round pressure check: skip first round (already gated by caller).
        if _round > 0 {
            crate::orchestrator::update_heap_state();
            match crate::orchestrator::can_call_llm_pub() {
                LlmDecision::Proceed => {}
                LlmDecision::RetryLater { .. } | LlmDecision::Degrade { .. } => {
                    if final_content.is_empty() {
                        final_content = LOW_MEMORY_USER_MESSAGE.to_string();
                    } else {
                        final_content.push_str("\n\n（因设备内存不足，后续步骤已省略）");
                    }
                    break;
                }
            }
        }
        let t0 = metrics::record_llm_call_start();
        let response = if config.llm_stream {
            let chat_id_for_cb = msg.chat_id.clone();
            let mut progress_cb = |_delta: &str, accumulated: &str| {
                crate::platform::task_wdt::feed_current_task();
                let Some(ed) = editor else { return };
                // Critical 压力下跳过流式编辑，节省 HTTP 连接与堆开销。
                if stream_edit_disabled
                    || matches!(
                        crate::orchestrator::current_pressure(),
                        crate::orchestrator::PressureLevel::Critical
                    )
                {
                    return;
                }
                let now = Instant::now();

                if stream_msg_id.is_none() {
                    // 首次收到文本：发送占位消息并记录 message_id。
                    match ed.send_initial(&chat_id_for_cb, accumulated) {
                        Ok(Some(id)) => {
                            stream_msg_id = Some(id);
                            last_edit_time = now;
                        }
                        Ok(None) => {}
                        Err(e) => {
                            log::warn!(
                                "[agent_stream] send_initial failed, disabling stream edit: {}",
                                e
                            );
                            stream_edit_disabled = true;
                        }
                    }
                } else if now.duration_since(last_edit_time)
                    >= Duration::from_millis(EDIT_THROTTLE_MS)
                {
                    if let Some(ref mid) = stream_msg_id {
                        if let Err(e) = ed.edit(&chat_id_for_cb, mid, accumulated) {
                            stream_edit_fail_count += 1;
                            log::debug!(
                                "[agent_stream] edit failed ({}/{}): {}",
                                stream_edit_fail_count,
                                MAX_EDIT_FAILURES,
                                e
                            );
                            if stream_edit_fail_count >= MAX_EDIT_FAILURES {
                                log::warn!(
                                    "[agent_stream] edit failed {} times, disabling stream edit",
                                    MAX_EDIT_FAILURES
                                );
                                stream_edit_disabled = true;
                            }
                        } else {
                            stream_edit_fail_count = 0;
                        }
                        last_edit_time = now;
                    }
                }
            };
            worker_llm.chat_with_progress(
                &mut tool_ctx,
                &system,
                &messages,
                Some(config.tool_specs),
                &mut progress_cb,
            )
        } else {
            worker_llm.chat(&mut tool_ctx, &system, &messages, Some(config.tool_specs))
        };
        let response = match response {
            Ok(r) => {
                metrics::record_llm_call_end(t0);
                r
            }
            Err(e) => {
                metrics::record_llm_call_end(t0);
                metrics::record_llm_error();
                metrics::record_error_by_stage("agent_chat");
                return Err(e.with_stage("agent_chat"));
            }
        };
        crate::platform::task_wdt::feed_current_task();
        metrics::record_wdt_feed();

        if response.stop_reason == StopReason::MaxTokens {
            let mut content = response.content;
            if !content.is_empty() {
                content.push_str("\n\n（回复因长度限制被截断）");
            }
            final_content = content;
            break;
        }

        if response.stop_reason == StopReason::EndTurn {
            let content = response.content;
            if content.contains(AGENT_MARKER_STOP) {
                let confirmation = content.replace(AGENT_MARKER_STOP, "").trim().to_string();
                // 流式编辑：更新为清理后的确认文案，避免用户看到原始标记。
                let streamed = if let (Some(ref mid), Some(ed)) = (&stream_msg_id, editor) {
                    if !confirmation.is_empty() {
                        let _ = ed.edit(&msg.chat_id, mid, &confirmation);
                    }
                    true
                } else {
                    false
                };
                return Ok((
                    WorkerOutcome::Interrupt(confirmation),
                    consumed_round,
                    streamed,
                ));
            }
            final_content = content;
            break;
        }

        if response.stop_reason == StopReason::ToolUse {
            let tool_calls = response.tool_calls.as_deref().unwrap_or(&[]);
            if tool_calls.is_empty() {
                final_content = response.content;
                break;
            }
            messages.push(Message {
                role: "assistant".to_string(),
                // Anthropic API 要求 tool_use 轮的 assistant content 非空；空时用占位符。
                content: if response.content.is_empty() {
                    "[tool_use]".to_string()
                } else {
                    response.content
                },
            });
            let mut user_content_raw = String::with_capacity(
                MAX_TOOL_RESULTS_USER_MESSAGE_LEN.min(tool_calls.len() * 256),
            );
            for (i, tc) in tool_calls.iter().enumerate() {
                // 工具执行门控
                let needs_net = registry.is_network_tool(&tc.name);
                let result = match crate::orchestrator::can_execute_tool_pub(&tc.name, needs_net) {
                    ToolDecision::Deny { reason } => {
                        log::info!("[agent_tool] {} denied: {}", tc.name, reason);
                        serde_json::json!({ "error": reason }).to_string()
                    }
                    ToolDecision::Allow => {
                        match registry.execute(&tc.name, &tc.input, &mut tool_ctx) {
                            Ok(s) => {
                                metrics::record_tool_call(true);
                                s
                            }
                            Err(e) => {
                                metrics::record_tool_call(false);
                                metrics::record_error_by_stage(e.stage());
                                log::error!(
                                    "[agent_tool] {} execute failed: {} (stage: {})",
                                    tc.name,
                                    e,
                                    e.stage()
                                );
                                state::set_last_error(&e);
                                format!("[tool error] {} (stage: {})", e, e.stage())
                            }
                        }
                    }
                };
                crate::platform::task_wdt::feed_current_task();
                if i > 0 {
                    user_content_raw.push('\n');
                }
                user_content_raw.push('[');
                user_content_raw.push_str(&tc.id);
                user_content_raw.push_str("]: ");
                user_content_raw.push_str(&result);
            }
            let user_content =
                truncate_to_byte_len(&user_content_raw, MAX_TOOL_RESULTS_USER_MESSAGE_LEN);
            messages.push(Message {
                role: "user".to_string(),
                content: format!("Tool results:\n{}", user_content),
            });
            // 流式编辑：tool_use 轮进入下一轮前，更新消息提示用户正在处理中。
            if let (Some(ref mid), Some(ed)) = (&stream_msg_id, editor) {
                let _ = ed.edit(&msg.chat_id, mid, "正在处理中…");
            }
            continue;
        }

        let content = response.content;
        if content.contains(AGENT_MARKER_STOP) {
            let confirmation = content.replace(AGENT_MARKER_STOP, "").trim().to_string();
            let streamed = if let (Some(ref mid), Some(ed)) = (&stream_msg_id, editor) {
                if !confirmation.is_empty() {
                    let _ = ed.edit(&msg.chat_id, mid, &confirmation);
                }
                true
            } else {
                false
            };
            return Ok((
                WorkerOutcome::Interrupt(confirmation),
                consumed_round,
                streamed,
            ));
        }
        final_content = content;
        break;
    }
    // 流式编辑：最终确认发送完整内容（tool_use 轮后 "正在处理中…" 需更新，或最终追加截断文字）。
    let streamed = if let (Some(ref mid), Some(ed)) = (&stream_msg_id, editor) {
        if !final_content.is_empty() {
            let _ = ed.edit(&msg.chat_id, mid, &final_content);
        }
        true
    } else {
        false
    };
    Ok((
        WorkerOutcome::Content(final_content),
        consumed_round,
        streamed,
    ))
}
