<p align="center">
  <img src="docs/assets/beetle-logo.svg" alt="甲虫" width="64" height="64" />
</p>

# Beetle（甲虫）

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

[English](README.md) | **中文**

**ESP32 边缘 AI Agent 固件** · 单板多协议 · Rust · ReAct · 零中继

飞书、钉钉、企微、QQ 频道、Telegram、WebSocket 汇聚到同一块 ESP32，无 Gateway、无常开 PC；配网用热点 + 浏览器，板型用 `BOARD=xxx` 切换。

---

## 目录

- [概述](#概述)
- [前置要求](#前置要求)
- [快速开始](#快速开始)
- [环境与构建](#环境与构建)
- [支持板型](#支持板型)
- [配置](#配置)
- [功能与特性](#功能与特性)
- [存储与安全](#存储与安全)
- [文档](#文档)
- [故障排除](#故障排除)
- [参考与许可](#参考与许可)

---

## 概述

- **单板即 Agent**：ReAct、工具、记忆均在 ESP32 内完成，无云端推理依赖。
- **多通道统一**：各通道同队列、同一 Agent；新通道实现 trait 即注册。
- **浏览器配网**：未配网时设备开热点 **Beetle**（无密码），浏览器打开 **192.168.4.1**；已连 WiFi 后使用 **http://beetle.local**（mDNS），配对码保护写操作。
- 受 [OpenClaw](https://github.com/openclaw/openclaw) 启发，用 Rust 在 MCU 上跑类型安全的全栈 Agent。

**系统拓扑：**

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

## 前置要求

| 环境 | 要求 |
|------|------|
| **Rust** | [esp-rs 工具链](https://docs.espressif.com/projects/rust-book/en/latest/introduction.html)，`espup install` |
| **烧录** | [espflash](https://github.com/esp-rs/espflash)，`cargo install espflash` |
| **macOS / Linux** | 无额外要求；首次缺工具链时 `build.sh` 会提示安装 espup/ldproxy |
| **Windows** | Visual Studio（「使用 C++ 的桌面开发」+ Windows 10/11 SDK） |

---

## 快速开始

### macOS / Linux

```bash
./build.sh                    # 仅构建
./build.sh --flash            # 构建后烧录（会提示擦除、选串口）
BOARD=esp32-s3-16mb ./build.sh --flash
ESPFLASH_PORT=/dev/cu.usbserial-xxx ./build.sh --flash   # 指定串口
```

### Windows

在项目根目录用 **PowerShell** 或 **cmd**（`build.cmd` 会调用 `build.ps1`）：

```powershell
.\build.ps1
.\build.ps1 --flash
$env:BOARD="esp32-s3-16mb"; .\build.ps1 --flash
$env:ESPFLASH_PORT="COM3"; .\build.ps1 --flash
```

路径过长时可先 `.\build.ps1 clean` 再构建。

**首次使用**：设备上电后开热点 **Beetle**，浏览器打开 **192.168.4.1** 填写 WiFi 与配对码。

---

## 环境与构建

```bash
cargo build --release
```

- **Target**：默认 `xtensa-esp32s3-espidf`；板型由 `BOARD` 与 `board_presets.toml` 决定。
- **Features**：`config_api`（默认）、`telegram`、`feishu`（默认）、`websocket`、`cli`、`ota`。  
  示例：`cargo build --release --features cli,ota`

烧录与串口：`--flash` 才烧录；`./build.sh clean` 清理；`--no-monitor` 不打开串口监控。指定串口：`ESPFLASH_PORT=/dev/cu.usbserial-xxx` 或 `COM3`。连接失败时检查 USB 线/口、板子进入下载模式（按住 BOOT 短按 RESET），脚本会在 erase/flash 失败时打印诊断提示。

---

## 支持板型

| BOARD | Flash | PSRAM | 说明 |
|-------|-------|-------|------|
| `esp32-s3-8mb` | 8MB | 8MB | N8R8；`BOARD=esp32-s3-8mb ./build.sh` |
| `esp32-s3-16mb` | 16MB | 8MB | N16R8；未设 BOARD 时默认 |
| `esp32-s3-32mb` | 32MB | 16MB | N32R16；`BOARD=esp32-s3-32mb ./build.sh` |

分区表与 Flash 大小由 `board_presets.toml` 及构建脚本写入的 `sdkconfig.defaults.esp32s3.board` 决定，**须使用项目自带分区表**，否则会报 `spiffs partition could not be found`。

---

## 配置

- **编译时**：构建前环境变量 `BEETLE_*`；NVS 有对应 key 则运行时覆盖。
- **运行时**：配置页写入 NVS；密钥不打印、不写 SPIFFS。

| 类别 | 配置键 |
|------|--------|
| WiFi | `WIFI_SSID`、`WIFI_PASS` |
| Telegram | `TG_TOKEN`、`TG_ALLOWED_CHAT_IDS` |
| 飞书 | `FEISHU_APP_ID`、`FEISHU_APP_SECRET`、`FEISHU_ALLOWED_CHAT_IDS` |
| 钉钉 | `DINGTALK_WEBHOOK_URL` |
| 企微 | `WECOM_CORP_ID`、`WECOM_CORP_SECRET`、`WECOM_AGENT_ID`、`WECOM_DEFAULT_TOUSER` |
| QQ 频道 | `QQ_CHANNEL_APP_ID`、`QQ_CHANNEL_SECRET` |
| LLM | 多源：`config/llm.json`（SPIFFS）；编译时环境变量作默认。字段：provider、api_key、model、api_url、stream、max_tokens；路由/工作源下标支持路由模式。 |
| 代理 / 搜索 | `PROXY_URL`、`SEARCH_KEY`、`TAVILY_KEY` |

完整键名与校验见 `src/config.rs`。运行时配置分段（LLM、通道、系统）与 API 见 [配置 API 契约](docs/zh-cn/config-api.md)。配网见 [配置与使用](docs/zh-cn/configuration.md)。

---

## 功能与特性

| 维度 | 说明 |
|------|------|
| 板子即 Agent | ReAct、工具、记忆均在 ESP32 内完成 |
| 多通道统一 | 飞书 / 钉钉 / 企微 / QQ 频道 / Telegram / WebSocket 同队列、同一 Agent |
| 浏览器配网 | 热点 Beetle → 192.168.4.1；已连 WiFi → http://beetle.local（mDNS），配对码保护写操作 |
| Rust 全栈 | 类型安全、统一错误与资源上界；新通道/工具/LLM 实现 trait 即注册 |
| 记忆与工具 | 长期记忆、会话摘要、到点提醒；GetTime、Cron、Files、WebSearch、AnalyzeImage、FetchUrl、HttpPost、RemindAt、KvStore、UpdateSessionSummary；**board_info** 查设备状态（芯片、堆、运行时间、压力、WiFi、SPIFFS）；**device_control** 按 config/hardware.json 控制 GPIO/PWM/ADC/蜂鸣器等；Skills 注入系统提示 |
| 资源与健康 | 编排器：堆/队列压力、HTTP 准入、通道熔断；健康与资源快照通过 API 暴露 |

---

## 存储与安全

- **SPIFFS**：`spiffs_data/` 打包烧录到 spiffs 分区，存记忆、会话、skills。
- **OTA**（feature `ota`）：从配置 URL 拉固件写备用分区，失败不改写当前分区。
- **安全**：密钥不打印、不写盘；队列/消息/响应体上界集中配置；配置页写操作需配对码。

---

## 文档

| 文档 | 说明 |
|------|------|
| [配置与使用](docs/zh-cn/configuration.md) | 配网、配置页、mDNS、常用配置 |
| [配置 API 契约](docs/zh-cn/config-api.md) | HTTP API：配对、配置分段、健康、OTA、webhook |
| [Agent 工具说明](docs/zh-cn/tools.md) | 面向用户：Agent 可用工具说明（get_time、web_search、board_info 等） |
| [硬件与资源](docs/zh-cn/hardware.md) | 板型、内存、PSRAM、看门狗、编译选项、排错 |
| [硬件设备配置与 LLM 驱动设计](docs/zh-cn/hardware-device-config.md) | 里程碑设计：JSON 配置即用、device_control 工具、GPIO/PWM/ADC/蜂鸣器 |
| [架构概要](docs/zh-cn/architecture.md) | 模块划分、数据流、扩展方式 |

---

## 故障排除

- **`spiffs partition could not be found`**：须使用项目自带分区表（见 [硬件与资源](docs/zh-cn/hardware.md)）。
- **烧录/连接失败**：检查 USB 线/口；板子进入下载模式（按住 BOOT 短按 RESET）；指定 `ESPFLASH_PORT`。
- **任务看门狗 / DNS 等**：见 [硬件与资源 - 已知问题与排错](docs/zh-cn/hardware.md#已知问题与排错)。

---

## 参考与许可

- [Rust on ESP Book](https://docs.espressif.com/projects/rust-book/)
- [esp-idf-svc](https://github.com/esp-rs/esp-idf-svc)

本项目采用 **MIT OR Apache-2.0** 双许可，见 [LICENSE](LICENSE)。
