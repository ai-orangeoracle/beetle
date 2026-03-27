//! 百度 TTS（REST）与 WAV(PCM16LE) 解析。
//! Baidu TTS REST and WAV(PCM16LE) parsing.
//!
//! OAuth `access_token` is fetched with **`stt` credentials** (`api_key` / `api_secret`); Baidu uses one app key for both ASR and TTS.
//! 鉴权与 STT 相同：使用配置里 `stt` 的密钥换 token，非独立 TTS 密钥。

use super::baidu_token::BaiduTokenCache;
use crate::config::{AudioSttConfig, AudioTtsConfig};
use crate::error::{Error, Result};
use crate::platform::{PlatformHttpClient, ResponseBody};

const BAIDU_TTS_URL: &str = "https://tsn.baidu.com/text2audio";

pub fn synthesize_wav(
    http: &mut dyn PlatformHttpClient,
    token_cache: &BaiduTokenCache,
    stt: &AudioSttConfig,
    tts: &AudioTtsConfig,
    text: &str,
) -> Result<ResponseBody> {
    let token = token_cache.get_or_fetch(http, &stt.api_key, &stt.api_secret)?;
    let per = tts.voice.trim().parse::<u32>().unwrap_or(0).min(4);
    let spd = map_speed_percent_to_baidu(&tts.rate);
    let pit = map_pitch_percent_to_baidu(&tts.pitch);
    let body = format!(
        "tex={}&tok={}&cuid=beetle&ctp=1&lan=zh&aue=6&per={}&spd={}&pit={}",
        urlencoding::encode(text),
        urlencoding::encode(&token),
        per,
        spd,
        pit
    );
    let headers = [("Content-Type", "application/x-www-form-urlencoded")];
    let (status, resp) = http
        .post(BAIDU_TTS_URL, &headers, body.as_bytes())
        .map_err(|e| Error::config("tts_baidu_request", e.to_string()))?;
    if status != 200 {
        return Err(Error::config(
            "tts_baidu_request",
            format!("tts http status {}", status),
        ));
    }
    let bytes = resp.as_slice();
    if bytes.starts_with(b"{") {
        let msg = extract_tts_error_message(bytes);
        return Err(Error::config("tts_baidu_request", msg));
    }
    Ok(resp)
}

pub fn play_wav_pcm16le_chunks<F>(
    wav: &ResponseBody,
    chunk_samples: usize,
    mut on_chunk: F,
) -> Result<usize>
where
    F: FnMut(&[i16]) -> Result<()>,
{
    let bytes = wav.as_slice();
    let (start, data_len) = wav_data_chunk(bytes)?;
    let mut played = 0usize;
    let mut idx = start;
    let end = start + data_len;
    let mut chunk = vec![0i16; chunk_samples.max(1)];
    while idx < end {
        let remain_bytes = end - idx;
        let take_samples = (remain_bytes / 2).min(chunk.len());
        if take_samples == 0 {
            break;
        }
        for i in 0..take_samples {
            let pos = idx + i * 2;
            chunk[i] = i16::from_le_bytes([bytes[pos], bytes[pos + 1]]);
        }
        on_chunk(&chunk[..take_samples])?;
        played += take_samples;
        idx += take_samples * 2;
    }
    Ok(played)
}

fn wav_data_chunk(wav: &[u8]) -> Result<(usize, usize)> {
    if wav.len() < 44 || &wav[0..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        return Err(Error::config("tts_baidu_wav", "invalid WAV header"));
    }
    let mut cursor = 12usize;
    let mut data_start = None;
    let mut data_len = 0usize;
    while cursor + 8 <= wav.len() {
        let chunk_id = &wav[cursor..cursor + 4];
        let chunk_len = u32::from_le_bytes([
            wav[cursor + 4],
            wav[cursor + 5],
            wav[cursor + 6],
            wav[cursor + 7],
        ]) as usize;
        cursor += 8;
        if cursor + chunk_len > wav.len() {
            return Err(Error::config("tts_baidu_wav", "corrupted WAV chunks"));
        }
        if chunk_id == b"data" {
            data_start = Some(cursor);
            data_len = chunk_len;
            break;
        }
        cursor += chunk_len;
    }
    let start = data_start.ok_or_else(|| Error::config("tts_baidu_wav", "missing data chunk"))?;
    if !data_len.is_multiple_of(2) {
        return Err(Error::config(
            "tts_baidu_wav",
            "PCM data length must be even",
        ));
    }
    Ok((start, data_len))
}

fn map_speed_percent_to_baidu(rate: &str) -> u32 {
    map_percent_like(rate)
}

fn map_pitch_percent_to_baidu(pitch: &str) -> u32 {
    map_percent_like(pitch)
}

/// 将类似 +0% / -20% / +5Hz 映射到百度 0..15，基线 5。
fn map_percent_like(s: &str) -> u32 {
    let trimmed = s.trim();
    let mut num = String::new();
    for ch in trimmed.chars() {
        if ch.is_ascii_digit() || ch == '+' || ch == '-' {
            num.push(ch);
        }
    }
    let delta = num.parse::<i32>().unwrap_or(0);
    let mapped = 5 + delta / 10;
    mapped.clamp(0, 15) as u32
}

fn extract_tts_error_message(bytes: &[u8]) -> String {
    let parsed = serde_json::from_slice::<serde_json::Value>(bytes).ok();
    if let Some(v) = parsed {
        if let Some(msg) = v.get("err_msg").and_then(|x| x.as_str()) {
            return msg.to_string();
        }
        if let Some(msg) = v.get("message").and_then(|x| x.as_str()) {
            return msg.to_string();
        }
    }
    std::str::from_utf8(bytes)
        .map(|s| s.to_string())
        .unwrap_or_else(|_| "unknown tts error".to_string())
}
