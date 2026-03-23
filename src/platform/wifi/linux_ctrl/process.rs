//! Single controlled command entry for Linux WiFi operations.

use crate::error::{Error, Result};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    path::Path,
};

use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

/// Minimal command output used by WiFi controllers.
#[derive(Debug, Clone)]
pub struct CmdOutput {
    pub stdout: String,
}

fn is_allowed_bin(bin: &str) -> bool {
    matches!(
        bin,
        "ip" | "iw" | "wpa_cli" | "wpa_supplicant" | "hostapd" | "dnsmasq" | "kill"
    )
}

pub fn run_checked(
    bin: &'static str,
    args: &[&str],
    timeout: Duration,
    stage: &'static str,
) -> Result<CmdOutput> {
    if !is_allowed_bin(bin) {
        return Err(Error::config(
            stage,
            format!("command not allowed: {}", bin),
        ));
    }
    for a in args {
        if a.contains('\0') || a.contains('\n') || a.contains('\r') {
            return Err(Error::config(stage, "invalid command argument"));
        }
    }

    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::io(stage, e))?;

    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| Error::io(stage, e))? {
            let mut stdout = String::new();
            let mut stderr = String::new();
            if let Some(mut out) = child.stdout.take() {
                let _ = out.read_to_string(&mut stdout);
            }
            if let Some(mut err) = child.stderr.take() {
                let _ = err.read_to_string(&mut stderr);
            }
            if status.success() {
                return Ok(CmdOutput { stdout });
            }
            return Err(Error::config(
                stage,
                format!("{} failed: {}", bin, stderr.trim()),
            ));
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::config(stage, format!("{} timeout", bin)));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

pub fn write_secure_atomic(path: &Path, data: &[u8], stage: &'static str) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::config(stage, "missing parent"))?;
    std::fs::create_dir_all(parent).map_err(|e| Error::io(stage, e))?;
    let fname = path
        .file_name()
        .and_then(|x| x.to_str())
        .ok_or_else(|| Error::config(stage, "invalid file name"))?;
    let tmp = parent.join(format!(".{}.tmp.{}", fname, std::process::id()));
    let mut f = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)
        .map_err(|e| Error::io(stage, e))?;
    f.write_all(data).map_err(|e| Error::io(stage, e))?;
    f.sync_all().map_err(|e| Error::io(stage, e))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        Error::io(stage, e)
    })?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| Error::io(stage, e))?;
    Ok(())
}
