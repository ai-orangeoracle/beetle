//! Agent 上下文构建：从 MemoryStore + SessionStore + 工具说明聚合 system 与 messages。
//! Pure logic; no platform dependency; for use by agent::loop.

use crate::constants::{AGENT_MARKER_MARK_IMPORTANT, AGENT_MARKER_SIGNAL_COMFORT, AGENT_MARKER_STOP};
use crate::bus::PcMsg;
use crate::error::Result;
use crate::memory::{build_system_prompt, ImportantMessageStore, MemoryStore, SessionStore};
use crate::llm::Message;
use crate::state;

pub use crate::constants::{DEFAULT_MESSAGES_MAX_LEN, DEFAULT_SYSTEM_MAX_LEN};
/// 从 SessionStore 加载的最近条数。
pub const SESSION_RECENT_N: usize = 32;
/// 每日笔记取最近条数。
const DAILY_RECENT_N: usize = 5;

/// 根据入站 PcMsg 与 store 构建 (system, messages)，供 LlmClient.chat 使用。
///
/// **system 组成顺序**：SOUL → USER → MEMORY → daily_notes → tool_descriptions → skill_descriptions → 群组/SILENT 约定；总长 ≤ system_max_len。
/// **截断策略**：base_max 用于 system_base（memory::build_system_prompt）；剩余空间依次给 tools、skills，超长从尾部截断；最后若仍超 system_max_len 则整体 truncate。
/// **失败降级**：任一源（get_soul/get_user/get_memory/list_daily_note_names）加载失败时降级为空字符串并打日志，不阻塞 build。
///
/// **messages**：历史会话（最近 session_max_messages 条）+ 当前用户 content，总长 ≤ messages_max_len；超限从最旧消息起丢弃。
/// **system_continuation_suffix**：多轮延续时追加到 system 末尾的上一轮产出说明；若提供则追加后再做最终截断。
pub fn build_context(
    msg: &PcMsg,
    memory: &dyn MemoryStore,
    session: &dyn SessionStore,
    important_message_store: &dyn ImportantMessageStore,
    tool_descriptions: &str,
    skill_descriptions: &str,
    system_max_len: usize,
    messages_max_len: usize,
    session_max_messages: usize,
    group_activation: &str,
    system_continuation_suffix: Option<&str>,
    emotion_signal_suffix: Option<&str>,
    summary_text: Option<&str>,
) -> Result<(String, Vec<Message>)> {
    let soul_res = memory.get_soul();
    state::set_soul_load_ok(soul_res.is_ok());
    let soul = soul_res.unwrap_or_else(|e| {
        log::warn!("[context] get_soul failed: {}", e);
        String::new()
    });
    let user = memory.get_user().unwrap_or_else(|e| {
        log::warn!("[context] get_user failed: {}", e);
        String::new()
    });
    let mem_res = memory.get_memory();
    state::set_memory_load_ok(mem_res.is_ok());
    let mem = mem_res.unwrap_or_else(|e| {
        log::warn!("[context] get_memory failed: {}", e);
        String::new()
    });
    let names = memory.list_daily_note_names(DAILY_RECENT_N).unwrap_or_else(|_| vec![]);
    let mut daily_contents: Vec<String> = Vec::with_capacity(names.len());
    for name in &names {
        if let Ok(c) = memory.get_daily_note(name) {
            daily_contents.push(c);
        }
    }
    let tools_max = system_max_len / 4; // 预留约 1/4 给工具说明
    let base_max = system_max_len.saturating_sub(tool_descriptions.len().min(tools_max));
    let system_base = build_system_prompt(&soul, &user, &mem, &daily_contents, base_max);
    let mut system = system_base;
    if !tool_descriptions.is_empty() {
        let remain = system_max_len.saturating_sub(system.len());
        if remain > 0 {
            let t = if tool_descriptions.len() <= remain {
                tool_descriptions.to_string()
            } else {
                tool_descriptions.chars().take(remain).collect::<String>()
            };
            system.push_str("\n\n## Tools\n");
            system.push_str(&t);
        }
    }
    if !skill_descriptions.is_empty() {
        let remain = system_max_len.saturating_sub(system.len());
        if remain > 0 {
            let s = if skill_descriptions.len() <= remain {
                skill_descriptions.to_string()
            } else {
                skill_descriptions.chars().take(remain).collect::<String>()
            };
            system.push_str("\n\n## Skills\n");
            system.push_str(&s);
        }
    }
    if msg.is_group {
        let remain = system_max_len.saturating_sub(system.len());
        if remain > 64 {
            if group_activation == "always" {
                system.push_str("\n\nIf no response is needed, reply with exactly SILENT and nothing else.");
            } else if group_activation == "mention" {
                system.push_str("\n\nYou are in a group; only reply when explicitly mentioned.");
            }
        }
    }
    if let Some(suffix) = system_continuation_suffix {
        system.push_str("\n\n");
        system.push_str(suffix);
    }
    let structured_block = format!(
        "\n\n## Structured output\nWhen the user clearly asks to stop or cancel the current task, reply with {} then a short confirmation. When you want to mark the current user message as important for context truncation, include {} in your reply. When you sense the user may need comfort or encouragement, include {} in your reply.",
        AGENT_MARKER_STOP,
        AGENT_MARKER_MARK_IMPORTANT,
        AGENT_MARKER_SIGNAL_COMFORT
    );
    if system.len().saturating_add(structured_block.len()) <= system_max_len {
        system.push_str(&structured_block);
    }
    if let Some(em) = emotion_signal_suffix {
        system.push_str("\n\n");
        system.push_str(em);
    }
    let hint = crate::resource::current_budget().llm_hint;
    if !hint.is_empty() && system.len().saturating_add(hint.len()).saturating_add(2) <= system_max_len {
        system.push_str("\n\n");
        system.push_str(hint);
    }
    if system.len() > system_max_len {
        let mut end = system_max_len;
        while end > 0 && !system.is_char_boundary(end) {
            end -= 1;
        }
        system.truncate(end);
    }

    let n = session_max_messages.max(1).min(128);
    let recent = session.load_recent(&msg.chat_id, n).unwrap_or_else(|_| vec![]);
    let cap = recent.len() + if summary_text.is_some() { 2 } else { 1 };
    let mut messages: Vec<Message> = Vec::with_capacity(cap);
    if let Some(summary) = summary_text {
        messages.push(Message {
            role: "user".to_string(),
            content: format!("[Previous conversation summary]\n{}", summary),
        });
    }
    messages.extend(recent.into_iter().map(|m| Message {
        role: m.role,
        content: m.content,
    }));
    messages.push(Message {
        role: "user".to_string(),
        content: msg.content.clone(),
    });
    let important_offset = important_message_store.get_important_offset(&msg.chat_id).ok().flatten();
    truncate_messages_to_len(&mut messages, messages_max_len, important_offset);
    if important_offset.is_some() {
        let _ = important_message_store.clear_important(&msg.chat_id);
    }

    Ok((system, messages))
}

