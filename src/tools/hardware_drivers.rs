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
        gpio_config_t, gpio_get_level, gpio_mode_t_GPIO_MODE_OUTPUT, gpio_reset_pin,
        gpio_set_level, gpio_pullup_t_GPIO_PULLUP_DISABLE,
        gpio_pulldown_t_GPIO_PULLDOWN_DISABLE, gpio_int_type_t_GPIO_INTR_DISABLE, ESP_OK,
    };

    let pin = *pins.get("pin").ok_or_else(|| Error::config("gpio_out", "missing pin"))?;
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
                source: Box::new(std::io::Error::other(
                    format!("gpio_config failed: {}", ret),
                )),
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
        gpio_config_t, gpio_get_level, gpio_mode_t_GPIO_MODE_INPUT, gpio_reset_pin,
        gpio_pullup_t_GPIO_PULLUP_ENABLE,
        gpio_pulldown_t_GPIO_PULLDOWN_DISABLE, gpio_pulldown_t_GPIO_PULLDOWN_ENABLE,
        gpio_pullup_t_GPIO_PULLUP_DISABLE,
        gpio_int_type_t_GPIO_INTR_DISABLE, ESP_OK,
    };

    let pin = *pins.get("pin").ok_or_else(|| Error::config("gpio_in", "missing pin"))?;
    let pull = options
        .get("pull")
        .and_then(|v| v.as_str())
        .unwrap_or("none");

    let (pull_up, pull_down) = match pull {
        "up" => (gpio_pullup_t_GPIO_PULLUP_ENABLE, gpio_pulldown_t_GPIO_PULLDOWN_DISABLE),
        "down" => (gpio_pullup_t_GPIO_PULLUP_DISABLE, gpio_pulldown_t_GPIO_PULLDOWN_ENABLE),
        _ => (gpio_pullup_t_GPIO_PULLUP_DISABLE, gpio_pulldown_t_GPIO_PULLDOWN_DISABLE),
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
                source: Box::new(std::io::Error::other(
                    format!("gpio_config failed: {}", ret),
                )),
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
    use esp_idf_svc::sys::ledc_timer_t;
    // C enum LEDC_TIMER_0=0 .. LEDC_TIMER_3=3; repr(C) enum is typically 4 bytes.
    unsafe { core::mem::transmute::<u32, ledc_timer_t>((i.min(3)) as u32) }
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
        ledc_channel_config_t, ledc_timer_config_t,
        ledc_mode_t_LEDC_LOW_SPEED_MODE,
        ledc_timer_bit_t_LEDC_TIMER_13_BIT, ledc_intr_type_t_LEDC_INTR_DISABLE,
        ledc_channel_config, ledc_timer_config,
        ledc_set_duty, ledc_update_duty, ESP_OK,
    };

    let pin = *pins.get("pin").ok_or_else(|| Error::config("pwm_out", "missing pin"))?;
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
                source: Box::new(std::io::Error::other(
                    format!("ledc_timer_config failed: {}", ret),
                )),
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
        };
        let ret = ledc_channel_config(&ch_conf);
        if ret != ESP_OK {
            return Err(Error::Other {
                source: Box::new(std::io::Error::other(
                    format!("ledc_channel_config failed: {}", ret),
                )),
                stage: "pwm_out",
            });
        }

        ledc_set_duty(speed_mode, channel, duty_raw);
        ledc_update_duty(speed_mode, channel);
    }

    Ok(format!(r#"{{"ok":true,"duty":{},"duty_raw":{}}}"#, duty, duty_raw))
}

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub fn drive_adc_in(pins: &PinConfig, _params: &Value, options: &Value) -> Result<String> {
    use esp_idf_svc::sys::{
        adc1_config_channel_atten, adc1_config_width, adc1_get_raw,
        adc_bits_width_t_ADC_WIDTH_BIT_12,
        adc_atten_t_ADC_ATTEN_DB_0, adc_atten_t_ADC_ATTEN_DB_2_5,
        adc_atten_t_ADC_ATTEN_DB_6, adc_atten_t_ADC_ATTEN_DB_11,
        ESP_OK,
    };

    let pin = *pins.get("pin").ok_or_else(|| Error::config("adc_in", "missing pin"))?;

    // ESP32-S3: ADC1 channels map GPIO 1–10 → channel 0–9
    if !(1..=10).contains(&pin) {
        return Err(Error::config(
            "adc_in",
            format!("pin {} not in ADC1 range (GPIO 1–10)", pin),
        ));
    }
    let adc_channel = (pin - 1) as u32;

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
        let ret = adc1_config_width(adc_bits_width_t_ADC_WIDTH_BIT_12);
        if ret != ESP_OK {
            return Err(Error::Other {
                source: Box::new(std::io::Error::other(
                    format!("adc1_config_width failed: {}", ret),
                )),
                stage: "adc_in",
            });
        }
        let ret = adc1_config_channel_atten(adc_channel, atten);
        if ret != ESP_OK {
            return Err(Error::Other {
                source: Box::new(std::io::Error::other(
                    format!("adc1_config_channel_atten failed: {}", ret),
                )),
                stage: "adc_in",
            });
        }
        let raw = adc1_get_raw(adc_channel);
        if raw < 0 {
            return Err(Error::Other {
                source: Box::new(std::io::Error::other(
                    format!("adc1_get_raw returned error: {}", raw),
                )),
                stage: "adc_in",
            });
        }
        Ok(format!(r#"{{"raw":{}}}"#, raw))
    }
}

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub fn drive_buzzer(pins: &PinConfig, params: &Value) -> Result<String> {
    use esp_idf_svc::sys::{
        gpio_config_t, gpio_mode_t_GPIO_MODE_OUTPUT, gpio_reset_pin,
        gpio_set_level, gpio_pullup_t_GPIO_PULLUP_DISABLE,
        gpio_pulldown_t_GPIO_PULLDOWN_DISABLE, gpio_int_type_t_GPIO_INTR_DISABLE, ESP_OK,
    };

    let pin = *pins.get("pin").ok_or_else(|| Error::config("buzzer", "missing pin"))?;

    // Determine duration: beep=true → 100ms, or duration_ms (clamped to MAX)
    let beep = params.get("beep").and_then(|v| v.as_bool()).unwrap_or(false);
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
                source: Box::new(std::io::Error::other(
                    format!("gpio_config failed: {}", ret),
                )),
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
    let pin = *pins.get("pin").ok_or_else(|| Error::config("gpio_out", "missing pin"))?;
    let value = params
        .get("value")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| Error::config("gpio_out", "missing or invalid 'value' (0 or 1)"))?;
    if value != 0 && value != 1 {
        return Err(Error::config("gpio_out", "value must be 0 or 1"));
    }
    log::info!("[gpio_out] stub: pin={} value={}", pin, value);
    Ok(format!(r#"{{"ok":true,"actual_value":{},"stub":true}}"#, value))
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub fn drive_gpio_in(pins: &PinConfig, _params: &Value, options: &Value) -> Result<String> {
    let pin = *pins.get("pin").ok_or_else(|| Error::config("gpio_in", "missing pin"))?;
    let _pull = options.get("pull").and_then(|v| v.as_str()).unwrap_or("none");
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
    let pin = *pins.get("pin").ok_or_else(|| Error::config("pwm_out", "missing pin"))?;
    let duty = params
        .get("duty")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| Error::config("pwm_out", "missing or invalid 'duty' (0–100)"))?;
    if duty > 100 {
        return Err(Error::config("pwm_out", "duty must be 0–100"));
    }
    let freq = options.get("frequency_hz").and_then(|v| v.as_u64()).unwrap_or(PWM_DEFAULT_FREQ_HZ as u64);
    let duty_raw = (duty as u32 * LEDC_DUTY_MAX) / 100;
    log::info!("[pwm_out] stub: pin={} ch={} timer={} duty={}% raw={} freq={}", pin, ledc_channel, ledc_timer_index, duty, duty_raw, freq);
    Ok(format!(r#"{{"ok":true,"duty":{},"duty_raw":{},"stub":true}}"#, duty, duty_raw))
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub fn drive_adc_in(pins: &PinConfig, _params: &Value, options: &Value) -> Result<String> {
    let pin = *pins.get("pin").ok_or_else(|| Error::config("adc_in", "missing pin"))?;
    if !(1..=10).contains(&pin) {
        return Err(Error::config("adc_in", format!("pin {} not in ADC1 range (GPIO 1–10)", pin)));
    }
    let _atten = options.get("atten").and_then(|v| v.as_str()).unwrap_or("11db");
    log::info!("[adc_in] stub: pin={}", pin);
    Ok(r#"{"raw":2048,"stub":true}"#.to_string())
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub fn drive_buzzer(pins: &PinConfig, params: &Value) -> Result<String> {
    let pin = *pins.get("pin").ok_or_else(|| Error::config("buzzer", "missing pin"))?;
    let beep = params.get("beep").and_then(|v| v.as_bool()).unwrap_or(false);
    let mut duration_ms = if beep {
        BUZZER_BEEP_MS
    } else {
        params.get("duration_ms").and_then(|v| v.as_u64()).unwrap_or(BUZZER_BEEP_MS)
    };
    let clamped = duration_ms > MAX_BUZZER_DURATION_MS;
    if clamped {
        duration_ms = MAX_BUZZER_DURATION_MS;
    }
    log::info!("[buzzer] stub: pin={} duration_ms={} clamped={}", pin, duration_ms, clamped);
    if clamped {
        Ok(format!(
            r#"{{"ok":true,"duration_ms":{},"warning":"clamped to max {}ms","stub":true}}"#,
            duration_ms, MAX_BUZZER_DURATION_MS
        ))
    } else {
        Ok(format!(r#"{{"ok":true,"duration_ms":{},"stub":true}}"#, duration_ms))
    }
}
