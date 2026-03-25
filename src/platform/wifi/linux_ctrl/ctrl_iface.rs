//! wpa_supplicant / hostapd 控制面共享协议：`COMMAND\\n`，响应为行，直至 `OK`/`FAIL`/`PONG`。
//! Shared ctrl protocol for wpa_supplicant / hostapd: `COMMAND\\n`, line-oriented until `OK`/`FAIL`/`PONG`.

use crate::error::{Error, Result};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

const MAX_REPLY_LINES: u32 = 10_000;
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
    let mut stream = UnixStream::connect(path).map_err(|e| Error::io(stage, e))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| Error::io(stage, e))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|e| Error::io(stage, e))?;
    stream
        .write_all(cmd.as_bytes())
        .and_then(|_| stream.write_all(b"\n"))
        .map_err(|e| Error::io(stage, e))?;
    stream.flush().map_err(|e| Error::io(stage, e))?;

    let mut reader = BufReader::new(&mut stream);
    let mut body = String::new();
    let mut line_buf = String::new();
    for _ in 0..MAX_REPLY_LINES {
        line_buf.clear();
        let n = reader
            .read_line(&mut line_buf)
            .map_err(|e| Error::io(stage, e))?;
        if n == 0 {
            break;
        }
        let line = line_buf.trim_end_matches(['\n', '\r']);
        if line == "OK" {
            return Ok(body.trim_end().to_string());
        }
        if line.starts_with("FAIL") {
            return Err(Error::config(stage, format!("ctrl iface: {line}")));
        }
        if line == "PONG" || line.starts_with("PONG") {
            return Ok(line.to_string());
        }
        if !body.is_empty() {
            body.push('\n');
        }
        body.push_str(line);
        if body.len() > MAX_REPLY_BYTES {
            return Err(Error::config(stage, "ctrl reply too large"));
        }
    }
    Err(Error::config(
        stage,
        "ctrl reply incomplete (missing OK/FAIL/PONG)",
    ))
}
