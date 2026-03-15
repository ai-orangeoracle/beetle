//! 企业微信通道：出站 Sink/flush，连通性检查；入站无。

mod send;
pub use send::{check_connectivity, flush_wecom_sends, run_wecom_sender_loop};
