<p align="center">
  <img src="configure-ui/public/logo.png" alt="甲壳虫" width="132" height="132" />
</p>

<h1 align="center">Beetle（甲壳虫）</h1>

<p align="center">
  <strong>硬件原生边缘 AI Agent 运行时</strong><br/>
  Rust · ReAct · 多通道 · 零中继
</p>

<p align="center">
  <a href="README.md">English</a> · <strong>中文</strong>
</p>

<p align="center">
  <a href="http://beetle.uno/"><img alt="在线配置 UI" src="https://img.shields.io/badge/%E5%9C%A8%E7%BA%BF%E9%85%8D%E7%BD%AE%20UI-beetle.uno-1f6feb" /></a>
  <a href="#快速开始"><img alt="快速开始" src="https://img.shields.io/badge/%E5%BF%AB%E9%80%9F%E5%90%AF%E5%8A%A8-5%E5%88%86%E9%92%9F-2ea043" /></a>
  <a href="#平台路线图"><img alt="平台路线图" src="https://img.shields.io/badge/%E5%B9%B3%E5%8F%B0-ESP32--S3%20%E2%86%92%20Linux%E7%B3%BB-orange" /></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg" /></a>
</p>

> **一块设备，一个 Agent 运行时。**  
> 一次部署，接入多通道，并让 LLM 决策直接驱动真实硬件。

