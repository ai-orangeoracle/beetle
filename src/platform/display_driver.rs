//! Display runtime state for platform implementations.
//! 平台层显示运行态封装 — SPI 硬件初始化 + embedded-graphics 渲染。

use crate::display::{
    DisplayChannelStatus, DisplayCommand, DisplayConfig, DisplayPressureLevel, DisplaySystemState,
};
use crate::error::Result;
use std::time::Instant;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  ESP32 target — real SPI backend
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
mod esp_backend {
    use super::*;
    use crate::display::{DisplayColorOrder, DisplayDriver};
    use crate::platform::heap;
    use embedded_graphics_core::{
        draw_target::DrawTarget,
        geometry::{OriginDimensions, Size},
        pixelcolor::Rgb565,
        Pixel,
    };
    use esp_idf_svc::sys::*;

    /// SPI-connected display backend (ST7789 / ILI9341).
    /// Framebuffer lives in PSRAM; rendering via `embedded-graphics` `DrawTarget`.
    pub(super) struct SpiDisplayBackend {
        spi_host: u32,
        spi_handle: spi_device_handle_t,
        dc_pin: i32,
        width: u16,
        height: u16,
        framebuf: *mut u8,
        framebuf_len: usize,
    }

    // SAFETY: SpiDisplayBackend is only accessed from the display thread (behind Mutex).
    unsafe impl Send for SpiDisplayBackend {}

    impl Drop for SpiDisplayBackend {
        fn drop(&mut self) {
            unsafe {
                spi_bus_remove_device(self.spi_handle);
                spi_bus_free(self.spi_host);
                heap::free_spiram_buffer(self.framebuf);
            }
        }
    }

    impl SpiDisplayBackend {
        pub fn new(config: &DisplayConfig) -> Result<Self> {
            let spi = &config.spi;
            let width = config.width;
            let height = config.height;
            let framebuf_len = width as usize * height as usize * 2;

            let framebuf = heap::alloc_spiram_buffer(framebuf_len).ok_or_else(|| {
                crate::error::Error::config(
                    "display_init",
                    format!(
                        "failed to allocate {}B PSRAM framebuffer",
                        framebuf_len
                    ),
                )
            })?;
            // Zero the framebuffer
            unsafe { core::ptr::write_bytes(framebuf, 0, framebuf_len) };

            // --- Configure DC pin as GPIO output ---
            unsafe {
                let dc_conf = gpio_config_t {
                    pin_bit_mask: 1u64 << spi.dc,
                    mode: gpio_mode_t_GPIO_MODE_OUTPUT,
                    pull_up_en: gpio_pullup_t_GPIO_PULLUP_DISABLE,
                    pull_down_en: gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
                    intr_type: gpio_int_type_t_GPIO_INTR_DISABLE,
                };
                let ret = gpio_config(&dc_conf);
                if ret != ESP_OK {
                    heap::free_spiram_buffer(framebuf);
                    return Err(crate::error::Error::Esp {
                        code: ret,
                        stage: "display_dc_gpio",
                    });
                }
            }

            // --- Optional RST pin: pulse low → high ---
            if let Some(rst) = spi.rst {
                unsafe {
                    let rst_conf = gpio_config_t {
                        pin_bit_mask: 1u64 << rst,
                        mode: gpio_mode_t_GPIO_MODE_OUTPUT,
                        pull_up_en: gpio_pullup_t_GPIO_PULLUP_DISABLE,
                        pull_down_en: gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
                        intr_type: gpio_int_type_t_GPIO_INTR_DISABLE,
                    };
                    let ret = gpio_config(&rst_conf);
                    if ret != ESP_OK {
                        heap::free_spiram_buffer(framebuf);
                        return Err(crate::error::Error::Esp {
                            code: ret,
                            stage: "display_rst_gpio",
                        });
                    }
                    gpio_set_level(rst, 0);
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    gpio_set_level(rst, 1);
                    std::thread::sleep(std::time::Duration::from_millis(120));
                }
            }

            // --- Optional BL pin: set high ---
            if let Some(bl) = spi.bl {
                unsafe {
                    let bl_conf = gpio_config_t {
                        pin_bit_mask: 1u64 << bl,
                        mode: gpio_mode_t_GPIO_MODE_OUTPUT,
                        pull_up_en: gpio_pullup_t_GPIO_PULLUP_DISABLE,
                        pull_down_en: gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
                        intr_type: gpio_int_type_t_GPIO_INTR_DISABLE,
                    };
                    let ret = gpio_config(&bl_conf);
                    if ret != ESP_OK {
                        heap::free_spiram_buffer(framebuf);
                        return Err(crate::error::Error::Esp {
                            code: ret,
                            stage: "display_bl_gpio",
                        });
                    }
                    gpio_set_level(bl, 1);
                }
            }

            // --- Initialize SPI bus ---
            // ESP-IDF 5.4 bindings use anonymous unions for pin fields.
            let mut bus_cfg: spi_bus_config_t = unsafe { core::mem::zeroed() };
            bus_cfg.__bindgen_anon_1.mosi_io_num = spi.mosi;
            bus_cfg.__bindgen_anon_2.miso_io_num = -1;
            bus_cfg.sclk_io_num = spi.sclk;
            bus_cfg.__bindgen_anon_3.quadwp_io_num = -1;
            bus_cfg.__bindgen_anon_4.quadhd_io_num = -1;
            bus_cfg.data4_io_num = -1;
            bus_cfg.data5_io_num = -1;
            bus_cfg.data6_io_num = -1;
            bus_cfg.data7_io_num = -1;
            bus_cfg.max_transfer_sz = framebuf_len as i32;
            bus_cfg.flags = SPICOMMON_BUSFLAG_MASTER;
            let spi_host = spi.host as u32;
            unsafe {
                let ret = spi_bus_initialize(
                    spi_host,
                    &bus_cfg,
                    spi_common_dma_t_SPI_DMA_CH_AUTO,
                );
                if ret != ESP_OK {
                    heap::free_spiram_buffer(framebuf);
                    return Err(crate::error::Error::Esp {
                        code: ret,
                        stage: "display_spi_bus_init",
                    });
                }
            }

            // --- Add SPI device ---
            let dev_cfg = spi_device_interface_config_t {
                clock_speed_hz: spi.freq_hz as i32,
                mode: 0,
                spics_io_num: spi.cs,
                queue_size: 1,
                ..Default::default()
            };
            let mut spi_handle: spi_device_handle_t = core::ptr::null_mut();
            unsafe {
                let ret = spi_bus_add_device(spi_host, &dev_cfg, &mut spi_handle);
                if ret != ESP_OK {
                    spi_bus_free(spi_host);
                    heap::free_spiram_buffer(framebuf);
                    return Err(crate::error::Error::Esp {
                        code: ret,
                        stage: "display_spi_add_device",
                    });
                }
            }

            let mut backend = Self {
                spi_host,
                spi_handle,
                dc_pin: spi.dc,
                width,
                height,
                framebuf,
                framebuf_len,
            };

            // --- Send display init commands ---
            backend.init_display_controller(config)?;

            log::info!(
                "[display] SPI backend ready: {}x{}, driver={:?}",
                width,
                height,
                config.driver
            );
            Ok(backend)
        }

