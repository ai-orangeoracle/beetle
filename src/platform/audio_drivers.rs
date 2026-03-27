//! ESP32 音频驱动状态机（I2S 配置入口）。
//! Audio driver state for ESP32 (I2S config entry points).

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
use crate::config::AudioSegment;
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
use crate::error::{Error, Result};
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
use std::time::Duration;

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
#[derive(Clone, Debug)]
struct MicState {
    sample_rate: u32,
    bits_per_sample: u16,
    _ws: i32,
    _sck: i32,
    _din: i32,
}

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
#[derive(Clone, Debug)]
struct SpeakerState {
    sample_rate: u32,
    bits_per_sample: u16,
    _ws: i32,
    _sck: i32,
    _dout: i32,
    _sd: Option<i32>,
}

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub(crate) struct AudioPipelineState {
    mic: Option<MicState>,
    speaker: Option<SpeakerState>,
}

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
impl AudioPipelineState {
    pub fn from_config(seg: &AudioSegment) -> Result<Self> {
        if !seg.enabled {
            return Ok(Self {
                mic: None,
                speaker: None,
            });
        }
        let mic = if seg.microphone.enabled {
            if seg.microphone.device_type != "i2s_inmp441" {
                return Err(Error::config(
                    "audio_init",
                    format!(
                        "unsupported microphone device_type '{}', only i2s_inmp441 is supported now",
                        seg.microphone.device_type
                    ),
                ));
            }
            Some(MicState {
                sample_rate: seg.microphone.sample_rate,
                bits_per_sample: seg.microphone.bits_per_sample,
                _ws: seg.microphone.pins.ws,
                _sck: seg.microphone.pins.sck,
                _din: seg.microphone.pins.din,
            })
        } else {
            None
        };
        let speaker = if seg.speaker.enabled {
            if seg.speaker.device_type != "i2s_max98357a" {
                return Err(Error::config(
                    "audio_init",
                    format!(
                        "unsupported speaker device_type '{}', only i2s_max98357a is supported now",
                        seg.speaker.device_type
                    ),
                ));
            }
            Some(SpeakerState {
                sample_rate: seg.speaker.sample_rate,
                bits_per_sample: seg.speaker.bits_per_sample,
                _ws: seg.speaker.pins.ws,
                _sck: seg.speaker.pins.sck,
                _dout: seg.speaker.pins.dout,
                _sd: seg.speaker.pins.sd,
            })
        } else {
            None
        };
        Ok(Self { mic, speaker })
    }

    #[inline]
    pub fn mic_ready(&self) -> bool {
        self.mic.is_some()
    }

    #[inline]
    pub fn speaker_ready(&self) -> bool {
        self.speaker.is_some()
    }

    pub fn read_mic_pcm_i16(&mut self, out: &mut [i16]) -> Result<usize> {
        let mic = self
            .mic
            .as_ref()
            .ok_or_else(|| Error::config("audio_mic", "microphone not initialized"))?;
        if mic.bits_per_sample != 16 {
            return Err(Error::config(
                "audio_mic",
                format!(
                    "only 16-bit microphone sampling is supported now (got {})",
                    mic.bits_per_sample
                ),
            ));
        }
        if out.is_empty() {
            return Ok(0);
        }
        // 首版先保证调用链闭合：输出静音帧，时序对齐采样率，后续可替换为真实 I2S DMA 读取。
        out.fill(0);
        let frame_ms = ((out.len() as u64) * 1000 / (mic.sample_rate as u64)).clamp(1, 50);
        std::thread::sleep(Duration::from_millis(frame_ms));
        Ok(out.len())
    }

    pub fn write_speaker_pcm_i16(&mut self, buf: &[i16]) -> Result<()> {
        let speaker = self
            .speaker
            .as_ref()
            .ok_or_else(|| Error::config("audio_speaker", "speaker not initialized"))?;
        if speaker.bits_per_sample != 16 {
            return Err(Error::config(
                "audio_speaker",
                format!(
                    "only 16-bit speaker output is supported now (got {})",
                    speaker.bits_per_sample
                ),
            ));
        }
        if buf.is_empty() {
            return Ok(());
        }
        let frame_ms = ((buf.len() as u64) * 1000 / (speaker.sample_rate as u64)).clamp(1, 80);
        std::thread::sleep(Duration::from_millis(frame_ms));
        Ok(())
    }
}
