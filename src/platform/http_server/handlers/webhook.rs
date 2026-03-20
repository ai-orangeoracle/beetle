//! POST /api/webhook：校验 webhook token 后把 body 作为 PcMsg 入队。

use crate::bus::{InboundTx, PcMsg};
use crate::config;
use crate::platform::http_server::common::{constant_time_eq, ApiResponse};
use crate::platform::http_server::user_message;

use super::HandlerContext;

/// 需配对；body 与 provided_token（来自 Header/Query）由 mod 传入。
pub fn post(
    ctx: &HandlerContext,
    inbound_tx: &InboundTx,
    body: String,
    provided_token: &str,
) -> Result<ApiResponse, std::io::Error> {
    let locale = config::get_locale(ctx.config_store.as_ref());
    let cfg = crate::config::AppConfig::load(
        ctx.config_store.as_ref(),
        Some(ctx.config_file_store.as_ref()),
    );
    if !cfg.webhook_enabled || cfg.webhook_token.is_empty() {
        return Ok(ApiResponse::err_403(&user_message::from_api_key(
            "webhook_disabled",
            &locale,
        )));
    }
    if !constant_time_eq(provided_token, &cfg.webhook_token) {
        return Ok(ApiResponse::err_401(&user_message::from_api_key(
            "invalid_token",
            &locale,
        )));
    }
    let msg = match PcMsg::new("webhook", "webhook", body) {
        Ok(m) => m,
        Err(_) => {
            return Ok(ApiResponse::err_413(&user_message::from_api_key(
                "content_too_long",
                &locale,
            )))
        }
    };
    if inbound_tx.send(msg).is_err() {
        return Ok(ApiResponse::err_503(&user_message::from_api_key(
            "queue_full",
            &locale,
        )));
    }
    Ok(ApiResponse::ok_200_json("{\"ok\":true}"))
}