        /// Send a command byte (DC=0).
        fn send_cmd(&self, cmd: u8) -> Result<()> {
            unsafe { gpio_set_level(self.dc_pin, 0) };
            self.spi_write(&[cmd])
        }

        /// Send data bytes (DC=1).
        fn send_data(&self, data: &[u8]) -> Result<()> {
            unsafe { gpio_set_level(self.dc_pin, 1) };
            self.spi_write(data)
        }

        fn spi_write(&self, data: &[u8]) -> Result<()> {
            if data.is_empty() {
                return Ok(());
            }
            let mut trans: spi_transaction_t = unsafe { core::mem::zeroed() };
            trans.length = data.len() * 8;
            trans.__bindgen_anon_1.tx_buffer = data.as_ptr() as *const _;
            let ret = unsafe { spi_device_transmit(self.spi_handle, &mut trans) };
            if ret != ESP_OK {
                return Err(crate::error::Error::Esp {
                    code: ret,
                    stage: "display_spi_write",
                });
            }
            Ok(())
        }

        fn init_display_controller(&mut self, config: &DisplayConfig) -> Result<()> {
            // SWRESET
            self.send_cmd(0x01)?;
            std::thread::sleep(std::time::Duration::from_millis(150));

            // SLPOUT
            self.send_cmd(0x11)?;
            std::thread::sleep(std::time::Duration::from_millis(120));

            // COLMOD: 16-bit RGB565
            self.send_cmd(0x3A)?;
            self.send_data(&[0x55])?;

            // MADCTL: rotation + color order
            let madctl = Self::compute_madctl(config.rotation, &config.color_order);
            self.send_cmd(0x36)?;
            self.send_data(&[madctl])?;

            // INVON / INVOFF
            // ST7789 panels are typically inverted by default, so normal display needs INVON.
            // ILI9341 panels are typically non-inverted, so normal display needs INVOFF.
            // config.invert_colors flips the driver's default behavior.
            let needs_invon = match config.driver {
                DisplayDriver::St7789 => !config.invert_colors,
                DisplayDriver::Ili9341 => config.invert_colors,
            };
            if needs_invon {
                self.send_cmd(0x21)?; // INVON
            } else {
                self.send_cmd(0x20)?; // INVOFF
            }

            // NORON
            self.send_cmd(0x13)?;
            std::thread::sleep(std::time::Duration::from_millis(10));

            // DISPON
            self.send_cmd(0x29)?;
            std::thread::sleep(std::time::Duration::from_millis(20));

            Ok(())
        }

        fn compute_madctl(rotation: u16, color_order: &DisplayColorOrder) -> u8 {
            let color_bit = match color_order {
                DisplayColorOrder::Rgb => 0x00,
                DisplayColorOrder::Bgr => 0x08,
            };
            let rot_bits = match rotation {
                90 => 0x60,  // MX + MV
                180 => 0xC0, // MX + MY
                270 => 0xA0, // MY + MV
                _ => 0x00,   // 0 degrees
            };
            rot_bits | color_bit
        }

        /// Set column/row address window then push full framebuf via SPI.
        pub fn flush(&self, offset_x: i16, offset_y: i16) -> Result<()> {
            self.flush_rows(offset_x, offset_y, 0, self.height)
        }

