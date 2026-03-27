//! voice_output：调用百度 TTS 并播放到喇叭。

use crate::audio::baidu_token::BaiduTokenCache;
use crate::audio::tts_baidu;
use crate::config::{AppConfig, AudioSegment};
use crate::constants::{AUDIO_TTS_MAX_TEXT_LEN, AUDIO_TTS_WRITE_CHUNK_SAMPLES};
use crate::error::{Error, Result};
use crate::tools::{parse_tool_args, Tool, ToolContext};
use crate::Platform;
use serde_json::json;
use std::sync::Arc;

pub struct VoiceOutputTool {
    platform: Arc<dyn Platform>,
    app_config: AppConfig,
    audio_cfg: AudioSegment,
    baidu_token: Arc<BaiduTokenCache>,
}

impl VoiceOutputTool {
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

impl Tool for VoiceOutputTool {
    fn name(&self) -> &str {
        "voice_output"
    }

    fn description(&self) -> &str {
        "Synthesize text with Baidu TTS and play via speaker."
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "Text to speak" }
            },
            "required": ["text"]
        })
    }

    fn requires_network(&self) -> bool {
        true
    }

    fn execute(&self, args: &str, _ctx: &mut dyn ToolContext) -> Result<String> {
        if !self.platform.audio_speaker_ready() {
            return Err(Error::config("tool_voice_output", "speaker not ready"));
        }
        let obj = parse_tool_args(args, "tool_voice_output")?;
        let text = obj
            .get("text")
            .and_then(|x| x.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| Error::config("tool_voice_output", "missing text"))?;
        if text.len() > AUDIO_TTS_MAX_TEXT_LEN {
            return Err(Error::config(
                "tool_voice_output",
                format!("text too long (max {})", AUDIO_TTS_MAX_TEXT_LEN),
            ));
        }
        let mut http = self.platform.create_http_client(&self.app_config)?;
        let wav = tts_baidu::synthesize_wav(
            http.as_mut(),
            self.baidu_token.as_ref(),
            &self.audio_cfg.stt,
            &self.audio_cfg.tts,
            text,
        )?;
        let pcm = tts_baidu::wav_pcm16le_to_i16(&wav)?;
        for chunk in pcm.chunks(AUDIO_TTS_WRITE_CHUNK_SAMPLES) {
            self.platform.write_speaker_pcm_i16(chunk)?;
        }
        Ok(json!({
            "ok": true,
            "played_samples": pcm.len()
        })
        .to_string())
    }
}
