//! 硬件设备驱动函数（条件编译）。ESP 目标调用 esp-idf-svc，host 返回模拟值。
//! Hardware device driver functions (conditional compilation).

use crate::config::PinConfig;
use crate::error::{Error, Result};
use serde_json::Value;

/// LEDC 定时器分辨率：13-bit (0–8191)。
const LEDC_DUTY_RESOLUTION_BITS: u32 = 13;
/// 13-bit 最大占空比值。
const LEDC_DUTY_MAX: u32 = (1 << LEDC_DUTY_RESOLUTION_BITS) - 1; // 8191
/// PWM 默认频率。
const PWM_DEFAULT_FREQ_HZ: u32 = 5000;
/// buzzer 最长响鸣时间（ms）。
const MAX_BUZZER_DURATION_MS: u64 = 3000;
/// buzzer 短鸣默认时长（ms）。
const BUZZER_BEEP_MS: u64 = 100;

// ── ESP32 target: real drivers ──

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub fn drive_gpio_out(pins: &PinConfig, params: &Value) -> Result<String> {
    use esp_idf_svc::sys::{
        gpio_config_t, gpio_get_level, gpio_int_type_t_GPIO_INTR_DISABLE,
        gpio_mode_t_GPIO_MODE_OUTPUT, gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
        gpio_pullup_t_GPIO_PULLUP_DISABLE, gpio_reset_pin, gpio_set_level, ESP_OK,
    };

    let pin = *pins
        .get("pin")
        .ok_or_else(|| Error::config("gpio_out", "missing pin"))?;
    let value = params
        .get("value")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| Error::config("gpio_out", "missing or invalid 'value' (0 or 1)"))?;
    if value != 0 && value != 1 {
        return Err(Error::config("gpio_out", "value must be 0 or 1"));
    }

    unsafe {
        gpio_reset_pin(pin);
        let conf = gpio_config_t {
            pin_bit_mask: 1u64 << pin,
            mode: gpio_mode_t_GPIO_MODE_OUTPUT,
            pull_up_en: gpio_pullup_t_GPIO_PULLUP_DISABLE,
            pull_down_en: gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
            intr_type: gpio_int_type_t_GPIO_INTR_DISABLE,
        };
        let ret = esp_idf_svc::sys::gpio_config(&conf);
        if ret != ESP_OK {
            return Err(Error::Other {
                source: Box::new(std::io::Error::other(format!(
                    "gpio_config failed: {}",
                    ret
                ))),
                stage: "gpio_out",
            });
        }
        gpio_set_level(pin, if value != 0 { 1 } else { 0 });
        // read-back to confirm
        let actual = gpio_get_level(pin);
        let ok = actual == (if value != 0 { 1 } else { 0 });
        if ok {
            Ok(format!(r#"{{"ok":true,"actual_value":{}}}"#, actual))
        } else {
            Ok(format!(
                r#"{{"ok":false,"expected":{},"actual_value":{},"warning":"read-back mismatch"}}"#,
                value, actual
            ))
        }
    }
}

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub fn drive_gpio_in(pins: &PinConfig, _params: &Value, options: &Value) -> Result<String> {
    use esp_idf_svc::sys::{
        gpio_config_t, gpio_get_level, gpio_int_type_t_GPIO_INTR_DISABLE,
        gpio_mode_t_GPIO_MODE_INPUT, gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
        gpio_pulldown_t_GPIO_PULLDOWN_ENABLE, gpio_pullup_t_GPIO_PULLUP_DISABLE,
        gpio_pullup_t_GPIO_PULLUP_ENABLE, gpio_reset_pin, ESP_OK,
    };

    let pin = *pins
        .get("pin")
        .ok_or_else(|| Error::config("gpio_in", "missing pin"))?;
    let pull = options
        .get("pull")
        .and_then(|v| v.as_str())
        .unwrap_or("none");

    let (pull_up, pull_down) = match pull {
        "up" => (
            gpio_pullup_t_GPIO_PULLUP_ENABLE,
            gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
        ),
        "down" => (
            gpio_pullup_t_GPIO_PULLUP_DISABLE,
            gpio_pulldown_t_GPIO_PULLDOWN_ENABLE,
        ),
        _ => (
            gpio_pullup_t_GPIO_PULLUP_DISABLE,
            gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
        ),
    };

    unsafe {
        gpio_reset_pin(pin);
        let conf = gpio_config_t {
            pin_bit_mask: 1u64 << pin,
            mode: gpio_mode_t_GPIO_MODE_INPUT,
            pull_up_en: pull_up,
            pull_down_en: pull_down,
            intr_type: gpio_int_type_t_GPIO_INTR_DISABLE,
        };
        let ret = esp_idf_svc::sys::gpio_config(&conf);
        if ret != ESP_OK {
            return Err(Error::Other {
                source: Box::new(std::io::Error::other(format!(
                    "gpio_config failed: {}",
                    ret
                ))),
                stage: "gpio_in",
            });
        }
        let level = gpio_get_level(pin);
        Ok(format!(r#"{{"value":{}}}"#, level))
    }
}

/// LEDC timer index to timer enum (ESP32-S3 has 4 timers 0..3; each pwm_out gets its own for independent frequency).
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
fn ledc_timer_from_index(i: u8) -> esp_idf_svc::sys::ledc_timer_t {
    // C enum LEDC_TIMER_0=0 .. LEDC_TIMER_3=3; repr(C) enum is typically 4 bytes.
    i.min(3) as u32
}

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub fn drive_pwm_out(
    pins: &PinConfig,
    params: &Value,
    options: &Value,
    ledc_channel: u8,
    ledc_timer_index: u8,
) -> Result<String> {
    use esp_idf_svc::sys::{
        ledc_channel_config, ledc_channel_config_t, ledc_intr_type_t_LEDC_INTR_DISABLE,
        ledc_mode_t_LEDC_LOW_SPEED_MODE, ledc_set_duty, ledc_timer_bit_t_LEDC_TIMER_13_BIT,
        ledc_timer_config, ledc_timer_config_t, ledc_update_duty, ESP_OK,
    };

    let pin = *pins
        .get("pin")
        .ok_or_else(|| Error::config("pwm_out", "missing pin"))?;
    let duty = params
        .get("duty")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| Error::config("pwm_out", "missing or invalid 'duty' (0–100)"))?;
    if duty > 100 {
        return Err(Error::config("pwm_out", "duty must be 0–100"));
    }
    let freq_hz = options
        .get("frequency_hz")
        .and_then(|v| v.as_u64())
        .unwrap_or(PWM_DEFAULT_FREQ_HZ as u64) as u32;

    // Map 0–100 to 0–8191
    let duty_raw = (duty as u32 * LEDC_DUTY_MAX) / 100;
    let speed_mode = ledc_mode_t_LEDC_LOW_SPEED_MODE;
    let channel = ledc_channel as u32;
    let timer_sel = ledc_timer_from_index(ledc_timer_index);

    unsafe {
        // Configure this device's timer (one timer per pwm_out so frequency is independent).
        let timer_conf = ledc_timer_config_t {
            speed_mode,
            duty_resolution: ledc_timer_bit_t_LEDC_TIMER_13_BIT,
            timer_num: timer_sel,
            freq_hz,
            clk_cfg: esp_idf_svc::sys::soc_periph_ledc_clk_src_legacy_t_LEDC_AUTO_CLK,
            deconfigure: false,
        };
        let ret = ledc_timer_config(&timer_conf);
        if ret != ESP_OK {
            return Err(Error::Other {
                source: Box::new(std::io::Error::other(format!(
                    "ledc_timer_config failed: {}",
                    ret
                ))),
                stage: "pwm_out",
            });
        }

        // Configure channel + bind to GPIO and this timer
        let ch_conf = ledc_channel_config_t {
            speed_mode,
            channel,
            timer_sel,
            intr_type: ledc_intr_type_t_LEDC_INTR_DISABLE,
            gpio_num: pin,
            duty: duty_raw,
            hpoint: 0,
            flags: Default::default(),
            ..Default::default()
        };
        let ret = ledc_channel_config(&ch_conf);
        if ret != ESP_OK {
            return Err(Error::Other {
                source: Box::new(std::io::Error::other(format!(
                    "ledc_channel_config failed: {}",
                    ret
                ))),
                stage: "pwm_out",
            });
        }

        ledc_set_duty(speed_mode, channel, duty_raw);
        ledc_update_duty(speed_mode, channel);
    }

    Ok(format!(
        r#"{{"ok":true,"duty":{},"duty_raw":{}}}"#,
        duty, duty_raw
    ))
}

/// ADC1 使用 oneshot 驱动（esp_adc/adc_oneshot.h），替代已弃用的 driver/adc.h legacy API。
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub fn drive_adc_in(pins: &PinConfig, _params: &Value, options: &Value) -> Result<String> {
    use esp_idf_svc::sys::{
        adc_atten_t_ADC_ATTEN_DB_0, adc_atten_t_ADC_ATTEN_DB_11, adc_atten_t_ADC_ATTEN_DB_2_5,
        adc_atten_t_ADC_ATTEN_DB_6, adc_bitwidth_t_ADC_BITWIDTH_12, adc_oneshot_chan_cfg_t,
        adc_oneshot_config_channel, adc_oneshot_del_unit, adc_oneshot_new_unit, adc_oneshot_read,
        adc_oneshot_unit_init_cfg_t, adc_ulp_mode_t_ADC_ULP_MODE_DISABLE, adc_unit_t_ADC_UNIT_1,
        soc_periph_adc_rtc_clk_src_t_ADC_RTC_CLK_SRC_DEFAULT, ESP_OK,
    };

    let pin = *pins
        .get("pin")
        .ok_or_else(|| Error::config("adc_in", "missing pin"))?;

    // ESP32-S3: ADC1 channels map GPIO 1–10 → channel 0–9
    if !(1..=10).contains(&pin) {
        return Err(Error::config(
            "adc_in",
            format!("pin {} not in ADC1 range (GPIO 1–10)", pin),
        ));
    }
    let channel = (pin - 1) as u32;

    let atten_str = options
        .get("atten")
        .and_then(|v| v.as_str())
        .unwrap_or("11db");
    let atten = match atten_str {
        "0db" => adc_atten_t_ADC_ATTEN_DB_0,
        "2.5db" => adc_atten_t_ADC_ATTEN_DB_2_5,
        "6db" => adc_atten_t_ADC_ATTEN_DB_6,
        _ => adc_atten_t_ADC_ATTEN_DB_11, // default 11db
    };

    unsafe {
        let init_cfg = adc_oneshot_unit_init_cfg_t {
            unit_id: adc_unit_t_ADC_UNIT_1,
            clk_src: soc_periph_adc_rtc_clk_src_t_ADC_RTC_CLK_SRC_DEFAULT,
            ulp_mode: adc_ulp_mode_t_ADC_ULP_MODE_DISABLE,
        };
        let mut handle = core::ptr::null_mut();
        let ret = adc_oneshot_new_unit(&init_cfg, &mut handle);
        if ret != ESP_OK {
            return Err(Error::Other {
                source: Box::new(std::io::Error::other(format!(
                    "adc_oneshot_new_unit failed: {}",
                    ret
                ))),
                stage: "adc_in",
            });
        }
        let chan_cfg = adc_oneshot_chan_cfg_t {
            atten,
            bitwidth: adc_bitwidth_t_ADC_BITWIDTH_12,
        };
        let ret = adc_oneshot_config_channel(handle, channel, &chan_cfg);
        if ret != ESP_OK {
            let _ = adc_oneshot_del_unit(handle);
            return Err(Error::Other {
                source: Box::new(std::io::Error::other(format!(
                    "adc_oneshot_config_channel failed: {}",
                    ret
                ))),
                stage: "adc_in",
            });
        }
        let mut raw: i32 = 0;
        let ret = adc_oneshot_read(handle, channel, &mut raw);
        let _ = adc_oneshot_del_unit(handle);
        if ret != ESP_OK {
            return Err(Error::Other {
                source: Box::new(std::io::Error::other(format!(
                    "adc_oneshot_read failed: {}",
                    ret
                ))),
                stage: "adc_in",
            });
        }
        Ok(format!(r#"{{"raw":{}}}"#, raw))
    }
}

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub fn drive_buzzer(pins: &PinConfig, params: &Value) -> Result<String> {
    use esp_idf_svc::sys::{
        gpio_config_t, gpio_int_type_t_GPIO_INTR_DISABLE, gpio_mode_t_GPIO_MODE_OUTPUT,
        gpio_pulldown_t_GPIO_PULLDOWN_DISABLE, gpio_pullup_t_GPIO_PULLUP_DISABLE, gpio_reset_pin,
        gpio_set_level, ESP_OK,
    };

    let pin = *pins
        .get("pin")
        .ok_or_else(|| Error::config("buzzer", "missing pin"))?;

    // Determine duration: beep=true → 100ms, or duration_ms (clamped to MAX)
    let beep = params
        .get("beep")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut duration_ms = if beep {
        BUZZER_BEEP_MS
    } else {
        params
            .get("duration_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(BUZZER_BEEP_MS)
    };
    let clamped = duration_ms > MAX_BUZZER_DURATION_MS;
    if clamped {
        duration_ms = MAX_BUZZER_DURATION_MS;
    }

    unsafe {
        gpio_reset_pin(pin);
        let conf = gpio_config_t {
            pin_bit_mask: 1u64 << pin,
            mode: gpio_mode_t_GPIO_MODE_OUTPUT,
            pull_up_en: gpio_pullup_t_GPIO_PULLUP_DISABLE,
            pull_down_en: gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
            intr_type: gpio_int_type_t_GPIO_INTR_DISABLE,
        };
        let ret = esp_idf_svc::sys::gpio_config(&conf);
        if ret != ESP_OK {
            return Err(Error::Other {
                source: Box::new(std::io::Error::other(format!(
                    "gpio_config failed: {}",
                    ret
                ))),
                stage: "buzzer",
            });
        }
        // Turn on
        gpio_set_level(pin, 1);
    }

    // Non-blocking: spawn a thread to turn off after duration
    let dur = std::time::Duration::from_millis(duration_ms);
    std::thread::Builder::new()
        .name("buzzer_off".into())
        .stack_size(2048)
        .spawn(move || {
            std::thread::sleep(dur);
            unsafe {
                esp_idf_svc::sys::gpio_set_level(pin, 0);
            }
        })
        .map_err(|e| Error::Other {
            source: Box::new(e),
            stage: "buzzer",
        })?;

    if clamped {
        Ok(format!(
            r#"{{"ok":true,"duration_ms":{},"warning":"clamped to max {}ms"}}"#,
            duration_ms, MAX_BUZZER_DURATION_MS
        ))
    } else {
        Ok(format!(r#"{{"ok":true,"duration_ms":{}}}"#, duration_ms))
    }
}

// ── Host target: stub drivers for cargo check / clippy ──

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub fn drive_gpio_out(pins: &PinConfig, params: &Value) -> Result<String> {
    let pin = *pins
        .get("pin")
        .ok_or_else(|| Error::config("gpio_out", "missing pin"))?;
    let value = params
        .get("value")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| Error::config("gpio_out", "missing or invalid 'value' (0 or 1)"))?;
    if value != 0 && value != 1 {
        return Err(Error::config("gpio_out", "value must be 0 or 1"));
    }
    log::info!("[gpio_out] stub: pin={} value={}", pin, value);
    Ok(format!(
        r#"{{"ok":true,"actual_value":{},"stub":true}}"#,
        value
    ))
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub fn drive_gpio_in(pins: &PinConfig, _params: &Value, options: &Value) -> Result<String> {
    let pin = *pins
        .get("pin")
        .ok_or_else(|| Error::config("gpio_in", "missing pin"))?;
    let _pull = options
        .get("pull")
        .and_then(|v| v.as_str())
        .unwrap_or("none");
    log::info!("[gpio_in] stub: pin={}", pin);
    Ok(r#"{"value":0,"stub":true}"#.to_string())
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub fn drive_pwm_out(
    pins: &PinConfig,
    params: &Value,
    options: &Value,
    ledc_channel: u8,
    ledc_timer_index: u8,
) -> Result<String> {
    let pin = *pins
        .get("pin")
        .ok_or_else(|| Error::config("pwm_out", "missing pin"))?;
    let duty = params
        .get("duty")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| Error::config("pwm_out", "missing or invalid 'duty' (0–100)"))?;
    if duty > 100 {
        return Err(Error::config("pwm_out", "duty must be 0–100"));
    }
    let freq = options
        .get("frequency_hz")
        .and_then(|v| v.as_u64())
        .unwrap_or(PWM_DEFAULT_FREQ_HZ as u64);
    let duty_raw = (duty as u32 * LEDC_DUTY_MAX) / 100;
    log::info!(
        "[pwm_out] stub: pin={} ch={} timer={} duty={}% raw={} freq={}",
        pin,
        ledc_channel,
        ledc_timer_index,
        duty,
        duty_raw,
        freq
    );
    Ok(format!(
        r#"{{"ok":true,"duty":{},"duty_raw":{},"stub":true}}"#,
        duty, duty_raw
    ))
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub fn drive_adc_in(pins: &PinConfig, _params: &Value, options: &Value) -> Result<String> {
    let pin = *pins
        .get("pin")
        .ok_or_else(|| Error::config("adc_in", "missing pin"))?;
    if !(1..=10).contains(&pin) {
        return Err(Error::config(
            "adc_in",
            format!("pin {} not in ADC1 range (GPIO 1–10)", pin),
        ));
    }
    let _atten = options
        .get("atten")
        .and_then(|v| v.as_str())
        .unwrap_or("11db");
    log::info!("[adc_in] stub: pin={}", pin);
    Ok(r#"{"raw":2048,"stub":true}"#.to_string())
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub fn drive_buzzer(pins: &PinConfig, params: &Value) -> Result<String> {
    let pin = *pins
        .get("pin")
        .ok_or_else(|| Error::config("buzzer", "missing pin"))?;
    let beep = params
        .get("beep")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut duration_ms = if beep {
        BUZZER_BEEP_MS
    } else {
        params
            .get("duration_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(BUZZER_BEEP_MS)
    };
    let clamped = duration_ms > MAX_BUZZER_DURATION_MS;
    if clamped {
        duration_ms = MAX_BUZZER_DURATION_MS;
    }
    log::info!(
        "[buzzer] stub: pin={} duration_ms={} clamped={}",
        pin,
        duration_ms,
        clamped
    );
    if clamped {
        Ok(format!(
            r#"{{"ok":true,"duration_ms":{},"warning":"clamped to max {}ms","stub":true}}"#,
            duration_ms, MAX_BUZZER_DURATION_MS
        ))
    } else {
        Ok(format!(
            r#"{{"ok":true,"duration_ms":{},"stub":true}}"#,
            duration_ms
        ))
    }
}

// ── I2C drivers (ESP-IDF `driver/i2c_master.h`, IDF 5.4+) ──

/// I2C master 总线状态：bus handle + 按 7 位地址缓存的 device handle。
/// I2C master bus state: bus handle and per-address device handles (lazy).
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub(crate) struct I2cBusState {
    bus: esp_idf_svc::sys::i2c_master_bus_handle_t,
    devices: std::collections::HashMap<u8, esp_idf_svc::sys::i2c_master_dev_handle_t>,
    freq_hz: u32,
}

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
unsafe impl Send for I2cBusState {}

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
impl I2cBusState {
    /// 使用配置的 SDA/SCL/频率创建 I2C master bus（端口 I2C_NUM_0）。
    pub(crate) fn new(sda: i32, scl: i32, freq_hz: u32) -> Result<Self> {
        use esp_idf_svc::sys::{i2c_new_master_bus, ESP_OK};

        let mut bus_config: esp_idf_svc::sys::i2c_master_bus_config_t =
            unsafe { core::mem::zeroed() };
        // `i2c_port_num_t`: I2C_NUM_0；全零的 clk_source union 为默认时钟源。
        bus_config.i2c_port = 0;
        bus_config.sda_io_num = sda;
        bus_config.scl_io_num = scl;
        bus_config.glitch_ignore_cnt = 7;
        bus_config.intr_priority = 0;
        bus_config.trans_queue_depth = 0;

        let mut bus: esp_idf_svc::sys::i2c_master_bus_handle_t = core::ptr::null_mut();
        let ret = unsafe { i2c_new_master_bus(&bus_config, &mut bus) };
        if ret != ESP_OK {
            return Err(Error::esp("i2c_init", ret));
        }
        Ok(Self {
            bus,
            devices: std::collections::HashMap::new(),
            freq_hz,
        })
    }

    fn ensure_device(&mut self, addr: u8) -> Result<esp_idf_svc::sys::i2c_master_dev_handle_t> {
        use esp_idf_svc::sys::{
            i2c_addr_bit_len_t_I2C_ADDR_BIT_LEN_7, i2c_master_bus_add_device, ESP_OK,
        };

        if let Some(&h) = self.devices.get(&addr) {
            return Ok(h);
        }
        let mut dev_cfg: esp_idf_svc::sys::i2c_device_config_t = unsafe { core::mem::zeroed() };
        dev_cfg.dev_addr_length = i2c_addr_bit_len_t_I2C_ADDR_BIT_LEN_7;
        dev_cfg.device_address = u16::from(addr);
        dev_cfg.scl_speed_hz = self.freq_hz;

        let mut dev: esp_idf_svc::sys::i2c_master_dev_handle_t = core::ptr::null_mut();
        let ret = unsafe { i2c_master_bus_add_device(self.bus, &dev_cfg, &mut dev) };
        if ret != ESP_OK {
            return Err(Error::esp("i2c_add_device", ret));
        }
        self.devices.insert(addr, dev);
        Ok(dev)
    }

    /// 寄存器读：写寄存器地址后 repeated start 再读（`i2c_master_transmit_receive`）。
    pub(crate) fn read(&mut self, addr: u8, register: u8, len: usize) -> Result<Vec<u8>> {
        use esp_idf_svc::sys::{i2c_master_transmit_receive, ESP_OK};

        let dev = self.ensure_device(addr)?;
        let write_buf = [register];
        let mut read_buf = vec![0u8; len];
        let ret = unsafe {
            i2c_master_transmit_receive(
                dev,
                write_buf.as_ptr(),
                write_buf.len(),
                read_buf.as_mut_ptr(),
                read_buf.len(),
                -1,
            )
        };
        if ret != ESP_OK {
            return Err(Error::esp("i2c_read", ret));
        }
        Ok(read_buf)
    }

    /// 寄存器写：单帧发送 `[register, ...data]`。
    pub(crate) fn write(&mut self, addr: u8, register: u8, data: &[u8]) -> Result<()> {
        use esp_idf_svc::sys::{i2c_master_transmit, ESP_OK};

        let dev = self.ensure_device(addr)?;
        let mut buf = Vec::with_capacity(1 + data.len());
        buf.push(register);
        buf.extend_from_slice(data);
        let ret = unsafe { i2c_master_transmit(dev, buf.as_ptr(), buf.len(), -1) };
        if ret != ESP_OK {
            return Err(Error::esp("i2c_write", ret));
        }
        Ok(())
    }
}

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
impl Drop for I2cBusState {
    fn drop(&mut self) {
        use esp_idf_svc::sys::{i2c_del_master_bus, i2c_master_bus_rm_device};

        for (_, h) in self.devices.drain() {
            unsafe {
                let _ = i2c_master_bus_rm_device(h);
            }
        }
        if !self.bus.is_null() {
            unsafe {
                let _ = i2c_del_master_bus(self.bus);
            }
        }
    }
}

/// I2C 读取：Host stub。
#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub fn drive_i2c_read(addr: u8, register: u8, len: usize) -> Result<Vec<u8>> {
    log::info!(
        "[i2c_read] stub: addr=0x{:02X} reg=0x{:02X} len={}",
        addr,
        register,
        len
    );
    Ok(vec![0u8; len])
}

/// I2C 写入：Host stub。
#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub fn drive_i2c_write(addr: u8, register: u8, data: &[u8]) -> Result<()> {
    log::info!(
        "[i2c_write] stub: addr=0x{:02X} reg=0x{:02X} data={:?}",
        addr,
        register,
        data
    );
    Ok(())
}
