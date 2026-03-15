//! 钉钉通道：出站 Sink/flush，连通性检查；入站无。

mod send;
pub use send::{check_connectivity, flush_dingtalk_sends, run_dingtalk_sender_loop};
