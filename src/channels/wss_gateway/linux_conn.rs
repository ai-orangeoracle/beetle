//! Linux / host：基于 `tungstenite` + rustls 的 WSS 客户端，实现 `WssConnection`。
//! 与 `esp_conn` 事件语义对齐：Binary/Text → `WssEvent::Binary`，Close/错误 → Disconnected。
//!
//! 读线程独占 `Arc<Mutex<WebSocket>>`；`TcpStream::set_read_timeout` 与 `loop.rs` 中 `WDT_RECV_CHUNK_SECS`
//! 同量级。`Drop` 时 shutdown TCP 以结束阻塞的 `read()`，再 `join` 读线程。

#![cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]

use std::io::ErrorKind;
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::channels::wss_gateway::connection::{
    WssConnection, WssEvent, MAX_WSS_SEND_PAYLOAD_BYTES,
};
use crate::error::{Error, Result};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{client::connect, protocol::Message, WebSocket};

/// 与 `esp_conn` 一致。
const WSS_TLS_ADMISSION_TIMEOUT_SECS: u64 = 10;
/// 与 `wss_gateway/loop.rs` 中 `WDT_RECV_CHUNK_SECS` 一致。
const SOCKET_READ_TIMEOUT_SECS: u64 = 25;

fn map_io(stage: &'static str, e: std::io::Error) -> Error {
    Error::Other {
        source: Box::new(e),
        stage,
    }
}

fn map_tungstenite(stage: &'static str, e: tungstenite::Error) -> Error {
    Error::Other {
        source: Box::new(std::io::Error::other(e.to_string())),
        stage,
    }
}

fn set_tcp_read_timeout(
    stream: &mut MaybeTlsStream<TcpStream>,
    d: Option<Duration>,
) -> std::io::Result<()> {
    match stream {
        MaybeTlsStream::Plain(s) => s.set_read_timeout(d),
        MaybeTlsStream::Rustls(s) => s.sock.set_read_timeout(d),
        _ => Err(std::io::Error::new(
            ErrorKind::Unsupported,
            "unexpected MaybeTlsStream variant for WSS",
        )),
    }
}

fn shutdown_tcp(stream: &mut MaybeTlsStream<TcpStream>) {
    let r = match stream {
        MaybeTlsStream::Plain(s) => s.shutdown(Shutdown::Both),
        MaybeTlsStream::Rustls(s) => s.sock.shutdown(Shutdown::Both),
        _ => Ok(()),
    };
    if let Err(e) = r {
        log::debug!("[wss_linux] tcp shutdown: {}", e);
    }
}

fn spawn_reader_thread(
    ws: Arc<Mutex<WebSocket<MaybeTlsStream<TcpStream>>>>,
    shutdown: Arc<AtomicBool>,
    tx: mpsc::SyncSender<WssEvent>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        while !shutdown.load(Ordering::SeqCst) {
            let msg = {
                let mut g = match ws.lock() {
                    Ok(g) => g,
                    Err(e) => {
                        log::warn!("[wss_linux] ws mutex poisoned: {}", e);
                        let _ = tx.try_send(WssEvent::Disconnected);
                        return;
                    }
                };
                g.read()
            };
            match msg {
                Ok(Message::Binary(b)) => {
                    if tx.try_send(WssEvent::Binary(b)).is_err() {
                        log::warn!("[wss_linux] event channel full, dropping binary frame");
                    }
                }
                Ok(Message::Text(t)) => {
                    if tx.try_send(WssEvent::Binary(t.into_bytes())).is_err() {
                        log::warn!("[wss_linux] event channel full, dropping text frame");
                    }
                }
                Ok(Message::Ping(payload)) => {
                    // RFC 6455：必须回复 Pong（与 ESP 侧 IDF 客户端 ping/pong 行为对齐）。
                    let mut g = match ws.lock() {
                        Ok(g) => g,
                        Err(e) => {
                            log::warn!("[wss_linux] ws mutex poisoned (ping): {}", e);
                            let _ = tx.try_send(WssEvent::Disconnected);
                            return;
                        }
                    };
                    if let Err(e) = g.send(Message::Pong(payload)) {
                        log::debug!("[wss_linux] pong reply failed: {}", e);
                    }
                }
                Ok(Message::Pong(_)) => {}
                Ok(Message::Close(_)) => {
                    let _ = tx.try_send(WssEvent::Closed);
                    break;
                }
                Ok(Message::Frame(_)) => {}
                Err(e) => {
                    if is_timed_out_or_would_block(&e) {
                        continue;
                    }
                    log::debug!("[wss_linux] read ended: {}", e);
                    let _ = tx.try_send(WssEvent::Disconnected);
                    break;
                }
            }
        }
    })
}

