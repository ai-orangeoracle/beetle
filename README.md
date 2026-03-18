# Beetle

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

**English** | [中文](README.zh-CN.md)

**ESP32 edge AI Agent firmware** · Single board, multi-protocol · Rust · ReAct · Zero relay

Feishu, DingTalk, WeCom, QQ Channel, Telegram, and WebSocket converge on one ESP32—no gateway, no always-on PC. Provision via hotspot + browser; switch board type with `BOARD=xxx`.

---

## Table of contents

- [Overview](#overview)
- [Prerequisites](#prerequisites)
- [Quick start](#quick-start)
- [Environment & build](#environment--build)
- [Supported boards](#supported-boards)
- [Configuration](#configuration)
- [Features](#features)
- [Storage & security](#storage--security)
- [Documentation](#documentation)
- [Troubleshooting](#troubleshooting)
- [References & license](#references--license)

---

## Overview

- **Board as Agent**: ReAct, tools, and memory run entirely on the ESP32; no cloud inference.
- **Unified multi-channel**: All channels share one queue and one Agent; new channels register by implementing a trait.
- **Browser provisioning**: When unprovisioned, the device opens hotspot **Beetle** (no password); open **http://192.168.4.1**. After WiFi is set, use **http://beetle.local** (mDNS); pairing code protects write operations.
- Inspired by [OpenClaw](https://github.com/openclaw/openclaw); type-safe full-stack Agent on MCU in Rust.

**System topology:**

```
  FEISHU  DINGTALK  WECOM  QQ  TG  WS
      \      |      |    /   |   /
       \     |      |   /    |  /
        \   ▼      ▼ ▼      ▼
    ┌─────────────────────────────────┐
    │  ◉ ESP32  ·  ONE BOARD ONE AGENT │
    │  ReAct │ TOOLS │ MEMORY         │
    └─────────────────────────────────┘
```

---

## Prerequisites

| Environment | Requirement |
|-------------|-------------|
| **Rust** | [esp-rs toolchain](https://docs.espressif.com/projects/rust-book/en/latest/introduction.html), `espup install` |
| **Flash** | [espflash](https://github.com/esp-rs/espflash), `cargo install espflash` |
| **macOS / Linux** | No extra deps; first run of `build.sh` may prompt for espup/ldproxy |
| **Windows** | Visual Studio (Desktop development with C++ + Windows 10/11 SDK) |

---

## Quick start

### macOS / Linux

```bash
./build.sh                    # Build only
./build.sh --flash            # Build and flash (prompts for erase, port)
BOARD=esp32-s3-16mb ./build.sh --flash
ESPFLASH_PORT=/dev/cu.usbserial-xxx ./build.sh --flash   # Specify port
```

### Windows

In project root, use **PowerShell** or **cmd** (`build.cmd` calls `build.ps1`):

```powershell
.\build.ps1
.\build.ps1 --flash
$env:BOARD="esp32-s3-16mb"; .\build.ps1 --flash
$env:ESPFLASH_PORT="COM3"; .\build.ps1 --flash
```

If path is too long, run `.\build.ps1 clean` then build again.

**First use**: Device powers on with hotspot **Beetle**; open **http://192.168.4.1** in a browser to set WiFi and pairing code.

---

## Environment & build

```bash
cargo build --release
```

- **Target**: Default `xtensa-esp32s3-espidf`; board type from `BOARD` and `board_presets.toml`.
- **Features**: `config_api` (default), `telegram`, `feishu` (default), `websocket`, `cli`, `ota`, `gpio`.  
  Example: `cargo build --release --features cli,ota`

Flash: use `--flash` to flash; `./build.sh clean` to clean; `--no-monitor` to skip serial monitor. Set `ESPFLASH_PORT` to e.g. `/dev/cu.usbserial-xxx` or `COM3`. On connect failure: check USB cable/port; put board in download mode (hold BOOT, tap RESET); script prints diagnostics on erase/flash failure.

---

## Supported boards

| BOARD | Description |
|-------|-------------|
| `esp32-s3-16mb` | ESP32-S3, 16MB Flash + PSRAM (default; only supported board) |

Partition table is defined by `board_presets.toml` and `sdkconfig.defaults.esp32s3`. **Use the project partition table** or you will get `spiffs partition could not be found`.

---

## Configuration

- **At build time**: Env vars `BEETLE_*` before build; NVS keys override at runtime if present.
- **At runtime**: Config page writes to NVS; secrets are not logged or written to SPIFFS.

| Category | Config keys |
|----------|-------------|
| WiFi | `WIFI_SSID`, `WIFI_PASS` |
| Telegram | `TG_TOKEN`, `TG_ALLOWED_CHAT_IDS` |
| Feishu | `FEISHU_APP_ID`, `FEISHU_APP_SECRET`, `FEISHU_ALLOWED_CHAT_IDS` |
| DingTalk | `DINGTALK_WEBHOOK_URL` |
| WeCom | `WECOM_CORP_ID`, `WECOM_CORP_SECRET`, `WECOM_AGENT_ID`, `WECOM_DEFAULT_TOUSER` |
| QQ Channel | `QQ_CHANNEL_APP_ID`, `QQ_CHANNEL_SECRET` |
| LLM | Multi-source: `config/llm.json` (SPIFFS); build-time env for defaults. Keys: provider, api_key, model, api_url, stream, max_tokens; router/worker indices for routing mode. |
| Proxy / search | `PROXY_URL`, `SEARCH_KEY`, `TAVILY_KEY` |

Full key names and validation: `src/config.rs`. Runtime config segments (LLM, channels, system) and API: [Config API](docs/en-us/config-api.md). Provisioning: [Configuration](docs/en-us/configuration.md).

---

## Features

| Area | Description |
|------|-------------|
| Board as Agent | ReAct, tools, memory on ESP32 |
| Unified channels | Feishu / DingTalk / WeCom / QQ Channel / Telegram / WebSocket, same queue, same Agent |
| Browser provisioning | Hotspot Beetle → 192.168.4.1; after WiFi → http://beetle.local (mDNS), pairing code for writes |
| Rust stack | Type-safe, unified errors and resource limits; new channel/tool/LLM via trait |
| Memory & tools | Long-term memory, session summary, reminders; GetTime, Cron, Files, WebSearch, AnalyzeImage, FetchUrl, HttpPost, RemindAt, KvStore, UpdateSessionSummary; **board_info** for device status (chip, heap, uptime, pressure, WiFi, SPIFFS); Skills in system prompt. Optional: GpioRead, GpioWrite (feature `gpio`) |
| Resource & health | Orchestrator: heap/queue pressure, HTTP admission, channel circuit breaker; health and resource snapshot via API |

---

## Storage & security

- **SPIFFS**: `spiffs_data/` is packed and flashed to the spiffs partition (memory, sessions, skills).
- **OTA** (feature `ota`): Fetches firmware from config URL to spare partition; failure does not overwrite current partition.
- **Security**: Secrets not logged or written to disk; queue/message/response size limits are centralized; config page writes require pairing code.

---

## Documentation

| Doc | Description |
|-----|-------------|
| [Configuration](docs/en-us/configuration.md) | Provisioning, config page, mDNS, common config |
| [Config API contract](docs/en-us/config-api.md) | HTTP API: pairing, config segments, health, OTA, webhook |
| [Agent tools](docs/en-us/tools.md) | User-facing guide: what tools the Agent can use (get_time, web_search, board_info, etc.) |
| [Hardware & resources](docs/en-us/hardware.md) | Boards, memory, PSRAM, watchdog, build options, troubleshooting |
| [Architecture](docs/en-us/architecture.md) | Modules, data flow, extension |

---

## Troubleshooting

- **`spiffs partition could not be found`**: Use the project partition table (see [Hardware](docs/en-us/hardware.md)).
- **Flash/connect failure**: Check USB cable/port; put board in download mode (hold BOOT, tap RESET); set `ESPFLASH_PORT`.
- **Task watchdog / DNS etc.**: See [Hardware – known issues](docs/en-us/hardware.md#known-issues-and-troubleshooting).

---

## References & license

- [Rust on ESP Book](https://docs.espressif.com/projects/rust-book/)
- [esp-idf-svc](https://github.com/esp-rs/esp-idf-svc)

This project is dual-licensed under **MIT OR Apache-2.0**. See [LICENSE](LICENSE).
