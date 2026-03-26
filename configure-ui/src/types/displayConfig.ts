export type DisplayDriver = 'st7789' | 'ili9341' | 'st7735'
export type DisplayBus = 'spi'
export type DisplayColorOrder = 'rgb' | 'bgr'

export interface DisplaySpiConfig {
  host: 2 | 3
  sclk: number
  mosi: number
  cs: number
  dc: number
  rst: number | null
  bl: number | null
  freq_hz: number
}

export interface DisplayConfig {
  version: number
  enabled: boolean
  driver: DisplayDriver
  bus: DisplayBus
  width: number
  height: number
  rotation: 0 | 90 | 180 | 270
  color_order: DisplayColorOrder
  invert_colors: boolean
  offset_x: number
  offset_y: number
  spi: DisplaySpiConfig
  sleep_timeout_secs: number
}

export function defaultDisplayConfig(): DisplayConfig {
  return {
    version: 1,
    enabled: false,
    driver: 'st7789',
    bus: 'spi',
    width: 240,
    height: 240,
    rotation: 0,
    color_order: 'rgb',
    invert_colors: false,
    offset_x: 0,
    offset_y: 0,
    spi: {
      host: 2,
      sclk: 42,
      mosi: 41,
      cs: 21,
      dc: 40,
      rst: null,
      bl: null,
      freq_hz: 40_000_000,
    },
    sleep_timeout_secs: 0,
  }
}
