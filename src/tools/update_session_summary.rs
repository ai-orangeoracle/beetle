//! update_session_summary 工具：由模型在认为需要时调用，将当前会话摘要落盘；build_context 将已有摘要注入 messages 首条。

use crate::error::{Error, Result};
use crate::i18n::{tr, Message as UiMessage};
use crate::memory::{SessionStore, SessionSummaryStore};
use crate::tools::{parse_tool_args, Tool, ToolContext};

/// 需注入 SessionSummaryStore 和 SessionStore；由 main 注册时传入。
pub struct UpdateSessionSummaryTool {
    store: std::sync::Arc<dyn SessionSummaryStore + Send + Sync>,
    session_store: std::sync::Arc<dyn SessionStore + Send + Sync>,
}

impl UpdateSessionSummaryTool {
    pub fn new(
        store: std::sync::Arc<dyn SessionSummaryStore + Send + Sync>,
        session_store: std::sync::Arc<dyn SessionStore + Send + Sync>,
    ) -> Self {
        Self {
            store,
            session_store,
        }
    }
}

impl Tool for UpdateSessionSummaryTool {
    fn name(&self) -> &'static str {
        "update_session_summary"
    }
    fn description(&self) -> &'static str {
        "Update the session summary for long conversations. Call this when the conversation has reached a natural break or topic change and you want to persist a brief summary for future context. Argument: summary (string), one paragraph."
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "summary": { "description": "One-paragraph summary of the conversation so far", "type": "string" }
            },
            "required": ["summary"]
        })
    }
    fn execute(&self, args: &str, ctx: &mut dyn ToolContext) -> Result<String> {
        let chat_id = ctx.current_chat_id().ok_or_else(|| {
            Error::config(
                "update_session_summary",
                "no current chat_id (tool used outside session)",
            )
        })?;
        let obj = parse_tool_args(args, "update_session_summary")?;
        let summary = obj.get("summary").and_then(|v| v.as_str()).unwrap_or("");
        // Get current message count for tracking when summary was last updated.
        let message_count = self.session_store.message_count(chat_id).unwrap_or(0);
        self.store.set_with_count(chat_id, summary, message_count)?;
        Ok(tr(UiMessage::SessionSummaryUpdated, ctx.user_locale()))
    }
}