        /// Push only the rows `[ry..ry+rh)` from the framebuffer, reducing SPI transfer.
        pub fn flush_rows(
            &self,
            offset_x: i16,
            offset_y: i16,
            ry: u16,
            rh: u16,
        ) -> Result<()> {
            if rh == 0 || self.width == 0 {
                return Ok(());
            }
            // Clamp to framebuffer bounds
            let ry = ry.min(self.height);
            let rh = rh.min(self.height.saturating_sub(ry));
            if rh == 0 {
                return Ok(());
            }

            // ST7789/ILI9341 require full-width rows for RAMWR; we send only the dirty row band.
            let x0 = offset_x.max(0) as u16;
            let y0 = offset_y.max(0) as u16 + ry;
            let x1 = x0 + self.width - 1;
            let y1 = y0 + rh - 1;

            // CASET
            self.send_cmd(0x2A)?;
            self.send_data(&[
                (x0 >> 8) as u8,
                x0 as u8,
                (x1 >> 8) as u8,
                x1 as u8,
            ])?;

            // RASET
            self.send_cmd(0x2B)?;
            self.send_data(&[
                (y0 >> 8) as u8,
                y0 as u8,
                (y1 >> 8) as u8,
                y1 as u8,
            ])?;

            // RAMWR
            self.send_cmd(0x2C)?;

            // Send only the dirty rows from framebuf
            let row_bytes = self.width as usize * 2;
            let start = ry as usize * row_bytes;
            let end = start + rh as usize * row_bytes;
            let buf = unsafe { core::slice::from_raw_parts(self.framebuf, self.framebuf_len) };
            const CHUNK_SIZE: usize = 32768;
            for chunk in buf[start..end].chunks(CHUNK_SIZE) {
                self.send_data(chunk)?;
            }
            Ok(())
        }

        /// Write a pixel at (x, y) into the framebuffer (no SPI transfer).
        #[inline]
        fn set_pixel(&mut self, x: u16, y: u16, color: Rgb565) {
            if x < self.width && y < self.height {
                let offset = (y as usize * self.width as usize + x as usize) * 2;
                let raw = RawU16::from(color).into_inner().to_be();
                unsafe {
                    let ptr = self.framebuf.add(offset) as *mut u16;
                    *ptr = raw;
                }
            }
        }
    }

    use embedded_graphics_core::pixelcolor::raw::RawU16;

    impl DrawTarget for SpiDisplayBackend {
        type Color = Rgb565;
        type Error = core::convert::Infallible;

        fn draw_iter<I>(&mut self, pixels: I) -> core::result::Result<(), Self::Error>
        where
            I: IntoIterator<Item = Pixel<Self::Color>>,
        {
            for Pixel(point, color) in pixels {
                if point.x >= 0
                    && point.y >= 0
                    && (point.x as u16) < self.width
                    && (point.y as u16) < self.height
                {
                    self.set_pixel(point.x as u16, point.y as u16, color);
                }
            }
            Ok(())
        }
    }

