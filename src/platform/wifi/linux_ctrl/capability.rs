//! Linux WiFi capability detection.

use crate::error::{Error, Result};
use std::path::Path;

fn iface_exists(name: &str) -> bool {
    Path::new("/sys/class/net").join(name).exists()
}

pub fn detect_wifi_iface() -> Result<String> {
    for iface in ["wlan0", "wlan1"] {
        if iface_exists(iface) {
            return Ok(iface.to_string());
        }
    }
    Err(Error::config(
        "wifi_capability_check",
        "no wlan interface found (wlan0/wlan1)",
    ))
}
