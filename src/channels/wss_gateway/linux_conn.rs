//! Linux / host：基于 `tungstenite` + rustls 的 WSS 客户端，实现 `WssConnection`。
//! 与 `esp_conn` 事件语义对齐：Binary/Text → `WssEvent::Binary`，Close/错误 → Disconnected。
//!
//! **不得**用单独读线程在 `read()` 持有 `Mutex` 的同时由主线程 `send`：QQ 等协议在 Hello 后需先发
//! Identify，服务端才会继续下帧；否则读线程永久占锁 → 与 `send_binary` 死锁。网关循环单线程交替
//! `recv_timeout` / `send_binary`，故此处直接在调用线程上读、写同一 `WebSocket`。
//!
//! `TcpStream::set_read_timeout` 单次等待上限与 `loop.rs` 中 `WDT_RECV_CHUNK_SECS` 同量级；
//! `recv_timeout` 用截止时间聚合多次短读，避免 Ping 处理或分片读越过调用方超时。

#![cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]

use std::io::ErrorKind;
use std::net::{Shutdown, TcpStream};
use std::time::{Duration, Instant};

use crate::channels::wss_gateway::connection::{
    WssConnection, WssEvent, MAX_WSS_SEND_PAYLOAD_BYTES,
};
use crate::error::{Error, Result};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{client::connect, protocol::Message, WebSocket};

/// 与 `esp_conn` 一致。
const WSS_TLS_ADMISSION_TIMEOUT_SECS: u64 = 10;
/// 与 `wss_gateway/loop.rs` 中 `WDT_RECV_CHUNK_SECS` 一致（单次 `read` 阻塞上限）。
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

fn is_timed_out_or_would_block(e: &tungstenite::Error) -> bool {
    match e {
        tungstenite::Error::Io(io) => {
            matches!(io.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
        }
        _ => false,
    }
}

/// Linux 上基于 tungstenite 的 WSS 连接（单线程读/写，与 `run_wss_gateway_loop` 用法一致）。
pub struct LinuxWssConnection {
    ws: WebSocket<MaybeTlsStream<TcpStream>>,
}

impl Drop for LinuxWssConnection {
    fn drop(&mut self) {
        shutdown_tcp(self.ws.get_mut());
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
        self.ws
            .send(Message::Binary(data.to_vec()))
            .map_err(|e| map_tungstenite("wss_linux_send", e))?;
        Ok(())
    }

    fn recv_timeout(&mut self, timeout: Duration) -> Result<Option<WssEvent>> {
        let deadline = Instant::now() + timeout;
        let chunk_cap = Duration::from_secs(SOCKET_READ_TIMEOUT_SECS);

        loop {
            let now = Instant::now();
            if now >= deadline {
                return Ok(None);
            }
            let remaining = deadline.saturating_duration_since(now);
            let read_wait = remaining.min(chunk_cap);
            if read_wait.is_zero() {
                return Ok(None);
            }
            set_tcp_read_timeout(self.ws.get_mut(), Some(read_wait))
                .map_err(|e| map_io("wss_linux_recv", e))?;

            match self.ws.read() {
                Ok(Message::Binary(b)) => return Ok(Some(WssEvent::Binary(b))),
                Ok(Message::Text(t)) => return Ok(Some(WssEvent::Binary(t.into_bytes()))),
                Ok(Message::Ping(payload)) => {
                    if let Err(e) = self.ws.send(Message::Pong(payload)) {
                        log::debug!("[wss_linux] pong reply failed: {}", e);
                    }
                    if let Err(e) = self.ws.flush() {
                        log::debug!("[wss_linux] flush after pong failed: {}", e);
                    }
                }
                Ok(Message::Pong(_)) => {}
                Ok(Message::Close(_)) => return Ok(Some(WssEvent::Closed)),
                Ok(Message::Frame(_)) => {}
                Err(e) if is_timed_out_or_would_block(&e) => continue,
                Err(e @ tungstenite::Error::ConnectionClosed) => {
                    log::debug!("[wss_linux] read ended: {}", e);
                    return Ok(Some(WssEvent::Closed));
                }
                Err(e @ tungstenite::Error::AlreadyClosed) => {
                    log::debug!("[wss_linux] read ended: {}", e);
                    return Ok(Some(WssEvent::Disconnected));
                }
                Err(e) => {
                    log::debug!("[wss_linux] read ended: {}", e);
                    return Ok(Some(WssEvent::Disconnected));
                }
            }
        }
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

    Ok(LinuxWssConnection { ws })
}