| 为什么是 Beetle | 你现在就能得到 |
|-----------------|----------------|
| 硬件原生架构 | 板上运行 ReAct + 工具 + 记忆 |
| 完整产品闭环 | 固件内置轻量配置页 + 完整 `configure-ui` Web 应用 + 官方入口 [beetle.uno](http://beetle.uno/) |
| 实用成本带 | 面向 **50-500 元** 设备快速落地 |
| 清晰扩展路径 | 当前 ESP32-S3 基线，后续 Linux 系与更高性能平台 |

定位在 **50-500 元（约 7-70 美元）** 的硬件设备带：当前以 ESP32 为起点，后续扩展到 Linux 系与其他常见硬件平台。  
当前基准与准入门槛是 **ESP32-S3**；凡性能高于 ESP32-S3 的平台，后续都会逐步增加兼容。

飞书、钉钉、企微、QQ 频道、Telegram、WebSocket 汇聚到同一块设备上，无 Gateway、无常开 PC；配网用热点 + 浏览器，板型用 `BOARD=xxx` 切换。

## 一眼看懂

- **这是什么**：面向硬件原生的 Agent 运行时，设计思路受 openClaw 启发。
- **核心价值**：一块设备就能同时承载通道、工具、记忆与设备控制。
- **现在跑在哪**：当前以 ESP32-S3 稳定落地，后续扩展 Linux 系与更高性能平台。
- **最终要做什么**：随身智能助理 + LLM 驱动的智能万物互联。
- **产品完整度**：固件内置轻量配置页；完整配置 Web 应用通过 `configure-ui` 与官方入口 [beetle.uno](http://beetle.uno/) 提供。

## 典型场景

- **随身助理**：一块板子随身运行，通过常用 IM 通道对话并调用本地工具。
- **智能空间联动**：让 LLM 决策直接驱动 GPIO/PWM/ADC/蜂鸣器等硬件能力。
- **产品原型验证**：快速做可定制、可商业化的 AI 硬件工作流原型。
- **配置与运维**：通过内置配置网页完成初始化、通道配置、健康检查和持续调优。

---

## 目录

- [概述](#概述)
- [一眼看懂](#一眼看懂)
- [典型场景](#典型场景)
- [适用人群](#适用人群)
- [平台路线图](#平台路线图)
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

- **硬件即 Agent 运行时**：ReAct、工具、记忆运行在设备本体（当前以 ESP32 为基线，后续扩展到更广硬件），不依赖必须的云端中继推理。
- **多通道统一**：各通道同队列、同一 Agent；新通道实现 trait 即注册。
- **浏览器配网**：未配网时设备开热点 **Beetle**（无密码），浏览器打开 **http://192.168.4.1**；已连 WiFi 后使用路由器分配的设备 IP，配对码保护写操作。
- **项目定位**：面向硬件原生 Agent，先从 ESP32-S3 打基线，再向更高性能硬件扩展。
- **长期构想**：随身智能助理 + 智能万物互联（LLM 驱动硬件能力），支持可定制化与商业化落地。
- 受 [OpenClaw](https://github.com/openclaw/openclaw) 启发，用 Rust 在 MCU 上跑类型安全的全栈 Agent。

**系统拓扑：**

```
 FEISHU  DINGTALK  WECOM  QQ  TG  WS
     \      |      |    /   |   /
      \     |      |   /    |  /
       \    ▼      ▼  ▼     ▼
    ┌────────────────────────────────────┐
    │ BEETLE RUNTIME · 单设备单 Agent    │
    │ ReAct │ Tools │ Memory │ Orchestrator │
    └────────────────────────────────────┘
            │                   │
            ▼                   ▼
    ┌──────────────┐   ┌────────────────┐
    │ 平台层        │   │ 显示 (SPI)     │
    │ ESP32-S3     │   │ ST7789/ILI9341 │
    │ （当前）      │   │ 仪表板 UI      │
    │ Linux+（下一步）│  └────────────────┘
    └──────────────┘
```

---

## 适用人群

- **硬件开发者 / Maker**：希望在低成本设备上直接落地 AI Agent。
- **IoT / 嵌入式工程师**：希望用一套 Rust 架构打通通道、工具和设备控制。
- **产品与创新团队**：希望先做随身助理和 LLM 驱动设备联动的可商用原型。
- **开源贡献者**：希望扩展通道、工具、模型和平台兼容能力。

---

## 平台路线图

| 阶段 | 平台范围 | 状态 |
|------|----------|------|
| **当前** | 以 ESP32-S3 为基线与准入门槛 | 稳定 |
| **下一步** | Linux 系边缘设备（SBC/嵌入式 Linux） | 规划中 |
| **后续** | 其他常见且性能高于 ESP32-S3 的硬件平台 | 规划中 |

当前仓库默认先围绕 ESP32-S3，是为了让启动、验证、资源边界更可控；这是**起点**，不是平台终点。

---

## 前置要求

| 环境              | 要求                                                                                                        |
| ----------------- | ----------------------------------------------------------------------------------------------------------- |
| **Rust**          | [esp-rs 工具链](https://docs.espressif.com/projects/rust-book/en/latest/introduction.html)，`espup install` |
| **烧录**          | [espflash](https://github.com/esp-rs/espflash)，`cargo install espflash`                                    |
| **macOS / Linux** | 无额外要求；首次缺工具链时 `build.sh` 会提示安装 espup/ldproxy                                              |
| **Windows**       | Visual Studio（「使用 C++ 的桌面开发」+ Windows 10/11 SDK）                                                 |

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

**首次使用**：设备上电后开热点 **Beetle**，浏览器打开 **http://192.168.4.1** 填写 WiFi 与配对码。

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

| BOARD           | Flash | PSRAM | 说明                                     |
| --------------- | ----- | ----- | ---------------------------------------- |
| `esp32-s3-8mb`  | 8MB   | 8MB   | N8R8；`BOARD=esp32-s3-8mb ./build.sh`    |
| `esp32-s3-16mb` | 16MB  | 8MB   | N16R8；未设 BOARD 时默认                 |
| `esp32-s3-32mb` | 32MB  | 16MB  | N32R16；`BOARD=esp32-s3-32mb ./build.sh` |

分区表与 Flash 大小由 `board_presets.toml` 及构建脚本写入的 `sdkconfig.defaults.esp32s3.board` 决定，**须使用项目自带分区表**，否则会报 `spiffs partition could not be found`。

---

## 配置

- **编译时**：构建前环境变量 `BEETLE_*`；NVS 有对应 key 则运行时覆盖。
- **运行时**：配置页写入 NVS；密钥不打印、不写 SPIFFS。

| 类别        | 配置键                                                                                                                                              |
| ----------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| WiFi        | `WIFI_SSID`、`WIFI_PASS`                                                                                                                            |
| Telegram    | `TG_TOKEN`、`TG_ALLOWED_CHAT_IDS`                                                                                                                   |
| 飞书        | `FEISHU_APP_ID`、`FEISHU_APP_SECRET`、`FEISHU_ALLOWED_CHAT_IDS`                                                                                     |
| 钉钉        | `DINGTALK_WEBHOOK_URL`                                                                                                                              |
| 企微        | `WECOM_CORP_ID`、`WECOM_CORP_SECRET`、`WECOM_AGENT_ID`、`WECOM_DEFAULT_TOUSER`                                                                      |
| QQ 频道     | `QQ_CHANNEL_APP_ID`、`QQ_CHANNEL_SECRET`                                                                                                            |
| LLM         | 多源：`config/llm.json`（SPIFFS）；编译时环境变量作默认。字段：provider、api_key、model、api_url、stream、max_tokens；路由/工作源下标支持路由模式。 |
| 代理 / 搜索 | `PROXY_URL`、`SEARCH_KEY`、`TAVILY_KEY`                                                                                                             |

完整键名与校验见 `src/config.rs`。运行时配置分段（LLM、通道、系统）与 API 见 [配置 API 契约](docs/zh-cn/config-api.md)。配网见 [配置与使用](docs/zh-cn/configuration.md)。

---

## 功能与特性

| 维度         | 说明                                                                                                                                                                                                                                                                                                      |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 板子即 Agent | ReAct、工具、记忆均在 ESP32 内完成                                                                                                                                                                                                                                                                        |
| 多通道统一   | 飞书 / 钉钉 / 企微 / QQ 频道 / Telegram / WebSocket 同队列、同一 Agent                                                                                                                                                                                                                                    |
| **显示仪表板** | 通过 SPI 连接的 TFT 屏（ST7789 / ILI9341）实时显示运行状态。甲壳虫图标动态反映系统状态（启动中/无 WiFi/空闲/繁忙/异常）；通道健康状态点、IP 地址、堆压力进度条一目了然。纯 `embedded-graphics` 原语绘制，无图片资源依赖，PSRAM 帧缓冲，局部行刷新最小化 SPI 流量。详见 [显示仪表板](docs/zh-cn/display.md)。 |
| 浏览器配网   | 热点 Beetle → http://192.168.4.1；已连 WiFi → 路由器分配 IP，配对码保护写操作                                                                                                                                                                                                                             |
| Rust 全栈    | 类型安全、统一错误与资源上界；新通道/工具/LLM 实现 trait 即注册                                                                                                                                                                                                                                           |
| 记忆与工具   | 长期记忆、会话摘要、到点提醒；GetTime、Cron、Files、WebSearch、AnalyzeImage、FetchUrl、HttpPost、RemindAt、KvStore、UpdateSessionSummary；**board_info** 查设备状态（芯片、堆、运行时间、压力、WiFi、SPIFFS）；**device_control** 按 config/hardware.json 控制 GPIO/PWM/ADC/蜂鸣器等；Skills 注入系统提示 |
| 资源与健康   | 编排器：堆/队列压力、HTTP 准入、通道熔断；健康与资源快照通过 API 暴露                                                                                                                                                                                                                                     |

---

## 存储与安全

- **SPIFFS**：`spiffs_data/` 打包烧录到 spiffs 分区，存记忆、会话、skills。
- **OTA**（feature `ota`）：从配置 URL 拉固件写备用分区，失败不改写当前分区。
- **安全**：密钥不打印、不写盘；队列/消息/响应体上界集中配置；配置页写操作需配对码。

---

## 文档

| 文档                                                                | 说明                                                                |
| ------------------------------------------------------------------- | ------------------------------------------------------------------- |
| [配置与使用](docs/zh-cn/configuration.md)                           | 配网、配置页、常用配置                                              |
| [配置 API 契约](docs/zh-cn/config-api.md)                           | HTTP API：配对、配置分段、健康、OTA、webhook                        |
| [Agent 工具说明](docs/zh-cn/tools.md)                               | 面向用户：Agent 可用工具说明（get_time、web_search、board_info 等） |
| [硬件与资源](docs/zh-cn/hardware.md)                                | 板型、内存、PSRAM、看门狗、编译选项、排错                           |
| [硬件设备配置与 LLM 驱动设计](docs/zh-cn/hardware-device-config.md) | 里程碑设计：JSON 配置即用、device_control 工具、GPIO/PWM/ADC/蜂鸣器 |
| [显示仪表板](docs/zh-cn/display.md)                                 | SPI 显示屏配置、接线、仪表板状态说明与注意事项                      |
| [架构概要](docs/zh-cn/architecture.md)                              | 模块划分、数据流、扩展方式                                          |

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
