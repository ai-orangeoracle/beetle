//! 百度 OAuth access_token 获取与进程内缓存（STT/TTS 共用）。
//! Fetch and in-process cache for Baidu OAuth access_token (shared by STT/TTS).

use crate::error::{Error, Result};
use crate::platform::PlatformHttpClient;
use serde_json::Value;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const BAIDU_TOKEN_URL: &str = "https://aip.baidubce.com/oauth/2.0/token";
/// 在官方 `expires_in` 之前提前刷新，避免边界时刻 401。
const EXPIRY_SAFETY_SECS: u64 = 120;
/// 响应缺少 `expires_in` 时的默认 TTL（约 30 天，与百度常见返回值一致）。
const DEFAULT_EXPIRES_IN_SECS: u64 = 2_592_000;

/// 按 `api_key` 维度缓存；更换 key 会重新拉取（secret 轮换在 token 未过期前仍用旧 token，与百度侧一致）。
#[derive(Default)]
pub struct BaiduTokenCache {
    state: Mutex<Option<CachedState>>,
}

struct CachedState {
    api_key: String,
    token: String,
    expires_at: Instant,
}

impl BaiduTokenCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// 返回有效 access_token，必要时向百度换取新 token。
    pub fn get_or_fetch(
        &self,
        http: &mut dyn PlatformHttpClient,
        api_key: &str,
        api_secret: &str,
    ) -> Result<String> {
        let key = api_key.trim();
        let secret = api_secret.trim();
        if key.is_empty() || secret.is_empty() {
            return Err(Error::config(
                "baidu_token",
                "api_key/api_secret is required",
            ));
        }

        let now = Instant::now();
        {
            let guard = self.state.lock().map_err(|e| {
                Error::config("baidu_token_lock", format!("mutex lock: {}", e))
            })?;
            if let Some(ref c) = *guard {
                if c.api_key == key && now < c.expires_at {
                    return Ok(c.token.clone());
                }
            }
        }

        let (token, expires_in_secs) = fetch_access_token(http, key, secret)?;
        let ttl_secs = expires_in_secs
            .unwrap_or(DEFAULT_EXPIRES_IN_SECS)
            .saturating_sub(EXPIRY_SAFETY_SECS)
            .max(60);

        let mut guard = self.state.lock().map_err(|e| {
            Error::config("baidu_token_lock", format!("mutex lock: {}", e))
        })?;
        *guard = Some(CachedState {
            api_key: key.to_string(),
            token: token.clone(),
            expires_at: Instant::now() + Duration::from_secs(ttl_secs),
        });
        Ok(token)
    }
}

fn fetch_access_token(
    http: &mut dyn PlatformHttpClient,
    api_key: &str,
    api_secret: &str,
) -> Result<(String, Option<u64>)> {
    let form = format!(
        "grant_type=client_credentials&client_id={}&client_secret={}",
        urlencoding::encode(api_key),
        urlencoding::encode(api_secret)
    );
    let headers = [("Content-Type", "application/x-www-form-urlencoded")];
    let (status, body_buf) = http
        .post(BAIDU_TOKEN_URL, &headers, form.as_bytes())
        .map_err(|e| Error::config("baidu_token", e.to_string()))?;
    if status != 200 {
        return Err(Error::config(
            "baidu_token",
            format!("token http status {}", status),
        ));
    }
    let v: Value = serde_json::from_slice(body_buf.as_slice())
        .map_err(|e| Error::config("baidu_token_parse", e.to_string()))?;
    let token = v
        .get("access_token")
        .and_then(|x| x.as_str())
        .ok_or_else(|| Error::config("baidu_token", "missing access_token"))?
        .to_string();
    let expires_in = v.get("expires_in").and_then(|x| x.as_u64());
    Ok((token, expires_in))
}