fn truncate_messages_to_len(
    messages: &mut Vec<Message>,
    max_len: usize,
    protected_offset_from_end: Option<u32>,
) {
    let mut total = 0usize;
    for m in messages.iter() {
        total = total.saturating_add(m.role.len()).saturating_add(m.content.len()).saturating_add(2);
    }
    let protected_idx = protected_offset_from_end.and_then(|off| {
        let len = messages.len();
        let idx = len.saturating_sub(1).saturating_sub(off as usize);
        if idx < len {
            Some(idx)
        } else {
            None
        }
    });
    let mut indices_to_remove = Vec::new();
    for (i, m) in messages.iter().enumerate() {
        if total <= max_len {
            break;
        }
        if Some(i) == protected_idx {
            continue;
        }
        if messages.len() - indices_to_remove.len() <= 1 {
            break;
        }
        let sz = m.role.len().saturating_add(m.content.len()).saturating_add(2);
        total = total.saturating_sub(sz);
        indices_to_remove.push(i);
    }
    for i in indices_to_remove.into_iter().rev() {
        messages.remove(i);
    }
    merge_consecutive_same_role(messages);
}

/// Merge consecutive messages with the same role (can happen after truncation removes intermediate messages).
fn merge_consecutive_same_role(messages: &mut Vec<Message>) {
    let mut i = 0;
    while i + 1 < messages.len() {
        if messages[i].role == messages[i + 1].role {
            let next_content = messages.remove(i + 1).content;
            messages[i].content.push('\n');
            messages[i].content.push_str(&next_content);
        } else {
            i += 1;
        }
    }
}
