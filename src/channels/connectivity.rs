//! 通道连通性检查：按配置逐通道单次 HTTP 探测，供 GET /api/channel_connectivity 使用。
//! 不依赖 Platform，仅依赖 ChannelHttpClient 与 AppConfig。
//!
//! 各通道实现 check_connectivity，本模块仅按固定顺序收集并返回列表。
//! 调用方（前端或网关）应设置合理 HTTP 超时。

use crate::config::AppConfig;
use crate::i18n::{tr, Locale, Message};
use serde::Serialize;

/// 单通道连通性结果；与前端约定字段名。
#[derive(Debug, Clone, Serialize)]
pub struct ChannelConnectivityItem {
    pub id: String,
    pub configured: bool,
    pub ok: bool,
    pub message: Option<String>,
}

/// 供各通道 check_connectivity 构建结果用。
pub(crate) fn item(
    id: &'static str,
    configured: bool,
    ok: bool,
    message: Option<String>,
) -> ChannelConnectivityItem {
    ChannelConnectivityItem {
        id: id.to_string(),
        configured,
        ok,
        message,
    }
}

fn webhook_configured(c: &AppConfig) -> bool {
    c.webhook_enabled && !c.webhook_token.trim().is_empty()
}

/// 按固定顺序检查各通道，返回列表；未配置的通道也列入，configured=false。
pub fn check_all<H: crate::channels::ChannelHttpClient + ?Sized>(
    config: &AppConfig,
    http: &mut H,
    loc: Locale,
) -> Vec<ChannelConnectivityItem> {
    let mut out = Vec::with_capacity(6);
    out.push(crate::channels::telegram::check_connectivity(config, http, loc));
    out.push(crate::channels::feishu::check_connectivity(config, http, loc));
    out.push(crate::channels::dingtalk::check_connectivity(config, http, loc));
    out.push(crate::channels::wecom::check_connectivity(config, http, loc));
    out.push(crate::channels::qq::check_connectivity(config, http, loc));
    let configured = webhook_configured(config);
    let msg = if configured {
        None
    } else {
        Some(tr(Message::ConnectivityNotConfigured, loc))
    };
    out.push(item("webhook", configured, configured, msg));
    out
}
