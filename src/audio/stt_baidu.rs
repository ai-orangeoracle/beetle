//! 百度 STT（REST）。
//! Baidu STT REST client.

use super::baidu_token::BaiduTokenCache;
use crate::config::AudioSttConfig;
use crate::error::{Error, Result};
use crate::platform::{PlatformHttpClient, ResponseBody};
use base64::Engine;
use serde_json::Value;

const BAIDU_STT_DEFAULT_URL: &str = "https://vop.baidu.com/server_api";

pub fn transcribe_pcm16(
    http: &mut dyn PlatformHttpClient,
    token_cache: &BaiduTokenCache,
    stt: &AudioSttConfig,
    pcm16le: &[u8],
    sample_rate: u32,
) -> Result<String> {
    let token = token_cache.get_or_fetch(http, &stt.api_key, &stt.api_secret)?;
    let speech_b64 = base64::engine::general_purpose::STANDARD.encode(pcm16le);
    let dev_pid = stt.model.trim().parse::<u32>().unwrap_or(1537);
    let body = serde_json::json!({
        "format": "pcm",
        "rate": sample_rate,
        "channel": 1,
        "cuid": "beetle",
        "token": token,
        "speech": speech_b64,
        "len": pcm16le.len(),
        "dev_pid": dev_pid,
    })
    .to_string();
    let api_url = if stt.api_url.trim().is_empty() {
        BAIDU_STT_DEFAULT_URL
    } else {
        stt.api_url.trim()
    };
    let headers = [("Content-Type", "application/json")];
    let (status, body_buf) = http
        .post(api_url, &headers, body.as_bytes())
        .map_err(|e| Error::config("stt_baidu_request", e.to_string()))?;
    parse_asr_response(status, body_buf)
}

fn parse_asr_response(status: u16, body: ResponseBody) -> Result<String> {
    if status != 200 {
        return Err(Error::config(
            "stt_baidu_request",
            format!("asr http status {}", status),
        ));
    }
    let text = response_to_string(body)?;
    let v: Value =
        serde_json::from_str(&text).map_err(|e| Error::config("stt_baidu_parse", e.to_string()))?;
    let err_no = v.get("err_no").and_then(|x| x.as_i64()).unwrap_or(-1);
    if err_no != 0 {
        let err_msg = v
            .get("err_msg")
            .and_then(|x| x.as_str())
            .unwrap_or("unknown asr error");
        return Err(Error::config(
            "stt_baidu_request",
            format!("baidu asr error {}: {}", err_no, err_msg),
        ));
    }
    let first = v
        .get("result")
        .and_then(|x| x.as_array())
        .and_then(|arr| arr.first())
        .and_then(|x| x.as_str())
        .ok_or_else(|| Error::config("stt_baidu_parse", "missing result text"))?;
    Ok(first.trim().to_string())
}

fn response_to_string(body: ResponseBody) -> Result<String> {
    let bytes = body.as_slice();
    std::str::from_utf8(bytes)
        .map(|s| s.to_string())
        .map_err(|e| Error::config("stt_baidu_utf8", e.to_string()))
}
