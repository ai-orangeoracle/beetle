//! Display configuration and command types.
//! 显示配置与指令模型（平台无关纯数据）。

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

pub const DISPLAY_CONFIG_VERSION: u32 = 1;
pub const DISPLAY_DIM_MIN: u16 = 1;
pub const DISPLAY_DIM_MAX: u16 = 480;
pub const DISPLAY_OFFSET_MIN: i16 = -480;
pub const DISPLAY_OFFSET_MAX: i16 = 480;
pub const DISPLAY_SPI_FREQ_MIN: u32 = 1_000_000;
pub const DISPLAY_SPI_FREQ_MAX: u32 = 80_000_000;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DisplayDriver {
    St7789,
    Ili9341,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DisplayBus {
    Spi,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DisplayColorOrder {
    Rgb,
    Bgr,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum DisplayPressureLevel {
    Normal,
    Cautious,
    Critical,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DisplaySpiConfig {
    #[serde(default = "default_spi_host")]
    pub host: u8,
    pub sclk: i32,
    pub mosi: i32,
    pub cs: i32,
    pub dc: i32,
    #[serde(default)]
    pub rst: Option<i32>,
    #[serde(default)]
    pub bl: Option<i32>,
    #[serde(default = "default_spi_freq_hz")]
    pub freq_hz: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DisplayConfig {
    #[serde(default = "default_config_version")]
    pub version: u32,
    pub enabled: bool,
    pub driver: DisplayDriver,
    pub bus: DisplayBus,
    pub width: u16,
    pub height: u16,
    #[serde(default = "default_rotation")]
    pub rotation: u16,
    #[serde(default = "default_color_order")]
    pub color_order: DisplayColorOrder,
    #[serde(default)]
    pub invert_colors: bool,
    #[serde(default)]
    pub offset_x: i16,
    #[serde(default)]
    pub offset_y: i16,
    pub spi: DisplaySpiConfig,
    /// 空闲自动熄屏超时（秒）。0 = 禁用。
    /// Auto-sleep timeout in seconds. 0 = disabled.
    #[serde(default)]
    pub sleep_timeout_secs: u16,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct DisplayChannelStatus {
    pub name: &'static str,
    pub enabled: bool,
    pub healthy: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum DisplaySystemState {
    Booting,
    NoWifi,
    Idle,
    Busy,
    Fault,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DisplayLayout {
    pub header_top: u16,
    pub icon_left: u16,
    pub icon_size: u16,
    pub title_left: u16,
    pub title_top: u16,
    pub subtitle_top: u16,
    pub middle_top: u16,
    pub footer_top: u16,
}

#[derive(Clone, Debug, Serialize)]
pub enum DisplayCommand {
    RefreshDashboard {
        state: DisplaySystemState,
        wifi_connected: bool,
        ip_address: Option<String>,
        channels: [DisplayChannelStatus; 5],
        pressure: DisplayPressureLevel,
        heap_percent: u8,
        messages_in: u32,
        messages_out: u32,
        last_active_epoch_secs: u32,
    },
    UpdateIp {
        ip: String,
    },
    UpdatePressure {
        level: DisplayPressureLevel,
        heap_percent: u8,
        messages_in: u32,
        messages_out: u32,
        last_active_epoch_secs: u32,
    },
    UpdateChannels {
        channels: [DisplayChannelStatus; 5],
    },
    Clear,
}

pub fn default_display_layout() -> DisplayLayout {
    DisplayLayout {
        header_top: 16,
        icon_left: 12,
        icon_size: 64,
        title_left: 88,
        title_top: 18,
        subtitle_top: 44,
        middle_top: 104,
        footer_top: 168,
    }
}

fn default_config_version() -> u32 {
    DISPLAY_CONFIG_VERSION
}

fn default_rotation() -> u16 {
    0
}

fn default_color_order() -> DisplayColorOrder {
    DisplayColorOrder::Rgb
}

fn default_spi_host() -> u8 {
    2
}

fn default_spi_freq_hz() -> u32 {
    40_000_000
}

pub fn default_disabled_display_config() -> DisplayConfig {
    DisplayConfig {
        version: DISPLAY_CONFIG_VERSION,
        enabled: false,
        driver: DisplayDriver::St7789,
        bus: DisplayBus::Spi,
        width: 240,
        height: 240,
        rotation: 0,
        color_order: DisplayColorOrder::Rgb,
        invert_colors: false,
        offset_x: 0,
        offset_y: 0,
        spi: DisplaySpiConfig {
            host: default_spi_host(),
            sclk: 42,
            mosi: 41,
            cs: 21,
            dc: 40,
            rst: None,
            bl: None,
            freq_hz: default_spi_freq_hz(),
        },
        sleep_timeout_secs: 0,
    }
}

pub fn validate_display_config_core(cfg: &DisplayConfig) -> Result<()> {
    if cfg.version != DISPLAY_CONFIG_VERSION {
        return Err(Error::config(
            "display",
            format!(
                "DISPLAY_CONFIG_INVALID_VERSION: expected {}, got {}",
                DISPLAY_CONFIG_VERSION, cfg.version
            ),
        ));
    }
    if !cfg.enabled {
        return Ok(());
    }
    if !(DISPLAY_DIM_MIN..=DISPLAY_DIM_MAX).contains(&cfg.width)
        || !(DISPLAY_DIM_MIN..=DISPLAY_DIM_MAX).contains(&cfg.height)
    {
        return Err(Error::config(
            "display",
            "DISPLAY_CONFIG_INVALID_DIMENSION: width/height must be 1..=480",
        ));
    }
    if !matches!(cfg.rotation, 0 | 90 | 180 | 270) {
        return Err(Error::config(
            "display",
            "DISPLAY_CONFIG_INVALID_ROTATION: must be one of 0/90/180/270",
        ));
    }
    if !(DISPLAY_OFFSET_MIN..=DISPLAY_OFFSET_MAX).contains(&cfg.offset_x)
        || !(DISPLAY_OFFSET_MIN..=DISPLAY_OFFSET_MAX).contains(&cfg.offset_y)
    {
        return Err(Error::config(
            "display",
            "DISPLAY_CONFIG_INVALID_OFFSET: offset must be -480..=480",
        ));
    }
    if cfg.spi.host != 2 && cfg.spi.host != 3 {
        return Err(Error::config(
            "display",
            "DISPLAY_CONFIG_INVALID_SPI_HOST: host must be 2 or 3",
        ));
    }
    if !(DISPLAY_SPI_FREQ_MIN..=DISPLAY_SPI_FREQ_MAX).contains(&cfg.spi.freq_hz) {
        return Err(Error::config(
            "display",
            "DISPLAY_CONFIG_INVALID_TIMING: spi.freq_hz must be 1_000_000..=80_000_000",
        ));
    }
    Ok(())
}
