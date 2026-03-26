# Display dashboard

**English** | [中文](../zh-cn/display.md)

The display module drives an SPI-connected TFT screen to show a real-time operational dashboard. A procedurally drawn beetle icon reflects system state at a glance, alongside channel health, IP address, and heap pressure — all without image assets or external fonts.

---

## Supported controllers

| Controller | Typical resolution | Notes |
|------------|-------------------|-------|
| **ST7789** | 240x240, 240x320 | Default; color inversion ON by default (panel-native) |
| **ILI9341** | 240x320 | Color inversion OFF by default |
| **ST7735** | 128x160, 128x128, 80x160, etc. | ST7735 / ST7735R / ST7735S family (register-compatible); inversion OFF by default; uses frame-rate, power, and gamma init |

ST7789 and ILI9341 share a short init (SWRESET → SLPOUT → COLMOD `0x55` → MADCTL → INV → NORON → DISPON). **ST7735** sends frame-rate, power, and gamma registers after SLPOUT, uses COLMOD `0x05`, then MADCTL → INV → NORON → DISPON. The `invert_colors` flag flips each driver's default inversion behavior.

---

## Hardware wiring

A typical SPI connection to ESP32-S3:

| Signal | Description | Required |
|--------|-------------|----------|
| SCLK | SPI clock | Yes |
| MOSI | SPI data out (MISO unused) | Yes |
| CS | Chip select | Yes |
| DC | Data/command select | Yes |
| RST | Hardware reset (pulse low→high on init) | Optional |
| BL | Backlight enable (set high on init) | Optional |

> All pin numbers are ESP32-S3 GPIO numbers. The SPI host must be 2 (HSPI) or 3 (VSPI).

---

## Configuration

Display configuration is stored in `config/display.json` on the SPIFFS partition. It can also be edited via the configuration web UI (Display section).

### Full config example

```json
{
  "version": 1,
  "enabled": true,
  "driver": "st7789",
  "bus": "spi",
  "width": 240,
  "height": 240,
  "rotation": 0,
  "color_order": "rgb",
  "invert_colors": false,
  "offset_x": 0,
  "offset_y": 0,
  "spi": {
    "host": 2,
    "sclk": 42,
    "mosi": 41,
    "cs": 21,
    "dc": 40,
    "rst": 39,
    "bl": 38,
    "freq_hz": 40000000
  }
}
```

### ST7735 example (1.8" 128×160)

```json
{
  "version": 1,
  "enabled": true,
  "driver": "st7735",
  "bus": "spi",
  "width": 128,
  "height": 160,
  "rotation": 0,
  "color_order": "bgr",
  "invert_colors": false,
  "offset_x": 2,
  "offset_y": 1,
  "spi": {
    "host": 2,
    "sclk": 42,
    "mosi": 41,
    "cs": 21,
    "dc": 40,
    "rst": null,
    "bl": null,
    "freq_hz": 15000000
  }
}
```

### Field reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `version` | u32 | 1 | Config schema version; must be `1` |
| `enabled` | bool | — | Enable/disable display. When `false`, no SPI hardware is initialized |
| `driver` | string | — | `"st7789"`, `"ili9341"`, or `"st7735"` |
| `bus` | string | — | `"spi"` (only option currently) |
| `width` | u16 | — | Panel width in pixels (1–480) |
| `height` | u16 | — | Panel height in pixels (1–480) |
| `rotation` | u16 | 0 | Display rotation: `0`, `90`, `180`, or `270` |
| `color_order` | string | `"rgb"` | `"rgb"` or `"bgr"` |
| `invert_colors` | bool | false | Flip the driver's default color inversion |
| `offset_x` | i16 | 0 | Horizontal pixel offset for the display window (-480 to 480) |
| `offset_y` | i16 | 0 | Vertical pixel offset for the display window (-480 to 480) |
| `spi.host` | u8 | 2 | SPI host: `2` (HSPI) or `3` (VSPI) |
| `spi.sclk` | i32 | — | SPI clock GPIO pin |
| `spi.mosi` | i32 | — | SPI MOSI GPIO pin |
| `spi.cs` | i32 | — | Chip select GPIO pin |
| `spi.dc` | i32 | — | Data/command GPIO pin |
| `spi.rst` | i32? | null | Reset GPIO pin (optional) |
| `spi.bl` | i32? | null | Backlight GPIO pin (optional) |
| `spi.freq_hz` | u32 | 40000000 | SPI clock frequency (1–80 MHz) |

---

