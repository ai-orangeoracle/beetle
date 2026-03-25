//! ESP 平台 WSS 传输：`esp-idf-svc::ws::client`（EspWebSocketClient）+ 事件 channel，实现 `WssConnection`。
//! 仅 xtensa/riscv32 编译；供飞书/QQ WSS 入站共用。
//! 与根 `Cargo.toml` 中 `[package.metadata.esp-idf-sys] extra_components` 的 `espressif/esp_websocket_client` 配套；相对「仅 `platform/` 依赖 esp-idf-svc」为**固定例外**（见架构文档）。

#![cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]

use crate::channels::wss_gateway::connection::{
    WssConnection, WssEvent, DEFAULT_WSS_BUFFER_SIZE, MAX_WSS_SEND_PAYLOAD_BYTES,
};
use crate::error::{Error, Result};
use esp_idf_svc::handle::RawHandle;
use esp_idf_svc::ws::client::WebSocketEventType;
use std::mem::ManuallyDrop;
use std::sync::mpsc;
use std::time::Duration;

const CONNECT_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_WS_TIMEOUT_MS: u64 = 30_000;
/// pingpong 超时：配合 TCP keep-alive 更快发现死连接（原 120s 太慢）。
const DEFAULT_PINGPONG_TIMEOUT_SEC: u64 = 60;
/// TCP keep-alive：30s idle + 3*10s probe = 最慢 60s 检测到死连接。
const KEEPALIVE_IDLE_SECS: u64 = 30;
const KEEPALIVE_INTERVAL_SECS: u64 = 10;
const KEEPALIVE_COUNT: u16 = 3;
/// close 超时 tick（FreeRTOS tick = 10ms, 200 ticks = 2s）。
const CLOSE_TIMEOUT_TICKS: u32 = 200;

// ---- C API 声明 ----
// `esp_websocket_client_handle_t` 是 `*mut esp_websocket_client`（不透明），
// `esp-idf-svc` 内部可见但未 re-export，我们通过 `RawHandle::handle()` 拿到后用 *mut c_void 桥接。
extern "C" {
    fn esp_websocket_client_close(client: *mut core::ffi::c_void, timeout: u32) -> i32;
    fn esp_websocket_client_destroy(client: *mut core::ffi::c_void) -> i32;
}

/// ESP 上的 WSS 连接。
///
/// `disable_auto_reconnect: true` 禁止 C 底层自动重连（否则它绕过 TLS 准入做 TLS 握手）。
/// 但 Rust wrapper 的 Drop 对 close() 做了 unwrap()，客户端已停止时 close() 返回 ESP_FAIL 会 panic。
/// 因此用 `ManuallyDrop` 阻止 Rust wrapper Drop，自行通过 C API 安全释放。
pub struct EspWssConnection {
    client: ManuallyDrop<esp_idf_svc::ws::client::EspWebSocketClient<'static>>,
    /// 缓存的 C handle，用于自行 close + destroy。
    raw_handle: *mut core::ffi::c_void,
    rx: mpsc::Receiver<WssEvent>,
}

// SAFETY: EspWebSocketClient 内部已 Send，raw_handle 只在 Drop 中使用。
unsafe impl Send for EspWssConnection {}

impl Drop for EspWssConnection {
    fn drop(&mut self) {
        unsafe {
            let rc_close = esp_websocket_client_close(self.raw_handle, CLOSE_TIMEOUT_TICKS);
            if rc_close != 0 {
                log::warn!("[wss] esp_websocket_client_close rc={}", rc_close);
            }
            let rc_destroy = esp_websocket_client_destroy(self.raw_handle);
            if rc_destroy != 0 {
                log::warn!("[wss] esp_websocket_client_destroy rc={}", rc_destroy);
            }
        }
        // 不调用 ManuallyDrop::drop —— Rust wrapper 的 callback Box (~100B) 会泄漏，
        // 相比 panic abort 这是可接受的代价。
    }
}

impl EspWssConnection {
    fn recv_to_event(
        r: std::result::Result<WssEvent, mpsc::RecvTimeoutError>,
    ) -> Result<Option<WssEvent>> {
        match r {
            Ok(ev) => Ok(Some(ev)),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(Error::Other {
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::ConnectionReset,
                    "wss event channel disconnected",
                )),
                stage: "wss_esp_recv",
            }),
        }
    }
}

