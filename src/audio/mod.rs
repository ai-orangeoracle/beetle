//! 音频业务逻辑（纯 Rust）：能量断句、百度 STT/TTS HTTP。
//! Pure-Rust audio logic: energy endpointing and Baidu STT/TTS HTTP.

pub mod baidu_token;
pub mod energy;
pub mod stt_baidu;
pub mod tts_baidu;
