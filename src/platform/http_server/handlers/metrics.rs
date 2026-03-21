//! GET /api/metrics：返回 MetricsSnapshot JSON。

use super::HandlerContext;
use crate::metrics;

/// 生成 metrics JSON body。
pub fn body(_ctx: &HandlerContext) -> Result<String, std::io::Error> {
    let snap = metrics::snapshot();
    serde_json::to_string(&snap).map_err(|e| std::io::Error::other(e))
}
