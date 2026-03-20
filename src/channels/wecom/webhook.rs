//! 企业微信入站 Webhook：消息回调。
//! 消息回调 POST 解析 XML 提取 Content + FromUserName。
//! URL 验证由 HTTP handler 直接处理（取 echostr 原样返回）。

use crate::bus::{InboundTx, PcMsg};
use crate::error::Result;

const TAG: &str = "wecom_webhook";

/// 验证企微回调签名。算法：SHA1(sort([token, timestamp, nonce]))。
/// Verify WeCom callback signature: SHA1(sort([token, timestamp, nonce])).
pub fn verify_signature(token: &str, timestamp: &str, nonce: &str, expected_sig: &str) -> bool {
    if token.is_empty() || expected_sig.is_empty() {
        // Token 未配置时跳过验签（向后兼容，但记录警告）。
        log::warn!(
            "[{}] signature verification skipped: token or signature empty",
            TAG
        );
        return true;
    }
    let mut parts = [token, timestamp, nonce];
    parts.sort();
    let combined = format!("{}{}{}", parts[0], parts[1], parts[2]);
    let computed = crate::util::sha1_hex(combined.as_bytes());
    // Constant-time comparison to prevent timing attacks.
    crate::util::constant_time_eq(&computed, expected_sig)
}

/// 解析企微消息回调 XML body，提取 Content 和 FromUserName。
/// 企微回调 body 格式为 XML：
/// ```xml
/// <xml><ToUserName>...</ToUserName><FromUserName>...</FromUserName><Content>...</Content>...</xml>
/// ```
pub fn handle_message(
    body: &str,
    wecom_token: &str,
    timestamp: &str,
    nonce: &str,
    msg_signature: &str,
    inbound_tx: &InboundTx,
) -> Result<()> {
    // 验签
    if !verify_signature(wecom_token, timestamp, nonce, msg_signature) {
        log::warn!("[{}] invalid signature, rejecting message", TAG);
        return Err(crate::error::Error::config(
            "wecom_webhook",
            "invalid signature",
        ));
    }

    // Parse simple XML fields without full XML parser.
    let content = extract_xml_field(body, "Content").unwrap_or_default();
    let from_user = extract_xml_field(body, "FromUserName").unwrap_or("wecom_default".to_string());

    if content.trim().is_empty() {
        log::debug!("[{}] empty content, skip", TAG);
        return Ok(());
    }

    log::info!(
        "[{}] received from user={} len={}",
        TAG,
        from_user,
        content.len()
    );

    let msg = PcMsg::new("wecom", &from_user, content)?;
    if inbound_tx.send(msg).is_err() {
        log::warn!("[{}] inbound_tx send failed (queue full?)", TAG);
    }
    Ok(())
}

/// 从 XML 字符串中提取指定标签的内容。支持 CDATA 和纯文本。
fn extract_xml_field(xml: &str, field: &str) -> Option<String> {
    let open = format!("<{}>", field);
    let close = format!("</{}>", field);
    let start = xml.find(&open).map(|i| i + open.len())?;
    let end = xml[start..].find(&close).map(|i| start + i)?;
    let value = &xml[start..end];
    // Strip CDATA wrapper if present.
    let value = value.trim();
    if value.starts_with("<![CDATA[") && value.ends_with("]]>") {
        Some(value[9..value.len() - 3].to_string())
    } else {
        Some(value.to_string())
    }
}
