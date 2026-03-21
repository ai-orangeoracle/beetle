//! GET /api/resource：返回 orchestrator ResourceSnapshot JSON。

use super::HandlerContext;
use crate::orchestrator;

/// 生成 resource JSON body。
pub fn body(_ctx: &HandlerContext) -> Result<String, std::io::Error> {
    let snap = orchestrator::snapshot();
    serde_json::to_string(&snap).map_err(std::io::Error::other)
}
