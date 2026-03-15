//! QQ 频道：入站 HTTP 回调（op=13/op=0 验签，AT_MESSAGE_CREATE 入队）与 WSS 入站；出站 Sink/flush、msg_id 被动回复，连通性检查。

mod send;
mod webhook;

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
mod ws;

pub use send::{check_connectivity, flush_qq_channel_sends, run_qq_sender_loop, QqMsgIdCache};
pub use webhook::{handle_webhook, QqHandlerResult, QQ_WEBHOOK_BODY_MAX};

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub use ws::run_qq_ws_loop;
