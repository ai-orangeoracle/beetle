//! voice_input：采集麦克风 PCM，能量断句后调用百度 STT。

use crate::audio::baidu_token::BaiduTokenCache;
use crate::audio::energy::{EndpointConfig, EndpointEvent, EndpointState};
use crate::audio::stt_baidu;
use crate::config::{AppConfig, AudioSegment};
use crate::constants::{AUDIO_CAPTURE_FRAME_SAMPLES, AUDIO_CAPTURE_MAX_MS, AUDIO_STT_MAX_PCM_BYTES};
use crate::error::{Error, Result};
use crate::tools::{parse_tool_args, Tool, ToolContext};
use crate::Platform;
use serde_json::json;
use std::sync::Arc;

pub struct VoiceInputTool {
    platform: Arc<dyn Platform>,
    app_config: AppConfig,
    audio_cfg: AudioSegment,
    baidu_token: Arc<BaiduTokenCache>,
}

impl VoiceInputTool {
    pub fn new(
        platform: Arc<dyn Platform>,
        app_config: AppConfig,
        audio_cfg: AudioSegment,
        baidu_token: Arc<BaiduTokenCache>,
    ) -> Self {
        Self {
            platform,
            app_config,
            audio_cfg,
            baidu_token,
        }
    }
}

impl Tool for VoiceInputTool {
    fn name(&self) -> &str {
        "voice_input"
    }

    fn description(&self) -> &str {
        "Capture microphone audio, detect endpoint by energy, and transcribe via Baidu STT."
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "max_ms": { "type": "integer", "description": "Max capture milliseconds, default 12000" }
            }
        })
    }

    fn requires_network(&self) -> bool {
        true
    }

    fn execute(&self, args: &str, _ctx: &mut dyn ToolContext) -> Result<String> {
        if !self.platform.audio_mic_ready() {
            return Err(Error::config("tool_voice_input", "microphone not ready"));
        }
        let obj = parse_tool_args(args, "tool_voice_input")?;
        let max_ms = obj
            .get("max_ms")
            .and_then(|x| x.as_u64())
            .map(|v| v.min(AUDIO_CAPTURE_MAX_MS as u64) as u32)
            .unwrap_or(AUDIO_CAPTURE_MAX_MS);

        let mut http = self.platform.create_http_client(&self.app_config)?;
        let mic_sr = self.audio_cfg.microphone.sample_rate.max(8_000);
        let frame_ms = ((AUDIO_CAPTURE_FRAME_SAMPLES as u64) * 1000 / (mic_sr as u64))
            .clamp(1, 40) as u32;
        let endpoint_cfg = EndpointConfig {
            threshold: if self.audio_cfg.vad.enabled {
                self.audio_cfg.vad.threshold
            } else {
                0.08
            },
            silence_duration_ms: if self.audio_cfg.vad.enabled {
                self.audio_cfg.vad.silence_duration_ms
            } else {
                1200
            },
        };
        let mut endpoint = EndpointState::new();
        let mut frame = vec![0i16; AUDIO_CAPTURE_FRAME_SAMPLES];
        let mut started = false;
        let mut elapsed = 0u32;
        let max_samples = (max_ms as usize).saturating_mul(mic_sr as usize) / 1000;
        let mut captured: Vec<i16> =
            Vec::with_capacity(max_samples.min(AUDIO_STT_MAX_PCM_BYTES / 2));
        while elapsed < max_ms {
            let n = self.platform.read_mic_pcm_i16(&mut frame)?;
            if n == 0 {
                elapsed = elapsed.saturating_add(frame_ms);
                continue;
            }
            let chunk = &frame[..n.min(frame.len())];
            match endpoint.update(chunk, frame_ms, &endpoint_cfg) {
                EndpointEvent::SpeechStart => {
                    started = true;
                    captured.extend_from_slice(chunk);
                }
                EndpointEvent::SpeechEnd => break,
                EndpointEvent::None => {
                    if started {
                        captured.extend_from_slice(chunk);
                    }
                }
            }
            if captured.len() * 2 >= AUDIO_STT_MAX_PCM_BYTES {
                break;
            }
            elapsed = elapsed.saturating_add(frame_ms);
        }
        if captured.is_empty() {
            return Err(Error::config(
                "tool_voice_input",
                "no speech captured within time window",
            ));
        }
        let mut pcm_bytes = Vec::with_capacity(captured.len() * 2);
        for s in captured {
            pcm_bytes.extend_from_slice(&s.to_le_bytes());
        }
        let text = stt_baidu::transcribe_pcm16(
            http.as_mut(),
            self.baidu_token.as_ref(),
            &self.audio_cfg.stt,
            &pcm_bytes,
            mic_sr,
        )?;
        Ok(text)
    }
}
