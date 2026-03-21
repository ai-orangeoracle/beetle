<p align="center">
  <img src="configure-ui/public/logo.png" alt="Beetle" width="132" height="132" />
</p>

<h1 align="center">Beetle</h1>

<p align="center">
  <strong>Hardware-native Edge AI Agent Runtime</strong><br/>
  Rust · ReAct · Multi-channel · Zero relay
</p>

<p align="center">
  <a href="README.zh-CN.md">中文</a> · <strong>English</strong>
</p>

<p align="center">
  <a href="http://beetle.uno/"><img alt="Live Config UI" src="https://img.shields.io/badge/Live%20Config%20UI-beetle.uno-1f6feb" /></a>
  <a href="#quick-start"><img alt="Quick Start" src="https://img.shields.io/badge/Quick%20Start-5%20minutes-2ea043" /></a>
  <a href="#platform-roadmap"><img alt="Platform Roadmap" src="https://img.shields.io/badge/Platform-ESP32--S3%20%E2%86%92%20Linux%20Class-orange" /></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg" /></a>
</p>

> **One device, one agent runtime.**  
> Build once, connect many channels, and drive real hardware with LLM decisions.

| Why Beetle | What you get now |
|------------|------------------|
| Hardware-native architecture | ReAct + tools + memory running on-board |
| Full product loop | Built-in lightweight config pages + full `configure-ui` web app + portal [beetle.uno](http://beetle.uno/) |
| Practical deployment target | Cost-effective devices in the **$7-$70** range |
| Expansion path | ESP32-S3 baseline today, Linux-class and stronger platforms next |

Built for the **$7-$70 device range** (roughly CNY 50-500): ESP32 now, Linux-class and other common platforms next.  
Current baseline and entry target is **ESP32-S3**; platforms with performance above ESP32-S3 are planned for compatibility expansion.

Feishu, DingTalk, WeCom, QQ Channel, Telegram, and WebSocket converge on one board—no gateway, no always-on PC. Provision via hotspot + browser; switch board type with `BOARD=xxx`.

## At a glance

- **What it is**: A hardware-native agent runtime inspired by OpenClaw.
- **Why it matters**: One board handles chat channels, tools, memory, and device actions.
- **Where it runs**: ESP32-S3 today; Linux-class and stronger platforms are on the roadmap.
- **What it enables**: Portable intelligent assistants and LLM-driven smart-device interconnection.
- **UI and delivery**: Firmware includes lightweight on-device config pages, and the full configuration web app is provided via `configure-ui` and [beetle.uno](http://beetle.uno/).

## Typical use cases

- **Pocket assistant**: Carry one board, talk through your preferred channel, trigger local tools.
- **Smart-space bridge**: Use LLM decisions to drive GPIO/PWM/ADC/buzzer and connect real devices.
- **Team prototype**: Build a customizable, commercializable AI hardware workflow before scaling.
- **Provisioning and operations UI**: Use the built-in configuration web UI for setup, channel config, health checks, and iterative tuning.

---

## Table of contents

- [Overview](#overview)
- [At a glance](#at-a-glance)
- [Typical use cases](#typical-use-cases)
- [Who this is for](#who-this-is-for)
- [Platform roadmap](#platform-roadmap)
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

- **Hardware as Agent runtime**: ReAct, tools, and memory run on the device itself (ESP32 baseline now, broader hardware targets next), with no mandatory cloud inference relay.
- **Unified multi-channel**: All channels share one queue and one Agent; new channels register by implementing a trait.
- **Browser provisioning**: When unprovisioned, the device opens hotspot **Beetle** (no password); open **http://192.168.4.1**. After WiFi is set, use the router-assigned device IP; pairing code protects write operations.
- **Positioning**: A hardware-native \"openClaw\" for low-cost to mid-cost devices, with ESP32-S3 as the current baseline.
- **Long-term vision**: Portable intelligent assistant + intelligent IoT interconnection, where LLM drives hardware capabilities for customizable and commercial deployments.
- Inspired by [OpenClaw](https://github.com/openclaw/openclaw); type-safe full-stack Agent on MCU in Rust.

**System topology:**

```
 FEISHU  DINGTALK  WECOM  QQ  TG  WS
     \      |      |    /   |   /
      \     |      |   /    |  /
       \    ▼      ▼  ▼     ▼
    ┌────────────────────────────────────┐
    │  BEETLE RUNTIME · ONE DEVICE AGENT │
    │  ReAct │ Tools │ Memory │ Orchestrator │
    └────────────────────────────────────┘
            │                   │
            ▼                   ▼
    ┌──────────────┐   ┌────────────────┐
    │Platform Layer│   │ Display (SPI)  │
    │ESP32-S3 (now)│   │ ST7789/ILI9341 │
    │Linux+ (next) │   │ Dashboard UI   │
    └──────────────┘   └────────────────┘
```

---

## Who this is for

- **Hardware developers / makers**: Build practical AI agents directly on low-cost boards.
- **IoT and embedded engineers**: Use one Rust codebase to connect chat channels, tools, and device control.
- **Product teams**: Prototype portable assistants and LLM-driven smart-device workflows before commercial rollout.
- **Open-source contributors**: Extend channels, tools, models, and platform compatibility through traits.

---

## Platform roadmap

| Stage | Platform scope | Status |
|-------|----------------|--------|
| **Now** | ESP32-S3 as baseline and entry threshold | Stable |
| **Next** | Linux-class edge devices (SBC/embedded Linux) | Planned |
| **Then** | Other common hardware platforms with performance above ESP32-S3 | Planned |

Current repo defaults target ESP32-S3 first to keep bring-up and validation deterministic. This is a **starting point**, not a hard platform limitation.

---

## Prerequisites

| Environment       | Requirement                                                                                                    |
| ----------------- | -------------------------------------------------------------------------------------------------------------- |
| **Rust**          | [esp-rs toolchain](https://docs.espressif.com/projects/rust-book/en/latest/introduction.html), `espup install` |
| **Flash**         | [espflash](https://github.com/esp-rs/espflash), `cargo install espflash`                                       |
| **macOS / Linux** | No extra deps; first run of `build.sh` may prompt for espup/ldproxy                                            |
| **Windows**       | Visual Studio (Desktop development with C++ + Windows 10/11 SDK)                                               |

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
- **Features**: `config_api` (default), `telegram`, `feishu` (default), `websocket`, `cli`, `ota`.  
  Example: `cargo build --release --features cli,ota`

Flash: use `--flash` to flash; `./build.sh clean` to clean; `--no-monitor` to skip serial monitor. Set `ESPFLASH_PORT` to e.g. `/dev/cu.usbserial-xxx` or `COM3`. On connect failure: check USB cable/port; put board in download mode (hold BOOT, tap RESET); script prints diagnostics on erase/flash failure.

---

## Supported boards

| BOARD           | Flash | PSRAM | Description                              |
| --------------- | ----- | ----- | ---------------------------------------- |
| `esp32-s3-8mb`  | 8MB   | 8MB   | N8R8; `BOARD=esp32-s3-8mb ./build.sh`    |
| `esp32-s3-16mb` | 16MB  | 8MB   | N16R8; default when BOARD unset          |
| `esp32-s3-32mb` | 32MB  | 16MB  | N32R16; `BOARD=esp32-s3-32mb ./build.sh` |

Partition table and flash size are chosen by `board_presets.toml` and the board overlay `sdkconfig.defaults.esp32s3.board` (written by the build script). **Use the project partition table** or you will get `spiffs partition could not be found`.

---

## Configuration

- **At build time**: Env vars `BEETLE_*` before build; NVS keys override at runtime if present.
- **At runtime**: Config page writes to NVS; secrets are not logged or written to SPIFFS.

| Category       | Config keys                                                                                                                                                                 |
| -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| WiFi           | `WIFI_SSID`, `WIFI_PASS`                                                                                                                                                    |
| Telegram       | `TG_TOKEN`, `TG_ALLOWED_CHAT_IDS`                                                                                                                                           |
| Feishu         | `FEISHU_APP_ID`, `FEISHU_APP_SECRET`, `FEISHU_ALLOWED_CHAT_IDS`                                                                                                             |
| DingTalk       | `DINGTALK_WEBHOOK_URL`                                                                                                                                                      |
| WeCom          | `WECOM_CORP_ID`, `WECOM_CORP_SECRET`, `WECOM_AGENT_ID`, `WECOM_DEFAULT_TOUSER`                                                                                              |
| QQ Channel     | `QQ_CHANNEL_APP_ID`, `QQ_CHANNEL_SECRET`                                                                                                                                    |
| LLM            | Multi-source: `config/llm.json` (SPIFFS); build-time env for defaults. Keys: provider, api_key, model, api_url, stream, max_tokens; router/worker indices for routing mode. |
| Proxy / search | `PROXY_URL`, `SEARCH_KEY`, `TAVILY_KEY`                                                                                                                                     |

Full key names and validation: `src/config.rs`. Runtime config segments (LLM, channels, system) and API: [Config API](docs/en-us/config-api.md). Provisioning: [Configuration](docs/en-us/configuration.md).

---

## Features

| Area                 | Description                                                                                                                                                                                                                                                                                                                           |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Board as Agent       | ReAct, tools, memory on ESP32                                                                                                                                                                                                                                                                                                         |
| Unified channels     | Feishu / DingTalk / WeCom / QQ Channel / Telegram / WebSocket, same queue, same Agent                                                                                                                                                                                                                                                 |
| **Display dashboard** | Real-time status display via SPI-connected TFT (ST7789 / ILI9341). Animated beetle icon reflects system state (Booting / NoWifi / Idle / Busy / Fault); live channel health dots, IP address, heap pressure bar. Pure `embedded-graphics` rendering—no image assets, PSRAM framebuffer, partial row flush for minimal SPI traffic. See [Display](docs/en-us/display.md). |
| Browser provisioning | Hotspot Beetle → http://192.168.4.1; after WiFi → router-assigned IP, pairing code for writes                                                                                                                                                                                                                                         |
| Rust stack           | Type-safe, unified errors and resource limits; new channel/tool/LLM via trait                                                                                                                                                                                                                                                         |
| Memory & tools       | Long-term memory, session summary, reminders; GetTime, Cron, Files, WebSearch, AnalyzeImage, FetchUrl, HttpPost, RemindAt, KvStore, UpdateSessionSummary; **board_info** for device status (chip, heap, uptime, pressure, WiFi, SPIFFS); **device_control** for GPIO/PWM/ADC/buzzer per config/hardware.json; Skills in system prompt |
| Resource & health    | Orchestrator: heap/queue pressure, HTTP admission, channel circuit breaker; health and resource snapshot via API                                                                                                                                                                                                                      |

---

## Storage & security

- **SPIFFS**: `spiffs_data/` is packed and flashed to the spiffs partition (memory, sessions, skills).
- **OTA** (feature `ota`): Fetches firmware from config URL to spare partition; failure does not overwrite current partition.
- **Security**: Secrets not logged or written to disk; queue/message/response size limits are centralized; config page writes require pairing code.

---

## Documentation

| Doc                                                                                 | Description                                                                              |
| ----------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| [Configuration](docs/en-us/configuration.md)                                        | Provisioning, config page, common config                                                 |
| [Config API contract](docs/en-us/config-api.md)                                     | HTTP API: pairing, config segments, health, OTA, webhook                                 |
| [Agent tools](docs/en-us/tools.md)                                                  | User-facing guide: what tools the Agent can use (get_time, web_search, board_info, etc.) |
| [Hardware & resources](docs/en-us/hardware.md)                                      | Boards, memory, PSRAM, watchdog, build options, troubleshooting                          |
| [Hardware device config & LLM-driven control](docs/en-us/hardware-device-config.md) | Milestone design: JSON config–driven device_control tool for GPIO/PWM/ADC/buzzer         |
| [Display dashboard](docs/en-us/display.md)                                          | SPI display setup, wiring, configuration, dashboard states, and caveats                  |
| [Architecture](docs/en-us/architecture.md)                                          | Modules, data flow, extension                                                            |

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
