//! HTTP 配置 API 服务器：SoftAP 下 0.0.0.0:80，仅 ESP 目标编译。
//! Config API over HTTP; ESP target only.

use crate::config::AppConfig;
use crate::error::{Error, Result};
use std::sync::Arc;
use std::time::Duration;

pub(crate) mod common;
mod handlers;
mod user_message;
use common::*;

/// 写 401 + JSON body，供鉴权失败时 return。
macro_rules! resp_401_json {
    ($req:expr, $msg:expr) => {{
        let body = format!(r#"{{"error":"{}"}}"#, $msg.replace('"', "\\\""));
        let mut r = $req
            .into_response(401, Some("Unauthorized"), CORS_HEADERS)
            .map_err(to_io)?;
        r.write_all(body.as_bytes()).map_err(to_io)?;
        Ok(())
    }};
}
/// OPTIONS 预检统一响应。写 1 字节 body 以迫使底层发送响应头（部分栈空 body 时不发头）。
macro_rules! resp_options {
    ($req:expr) => {{
        const OPTIONS_BODY: &[u8] = b" ";
        let mut resp = $req
            .into_response(200, Some("OK"), CORS_OPTIONS_HEADERS)
            .map_err(to_io)?;
        resp.write_all(OPTIONS_BODY).map_err(to_io)?;
        Ok(())
    }};
}
/// 已激活检查：未设置配对码则 return 401（文案按 locale）。
macro_rules! require_activated {
    ($req:expr, $store:expr) => {{
        if !crate::platform::pairing::code_set($store.as_ref()) {
            let locale = crate::config::get_locale($store.as_ref());
            let msg = crate::platform::http_server::user_message::from_api_key(
                "pairing_required",
                &locale,
            );
            return resp_401_json!($req, msg);
        }
    }};
}
/// 写操作鉴权：未激活 401；已激活则从 req 取码并校验，失败 401（文案按 locale）。
macro_rules! require_pairing_code {
    ($req:expr, $store:expr) => {{
        if !crate::platform::pairing::code_set($store.as_ref()) {
            let locale = crate::config::get_locale($store.as_ref());
            let msg = crate::platform::http_server::user_message::from_api_key(
                "pairing_required",
                &locale,
            );
            return resp_401_json!($req, msg);
        }
        let code = code_from_uri($req.uri())
            .map(String::from)
            .or_else(|| $req.header("X-Pairing-Code").map(|s| s.to_string()));
        match code.as_deref() {
            Some(c) if !c.is_empty() => {
                if !crate::platform::pairing::verify_code($store.as_ref(), c) {
                    let locale = crate::config::get_locale($store.as_ref());
                    let msg = crate::platform::http_server::user_message::from_api_key(
                        "pairing_code_wrong",
                        &locale,
                    );
                    return resp_401_json!($req, msg);
                }
            }
            _ => {
                let locale = crate::config::get_locale($store.as_ref());
                let msg = crate::platform::http_server::user_message::from_api_key(
                    "pairing_code_wrong",
                    &locale,
                );
                return resp_401_json!($req, msg);
            }
        }
    }};
}
/// 已激活 GET + 调用 handler 返回 body + 写 200 JSON。收敛 require_activated + body + write_json_200。
macro_rules! activated_get_json {
    ($req:expr, $store:expr, $ctx:expr, $handler:path) => {{
        require_activated!($req, $store);
        let body = $handler($ctx.as_ref()).map_err(to_io)?;
        write_json_200!($req, body)
    }};
}
/// 注册 handler 并统一 map_err。
macro_rules! register {
    ($server:expr, $path:expr, $method:expr, $closure:expr) => {
        $server
            .fn_handler($path, $method, $closure)
            .map_err(|e| Error::Other {
                source: Box::new(e),
                stage: "http_server_handler",
            })?
    };
}
/// 统一写响应：into_response + write_all；供 write_json_200/write_text_200/write_api_resp 复用。
macro_rules! write_response {
    ($req:expr, $status:expr, $status_text:expr, $headers:expr, $body:expr) => {{
        let mut resp = $req
            .into_response($status, Some($status_text), $headers)
            .map_err(to_io)?;
        resp.write_all($body).map_err(to_io)?;
        Ok(()) as HandlerResult
    }};
}
/// 200 OK + CORS + application/json，用于 GET /api/skills 列表。
macro_rules! write_json_200 {
    ($req:expr, $body:expr) => {{
        let b = $body;
        write_response!($req, 200, "OK", CORS_HEADERS, b.as_bytes())
    }};
}
/// 200 OK + CORS + text/plain，用于 GET /api/soul、GET /api/user。
macro_rules! write_text_200 {
    ($req:expr, $body:expr) => {{
        let b = $body;
        write_response!($req, 200, "OK", CORS_AND_TEXT_PLAIN, b.as_bytes())
    }};
}
/// 将 ApiResponse 写入请求响应流。
macro_rules! write_api_resp {
    ($req:expr, $r:expr) => {{
        let r: crate::platform::http_server::common::ApiResponse = $r;
        write_response!($req, r.status, r.status_text, CORS_HEADERS, &r.body)
    }};
}
/// 从请求读 body 为 UTF-8 字符串，超长截断为 max_len；读失败 return 500，非 UTF-8 return 400。需传入 store 以按 locale 返回错误文案。
/// 无 Content-Length 时按块读取，避免小 POST 也占满 4KB。
macro_rules! read_body_utf8 {
    ($req:expr, $max_len:expr, $store:expr) => {{
        let content_len = $req.content_len();
        match crate::platform::http_server::common::read_body_utf8_impl(
            &mut $req,
            content_len,
            $max_len,
        ) {
            Ok(s) => s,
            Err(crate::platform::http_server::common::BodyReadError::ReadFailed) => {
                let locale = crate::config::get_locale($store.as_ref());
                let msg = crate::platform::http_server::user_message::from_api_key(
                    "body_read_failed",
                    &locale,
                );
                let r = ApiResponse::err_500(&msg);
                return write_api_resp!($req, r);
            }
            Err(crate::platform::http_server::common::BodyReadError::InvalidUtf8) => {
                let locale = crate::config::get_locale($store.as_ref());
                let msg = crate::platform::http_server::user_message::from_api_key(
                    "invalid_utf8",
                    &locale,
                );
                let r = ApiResponse::err_400(&msg);
                return write_api_resp!($req, r);
            }
        }
    }};
}

/// 飞书事件回调 body 最大字节。
const FEISHU_EVENT_BODY_MAX: usize = 64 * 1024;

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub fn run(
    platform: std::sync::Arc<dyn crate::platform::Platform>,
    wifi_connected: bool,
    inbound_depth: Arc<std::sync::atomic::AtomicUsize>,
    outbound_depth: Arc<std::sync::atomic::AtomicUsize>,
    memory_store: Arc<dyn crate::memory::MemoryStore + Send + Sync>,
    session_store: Arc<dyn crate::memory::SessionStore + Send + Sync>,
    inbound_tx: crate::bus::InboundTx,
    msg_id_cache: crate::channels::QqMsgIdCache,
) -> Result<()> {
    let config_store = platform.config_store();
    let config_file_store: std::sync::Arc<dyn crate::config::ConfigFileStore + Send + Sync> =
        std::sync::Arc::new(crate::config::PlatformConfigFileStore(std::sync::Arc::clone(&platform)));
    let skill_storage = platform.skill_storage();
    let skill_meta_store = platform.skill_meta_store();
    use embedded_io::Write as _;
    use embedded_svc::http::Headers as _;
    use embedded_svc::http::Method;
    use esp_idf_svc::http::server::{Configuration, EspHttpServer};

    let mut server_config = Configuration::default();
    server_config.max_open_sockets = MAX_OPEN_SOCKETS;
    // 路由多（每 URI 常含 Get+Options，部分有 Post），默认 32 槽位不足导致 ESP_ERR_HTTPD_HANDLERS_FULL
    server_config.max_uri_handlers = 64;
    // 默认 6KB 栈在 Rust handler（闭包+JSON+深层调用）下易溢出；GET /api/channel_connectivity 在任务内串行执行多次外网 HTTP，栈压力大，故提高到 12KB
    server_config.stack_size = 12 * 1024;

    let mut server = EspHttpServer::new(&server_config).map_err(|e| Error::Other {
        source: Box::new(e),
        stage: "http_server_new",
    })?;

    #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
    let http_for_fetch = std::sync::RwLock::new(Some(()));
    #[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
    let http_for_fetch = match crate::platform::EspHttpClient::new() {
        Ok(c) => std::cell::RefCell::new(Some(c)),
        Err(e) => return Err(e.with_stage("http_server_fetch_client")),
    };
    let ctx = Arc::new(handlers::HandlerContext {
        config_store: Arc::clone(&config_store),
        config_file_store: Arc::clone(&config_file_store),
        platform: Arc::clone(&platform),
        memory_store: Arc::clone(&memory_store),
        session_store: Arc::clone(&session_store),
        skill_storage: Arc::clone(&skill_storage),
        skill_meta_store: Arc::clone(&skill_meta_store),
        inbound_depth: Arc::clone(&inbound_depth),
        outbound_depth: Arc::clone(&outbound_depth),
        wifi_connected,
        version: Arc::from(env!("CARGO_PKG_VERSION")),
        board_id: Arc::from(crate::build_board_id()),
        http_for_fetch,
    });

    let store_root = std::sync::Arc::clone(&config_store);
    let ctx_root = Arc::clone(&ctx);
    register!(server, "/", Method::Get, move |req| -> HandlerResult {
        if !crate::platform::pairing::code_set(store_root.as_ref()) {
            let mut resp = req
                .into_response(302, Some("Found"), REDIRECT_PAIRING_HEADERS)
                .map_err(to_io)?;
            resp.write_all(&[]).map_err(to_io)?;
            return Ok(());
        }
        let body = handlers::root::body(ctx_root.as_ref()).map_err(to_io)?;
        let mut resp = req
            .into_response(200, Some("OK"), CORS_HEADERS)
            .map_err(to_io)?;
        resp.write_all(body.as_bytes()).map_err(to_io)?;
        Ok(())
    });

    register!(server, "/", Method::Options, |req| -> HandlerResult {
        resp_options!(req)
    });

    register!(server, "/wifi", Method::Get, |req| -> HandlerResult {
        let html = handlers::config_page::html();
        let mut resp = req
            .into_response(200, Some("OK"), HTML_HEADERS)
            .map_err(to_io)?;
        resp.write_all(html.as_bytes()).map_err(to_io)?;
        Ok(())
    });

    register!(server, "/wifi", Method::Options, |req| -> HandlerResult {
        resp_options!(req)
    });

    register!(server, "/pairing", Method::Get, |req| -> HandlerResult {
        let html = handlers::config_page::pairing_html();
        let mut resp = req
            .into_response(200, Some("OK"), HTML_HEADERS)
            .map_err(to_io)?;
        resp.write_all(html.as_bytes()).map_err(to_io)?;
        Ok(())
    });

    register!(
        server,
        "/pairing",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );

    register!(server, "/common.css", Method::Get, |req| -> HandlerResult {
        let css = handlers::config_page::common_css();
        let mut resp = req
            .into_response(200, Some("OK"), CSS_HEADERS)
            .map_err(to_io)?;
        resp.write_all(css.as_bytes()).map_err(to_io)?;
        Ok(())
    });
    register!(
        server,
        "/common.css",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );

    register!(server, "/common.js", Method::Get, |req| -> HandlerResult {
        let js = handlers::config_page::common_js();
        let mut resp = req
            .into_response(200, Some("OK"), JS_HEADERS)
            .map_err(to_io)?;
        resp.write_all(js.as_bytes()).map_err(to_io)?;
        Ok(())
    });
    register!(
        server,
        "/common.js",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );

    let ctx_pairing = Arc::clone(&ctx);
    register!(
        server,
        "/api/pairing_code",
        Method::Get,
        move |req| -> HandlerResult {
            let body = handlers::pairing::body(ctx_pairing.as_ref());
            let mut resp = req
                .into_response(200, Some("OK"), CORS_HEADERS)
                .map_err(to_io)?;
            resp.write_all(body.as_bytes()).map_err(to_io)?;
            Ok(())
        }
    );

    let store_pairing_post = std::sync::Arc::clone(&config_store);
    let ctx_pairing_post = Arc::clone(&ctx);
    register!(
        server,
        "/api/pairing_code",
        Method::Post,
        move |mut req| -> HandlerResult {
            let body = read_body_utf8!(req, POST_BODY_MAX_LEN, store_pairing_post);
            let r = handlers::pairing::post_body(ctx_pairing_post.as_ref(), &body);
            write_api_resp!(req, r)
        }
    );

    register!(
        server,
        "/api/pairing_code",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );

    let store_cfg = std::sync::Arc::clone(&config_store);
    let ctx_cfg = Arc::clone(&ctx);
    register!(
        server,
        "/api/config",
        Method::Get,
        move |req| -> HandlerResult { activated_get_json!(req, store_cfg, ctx_cfg, handlers::config::get_body) }
    );

    register!(
        server,
        "/api/config",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );

    let store_wifi = std::sync::Arc::clone(&config_store);
    let ctx_config_wifi = Arc::clone(&ctx);
    register!(
        server,
        "/api/config/wifi",
        Method::Post,
        move |mut req| -> HandlerResult {
            require_pairing_code!(req, store_wifi);
            let body = read_body_utf8!(req, POST_BODY_MAX_LEN, store_wifi);
            let r = handlers::config::post_wifi(ctx_config_wifi.as_ref(), &body).map_err(to_io)?;
            let should_restart = r.status == 200 && common::restart_requested_from_uri(req.uri());
            write_api_resp!(req, r)?;
            if should_restart {
                std::thread::spawn(|| {
                    std::thread::sleep(Duration::from_millis(300));
                    unsafe { esp_idf_svc::sys::esp_restart() };
                });
            }
            Ok(())
        }
    );

    register!(
        server,
        "/api/config/wifi",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );

    let store_llm = std::sync::Arc::clone(&config_store);
    let ctx_llm = Arc::clone(&ctx);
    register!(
        server,
        "/api/config/llm",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );
    register!(
        server,
        "/api/config/llm",
        Method::Post,
        move |mut req| -> HandlerResult {
            if req.method() == Method::Options {
                return resp_options!(req);
            }
            require_pairing_code!(req, store_llm);
            let body = read_body_utf8!(req, POST_BODY_MAX_LEN, store_llm);
            let r = handlers::config::post_llm(ctx_llm.as_ref(), &body).map_err(to_io)?;
            write_api_resp!(req, r)
        }
    );

    let store_channels = std::sync::Arc::clone(&config_store);
    let ctx_channels = Arc::clone(&ctx);
    register!(
        server,
        "/api/config/channels",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );
    register!(
        server,
        "/api/config/channels",
        Method::Post,
        move |mut req| -> HandlerResult {
            if req.method() == Method::Options {
                return resp_options!(req);
            }
            require_pairing_code!(req, store_channels);
            let body = read_body_utf8!(req, POST_BODY_MAX_LEN, store_channels);
            let r = handlers::config::post_channels(ctx_channels.as_ref(), &body).map_err(to_io)?;
            write_api_resp!(req, r)
        }
    );

    let store_system = std::sync::Arc::clone(&config_store);
    let ctx_system = Arc::clone(&ctx);
    register!(
        server,
        "/api/config/system",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );
    register!(
        server,
        "/api/config/system",
        Method::Post,
        move |mut req| -> HandlerResult {
            if req.method() == Method::Options {
                return resp_options!(req);
            }
            require_pairing_code!(req, store_system);
            let body = read_body_utf8!(req, POST_BODY_MAX_LEN, store_system);
            let r = handlers::config::post_system(ctx_system.as_ref(), &body).map_err(to_io)?;
            write_api_resp!(req, r)
        }
    );

    // ── /api/config/hardware ──
    let store_hw_get = std::sync::Arc::clone(&config_store);
    let ctx_hw_get = Arc::clone(&ctx);
    register!(
        server,
        "/api/config/hardware",
        Method::Get,
        move |req| -> HandlerResult { activated_get_json!(req, store_hw_get, ctx_hw_get, handlers::config::get_hardware_body) }
    );
    register!(
        server,
        "/api/config/hardware",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );
    let store_hw_post = std::sync::Arc::clone(&config_store);
    let ctx_hw_post = Arc::clone(&ctx);
    register!(
        server,
        "/api/config/hardware",
        Method::Post,
        move |mut req| -> HandlerResult {
            if req.method() == Method::Options {
                return resp_options!(req);
            }
            require_pairing_code!(req, store_hw_post);
            let body = read_body_utf8!(req, POST_BODY_MAX_LEN, store_hw_post);
            let r = handlers::config::post_hardware(ctx_hw_post.as_ref(), &body).map_err(to_io)?;
            write_api_resp!(req, r)
        }
    );

    let ctx_wifi_scan = Arc::clone(&ctx);
    register!(
        server,
        "/api/wifi/scan",
        Method::Get,
        move |req| -> HandlerResult {
            match handlers::wifi_scan::get_body(ctx_wifi_scan.as_ref()) {
                Ok(body) => write_json_200!(req, body),
                Err(handlers::wifi_scan::WifiScanError::Unavailable) => {
                    let body = serde_json::json!({ "error": "wifi scan not available (non-ESP or wifi not ready)" }).to_string();
                    write_response!(req, 503, "Service Unavailable", CORS_HEADERS, body.as_bytes())
                }
                Err(handlers::wifi_scan::WifiScanError::Other(e)) => {
                    let body = serde_json::json!({ "error": e.to_string() }).to_string();
                    write_response!(req, 500, "Internal Server Error", CORS_HEADERS, body.as_bytes())
                }
            }
        }
    );
    register!(
        server,
        "/api/wifi/scan",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );

    let store_health = std::sync::Arc::clone(&config_store);
    let ctx_health = Arc::clone(&ctx);
    register!(
        server,
        "/api/health",
        Method::Get,
        move |req| -> HandlerResult {
            require_activated!(req, store_health);
            match handlers::health::body(ctx_health.as_ref()) {
                Ok(body) => {
                    let mut resp = req
                        .into_response(200, Some("OK"), CORS_HEADERS)
                        .map_err(to_io)?;
                    resp.write_all(body.as_bytes()).map_err(to_io)?;
                }
                Err(_) => {
                    let locale = crate::config::get_locale(store_health.as_ref());
                    let msg = crate::platform::http_server::user_message::from_api_key(
                        "operation_failed",
                        &locale,
                    );
                    let r = ApiResponse::err_500(&msg);
                    write_api_resp!(req, r)?;
                }
            }
            Ok(())
        }
    );

    register!(
        server,
        "/api/health",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );

    let store_metrics = std::sync::Arc::clone(&config_store);
    let ctx_metrics = Arc::clone(&ctx);
    register!(
        server,
        "/api/metrics",
        Method::Get,
        move |req| -> HandlerResult { activated_get_json!(req, store_metrics, ctx_metrics, handlers::metrics::body) }
    );
    register!(
        server,
        "/api/metrics",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );

    let store_resource = std::sync::Arc::clone(&config_store);
    let ctx_resource = Arc::clone(&ctx);
    register!(
        server,
        "/api/resource",
        Method::Get,
        move |req| -> HandlerResult { activated_get_json!(req, store_resource, ctx_resource, handlers::resource::body) }
    );
    register!(
        server,
        "/api/resource",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );

    let store_diag = std::sync::Arc::clone(&config_store);
    let ctx_diag = Arc::clone(&ctx);
    register!(
        server,
        "/api/diagnose",
        Method::Get,
        move |req| -> HandlerResult { activated_get_json!(req, store_diag, ctx_diag, handlers::diagnose::body) }
    );

    register!(
        server,
        "/api/diagnose",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );

    let store_sysinfo = std::sync::Arc::clone(&config_store);
    let ctx_sysinfo = Arc::clone(&ctx);
    register!(
        server,
        "/api/system_info",
        Method::Get,
        move |req| -> HandlerResult { activated_get_json!(req, store_sysinfo, ctx_sysinfo, handlers::system_info::body) }
    );
    register!(
        server,
        "/api/system_info",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );

    let store_conn = std::sync::Arc::clone(&config_store);
    let ctx_conn = Arc::clone(&ctx);
    register!(
        server,
        "/api/channel_connectivity",
        Method::Get,
        move |req| -> HandlerResult {
            require_activated!(req, store_conn);
            match handlers::channel_connectivity::body(ctx_conn.as_ref()) {
                Ok(body) => write_json_200!(req, body),
                Err(msg) => {
                    let body = format!(r#"{{"error":"{}"}}"#, msg.replace('"', "\\\""));
                    let mut resp = req
                        .into_response(500, Some("Internal Server Error"), CORS_HEADERS)
                        .map_err(to_io)?;
                    resp.write_all(body.as_bytes()).map_err(to_io)?;
                    Ok(())
                }
            }
        }
    );
    register!(
        server,
        "/api/channel_connectivity",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );

    register!(
        server,
        "/api/soul",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );
    register!(
        server,
        "/api/user",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );

    let store_sess = std::sync::Arc::clone(&config_store);
    let ctx_sess = Arc::clone(&ctx);
    register!(
        server,
        "/api/sessions",
        Method::Get,
        move |req| -> HandlerResult {
            require_activated!(req, store_sess);
            // ?chat_id=xxx returns detail; no param returns list.
            let chat_id = common::name_from_uri(req.uri())
                .or_else(|| {
                    let uri = req.uri();
                    let query = uri.find('?').map(|i| &uri[i + 1..]).unwrap_or("");
                    for pair in query.split('&') {
                        let mut it = pair.splitn(2, '=');
                        if it.next().map_or(false, |k| k.eq_ignore_ascii_case("chat_id")) {
                            return it.next().filter(|s| !s.is_empty()).map(String::from);
                        }
                    }
                    None
                });
            let result = match chat_id {
                Some(id) => handlers::sessions::detail(ctx_sess.as_ref(), &id),
                None => handlers::sessions::body(ctx_sess.as_ref()),
            };
            match result {
                Ok(body) => write_json_200!(req, body),
                Err(msg) => {
                    let body = format!(r#"{{"error":"{}"}}"#, msg.replace('"', "\\\""));
                    write_response!(req, 500, "Internal Server Error", CORS_HEADERS, body.as_bytes())
                }
            }
        }
    );

    let store_sess_del = std::sync::Arc::clone(&config_store);
    let ctx_sess_del = Arc::clone(&ctx);
    register!(
        server,
        "/api/sessions",
        Method::Delete,
        move |req| -> HandlerResult {
            require_pairing_code!(req, store_sess_del);
            let chat_id = {
                let uri = req.uri();
                let query = uri.find('?').map(|i| &uri[i + 1..]).unwrap_or("");
                let mut found = None;
                for pair in query.split('&') {
                    let mut it = pair.splitn(2, '=');
                    if it.next().map_or(false, |k| k.eq_ignore_ascii_case("chat_id")) {
                        found = it.next().filter(|s| !s.is_empty()).map(String::from);
                        break;
                    }
                }
                found
            };
            match chat_id {
                Some(id) => {
                    match handlers::sessions::delete(ctx_sess_del.as_ref(), &id) {
                        Ok(body) => write_json_200!(req, body),
                        Err(msg) => {
                            let r = ApiResponse::err_500(&msg);
                            write_api_resp!(req, r)
                        }
                    }
                }
                None => {
                    let r = ApiResponse::err_400("missing chat_id query param");
                    write_api_resp!(req, r)
                }
            }
        }
    );

    register!(
        server,
        "/api/sessions",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );

    let store_mem = std::sync::Arc::clone(&config_store);
    let ctx_mem = Arc::clone(&ctx);
    register!(
        server,
        "/api/memory/status",
        Method::Get,
        move |req| -> HandlerResult {
            require_activated!(req, store_mem);
            let body = handlers::memory::body(ctx_mem.as_ref());
            let mut resp = req
                .into_response(200, Some("OK"), CORS_HEADERS)
                .map_err(to_io)?;
            resp.write_all(body.as_bytes()).map_err(to_io)?;
            Ok(())
        }
    );

    register!(
        server,
        "/api/memory/status",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );

    let ctx_skills_get = Arc::clone(&ctx);
    let store_skills_get = std::sync::Arc::clone(&config_store);
    register!(
        server,
        "/api/skills",
        Method::Get,
        move |req| -> HandlerResult {
            require_activated!(req, store_skills_get);
            let name = name_from_uri(req.uri());
            match handlers::skills::get(ctx_skills_get.as_ref(), name) {
                Ok(handlers::skills::SkillsGetResult::TextPlain(s)) => write_text_200!(req, s),
                Ok(handlers::skills::SkillsGetResult::Json(s)) => write_json_200!(req, s),
                Err(r) => write_api_resp!(req, r),
            }
        }
    );

    let ctx_skills_post = Arc::clone(&ctx);
    let store_skills_post = std::sync::Arc::clone(&config_store);
    register!(
        server,
        "/api/skills",
        Method::Post,
        move |mut req| -> HandlerResult {
            require_pairing_code!(req, store_skills_post);
            let body = read_body_utf8!(req, POST_BODY_MAX_LEN, store_skills_post);
            let r = handlers::skills::post(ctx_skills_post.as_ref(), &body);
            write_api_resp!(req, r)
        }
    );

    let ctx_skills_del = Arc::clone(&ctx);
    let store_skills_del = std::sync::Arc::clone(&config_store);
    register!(
        server,
        "/api/skills",
        Method::Delete,
        move |req| -> HandlerResult {
            require_pairing_code!(req, store_skills_del);
            let name = match name_from_uri(req.uri()) {
                Some(n) => n,
                None => {
                    let locale = crate::config::get_locale(store_skills_del.as_ref());
                    let msg = crate::platform::http_server::user_message::from_api_key(
                        "missing_name_query",
                        &locale,
                    );
                    let r = ApiResponse::err_400(&msg);
                    return write_api_resp!(req, r);
                }
            };
            let r = handlers::skills::delete(ctx_skills_del.as_ref(), &name);
            write_api_resp!(req, r)
        }
    );

    let ctx_skills_import = Arc::clone(&ctx);
    let store_skills_import = std::sync::Arc::clone(&config_store);
    register!(
        server,
        "/api/skills/import",
        Method::Post,
        move |mut req| -> HandlerResult {
            require_pairing_code!(req, store_skills_import);
            let body = read_body_utf8!(req, POST_BODY_MAX_LEN, store_skills_import);
            let r = handlers::skills::import(ctx_skills_import.as_ref(), &body).map_err(to_io)?;
            write_api_resp!(req, r)
        }
    );

    register!(
        server,
        "/api/skills/import",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );
    register!(
        server,
        "/api/skills",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );

    let store_soul = std::sync::Arc::clone(&config_store);
    let ctx_soul = Arc::clone(&ctx);
    register!(
        server,
        "/api/soul",
        Method::Get,
        move |req| -> HandlerResult {
            require_activated!(req, store_soul);
            match handlers::soul::get_body(ctx_soul.as_ref()) {
                Ok(content) => write_text_200!(req, content),
                Err(_) => {
                    let locale = crate::config::get_locale(store_soul.as_ref());
                    let msg = crate::platform::http_server::user_message::from_api_key(
                        "operation_failed",
                        &locale,
                    );
                    write_api_resp!(req, ApiResponse::err_500(&msg))
                }
            }
        }
    );

    let store_user = std::sync::Arc::clone(&config_store);
    let ctx_user = Arc::clone(&ctx);
    register!(
        server,
        "/api/user",
        Method::Get,
        move |req| -> HandlerResult {
            require_activated!(req, store_user);
            match handlers::user::get_body(ctx_user.as_ref()) {
                Ok(content) => write_text_200!(req, content),
                Err(_) => {
                    let locale = crate::config::get_locale(store_user.as_ref());
                    let msg = crate::platform::http_server::user_message::from_api_key(
                        "operation_failed",
                        &locale,
                    );
                    write_api_resp!(req, ApiResponse::err_500(&msg))
                }
            }
        }
    );

    let store_soul_post = std::sync::Arc::clone(&config_store);
    let ctx_soul_post = Arc::clone(&ctx);
    register!(
        server,
        "/api/soul",
        Method::Post,
        move |mut req| -> HandlerResult {
            require_pairing_code!(req, store_soul_post);
            let is_json = req
                .header("Content-Type")
                .map(|ct| ct.contains("application/json"))
                .unwrap_or(false);
            let body = read_body_utf8!(req, crate::memory::MAX_SOUL_USER_LEN, store_soul_post);
            let r = handlers::soul::post(ctx_soul_post.as_ref(), body, is_json);
            write_api_resp!(req, r)
        }
    );

    let store_user_post = std::sync::Arc::clone(&config_store);
    let ctx_user_post = Arc::clone(&ctx);
    register!(
        server,
        "/api/user",
        Method::Post,
        move |mut req| -> HandlerResult {
            require_pairing_code!(req, store_user_post);
            let is_json = req
                .header("Content-Type")
                .map(|ct| ct.contains("application/json"))
                .unwrap_or(false);
            let body = read_body_utf8!(req, crate::memory::MAX_SOUL_USER_LEN, store_user_post);
            let r = handlers::user::post(ctx_user_post.as_ref(), body, is_json);
            write_api_resp!(req, r)
        }
    );

    let store_restart = std::sync::Arc::clone(&config_store);
    let ctx_restart = Arc::clone(&ctx);
    register!(
        server,
        "/api/restart",
        Method::Post,
        move |req| -> HandlerResult {
            require_pairing_code!(req, store_restart);
            let (r, do_restart) = handlers::restart::post(ctx_restart.as_ref()).map_err(to_io)?;
            write_api_resp!(req, r)?;
            if do_restart {
                std::thread::spawn(|| {
                    std::thread::sleep(Duration::from_millis(300));
                    unsafe { esp_idf_svc::sys::esp_restart() };
                });
            }
            Ok(())
        }
    );

    register!(
        server,
        "/api/restart",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );

    let store_reset = std::sync::Arc::clone(&config_store);
    let ctx_reset = Arc::clone(&ctx);
    register!(
        server,
        "/api/config_reset",
        Method::Post,
        move |req| -> HandlerResult {
            require_pairing_code!(req, store_reset);
            let r = handlers::config_reset::post(ctx_reset.as_ref()).map_err(to_io)?;
            write_api_resp!(req, r)
        }
    );

    register!(
        server,
        "/api/config_reset",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );

    let webhook_tx = inbound_tx.clone();
    let ctx_webhook = Arc::clone(&ctx);
    let store_webhook = std::sync::Arc::clone(&config_store);
    register!(
        server,
        "/api/webhook",
        Method::Post,
        move |mut req| -> HandlerResult {
            require_pairing_code!(req, store_webhook);
            let body_str = read_body_utf8!(req, POST_BODY_MAX_LEN, store_webhook);
            let token = req
                .header("X-Webhook-Token")
                .or_else(|| req.header("x-webhook-token"))
                .or_else(|| token_from_uri(req.uri()));
            let provided = token.unwrap_or("");
            let r = handlers::webhook::post(ctx_webhook.as_ref(), &webhook_tx, body_str, provided)
                .map_err(to_io)?;
            write_api_resp!(req, r)
        }
    );

    register!(
        server,
        "/api/webhook",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );

    let feishu_tx = inbound_tx.clone();
    let ctx_feishu = Arc::clone(&ctx);
    let store_feishu = std::sync::Arc::clone(&config_store);
    register!(
        server,
        "/api/feishu/event",
        Method::Post,
        move |mut req| -> HandlerResult {
            let body_str = read_body_utf8!(req, FEISHU_EVENT_BODY_MAX, store_feishu);
            let r = handlers::feishu_event::post(ctx_feishu.as_ref(), &feishu_tx, &body_str)
                .map_err(to_io)?;
            write_api_resp!(req, r)
        }
    );
    register!(
        server,
        "/api/feishu/event",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );

    // --- DingTalk webhook inbound ---
    let dingtalk_tx = inbound_tx.clone();
    let store_dingtalk_wh = std::sync::Arc::clone(&config_store);
    register!(
        server,
        "/api/dingtalk/webhook",
        Method::Post,
        move |mut req| -> HandlerResult {
            let body_str = read_body_utf8!(req, POST_BODY_MAX_LEN, store_dingtalk_wh);
            let r = handlers::dingtalk_webhook::post(&dingtalk_tx, &body_str)
                .map_err(to_io)?;
            write_api_resp!(req, r)
        }
    );
    register!(
        server,
        "/api/dingtalk/webhook",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );

    // --- WeCom webhook inbound ---
    register!(
        server,
        "/api/wecom/webhook",
        Method::Get,
        move |req| -> HandlerResult {
            let r = handlers::wecom_webhook::get_verify(req.uri());
            write_response!(req, r.status, r.status_text, CORS_HEADERS, &r.body)
        }
    );
    let ctx_wecom_wh_post = Arc::clone(&ctx);
    let store_wecom_wh_post = std::sync::Arc::clone(&config_store);
    let wecom_tx_post = inbound_tx.clone();
    register!(
        server,
        "/api/wecom/webhook",
        Method::Post,
        move |mut req| -> HandlerResult {
            let body_str = read_body_utf8!(req, POST_BODY_MAX_LEN, store_wecom_wh_post);
            let r = handlers::wecom_webhook::post(ctx_wecom_wh_post.as_ref(), &wecom_tx_post, &body_str)
                .map_err(to_io)?;
            write_api_resp!(req, r)
        }
    );
    register!(
        server,
        "/api/wecom/webhook",
        Method::Options,
        |req| -> HandlerResult { resp_options!(req) }
    );

    let config_for_qq =
        AppConfig::load(config_store.as_ref(), Some(config_file_store.as_ref()));
    if !config_for_qq.qq_channel_app_id.trim().is_empty()
        && !config_for_qq.qq_channel_secret.trim().is_empty()
    {
        let store_qq = std::sync::Arc::clone(&config_store);
        let qq_tx = inbound_tx.clone();
        let qq_cache = Arc::clone(&msg_id_cache);
        let qq_app_id = config_for_qq.qq_channel_app_id.clone();
        let qq_secret = config_for_qq.qq_channel_secret.clone();
        register!(
            server,
            "/api/webhook/qq",
            Method::Post,
            move |mut req| -> HandlerResult {
                let max_len = req
                    .content_len()
                    .map(|u| u.min(crate::channels::QQ_WEBHOOK_BODY_MAX as u64) as usize)
                    .unwrap_or(crate::channels::QQ_WEBHOOK_BODY_MAX);
                let mut buf = vec![0u8; max_len];
                let n = match req.read(&mut buf) {
                    Ok(n) => n,
                    Err(_) => {
                        let locale = crate::config::get_locale(store_qq.as_ref());
                        let msg = crate::platform::http_server::user_message::from_api_key(
                            "body_read_failed",
                            &locale,
                        );
                        let r = ApiResponse::err_500(&msg);
                        return write_api_resp!(req, r);
                    }
                };
                buf.truncate(n);
                let ts = req
                    .header("X-Signature-Timestamp")
                    .or_else(|| req.header("x-signature-timestamp"));
                let sig = req
                    .header("X-Signature-Ed25519")
                    .or_else(|| req.header("x-signature-ed25519"));
                match handlers::qq_webhook::post(
                    store_qq.as_ref(),
                    &buf,
                    ts,
                    sig,
                    &qq_app_id,
                    &qq_secret,
                    &qq_tx,
                    Arc::clone(&qq_cache),
                ) {
                    Ok(handlers::qq_webhook::QqWebhookOutcome::UrlVerification {
                        plain_token,
                        signature,
                    }) => {
                        let body = serde_json::json!({
                            "plain_token": plain_token,
                            "signature": signature
                        });
                        let body_bytes = body.to_string().into_bytes();
                        write_response!(req, 200, "OK", CORS_HEADERS, &body_bytes)
                    }
                    Ok(handlers::qq_webhook::QqWebhookOutcome::EventHandled) => {
                        write_response!(req, 200, "OK", CORS_HEADERS, &[])
                    }
                    Err(r) => write_api_resp!(req, r),
                }
            }
        );
    }

    #[cfg(feature = "ota")]
    {
        let store_ota = std::sync::Arc::clone(&config_store);
        let ctx_ota = Arc::clone(&ctx);
        let store_ota_check = std::sync::Arc::clone(&config_store);
        let ctx_ota_check = Arc::clone(&ctx);
        register!(
            server,
            "/api/ota/check",
            Method::Get,
            move |req| -> HandlerResult {
                require_activated!(req, store_ota_check);
                let channel = common::channel_from_uri(req.uri());
                let body =
                    handlers::ota::get_check(ctx_ota_check.as_ref(), &channel).map_err(to_io)?;
                let mut resp = req
                    .into_response(200, Some("OK"), CORS_HEADERS)
                    .map_err(to_io)?;
                resp.write_all(body.as_bytes()).map_err(to_io)?;
                Ok(())
            }
        );
        register!(
            server,
            "/api/ota/check",
            Method::Options,
            |req| -> HandlerResult { resp_options!(req) }
        );
        register!(
            server,
            "/api/ota",
            Method::Post,
            move |mut req| -> HandlerResult {
                require_pairing_code!(req, store_ota);
                let body = read_body_utf8!(req, POST_BODY_MAX_LEN, store_ota);
                let (r, do_restart) =
                    handlers::ota::post(ctx_ota.as_ref(), &body).map_err(to_io)?;
                write_api_resp!(req, r)?;
                if do_restart {
                    std::thread::spawn(|| {
                        std::thread::sleep(Duration::from_millis(300));
                        unsafe { esp_idf_svc::sys::esp_restart() };
                    });
                }
                Ok(())
            }
        );
        register!(
            server,
            "/api/ota",
            Method::Options,
            |req| -> HandlerResult { resp_options!(req) }
        );
    }

    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub fn run(
    _platform: std::sync::Arc<dyn crate::platform::Platform>,
    _wifi_connected: bool,
    _inbound_depth: Arc<std::sync::atomic::AtomicUsize>,
    _outbound_depth: Arc<std::sync::atomic::AtomicUsize>,
    _memory_store: Arc<dyn crate::memory::MemoryStore + Send + Sync>,
    _session_store: Arc<dyn crate::memory::SessionStore + Send + Sync>,
    _inbound_tx: crate::bus::InboundTx,
    _msg_id_cache: crate::channels::QqMsgIdCache,
) -> Result<()> {
    Ok(())
}