fn is_timed_out_or_would_block(e: &tungstenite::Error) -> bool {
    match e {
        tungstenite::Error::Io(io) => {
            matches!(io.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
        }
        _ => false,
    }
}

/// Linux 上基于 tungstenite 的 WSS 连接。
pub struct LinuxWssConnection {
    rx: mpsc::Receiver<WssEvent>,
    ws: Arc<Mutex<WebSocket<MaybeTlsStream<TcpStream>>>>,
    shutdown: Arc<AtomicBool>,
    reader: Option<JoinHandle<()>>,
}

impl LinuxWssConnection {
    fn recv_to_event(
        r: std::result::Result<WssEvent, mpsc::RecvTimeoutError>,
    ) -> Result<Option<WssEvent>> {
        match r {
            Ok(ev) => Ok(Some(ev)),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(Error::Other {
                source: Box::new(std::io::Error::new(
                    ErrorKind::ConnectionReset,
                    "wss event channel disconnected",
                )),
                stage: "wss_linux_recv",
            }),
        }
    }
}

impl Drop for LinuxWssConnection {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Ok(mut g) = self.ws.lock() {
            shutdown_tcp(g.get_mut());
        }
        if let Some(h) = self.reader.take() {
            let _ = h.join();
        }
    }
}

impl WssConnection for LinuxWssConnection {
    fn send_binary(&mut self, data: &[u8]) -> Result<()> {
        if data.len() > MAX_WSS_SEND_PAYLOAD_BYTES {
            return Err(Error::Other {
                source: Box::new(std::io::Error::new(
                    ErrorKind::InvalidInput,
                    format!(
                        "wss payload too large: {} > {}",
                        data.len(),
                        MAX_WSS_SEND_PAYLOAD_BYTES
                    ),
                )),
                stage: "wss_linux_send",
            });
        }
        let mut g = self.ws.lock().map_err(|e| Error::Other {
            source: Box::new(std::io::Error::other(e.to_string())),
            stage: "wss_linux_send",
        })?;
        g.send(Message::Binary(data.to_vec()))
            .map_err(|e| map_tungstenite("wss_linux_send", e))?;
        Ok(())
    }

    fn recv_timeout(&mut self, timeout: Duration) -> Result<Option<WssEvent>> {
        Self::recv_to_event(self.rx.recv_timeout(timeout))
    }
}

/// 建立 WSS 连接（`wss://`）；与 ESP 相同在握手前申请 orchestrator TLS 准入。
pub fn connect_linux_wss(url: &str) -> Result<LinuxWssConnection> {
    let _permit = crate::orchestrator::request_http_permit(
        crate::orchestrator::Priority::Normal,
        Duration::from_secs(WSS_TLS_ADMISSION_TIMEOUT_SECS),
    )?;

    let (mut ws, _resp) = connect(url).map_err(|e| map_tungstenite("wss_linux_connect", e))?;

    set_tcp_read_timeout(
        ws.get_mut(),
        Some(Duration::from_secs(SOCKET_READ_TIMEOUT_SECS)),
    )
    .map_err(|e| map_io("wss_linux_connect", e))?;

    let ws = Arc::new(Mutex::new(ws));
    let shutdown = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::sync_channel::<WssEvent>(32);
    let reader = spawn_reader_thread(Arc::clone(&ws), Arc::clone(&shutdown), tx);

    Ok(LinuxWssConnection {
        rx,
        ws,
        shutdown,
        reader: Some(reader),
    })
}