impl WssConnection for EspWssConnection {
    fn send_binary(&mut self, data: &[u8]) -> Result<()> {
        if data.len() > MAX_WSS_SEND_PAYLOAD_BYTES {
            return Err(Error::Other {
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "wss payload too large: {} > {}",
                        data.len(),
                        MAX_WSS_SEND_PAYLOAD_BYTES
                    ),
                )),
                stage: "wss_esp_send",
            });
        }
        // esp-idf-svc：仅 `Binary(false)` / `Text(false)` 走整帧发送；`Binary(true)` 会 panic（fragmented）。
        log::debug!("wss send_binary len={}", data.len());
        self.client
            .send(esp_idf_svc::ws::FrameType::Binary(false), data)
            .map_err(|e| Error::Other {
                source: Box::new(e),
                stage: "wss_esp_send",
            })
    }

    fn recv_timeout(&mut self, timeout: Duration) -> Result<Option<WssEvent>> {
        Self::recv_to_event(self.rx.recv_timeout(timeout))
    }
}

const WSS_TLS_ADMISSION_TIMEOUT_SECS: u64 = 10;

pub fn connect_esp_wss(url: &str) -> Result<EspWssConnection> {
    let _permit = crate::orchestrator::request_http_permit(
        crate::orchestrator::Priority::Normal,
        Duration::from_secs(WSS_TLS_ADMISSION_TIMEOUT_SECS),
    )?;

    // 与 `platform/http_client.rs` 一致：仅用 `crt_bundle_attach` 挂接证书包；勿与 `use_global_ca_store` 同时开启，
    // 否则 esp-tls 可能在校验阶段异常，表现为 CONNECTED 未到即 DISCONNECTED / 回调里 `WebSocketEvent::new` 失败。
    let config = esp_idf_svc::ws::client::EspWebSocketClientConfig {
        buffer_size: DEFAULT_WSS_BUFFER_SIZE,
        transport: esp_idf_svc::ws::client::EspWebSocketTransport::TransportOverSSL,
        use_global_ca_store: false,
        disable_auto_reconnect: true,
        #[cfg(not(esp_idf_version_major = "4"))]
        crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
        pingpong_timeout_sec: Duration::from_secs(DEFAULT_PINGPONG_TIMEOUT_SEC),
        network_timeout_ms: Duration::from_millis(DEFAULT_WS_TIMEOUT_MS),
        ping_interval_sec: Duration::from_secs(10),
        keep_alive_idle: Some(Duration::from_secs(KEEPALIVE_IDLE_SECS)),
        keep_alive_interval: Some(Duration::from_secs(KEEPALIVE_INTERVAL_SECS)),
        keep_alive_count: Some(KEEPALIVE_COUNT),
        ..Default::default()
    };
    let timeout = Duration::from_millis(CONNECT_TIMEOUT_MS);
    let (tx, rx) = mpsc::sync_channel::<WssEvent>(32);
    let tx = std::sync::Arc::new(std::sync::Mutex::new(tx));
    let tx_cb = tx.clone();
    let client = esp_idf_svc::ws::client::EspWebSocketClient::new(
        url,
        &config,
        timeout,
        move |ev_result: &std::result::Result<esp_idf_svc::ws::client::WebSocketEvent<'_>, _>| {
            let tx = match tx_cb.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            let event = match ev_result {
                Ok(ev) => match &ev.event_type {
                    WebSocketEventType::Binary(data) => Some(WssEvent::Binary(data.to_vec())),
                    WebSocketEventType::Text(data) => {
                        Some(WssEvent::Binary(data.as_bytes().to_vec()))
                    }
                    WebSocketEventType::Disconnected => Some(WssEvent::Disconnected),
                    WebSocketEventType::Closed | WebSocketEventType::Close(_) => {
                        Some(WssEvent::Closed)
                    }
                    WebSocketEventType::BeforeConnect | WebSocketEventType::Connected => None,
                    WebSocketEventType::Ping | WebSocketEventType::Pong => None,
                },
                Err(e) => {
                    log::warn!(
                        "[wss] websocket event decode/handshake error (mapping to disconnect): {}",
                        e
                    );
                    Some(WssEvent::Disconnected)
                }
            };
            if let Some(e) = event {
                if tx.try_send(e).is_err() {
                    log::warn!("[wss] event channel full, dropping event");
                }
            }
        },
    )
    .map_err(|e| Error::Other {
        source: Box::new(e),
        stage: "wss_esp_connect",
    })?;
    let raw_handle = client.handle() as *mut core::ffi::c_void;
    log::info!(
        "wss client started (handshake runs asynchronously), url_len={}",
        url.len()
    );
    Ok(EspWssConnection {
        client: ManuallyDrop::new(client),
        raw_handle,
        rx,
    })
}
