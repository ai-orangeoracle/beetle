//! wpa_supplicant / hostapd 控制面共享协议：`COMMAND\\n`，响应为行，直至 `OK`/`FAIL`/`PONG`。
//! Shared ctrl protocol for wpa_supplicant / hostapd: `COMMAND\\n`, line-oriented until `OK`/`FAIL`/`PONG`.

use crate::error::{Error, Result};
use std::os::unix::net::UnixDatagram;
use std::path::Path;
use std::time::Duration;

const MAX_REPLY_BYTES: usize = 512 * 1024;

pub fn request_unix(
    path: &Path,
    cmd: &str,
    timeout: Duration,
    stage: &'static str,
) -> Result<String> {
    if cmd.contains('\n') || cmd.contains('\r') {
        return Err(Error::config(stage, "invalid ctrl command"));
    }

    let client_path = temp_client_socket_path();
    let sock = UnixDatagram::bind(&client_path).map_err(|e| Error::io(stage, e))?;
    sock.connect(path).map_err(|e| Error::io(stage, e))?;
    sock.set_read_timeout(Some(timeout))
        .map_err(|e| Error::io(stage, e))?;
    sock.set_write_timeout(Some(timeout))
        .map_err(|e| Error::io(stage, e))?;
    sock.send(cmd.as_bytes()).map_err(|e| Error::io(stage, e))?;

    let mut buf = vec![0u8; MAX_REPLY_BYTES];
    let n = sock.recv(&mut buf).map_err(|e| Error::io(stage, e))?;
    let _ = std::fs::remove_file(&client_path);
    let reply = String::from_utf8_lossy(&buf[..n]).trim_end().to_string();
    if reply.starts_with("FAIL") {
        return Err(Error::config(stage, format!("ctrl iface: {reply}")));
    }
    Ok(reply)
}

fn temp_client_socket_path() -> std::path::PathBuf {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("beetle-ctrl-{}-{}.sock", pid, nanos))
}