    impl OriginDimensions for SpiDisplayBackend {
        fn size(&self) -> Size {
            Size::new(self.width as u32, self.height as u32)
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  DisplayState
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct DisplayState {
    pub config: DisplayConfig,
    pub available: bool,
    pub last_command_at: Option<Instant>,
    /// BL GPIO pin number (if configured). Used for backlight on/off control.
    bl_pin: Option<i32>,
    #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
    backend: Option<esp_backend::SpiDisplayBackend>,
}

impl DisplayState {
    pub fn init(config: &DisplayConfig) -> Result<Self> {
        if !config.enabled {
            return Ok(Self {
                config: config.clone(),
                available: false,
                last_command_at: None,
                bl_pin: None,
                #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
                backend: None,
            });
        }

        let bl_pin = config.spi.bl;

        #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
        {
            let backend = esp_backend::SpiDisplayBackend::new(config)?;
            Ok(Self {
                config: config.clone(),
                available: true,
                last_command_at: None,
                bl_pin,
                backend: Some(backend),
            })
        }

        #[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
        {
            log::info!("[display] host stub: init skipped (no SPI hardware)");
            Ok(Self {
                config: config.clone(),
                available: false,
                last_command_at: None,
                bl_pin,
            })
        }
    }

    pub fn execute(&mut self, cmd: DisplayCommand) -> Result<()> {
        if !self.available {
            return Ok(());
        }

        #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
        {
            let backend = match self.backend.as_mut() {
                Some(b) => b,
                None => return Ok(()),
            };
            match &cmd {
                DisplayCommand::RefreshDashboard {
                    state,
                    wifi_connected: _,
                    ip_address,
                    channels,
                    pressure,
                    heap_percent,
                } => {
                    render_dashboard(
                        backend,
                        &DashboardParams {
                            state: *state,
                            ip_address: ip_address.as_deref(),
                            channels,
                            pressure,
                            heap_percent: *heap_percent,
                            width: self.config.width,
                            height: self.config.height,
                        },
                    );
                    backend.flush(self.config.offset_x, self.config.offset_y)?;
                }
                DisplayCommand::UpdateIp { ip } => {
                    let bg = DISPLAY_BG;
                    render_ip_partial(backend, ip, bg, self.config.width);
                    let layout = LAYOUT;
                    backend.flush_rows(
                        self.config.offset_x,
                        self.config.offset_y,
                        layout.subtitle_top,
                        16,
                    )?;
                }
                DisplayCommand::UpdatePressure {
                    level,
                    heap_percent,
                } => {
                    let bg = DISPLAY_BG;
                    render_pressure_partial(
                        backend,
                        level,
                        *heap_percent,
                        bg,
                        self.config.width,
                        self.config.height,
                    );
                    let layout = LAYOUT;
                    let footer_h = self.config.height.saturating_sub(layout.footer_top);
                    backend.flush_rows(
                        self.config.offset_x,
                        self.config.offset_y,
                        layout.footer_top,
                        footer_h,
                    )?;
                }
                DisplayCommand::UpdateChannels { channels } => {
                    let bg = DISPLAY_BG;
                    render_channels_partial(backend, channels, bg, self.config.width);
                    let layout = LAYOUT;
                    let ch_h = layout.footer_top.saturating_sub(layout.middle_top);
                    backend.flush_rows(
                        self.config.offset_x,
                        self.config.offset_y,
                        layout.middle_top,
                        ch_h,
                    )?;
                }
                DisplayCommand::Clear => {
                    use embedded_graphics::prelude::*;
                    use embedded_graphics_core::pixelcolor::Rgb565;
                    let _ = backend.clear(Rgb565::BLACK);
                    backend.flush(self.config.offset_x, self.config.offset_y)?;
                }
            }
            self.last_command_at = Some(Instant::now());
        }

        #[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
        {
            let _ = cmd;
            self.last_command_at = Some(Instant::now());
        }

        Ok(())
    }

    /// 背光控制是否可用（显示器已初始化且有 BL 引脚）。
    /// Whether backlight control is available.
    pub fn backlight_available(&self) -> bool {
        self.available && self.bl_pin.is_some()
    }

    /// 设置背光开关。on=true 开启（GPIO HIGH），on=false 关闭（GPIO LOW）。
    /// Set backlight on/off via GPIO level.
    pub fn set_backlight(&self, on: bool) -> Result<()> {
        #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
        {
            if let Some(bl) = self.bl_pin {
                let level = if on { 1 } else { 0 };
                unsafe {
                    esp_idf_svc::sys::gpio_set_level(bl, level);
                }
            }
        }
        #[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
        {
            let _ = on;
        }
        Ok(())
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Platform-agnostic rendering (embedded-graphics)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

use embedded_graphics::{
    mono_font::{ascii::FONT_6X13, ascii::FONT_9X18_BOLD, MonoTextStyle},
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle, Rectangle},
    text::Text,
};
use embedded_graphics_core::pixelcolor::Rgb565;

/// Cached layout constants (avoid re-creating on each call).
const LAYOUT: crate::display::DisplayLayout = crate::display::DisplayLayout {
    header_top: 16,
    icon_left: 12,
    icon_size: 64,
    title_left: 88,
    title_top: 18,
    subtitle_top: 44,
    middle_top: 104,
    footer_top: 168,
};

/// Beetle drawing options.
#[derive(Default)]
struct BeetleOpts {
    flipped: bool,
    x_eyes: bool,
    wings: bool,
}


/// RGB565 color helpers.
fn rgb565(r: u8, g: u8, b: u8) -> Rgb565 {
    Rgb565::new(r >> 3, g >> 2, b >> 3)
}

fn darken(c: Rgb565, amt: u8) -> Rgb565 {
    let r = c.r().saturating_sub(amt >> 3);
    let g = c.g().saturating_sub(amt >> 2);
    let b = c.b().saturating_sub(amt >> 3);
    Rgb565::new(r, g, b)
}

/// Draw a beetle icon using embedded-graphics primitives.
///
/// `x`, `y` is the top-left corner of the bounding box; `size` is the box side length.
/// Returns (cx, body_cy, body_r, head_cy, head_r) for overlay placement.
fn draw_beetle<D: DrawTarget<Color = Rgb565>>(
    target: &mut D,
    x: i32,
    y: i32,
    size: i32,
    color: Rgb565,
    opts: &BeetleOpts,
) -> (i32, i32, i32, i32, i32) {
    let cx = x + size / 2;
    let dir: i32 = if opts.flipped { -1 } else { 1 };

    let body_r = size * 28 / 100;
    let head_r = size * 10 / 100;
    let head_cy = if opts.flipped {
        y + size * 72 / 100
    } else {
        y + size * 30 / 100
    };
    let body_cy = if opts.flipped {
        y + size * 40 / 100
    } else {
        y + size * 62 / 100
    };

    let line_style = PrimitiveStyle::with_stroke(color, 2);

    // --- Antennae (simplified as 2 straight lines) ---
    let ant_tip_dy = dir * size * 20 / 100;
    let ant_base_dy = dir * head_r * 8 / 10;
    // Left antenna
    let _ = Line::new(
        Point::new(cx - head_r / 2, head_cy - ant_base_dy),
        Point::new(cx - size * 20 / 100, head_cy - ant_tip_dy),
    )
    .into_styled(line_style)
    .draw(target);
    // Right antenna
    let _ = Line::new(
        Point::new(cx + head_r / 2, head_cy - ant_base_dy),
        Point::new(cx + size * 20 / 100, head_cy - ant_tip_dy),
    )
    .into_styled(line_style)
    .draw(target);

    // --- Legs (3 pairs, each with 2 segments) ---
    let leg_attach_fracs: [i32; 3] = [-25, 0, 25]; // percent of body_r
    let leg_angles_deg: [i32; 3] = [-20, 5, 25];
    let leg_len1 = size * 10 / 100;
    let leg_len2 = size * 7 / 100;

    for (i, &frac) in leg_attach_fracs.iter().enumerate() {
        let leg_y = body_cy + body_r * frac / 100;
        let ang_deg = leg_angles_deg[i] * dir;
        // Simplified: approximate cos/sin with fixed ratios for small angles
        let (cos_a, sin_a) = approx_cos_sin(ang_deg);

        for &side in &[-1i32, 1] {
            let ax = cx + side * body_r * 95 / 100;
            let kx = ax + side * leg_len1 * cos_a / 100;
            let ky = leg_y + leg_len1 * sin_a / 100;
            let fx = kx + side * leg_len2 * 50 / 100;
            let fy = ky + leg_len2 * 90 / 100 * dir;

            let _ = Line::new(Point::new(ax, leg_y), Point::new(kx, ky))
                .into_styled(line_style)
                .draw(target);
            let _ = Line::new(Point::new(kx, ky), Point::new(fx, fy))
                .into_styled(line_style)
                .draw(target);
        }
    }

    // --- Wings (busy state: membrane wings peeking out from under elytra) ---
    if opts.wings {
        let wing_color = darken(color, 30);
        let wing_style = PrimitiveStyle::with_stroke(wing_color, 1);

        // Wing shape: elongated oval approximated by a filled region + outline arcs.
        // Each wing extends from the body edge outward and slightly upward.
        let wing_span = size * 18 / 100; // horizontal extent from body edge
        let wing_h = size * 22 / 100; // vertical height of wing
        let wing_top = body_cy - wing_h * 6 / 10; // wings start above body center

        for &side in &[-1i32, 1] {
            let base_x = cx + side * body_r; // wing root at body edge
            let tip_x = base_x + side * wing_span; // wing tip
            let mid_x = base_x + side * wing_span * 7 / 10; // control point x

            // Wing outline: 5-segment curve (top arc + bottom arc)
            // Top edge: gentle upward curve from root to tip
            let t0 = Point::new(base_x, wing_top + wing_h * 2 / 10);
            let t1 = Point::new(mid_x, wing_top);
            let t2 = Point::new(tip_x, wing_top + wing_h * 3 / 10);
            // Bottom edge: curves back to root
            let b1 = Point::new(mid_x, wing_top + wing_h);
            let b0 = Point::new(base_x, wing_top + wing_h * 7 / 10);

            // Fill: draw horizontal lines between top and bottom edges (simple scanline fill)
            // For minimal overhead, just draw the outline — looks like translucent membrane wings
            let _ = Line::new(t0, t1).into_styled(wing_style).draw(target);
            let _ = Line::new(t1, t2).into_styled(wing_style).draw(target);
            let _ = Line::new(t2, b1).into_styled(wing_style).draw(target);
            let _ = Line::new(b1, b0).into_styled(wing_style).draw(target);
            let _ = Line::new(b0, t0).into_styled(wing_style).draw(target);

            // Wing vein (central line for realism)
            let vein_style = PrimitiveStyle::with_stroke(wing_color, 1);
            let vein_mid = Point::new(
                mid_x - side * wing_span / 10,
                wing_top + wing_h / 2,
            );
            let _ = Line::new(
                Point::new(base_x, wing_top + wing_h * 4 / 10),
                vein_mid,
            )
            .into_styled(vein_style)
            .draw(target);
            let _ = Line::new(
                vein_mid,
                Point::new(tip_x - side * 2, wing_top + wing_h * 4 / 10),
            )
            .into_styled(vein_style)
            .draw(target);
        }
    }

    // --- Body (large filled circle) ---
    let body_fill = PrimitiveStyle::with_fill(color);
    let _ = Circle::new(
        Point::new(cx - body_r, body_cy - body_r),
        (body_r * 2) as u32,
    )
    .into_styled(body_fill)
    .draw(target);

    // --- Elytra seam (center line) ---
    let seam_color = darken(color, 50);
    let seam_style = PrimitiveStyle::with_stroke(seam_color, 1);
    let _ = Line::new(
        Point::new(cx, body_cy - body_r + 3),
        Point::new(cx, body_cy + body_r - 3),
    )
    .into_styled(seam_style)
    .draw(target);

    // --- Elytra spots (2 pairs of small dots, like a ladybug pattern) ---
    let spot_color = darken(color, 40);
    let spot_fill = PrimitiveStyle::with_fill(spot_color);
    let spot_r = (body_r * 15 / 100).max(2);
    // Upper pair
    for &sx in &[-1i32, 1] {
        let _ = Circle::new(
            Point::new(cx + sx * body_r * 45 / 100 - spot_r, body_cy - body_r * 30 / 100 - spot_r),
            (spot_r * 2) as u32,
        )
        .into_styled(spot_fill)
        .draw(target);
    }
    // Lower pair
    for &sx in &[-1i32, 1] {
        let _ = Circle::new(
            Point::new(cx + sx * body_r * 40 / 100 - spot_r, body_cy + body_r * 20 / 100 - spot_r),
            (spot_r * 2) as u32,
        )
        .into_styled(spot_fill)
        .draw(target);
    }

    // --- Head (smaller filled circle, slightly darker) ---
    let head_color = darken(color, 20);
    let head_fill = PrimitiveStyle::with_fill(head_color);
    let _ = Circle::new(
        Point::new(cx - head_r, head_cy - head_r),
        (head_r * 2) as u32,
    )
    .into_styled(head_fill)
    .draw(target);

    // --- Eyes ---
    let eye_spread = head_r * 6 / 10;
    let eye_y = head_cy - dir;

    if opts.x_eyes {
        // X eyes (fault state)
        let x_style = PrimitiveStyle::with_stroke(Rgb565::WHITE, 2);
        for &sx in &[-1i32, 1] {
            let ex = cx + sx * eye_spread;
            let _ = Line::new(
                Point::new(ex - 2, eye_y - 2),
                Point::new(ex + 2, eye_y + 2),
            )
            .into_styled(x_style)
            .draw(target);
            let _ = Line::new(
                Point::new(ex + 2, eye_y - 2),
                Point::new(ex - 2, eye_y + 2),
            )
            .into_styled(x_style)
            .draw(target);
        }
    } else {
        // Normal eyes (white dots)
        let eye_fill = PrimitiveStyle::with_fill(Rgb565::WHITE);
        for &sx in &[-1i32, 1] {
            let _ = Circle::new(
                Point::new(cx + sx * eye_spread - 2, eye_y - 2),
                5,
            )
            .into_styled(eye_fill)
            .draw(target);
        }
    }

    (cx, body_cy, body_r, head_cy, head_r)
}

/// Approximate cos/sin * 100 for small angles (integer arithmetic, no libm).
fn approx_cos_sin(deg: i32) -> (i32, i32) {
    match deg {
        -25..=-16 => (91, -37),
        -15..=-6 => (97, -17),
        -5..=5 => (100, deg * 2),
        6..=15 => (97, 17),
        16..=25 => (91, 37),
        26..=35 => (82, 50),
        _ => (100, 0),
    }
}

/// Draw a top-half arc (WiFi signal style) centered at (cx, cy) with radius `r`.
/// Uses 6 line segments covering roughly -150° to -30° (i.e. the upper arc).
fn draw_top_arc<D: DrawTarget<Color = Rgb565>>(
    target: &mut D,
    cx: i32,
    cy: i32,
    r: i32,
    style: &PrimitiveStyle<Rgb565>,
) {
    // Pre-computed (cos, sin) * 1000 for angles -150, -130, -110, -90, -70, -50, -30 degrees.
    const POINTS: [(i32, i32); 7] = [
        (-866, -500),  // -150°
        (-643, -766),  // -130°
        (-342, -940),  // -110°
        (0, -1000),    // -90°
        (342, -940),   // -70°
        (643, -766),   // -50°
        (866, -500),   // -30°
    ];
    for pair in POINTS.windows(2) {
        let (c0, s0) = pair[0];
        let (c1, s1) = pair[1];
        let _ = Line::new(
            Point::new(cx + r * c0 / 1000, cy + r * s0 / 1000),
            Point::new(cx + r * c1 / 1000, cy + r * s1 / 1000),
        )
        .into_styled(*style)
        .draw(target);
    }
}

/// Draw a dashed top-half arc using dots along the arc path.
fn draw_dashed_top_arc<D: DrawTarget<Color = Rgb565>>(
    target: &mut D,
    cx: i32,
    cy: i32,
    r: i32,
    color: Rgb565,
) {
    // Same angle sample points as draw_top_arc, but render as individual dots.
    const POINTS: [(i32, i32); 7] = [
        (-866, -500),
        (-643, -766),
        (-342, -940),
        (0, -1000),
        (342, -940),
        (643, -766),
        (866, -500),
    ];
    let dot_style = PrimitiveStyle::with_fill(color);
    for &(c, s) in &POINTS {
        let px = cx + r * c / 1000;
        let py = cy + r * s / 1000;
        let _ = Circle::new(Point::new(px - 1, py - 1), 3)
            .into_styled(dot_style)
            .draw(target);
    }
}

/// Dashboard render parameters (avoids clippy::too_many_arguments).
struct DashboardParams<'a> {
    state: DisplaySystemState,
    ip_address: Option<&'a str>,
    channels: &'a [DisplayChannelStatus; 5],
    pressure: &'a DisplayPressureLevel,
    heap_percent: u8,
    width: u16,
    height: u16,
}

/// Render the full dashboard UI.
fn render_dashboard<D: DrawTarget<Color = Rgb565>>(target: &mut D, p: &DashboardParams<'_>) {
    let layout = LAYOUT;

    // --- Background fill ---
    let bg_color = DISPLAY_BG;
    let _ = Rectangle::new(
        Point::new(0, 0),
        Size::new(p.width as u32, p.height as u32),
    )
    .into_styled(PrimitiveStyle::with_fill(bg_color))
    .draw(target);

    // --- Beetle icon ---
    let beetle_color = match p.state {
        DisplaySystemState::Booting => rgb565(0xdd, 0x99, 0x22),
        DisplaySystemState::NoWifi => rgb565(0xaa, 0xaa, 0xaa),
        DisplaySystemState::Idle => rgb565(0x22, 0xcc, 0x22),
        DisplaySystemState::Busy => rgb565(0x44, 0x88, 0xee),
        DisplaySystemState::Fault => rgb565(0xee, 0x33, 0x33),
    };
    let icon_size = layout.icon_size as i32;
    let opts = match p.state {
        DisplaySystemState::Busy => BeetleOpts {
            wings: true,
            ..Default::default()
        },
        DisplaySystemState::Fault => BeetleOpts {
            flipped: true,
            x_eyes: true,
            ..Default::default()
        },
        _ => BeetleOpts::default(),
    };
    let (cx, body_cy, body_r, head_cy, head_r) = draw_beetle(
        target,
        layout.icon_left as i32,
        layout.header_top as i32,
        icon_size,
        beetle_color,
        &opts,
    );

    // --- State-specific overlays ---
    match p.state {
        DisplaySystemState::Booting => {
            // Loading dots on body: 3 circles of increasing size
            let dot_color = Rgb565::WHITE;
            let dot_style = PrimitiveStyle::with_fill(dot_color);
            let dot_y = body_cy;
            let spacing = body_r * 35 / 100;
            for (i, &r) in [2u32, 3, 4].iter().enumerate() {
                let dx = (i as i32 - 1) * spacing;
                let _ = Circle::new(
                    Point::new(cx + dx - r as i32, dot_y - r as i32),
                    r * 2,
                )
                .into_styled(dot_style)
                .draw(target);
            }
            // Dashed WiFi arcs above head (dots instead of solid lines)
            let sig_y = head_cy - head_r - 4;
            draw_dashed_top_arc(target, cx, sig_y, 7, beetle_color);
            draw_dashed_top_arc(target, cx, sig_y, 13, beetle_color);
        }
        DisplaySystemState::NoWifi => {
            // Solid WiFi signal arcs above head
            let sig_y = head_cy - head_r - 4;
            let arc_style = PrimitiveStyle::with_stroke(beetle_color, 2);
            for &r in &[7i32, 13] {
                draw_top_arc(target, cx, sig_y, r, &arc_style);
            }
            // X mark over WiFi (signal crossed out)
            let x_style = PrimitiveStyle::with_stroke(rgb565(0xcc, 0x22, 0x22), 2);
            let x_sz = 6i32;
            let _ = Line::new(
                Point::new(cx - x_sz, sig_y - 13 - x_sz),
                Point::new(cx + x_sz, sig_y - 13 + x_sz),
            )
            .into_styled(x_style)
            .draw(target);
            let _ = Line::new(
                Point::new(cx + x_sz, sig_y - 13 - x_sz),
                Point::new(cx - x_sz, sig_y - 13 + x_sz),
            )
            .into_styled(x_style)
            .draw(target);
        }
        DisplaySystemState::Idle => {
            // Checkmark on body
            let check_style = PrimitiveStyle::with_stroke(Rgb565::WHITE, 3);
            let m = 8i32;
            let _ = Line::new(
                Point::new(cx - m, body_cy),
                Point::new(cx - 2, body_cy + m * 7 / 10),
            )
            .into_styled(check_style)
            .draw(target);
            let _ = Line::new(
                Point::new(cx - 2, body_cy + m * 7 / 10),
                Point::new(cx + m, body_cy - m / 2),
            )
            .into_styled(check_style)
            .draw(target);
        }
        DisplaySystemState::Fault => {
            // Exclamation mark on body
            let ex_style = PrimitiveStyle::with_stroke(Rgb565::WHITE, 2);
            let _ = Line::new(
                Point::new(cx, body_cy - body_r * 40 / 100),
                Point::new(cx, body_cy + body_r * 15 / 100),
            )
            .into_styled(ex_style)
            .draw(target);
            let dot_fill = PrimitiveStyle::with_fill(Rgb565::WHITE);
            let _ = Circle::new(
                Point::new(cx - 2, body_cy + body_r * 30 / 100),
                4,
            )
            .into_styled(dot_fill)
            .draw(target);
        }
        _ => {} // Busy — wings already drawn
    }

    // --- Title text: state name ---
    let title_style = MonoTextStyle::new(&FONT_9X18_BOLD, rgb565(0x22, 0x22, 0x22));
    let state_name = match p.state {
        DisplaySystemState::Booting => "BOOTING",
        DisplaySystemState::NoWifi => "NO WIFI",
        DisplaySystemState::Idle => "IDLE",
        DisplaySystemState::Busy => "BUSY",
        DisplaySystemState::Fault => "FAULT",
    };
    let _ = Text::new(
        state_name,
        Point::new(layout.title_left as i32, layout.title_top as i32 + 14),
        title_style,
    )
    .draw(target);

    // --- Subtitle: IP address or version ---
    let subtitle_style = MonoTextStyle::new(&FONT_6X13, rgb565(0x66, 0x66, 0x66));
    let ip_text = if p.state == DisplaySystemState::Booting {
        p.ip_address.unwrap_or(concat!("beetle v", env!("CARGO_PKG_VERSION")))
    } else {
        p.ip_address.unwrap_or("---.---.---.---")
    };
    let _ = Text::new(
        ip_text,
        Point::new(layout.title_left as i32, layout.subtitle_top as i32 + 11),
        subtitle_style,
    )
    .draw(target);

    // --- Channel status (middle section) ---
    let healthy_color = rgb565(0x22, 0xcc, 0x22);
    let unhealthy_color = rgb565(0xcc, 0x22, 0x22);
    let disabled_color = rgb565(0xbb, 0xbb, 0xbb);
    let text_style = MonoTextStyle::new(&FONT_6X13, rgb565(0x33, 0x33, 0x33));
    let disabled_text_style = MonoTextStyle::new(&FONT_6X13, rgb565(0x99, 0x99, 0x99));

    let col_width = p.width as i32 / 2;
    let mut col = 0i32;
    let mut row = 0i32;
    for ch in p.channels.iter() {
        let px = 8 + col * col_width;
        let py = layout.middle_top as i32 + row * 18;

        if ch.enabled {
            // Enabled channel: filled dot (green=healthy, red=unhealthy)
            let dot_color = if ch.healthy {
                healthy_color
            } else {
                unhealthy_color
            };
            let _ = Circle::new(Point::new(px, py + 2), 8)
                .into_styled(PrimitiveStyle::with_fill(dot_color))
                .draw(target);
        } else {
            // Disabled channel: hollow circle (gray stroke)
            let _ = Circle::new(Point::new(px, py + 2), 8)
                .into_styled(PrimitiveStyle::with_stroke(disabled_color, 1))
                .draw(target);
        }

        // Channel name
        let name_style = if ch.enabled { text_style } else { disabled_text_style };
        let _ = Text::new(ch.name, Point::new(px + 14, py + 11), name_style).draw(target);

        col += 1;
        if col >= 2 {
            col = 0;
            row += 1;
        }
    }

    // --- Footer: pressure level + heap progress bar ---
    render_footer(target, p.pressure, p.heap_percent, p.width);
}

/// Dashboard background color (white).
const DISPLAY_BG: Rgb565 = Rgb565::WHITE;

/// Partial update: repaint only the IP subtitle region.
fn render_ip_partial<D: DrawTarget<Color = Rgb565>>(
    target: &mut D,
    ip: &str,
    bg: Rgb565,
    width: u16,
) {
    let layout = LAYOUT;
    // Clear subtitle row: from title_left to right edge, height = font height (13px)
    let y = layout.subtitle_top as i32;
    let x = layout.title_left as i32;
    let clear_w = (width as i32 - x).max(0) as u32;
    let _ = Rectangle::new(Point::new(x, y), Size::new(clear_w, 16))
        .into_styled(PrimitiveStyle::with_fill(bg))
        .draw(target);

    let subtitle_style = MonoTextStyle::new(&FONT_6X13, rgb565(0x66, 0x66, 0x66));
    let _ = Text::new(ip, Point::new(x, y + 11), subtitle_style).draw(target);
}

/// Partial update: repaint only the channel status (middle) region.
fn render_channels_partial<D: DrawTarget<Color = Rgb565>>(
    target: &mut D,
    channels: &[DisplayChannelStatus; 5],
    bg: Rgb565,
    width: u16,
) {
    let layout = LAYOUT;
    let middle_y = layout.middle_top as i32;
    let ch_h = (layout.footer_top - layout.middle_top) as u32;

    // Clear middle region
    let _ = Rectangle::new(
        Point::new(0, middle_y),
        Size::new(width as u32, ch_h),
    )
    .into_styled(PrimitiveStyle::with_fill(bg))
    .draw(target);

    // Redraw channel dots and names
    let healthy_color = rgb565(0x22, 0xcc, 0x22);
    let unhealthy_color = rgb565(0xcc, 0x22, 0x22);
    let disabled_color = rgb565(0xbb, 0xbb, 0xbb);
    let text_style = MonoTextStyle::new(&FONT_6X13, rgb565(0x33, 0x33, 0x33));
    let disabled_text_style = MonoTextStyle::new(&FONT_6X13, rgb565(0x99, 0x99, 0x99));

    let col_width = width as i32 / 2;
    let mut col = 0i32;
    let mut row = 0i32;
    for ch in channels.iter() {
        let px = 8 + col * col_width;
        let py = middle_y + row * 18;

        if ch.enabled {
            let dot_color = if ch.healthy {
                healthy_color
            } else {
                unhealthy_color
            };
            let _ = Circle::new(Point::new(px, py + 2), 8)
                .into_styled(PrimitiveStyle::with_fill(dot_color))
                .draw(target);
        } else {
            let _ = Circle::new(Point::new(px, py + 2), 8)
                .into_styled(PrimitiveStyle::with_stroke(disabled_color, 1))
                .draw(target);
        }

        let name_style = if ch.enabled { text_style } else { disabled_text_style };
        let _ = Text::new(ch.name, Point::new(px + 14, py + 11), name_style).draw(target);

        col += 1;
        if col >= 2 {
            col = 0;
            row += 1;
        }
    }
}

/// Partial update: repaint only the footer pressure + progress bar region.
fn render_pressure_partial<D: DrawTarget<Color = Rgb565>>(
    target: &mut D,
    level: &DisplayPressureLevel,
    heap_percent: u8,
    bg: Rgb565,
    width: u16,
    height: u16,
) {
    let layout = LAYOUT;
    let footer_y = layout.footer_top as i32;

    // Clear entire footer region
    let footer_h = (height as i32 - footer_y).max(1) as u32;
    let _ = Rectangle::new(
        Point::new(0, footer_y),
        Size::new(width as u32, footer_h),
    )
    .into_styled(PrimitiveStyle::with_fill(bg))
    .draw(target);

    render_footer(target, level, heap_percent, width);
}

/// Shared footer rendering: pressure label + progress bar + percentage text.
fn render_footer<D: DrawTarget<Color = Rgb565>>(
    target: &mut D,
    level: &DisplayPressureLevel,
    heap_percent: u8,
    width: u16,
) {
    let footer_y = LAYOUT.footer_top as i32;
    let pressure_text = match level {
        DisplayPressureLevel::Normal => "NORMAL",
        DisplayPressureLevel::Cautious => "CAUTIOUS",
        DisplayPressureLevel::Critical => "CRITICAL",
    };
    let pressure_color = match level {
        DisplayPressureLevel::Normal => rgb565(0x22, 0xcc, 0x22),
        DisplayPressureLevel::Cautious => rgb565(0xdd, 0xaa, 0x00),
        DisplayPressureLevel::Critical => rgb565(0xee, 0x33, 0x33),
    };
    let pressure_style = MonoTextStyle::new(&FONT_6X13, pressure_color);
    let _ = Text::new(pressure_text, Point::new(8, footer_y + 11), pressure_style).draw(target);

    let bar_x = 8i32;
    let bar_y = footer_y + 20;
    let bar_w = (width as i32 - 56).max(40) as u32;
    let bar_h = 12u32;

    let bar_border = PrimitiveStyle::with_stroke(rgb565(0xaa, 0xaa, 0xaa), 1);
    let _ = Rectangle::new(Point::new(bar_x, bar_y), Size::new(bar_w, bar_h))
        .into_styled(bar_border)
        .draw(target);

    let fill_w = ((heap_percent as u32).min(100) * (bar_w - 2)) / 100;
    if fill_w > 0 {
        let _ = Rectangle::new(
            Point::new(bar_x + 1, bar_y + 1),
            Size::new(fill_w, bar_h - 2),
        )
        .into_styled(PrimitiveStyle::with_fill(pressure_color))
        .draw(target);
    }

    let text_style = MonoTextStyle::new(&FONT_6X13, rgb565(0x33, 0x33, 0x33));
    let mut pct_buf = [0u8; 5];
    let pct_str = format_pct(heap_percent, &mut pct_buf);
    let _ = Text::new(
        pct_str,
        Point::new(bar_x + bar_w as i32 + 4, bar_y + 10),
        text_style,
    )
    .draw(target);
}

/// Format a percentage value into a static buffer (no heap alloc).
fn format_pct(val: u8, buf: &mut [u8; 5]) -> &str {
    let val = val.min(100);
    let mut pos = 0;
    if val >= 100 {
        buf[pos] = b'1';
        pos += 1;
        buf[pos] = b'0';
        pos += 1;
        buf[pos] = b'0';
        pos += 1;
    } else if val >= 10 {
        buf[pos] = b'0' + val / 10;
        pos += 1;
        buf[pos] = b'0' + val % 10;
        pos += 1;
    } else {
        buf[pos] = b'0' + val;
        pos += 1;
    }
    buf[pos] = b'%';
    pos += 1;
    core::str::from_utf8(&buf[..pos]).unwrap_or("?%")
}
