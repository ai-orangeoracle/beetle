//! GPIO 工具：gpio_read / gpio_write；仅 feature "gpio" 且 ESP 目标时编译。
//! GPIO tools: gpio_read / gpio_write; only when feature "gpio" and ESP target.

use crate::error::{Error, Result};
use crate::tools::{Tool, ToolContext};
use serde_json::json;

/// 允许的 GPIO 引脚列表（白名单，避免系统关键引脚）。
const ALLOWED_PINS: [i32; 2] = [2, 13];

fn pin_allowed(pin: i32) -> bool {
    ALLOWED_PINS.contains(&pin)
}

#[cfg(all(feature = "gpio", any(target_arch = "xtensa", target_arch = "riscv32")))]
fn gpio_read_level(pin: i32) -> Result<i32> {
    use esp_idf_svc::sys::{gpio_config_t, gpio_get_level, gpio_mode_t_GPIO_MODE_INPUT, gpio_reset_pin};
    unsafe {
        gpio_reset_pin(pin);
        let conf = gpio_config_t {
            pin_bit_mask: 1u64 << pin,
            mode: gpio_mode_t_GPIO_MODE_INPUT,
            pull_up_en: esp_idf_svc::sys::gpio_pullup_t_GPIO_PULLUP_DISABLE,
            pull_down_en: esp_idf_svc::sys::gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
            intr_type: esp_idf_svc::sys::gpio_int_type_t_GPIO_INTR_DISABLE,
        };
        let ret = esp_idf_svc::sys::gpio_config(&conf);
        if ret != esp_idf_svc::sys::ESP_OK {
            return Err(Error::Other {
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("gpio_config failed {}", ret),
                )),
                stage: "gpio_read",
            });
        }
        let level = gpio_get_level(pin);
        Ok(level)
    }
}

#[cfg(all(feature = "gpio", any(target_arch = "xtensa", target_arch = "riscv32")))]
fn gpio_write_level(pin: i32, level: i32) -> Result<()> {
    use esp_idf_svc::sys::{gpio_config_t, gpio_mode_t_GPIO_MODE_OUTPUT, gpio_reset_pin, gpio_set_level};
    unsafe {
        gpio_reset_pin(pin);
        let conf = gpio_config_t {
            pin_bit_mask: 1u64 << pin,
            mode: gpio_mode_t_GPIO_MODE_OUTPUT,
            pull_up_en: esp_idf_svc::sys::gpio_pullup_t_GPIO_PULLUP_DISABLE,
            pull_down_en: esp_idf_svc::sys::gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
            intr_type: esp_idf_svc::sys::gpio_int_type_t_GPIO_INTR_DISABLE,
        };
        let ret = esp_idf_svc::sys::gpio_config(&conf);
        if ret != esp_idf_svc::sys::ESP_OK {
            return Err(Error::Other {
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("gpio_config failed {}", ret),
                )),
                stage: "gpio_write",
            });
        }
        gpio_set_level(pin, if level != 0 { 1 } else { 0 });
        Ok(())
    }
}

#[cfg(all(feature = "gpio", any(target_arch = "xtensa", target_arch = "riscv32")))]
pub struct GpioReadTool;

#[cfg(all(feature = "gpio", any(target_arch = "xtensa", target_arch = "riscv32")))]
impl Tool for GpioReadTool {
    fn name(&self) -> &str {
        "gpio_read"
    }
    fn description(&self) -> &str {
        "Read GPIO pin level (0 or 1). Only pins 2, 13 are allowed."
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "pin": { "type": "integer", "description": "GPIO pin number (2 or 13)" }
            },
            "required": ["pin"]
        })
    }
    fn execute(&self, args: &str, _ctx: &mut dyn ToolContext) -> Result<String> {
        let m = crate::tools::parse_tool_args(args, "gpio_read")?;
        let pin = m
            .get("pin")
            .and_then(|p| p.as_i64())
            .filter(|&p| p >= 0 && p <= 255)
            .map(|p| p as i32)
            .ok_or_else(|| Error::config("gpio_read", "invalid or missing pin"))?;
        if !pin_allowed(pin) {
            return Err(Error::config(
                "gpio_read",
                "pin not in allowed list (2, 13)",
            ));
        }
        let level = gpio_read_level(pin)?;
        Ok(if level != 0 { "1" } else { "0" }.to_string())
    }
}

#[cfg(all(feature = "gpio", any(target_arch = "xtensa", target_arch = "riscv32")))]
pub struct GpioWriteTool;

#[cfg(all(feature = "gpio", any(target_arch = "xtensa", target_arch = "riscv32")))]
impl Tool for GpioWriteTool {
    fn name(&self) -> &str {
        "gpio_write"
    }
    fn description(&self) -> &str {
        "Set GPIO pin output level (0 or 1). Only pins 2, 13 are allowed."
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "pin": { "type": "integer", "description": "GPIO pin number (2 or 13)" },
                "value": { "type": "integer", "description": "0 or 1" }
            },
            "required": ["pin", "value"]
        })
    }
    fn execute(&self, args: &str, _ctx: &mut dyn ToolContext) -> Result<String> {
        let m = crate::tools::parse_tool_args(args, "gpio_write")?;
        let pin = m
            .get("pin")
            .and_then(|p| p.as_i64())
            .filter(|&p| p >= 0 && p <= 255)
            .map(|p| p as i32)
            .ok_or_else(|| Error::config("gpio_write", "invalid or missing pin"))?;
        let value = m
            .get("value")
            .and_then(|x| x.as_i64())
            .filter(|&x| x == 0 || x == 1)
            .ok_or_else(|| Error::config("gpio_write", "invalid or missing value (0 or 1)"))?;
        if !pin_allowed(pin) {
            return Err(Error::config(
                "gpio_write",
                "pin not in allowed list (2, 13)",
            ));
        }
        gpio_write_level(pin, value as i32)?;
        Ok("ok".to_string())
    }
}

#[cfg(not(all(feature = "gpio", any(target_arch = "xtensa", target_arch = "riscv32"))))]
pub struct GpioReadTool;

#[cfg(not(all(feature = "gpio", any(target_arch = "xtensa", target_arch = "riscv32"))))]
impl Tool for GpioReadTool {
    fn name(&self) -> &str {
        "gpio_read"
    }
    fn description(&self) -> &str {
        "GPIO read (disabled on this build)"
    }
    fn schema(&self) -> serde_json::Value {
        json!({})
    }
    fn execute(&self, _args: &str, _ctx: &mut dyn ToolContext) -> Result<String> {
        Err(Error::config("gpio_read", "gpio feature not enabled or not ESP target"))
    }
}

#[cfg(not(all(feature = "gpio", any(target_arch = "xtensa", target_arch = "riscv32"))))]
pub struct GpioWriteTool;

#[cfg(not(all(feature = "gpio", any(target_arch = "xtensa", target_arch = "riscv32"))))]
impl Tool for GpioWriteTool {
    fn name(&self) -> &str {
        "gpio_write"
    }
    fn description(&self) -> &str {
        "GPIO write (disabled on this build)"
    }
    fn schema(&self) -> serde_json::Value {
        json!({})
    }
    fn execute(&self, _args: &str, _ctx: &mut dyn ToolContext) -> Result<String> {
        Err(Error::config("gpio_write", "gpio feature not enabled or not ESP target"))
    }
}
