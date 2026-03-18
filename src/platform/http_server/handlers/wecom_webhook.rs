//! GET/POST /api/wecom/webhook：企业微信回调。
//! GET: URL 验证（返回 echostr）。
//! POST: 消息回调（解析 XML 入队）。

use crate::bus::InboundTx;
use crate::platform::http_server::common::ApiResponse;

use super::HandlerContext;

/// GET URL 验证：从 query 取 echostr 并原样返回。
pub fn get_verify(uri: &str) -> ApiResponse {
    let echostr = extract_query_param(uri, "echostr").unwrap_or_default();
    if echostr.is_empty() {
        ApiResponse::err_400("missing echostr")
    } else {
        ApiResponse {
            status: 200,
            status_text: "OK",
            body: echostr.into_bytes(),
        }
    }
}

/// POST 消息回调。
pub fn post(
    ctx: &HandlerContext,
    inbound_tx: &InboundTx,
    body: &str,
) -> Result<ApiResponse, std::io::Error> {
    let config = crate::config::AppConfig::load(
        ctx.config_store.as_ref(),
        Some(ctx.config_file_store.as_ref()),
    );
    let wecom_token = &config.wecom_token;
    match crate::channels::wecom::webhook::handle_message(body, wecom_token, inbound_tx) {
        Ok(()) => Ok(ApiResponse::ok_200_json("{\"ok\":true}")),
        Err(e) => {
            log::warn!("[wecom_webhook_handler] {}", e);
            Ok(ApiResponse::err_400(&e.to_string()))
        }
    }
}

fn extract_query_param<'a>(uri: &'a str, key: &str) -> Option<String> {
    let query = uri.find('?').map(|i| &uri[i + 1..]).unwrap_or("");
    for pair in query.split('&') {
        let mut it = pair.splitn(2, '=');
        if it
            .next()
            .map_or(false, |k| k.eq_ignore_ascii_case(key))
        {
            return it
                .next()
                .filter(|s| !s.is_empty())
                .map(|s| crate::util::percent_decode_query(s));
        }
    }
    None
}