## Dashboard layout

The dashboard renders on a white background:

```
┌──────────────────────────────────┐ y=0
│ ┌──────┐                         │
│ │beetle│  STATE_NAME              │ Title (bold)
│ │64x64 │  192.168.1.100           │ IP subtitle
│ └──────┘                         │
├──────────────────────────────────┤ y≈104
│  ● telegram    ● feishu          │ Channel status
│  ○ dingtalk    ● wecom           │ ● filled=enabled
│  ● qq_channel                    │ ○ hollow=disabled
├──────────────────────────────────┤ y≈168
│  NORMAL   ████████░░░░ 62%       │ Pressure + heap bar
└──────────────────────────────────┘ y=240
```

### Beetle icon states

The beetle is drawn entirely with `embedded-graphics` primitives (circles, lines) — no bitmap or PNG resources.

| State | Beetle color | Visual cues |
|-------|-------------|-------------|
| **Booting** | Orange | Loading dots on body; dashed WiFi arcs above head; subtitle shows firmware version |
| **NoWifi** | Gray | Solid WiFi arcs above head, crossed out with red X |
| **Idle** | Green | White checkmark on body |
| **Busy** | Blue | Membrane wings spread from under elytra |
| **Fault** | Red | Flipped upside-down; X eyes; exclamation mark on body |

### Channel status indicators

| Visual | Meaning |
|--------|---------|
| Filled green dot + normal text | Enabled channel, healthy |
| Filled red dot + normal text | Enabled channel, unhealthy |
| Hollow gray circle + dimmed text | Disabled channel |

### Footer

- Pressure label: **NORMAL** (green) / **CAUTIOUS** (yellow) / **CRITICAL** (red)
- Heap usage progress bar with percentage

---

## Partial updates

To reduce SPI traffic, partial updates are used for frequently changing regions:

| Command | What updates | Rows flushed |
|---------|-------------|-------------|
| `UpdateIp` | IP address subtitle only | ~16 rows |
| `UpdatePressure` | Footer pressure bar only | ~72 rows |
| `RefreshDashboard` | Full screen | All rows |

Partial row flush reduces SPI data by ~85% compared to full-screen refresh.

---

## Memory impact

| Item | Size | Location |
|------|------|----------|
| Framebuffer (240x240 RGB565) | 115,200 bytes | PSRAM |
| `SpiDisplayBackend` struct | ~64 bytes | Heap (internal) |
| Rendering stack usage | ~200 bytes | Stack |
| **Net internal DRAM impact** | **~0** | PSRAM absorbs the framebuffer |

The display thread runs with a 6 KB stack and refreshes every 5 seconds from `orchestrator::snapshot()`.

---

## Caveats and tips

1. **`enabled: false` means zero hardware init** — no SPI bus, no GPIO, no PSRAM allocation. Safe to leave in config even without a display connected.

2. **Offset values** — Some panels (notably 240x240 ST7789 on 240x320 glass) need `offset_x` or `offset_y` to shift the visible window. Common: `offset_y: 80` for 240x240 on a 240x320 panel.

3. **Color order** — If colors appear inverted (red↔blue), toggle `color_order` between `"rgb"` and `"bgr"`.

4. **`invert_colors`** — If the display looks like a photo negative, toggle this flag. ST7789 panels typically need inversion ON (the default), while ILI9341 panels need it OFF.

5. **SPI frequency** — 40 MHz works for most ST7789/ILI9341 panels. If you see visual artifacts or corruption, try lowering to 20 MHz or 10 MHz. Maximum supported is 80 MHz. **ST7735** is best at ≤15 MHz (e.g. `15_000_000`); higher speeds often cause corruption.

6. **ST7735 (1.8" 128×160, etc.)** — Many modules use 132×162 glass; set `offset_x: 2`, `offset_y: 1` with `width`/`height` 128/160. Some “black tab” variants use `0`/`0`—tune to your module. `color_order` is often `bgr`; try `rgb` if red and blue are swapped.

7. **Display thread stack** — The rendering thread uses 6 KB stack. This is sufficient for the current dashboard layout. If you add significantly more complex rendering, monitor for stack overflow.

8. **Host compilation** — On non-ESP targets (`cargo check`, `cargo clippy`), the display backend returns `available: false` and all commands are no-ops. This ensures the codebase compiles cleanly on the host.

9. **Rotation** — The `rotation` field applies a MADCTL transform at the controller level. The framebuffer dimensions (`width` x `height`) should match the post-rotation visible area.
