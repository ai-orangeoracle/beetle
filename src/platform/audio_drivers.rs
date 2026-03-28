//! ESP32 音频驱动（I2S DMA 真实实现）。
//! Audio driver for ESP32 using I2S DMA via esp-idf-sys new channel API.

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
use crate::config::AudioSegment;
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
use crate::error::{Error, Result};

// ---------------------------------------------------------------------------
// I2S handle wrapper types
// ---------------------------------------------------------------------------

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
struct MicState {
    rx_handle: esp_idf_svc::sys::i2s_chan_handle_t,
    /// Pre-allocated i32 buffer for I2S 32-bit reads, reused across calls.
    read_buf: Vec<i32>,
}

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
struct SpeakerState {
    tx_handle: esp_idf_svc::sys::i2s_chan_handle_t,
    sd_pin: Option<i32>,
    /// Pre-allocated i32 buffer for I2S 32-bit writes, reused across calls.
    write_buf: Vec<i32>,
}

// SAFETY: Handles are accessed exclusively behind Mutex<Option<AudioPipelineState>>
// in Esp32Platform, guaranteeing single-thread access (same pattern as I2cBusState).
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
unsafe impl Send for MicState {}
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
unsafe impl Send for SpeakerState {}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// I2S read/write timeout in milliseconds.
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
const I2S_IO_TIMEOUT_MS: u32 = 1000;

/// FreeRTOS tick period (ms). ESP32 default configTICK_RATE_HZ = 100 → 10ms/tick.
/// portTICK_PERIOD_MS is a C macro not exported by bindgen, so we hardcode.
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
const PORT_TICK_PERIOD_MS: u32 = 10;

