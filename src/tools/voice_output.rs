//! voice_output：调用百度 TTS 并播放到喇叭。

use crate::audio::baidu_token::BaiduTokenCache;
use crate::audio::tts_baidu;
use crate::config::AudioSegment;
use crate::constants::{AUDIO_TTS_MAX_TEXT_LEN, AUDIO_TTS_WRITE_CHUNK_SAMPLES};
use crate::error::{Error, Result};
use crate::tools::http_bridge::ToolContextHttpClient;
use crate::tools::{parse_tool_args, Tool, ToolContext};
use crate::Platform;
use serde_json::json;
use std::sync::Arc;

pub struct VoiceOutputTool {
    platform: Arc<dyn Platform>,
    audio_cfg: AudioSegment,
    baidu_token: Arc<BaiduTokenCache>,
}

impl VoiceOutputTool {
    pub fn new(
        platform: Arc<dyn Platform>,
        audio_cfg: AudioSegment,
        baidu_token: Arc<BaiduTokenCache>,
    ) -> Self {
        Self {
            platform,
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

    fn execute(&self, args: &str, ctx: &mut dyn ToolContext) -> Result<String> {
        log_audio_resource_snapshot("voice_output_start");
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
        let mut http = ToolContextHttpClient::new(ctx);
        let wav = tts_baidu::synthesize_wav(
            &mut http,
            self.baidu_token.as_ref(),
            &self.audio_cfg.stt,
            &self.audio_cfg.tts,
            text,
        )?;
        let played_samples = tts_baidu::play_wav_pcm16le_chunks(
            &wav,
            AUDIO_TTS_WRITE_CHUNK_SAMPLES,
            |chunk| self.platform.write_speaker_pcm_i16(chunk),
        )?;
        log_audio_resource_snapshot("voice_output_done");
        Ok(json!({
            "ok": true,
            "played_samples": played_samples
        })
        .to_string())
    }
}

fn log_audio_resource_snapshot(stage: &str) {
    let snap = crate::orchestrator::snapshot();
    log::info!(
        "[tool_voice_output] {} heap_internal={} heap_spiram={} heap_largest={} pressure={:?}",
        stage,
        snap.heap_free_internal,
        snap.heap_free_spiram,
        snap.heap_largest_block_internal,
        snap.pressure
    );
}
