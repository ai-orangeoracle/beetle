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
    /// ST7735 / ST7735R / ST7735S 家族（寄存器兼容）。
    St7735,
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
    /// 连续失败次数（F5: 通道失败计数）。
    pub consecutive_failures: u32,
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
    /// 水平边距（与 240×240 参考设计等比缩放）。
    /// Horizontal margin scaled from 240×240 reference layout.
    pub margin_x: u16,
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
        /// F3: 系统运行时间（秒）。
        uptime_secs: u64,
        /// F4: Busy 呼吸动画相位。
        busy_phase: bool,
        /// F6: 最近一次 LLM 调用延迟（毫秒），0 表示无数据。
        llm_last_ms: u32,
        /// F7: 错误闪烁标志（本轮有新错误时为 true）。
        error_flash: bool,
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
        /// F6: 最近一次 LLM 调用延迟（毫秒），0 表示无数据。
        llm_last_ms: u32,
        /// F7: 错误闪烁标志。
        error_flash: bool,
    },
    UpdateChannels {
        channels: [DisplayChannelStatus; 5],
    },
    /// F8: 启动进度条。stage: 0=WiFi前, 1=WiFi后, 2=SNTP后, 3=Channels后, 4=Agent前。
    UpdateBootProgress {
        stage: u8,
    },
    Clear,
}

/// 按 `width`×`height` 从 240×240 参考设计等比例计算仪表盘布局。
/// Computes dashboard layout proportional to a 240×240 reference design.
pub fn compute_layout(width: u16, height: u16) -> DisplayLayout {
    const REF: u32 = 240;
    let w = width as u32;
    let h = height as u32;
    let dim_min = w.min(h);

    let icon_left = (w * 12 / REF) as u16;
    let icon_size = (dim_min * 64 / REF).max(16) as u16;
    let gap = icon_left;

    DisplayLayout {
        header_top: (h * 16 / REF) as u16,
        icon_left,
        icon_size,
        title_left: icon_left.saturating_add(icon_size).saturating_add(gap),
        title_top: (h * 18 / REF) as u16,
        subtitle_top: (h * 44 / REF) as u16,
        middle_top: (h * 104 / REF) as u16,
        footer_top: (h * 168 / REF) as u16,
        margin_x: ((w * 8 / REF).max(2)) as u16,
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
    1
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
    if cfg.spi.host != 1 && cfg.spi.host != 2 {
        return Err(Error::config(
            "display",
            "DISPLAY_CONFIG_INVALID_SPI_HOST: host must be 1 or 2",
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