/// Check ESP-IDF return code; wrap non-OK as `Error::Esp`.
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
fn check_esp(stage: &'static str, ret: i32) -> Result<()> {
    if ret != esp_idf_svc::sys::ESP_OK {
        return Err(Error::esp(stage, ret));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Channel init helpers
// ---------------------------------------------------------------------------

/// 初始化 INMP441 麦克风 I2S RX 通道（I2S0）。
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
fn init_mic_channel(seg: &AudioSegment) -> Result<MicState> {
    use esp_idf_svc::sys::*;

    let mic = &seg.microphone;
    let mut rx_handle: i2s_chan_handle_t = core::ptr::null_mut();

    // -- 1. Allocate channel on I2S port 0, RX only --------------------------
    let chan_cfg = i2s_chan_config_t {
        id: i2s_port_t_I2S_NUM_0,
        role: i2s_role_t_I2S_ROLE_MASTER,
        dma_desc_num: 6,
        dma_frame_num: 240,
        auto_clear_before_cb: true,
        ..unsafe { core::mem::zeroed() }
    };

    check_esp("i2s_mic_new_channel", unsafe {
        i2s_new_channel(&chan_cfg, core::ptr::null_mut(), &mut rx_handle)
    })?;

    // -- 2. Configure STD mode -----------------------------------------------
    // INMP441 outputs 24-bit data in 32-bit I2S frames; must use 32-bit width.
    let clk_cfg = i2s_std_clk_config_t {
        sample_rate_hz: mic.sample_rate,
        clk_src: soc_periph_i2s_clk_src_t_I2S_CLK_SRC_DEFAULT,
        mclk_multiple: i2s_mclk_multiple_t_I2S_MCLK_MULTIPLE_256,
        ..unsafe { core::mem::zeroed() }
    };

    let slot_cfg = i2s_std_slot_config_t {
        data_bit_width: i2s_data_bit_width_t_I2S_DATA_BIT_WIDTH_32BIT,
        slot_bit_width: i2s_slot_bit_width_t_I2S_SLOT_BIT_WIDTH_AUTO,
        slot_mode: i2s_slot_mode_t_I2S_SLOT_MODE_MONO,
        slot_mask: i2s_std_slot_mask_t_I2S_STD_SLOT_LEFT,
        ws_width: i2s_data_bit_width_t_I2S_DATA_BIT_WIDTH_32BIT,
        ws_pol: false,
        bit_shift: true, // Philips standard
        ..unsafe { core::mem::zeroed() }
    };

    let gpio_cfg = i2s_std_gpio_config_t {
        mclk: gpio_num_t_GPIO_NUM_NC,
        bclk: mic.pins.sck as gpio_num_t,
        ws: mic.pins.ws as gpio_num_t,
        dout: gpio_num_t_GPIO_NUM_NC,
        din: mic.pins.din as gpio_num_t,
        invert_flags: unsafe { core::mem::zeroed() },
    };

    let std_cfg = i2s_std_config_t {
        clk_cfg,
        slot_cfg,
        gpio_cfg,
    };

    let init_ret = unsafe { i2s_channel_init_std_mode(rx_handle, &std_cfg) };
    if init_ret != ESP_OK {
        unsafe {
            i2s_del_channel(rx_handle);
        }
        return Err(Error::esp("i2s_mic_init_std", init_ret));
    }

    // -- 3. Enable channel ---------------------------------------------------
    let en_ret = unsafe { i2s_channel_enable(rx_handle) };
    if en_ret != ESP_OK {
        unsafe {
            i2s_del_channel(rx_handle);
        }
        return Err(Error::esp("i2s_mic_enable", en_ret));
    }

    log::info!(
        "[audio] mic I2S0 RX ready: {}Hz 32bit-i2s (ws={} sck={} din={})",
        mic.sample_rate,
        mic.pins.ws,
        mic.pins.sck,
        mic.pins.din,
    );
    Ok(MicState {
        rx_handle,
        read_buf: vec![0i32; 240], // matches DMA frame_num
    })
}

/// 初始化 MAX98357A 喇叭 I2S TX 通道（I2S1）。
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
fn init_speaker_channel(seg: &AudioSegment) -> Result<SpeakerState> {
    use esp_idf_svc::sys::*;

    let spk = &seg.speaker;
    let sd_pin = spk.pins.sd;

    // -- 0. Enable SD pin (shutdown control for MAX98357A) -------------------
    if let Some(pin) = sd_pin {
        unsafe {
            let _ = gpio_reset_pin(pin as gpio_num_t);
            let conf: gpio_config_t = gpio_config_t {
                pin_bit_mask: 1u64 << pin,
                mode: gpio_mode_t_GPIO_MODE_OUTPUT,
                pull_up_en: gpio_pullup_t_GPIO_PULLUP_DISABLE,
                pull_down_en: gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
                intr_type: gpio_int_type_t_GPIO_INTR_DISABLE,
            };
            check_esp("i2s_spk_sd_gpio_config", gpio_config(&conf))?;
            check_esp(
                "i2s_spk_sd_gpio_set",
                gpio_set_level(pin as gpio_num_t, 1),
            )?;
        }
    }

    let mut tx_handle: i2s_chan_handle_t = core::ptr::null_mut();

    // -- 1. Allocate channel on I2S port 1, TX only --------------------------
    let chan_cfg = i2s_chan_config_t {
        id: i2s_port_t_I2S_NUM_1,
        role: i2s_role_t_I2S_ROLE_MASTER,
        dma_desc_num: 6,
        dma_frame_num: 240,
        auto_clear_before_cb: true,
        ..unsafe { core::mem::zeroed() }
    };

    let new_ret = unsafe { i2s_new_channel(&chan_cfg, &mut tx_handle, core::ptr::null_mut()) };
    if new_ret != ESP_OK {
        // Revert SD pin on failure
        if let Some(pin) = sd_pin {
            unsafe {
                let _ = gpio_set_level(pin as gpio_num_t, 0);
            }
        }
        return Err(Error::esp("i2s_spk_new_channel", new_ret));
    }

    // -- 2. Configure STD mode -----------------------------------------------
    // MAX98357A expects 32-bit I2S frames (matching mic config).
    let clk_cfg = i2s_std_clk_config_t {
        sample_rate_hz: spk.sample_rate,
        clk_src: soc_periph_i2s_clk_src_t_I2S_CLK_SRC_DEFAULT,
        mclk_multiple: i2s_mclk_multiple_t_I2S_MCLK_MULTIPLE_256,
        ..unsafe { core::mem::zeroed() }
    };

    let slot_cfg = i2s_std_slot_config_t {
        data_bit_width: i2s_data_bit_width_t_I2S_DATA_BIT_WIDTH_32BIT,
        slot_bit_width: i2s_slot_bit_width_t_I2S_SLOT_BIT_WIDTH_AUTO,
        slot_mode: i2s_slot_mode_t_I2S_SLOT_MODE_MONO,
        slot_mask: i2s_std_slot_mask_t_I2S_STD_SLOT_LEFT,
        ws_width: i2s_data_bit_width_t_I2S_DATA_BIT_WIDTH_32BIT,
        ws_pol: false,
        bit_shift: true,
        ..unsafe { core::mem::zeroed() }
    };

    let gpio_cfg = i2s_std_gpio_config_t {
        mclk: gpio_num_t_GPIO_NUM_NC,
        bclk: spk.pins.sck as gpio_num_t,
        ws: spk.pins.ws as gpio_num_t,
        dout: spk.pins.dout as gpio_num_t,
        din: gpio_num_t_GPIO_NUM_NC,
        invert_flags: unsafe { core::mem::zeroed() },
    };

    let std_cfg = i2s_std_config_t {
        clk_cfg,
        slot_cfg,
        gpio_cfg,
    };

    let init_ret = unsafe { i2s_channel_init_std_mode(tx_handle, &std_cfg) };
    if init_ret != ESP_OK {
        unsafe {
            i2s_del_channel(tx_handle);
        }
        if let Some(pin) = sd_pin {
            unsafe {
                let _ = gpio_set_level(pin as gpio_num_t, 0);
            }
        }
        return Err(Error::esp("i2s_spk_init_std", init_ret));
    }

    // -- 3. Enable channel ---------------------------------------------------
    let en_ret = unsafe { i2s_channel_enable(tx_handle) };
    if en_ret != ESP_OK {
        unsafe {
            i2s_del_channel(tx_handle);
        }
        if let Some(pin) = sd_pin {
            unsafe {
                let _ = gpio_set_level(pin as gpio_num_t, 0);
            }
        }
        return Err(Error::esp("i2s_spk_enable", en_ret));
    }

    log::info!(
        "[audio] speaker I2S1 TX ready: {}Hz 32bit-i2s (ws={} sck={} dout={} sd={:?})",
        spk.sample_rate,
        spk.pins.ws,
        spk.pins.sck,
        spk.pins.dout,
        spk.pins.sd,
    );
    Ok(SpeakerState {
        tx_handle,
        sd_pin,
        write_buf: Vec::new(), // grown on first use
    })
}

// ---------------------------------------------------------------------------
// AudioPipelineState
// ---------------------------------------------------------------------------

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

        // -- Mic --------------------------------------------------------------
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
            if seg.microphone.bits_per_sample != 16 {
                return Err(Error::config(
                    "audio_init",
                    format!(
                        "only 16-bit microphone sampling is supported (got {})",
                        seg.microphone.bits_per_sample
                    ),
                ));
            }
            Some(init_mic_channel(seg)?)
        } else {
            None
        };

        // -- Speaker ----------------------------------------------------------
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
            if seg.speaker.bits_per_sample != 16 {
                return Err(Error::config(
                    "audio_init",
                    format!(
                        "only 16-bit speaker output is supported (got {})",
                        seg.speaker.bits_per_sample
                    ),
                ));
            }
            match init_speaker_channel(seg) {
                Ok(s) => Some(s),
                Err(e) => {
                    // Rollback: tear down mic channel if speaker init fails
                    if let Some(m) = mic {
                        unsafe {
                            let _ = esp_idf_svc::sys::i2s_channel_disable(m.rx_handle);
                            let _ = esp_idf_svc::sys::i2s_del_channel(m.rx_handle);
                        }
                    }
                    return Err(e);
                }
            }
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

    /// 从 INMP441 麦克风读取 PCM16 样本。
    /// I2S 以 32bit 帧传输，高 16 位为有效数据，读出后右移 16 位转为 i16。
    pub fn read_mic_pcm_i16(&mut self, out: &mut [i16]) -> Result<usize> {
        let mic = self
            .mic
            .as_mut()
            .ok_or_else(|| Error::config("audio_mic", "microphone not initialized"))?;
        if out.is_empty() {
            return Ok(0);
        }
        // Reuse pre-allocated i32 buffer; grow only if caller requests more than DMA frame.
        let sample_count = out.len();
        if mic.read_buf.len() < sample_count {
            mic.read_buf.resize(sample_count, 0);
        }
        let buf32 = &mut mic.read_buf[..sample_count];

        let byte_len = sample_count * 4;
        let mut bytes_read: usize = 0;
        let timeout_ticks: u32 = I2S_IO_TIMEOUT_MS / PORT_TICK_PERIOD_MS;

        check_esp("i2s_mic_read", unsafe {
            esp_idf_svc::sys::i2s_channel_read(
                mic.rx_handle,
                buf32.as_mut_ptr() as *mut core::ffi::c_void,
                byte_len,
                &mut bytes_read,
                timeout_ticks,
            )
        })?;

        let samples_read = bytes_read / 4;
        // INMP441 outputs 24-bit data left-aligned in 32-bit frame.
        // Shift right 16 bits to get the top 16 bits as i16.
        for i in 0..samples_read {
            out[i] = (buf32[i] >> 16) as i16;
        }

        Ok(samples_read)
    }

    /// 向 MAX98357A 喇叭写入 PCM16 样本。
    /// 将 i16 左移 16 位扩展到 32bit I2S 帧后写入。
    pub fn write_speaker_pcm_i16(&mut self, buf: &[i16]) -> Result<()> {
        let speaker = self
            .speaker
            .as_mut()
            .ok_or_else(|| Error::config("audio_speaker", "speaker not initialized"))?;
        if buf.is_empty() {
            return Ok(());
        }
        // Reuse pre-allocated buffer; grow only if needed (stays at high-water mark).
        if speaker.write_buf.len() < buf.len() {
            speaker.write_buf.resize(buf.len(), 0);
        }
        let buf32 = &mut speaker.write_buf[..buf.len()];
        for (i, &s) in buf.iter().enumerate() {
            buf32[i] = (s as i32) << 16;
        }

        let byte_len = buf32.len() * 4;
        let mut bytes_written: usize = 0;
        let timeout_ticks: u32 = I2S_IO_TIMEOUT_MS / PORT_TICK_PERIOD_MS;

        check_esp("i2s_spk_write", unsafe {
            esp_idf_svc::sys::i2s_channel_write(
                speaker.tx_handle,
                buf32.as_ptr() as *const core::ffi::c_void,
                byte_len,
                &mut bytes_written,
                timeout_ticks,
            )
        })?;

        if bytes_written < byte_len {
            log::warn!(
                "[audio] speaker partial write: {}/{} bytes",
                bytes_written,
                byte_len,
            );
        }
        Ok(())
    }
}

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
impl Drop for AudioPipelineState {
    fn drop(&mut self) {
        // -- Mic cleanup ------------------------------------------------------
        if let Some(ref mic) = self.mic {
            unsafe {
                let _ = esp_idf_svc::sys::i2s_channel_disable(mic.rx_handle);
                let _ = esp_idf_svc::sys::i2s_del_channel(mic.rx_handle);
            }
            log::debug!("[audio] mic I2S0 RX channel released");
        }
        // -- Speaker cleanup --------------------------------------------------
        if let Some(ref spk) = self.speaker {
            unsafe {
                let _ = esp_idf_svc::sys::i2s_channel_disable(spk.tx_handle);
                let _ = esp_idf_svc::sys::i2s_del_channel(spk.tx_handle);
            }
            // Pull SD pin low to shut down MAX98357A amplifier
            if let Some(pin) = spk.sd_pin {
                unsafe {
                    let _ = esp_idf_svc::sys::gpio_set_level(
                        pin as esp_idf_svc::sys::gpio_num_t,
                        0,
                    );
                }
            }
            log::debug!("[audio] speaker I2S1 TX channel released");
        }
    }
}
