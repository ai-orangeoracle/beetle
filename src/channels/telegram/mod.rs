//! Telegram 通道：入站 long poll 推 bus，出站经 MessageSink 队列后由 main 用 HTTP 发送。

mod poll;
mod send;

pub use poll::{poll_telegram_once, run_telegram_poll_loop, TelegramCommandCtx};
pub use send::{
    check_connectivity, flush_telegram_sends, get_bot_username, run_telegram_sender_loop,
    send_chat_action,
};
