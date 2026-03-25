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

> **一块板子，一个 Agent。**
> 编译一次，接入多个聊天平台，让 LLM 直接控制真实硬件。

| 为什么选 Beetle | 现在就能用上什么 |
|-----------------|----------------|
| 直接跑在硬件上 | ReAct、工具调用、记忆管理都在板子上运行 |
| 开箱即用 | 固件自带配置页面，还有完整的 Web 管理界面 [beetle.uno](http://beetle.uno/) |
| 成本友好 | 瞄准 **50-500 元**价位的设备 |
| 平台扩展 | 现在支持 ESP32-S3，接下来会支持 Linux 开发板 |

**价格定位**：50-500 元（约 7-70 美元）的硬件。现在从 ESP32 起步，后面会扩展到 Linux 单板机和其他常见平台。
**入门门槛**：ESP32-S3 是当前基准，比它性能更好的板子后续都会逐步支持。

把飞书、钉钉、企微、QQ 频道、Telegram、WebSocket 都接到一块板子上——不需要网关，不需要一直开着电脑。配网就用手机连热点打开浏览器，换板子就改个 `BOARD=xxx` 环境变量。

## 一眼看懂

- **这是啥**：一个跑在硬件上的 AI Agent 运行时，灵感来自 openClaw。
- **能干啥**：一块板子搞定聊天通道、工具调用、记忆存储和硬件控制。
- **跑在哪**：现在稳定支持 ESP32-S3，后面会扩展到 Linux 开发板。
- **最终目标**：做成随身智能助理，让 LLM 能控制各种智能设备。
- **完整度如何**：固件自带轻量配置页，完整的 Web 管理界面在 `configure-ui` 和 [beetle.uno](http://beetle.uno/)。

## 典型场景

- **随身助理**：带一块板子出门，用微信、Telegram 等聊天就能调用本地工具。
- **智能家居联动**：让 LLM 直接控制 GPIO、PWM、传感器、蜂鸣器这些硬件。
- **产品原型**：快速做一个能定制、能商用的 AI 硬件原型。
- **配置管理**：用内置的网页界面完成初始化、通道配置、健康检查。

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

- **硬件上跑 Agent**：ReAct、工具调用、记忆管理都在设备本地运行（现在是 ESP32，后面会支持更多硬件），不强制依赖云端推理。
- **多通道统一**：所有聊天平台共用一个消息队列和一个 Agent，新平台只需实现一个 trait 就能接入。
- **浏览器配网**：没配网时板子会开一个叫 **Beetle** 的热点（无密码），手机连上后打开 **http://192.168.4.1** 就能配置。配好 WiFi 后用路由器分配的 IP 访问，配对码保护写操作。
- **项目定位**：做一个硬件原生的 Agent 运行时，先在 ESP32-S3 上打稳基础，再扩展到更强的硬件。
- **长期目标**：做成随身智能助理，让 LLM 能控制各种智能设备，支持定制和商业化。
- 灵感来自 [OpenClaw](https://github.com/openclaw/openclaw)，用 Rust 在单片机上跑类型安全的全栈 Agent。

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

- **硬件开发者 / Maker**：想在便宜的板子上直接跑 AI Agent。
- **IoT / 嵌入式工程师**：想用一套 Rust 代码打通聊天平台、工具调用和硬件控制。
- **产品和创新团队**：想快速做一个随身助理或 LLM 控制设备的可商用原型。
- **开源贡献者**：想扩展更多聊天平台、工具、模型或硬件支持。

---

## 平台路线图

| 阶段 | 支持的硬件 | 状态 |
|------|----------|------|
| **现在** | ESP32-S3（基准） | ✅ 稳定 |
| **下一步** | Linux 开发板（树莓派、香橙派等） | 🚧 开发中 |
| **后续** | 其他性能更好的硬件平台 | 📋 计划中 |

现在先围绕 ESP32-S3 做，是为了让启动、验证、资源控制更好把握。这只是**起点**，不是终点。

---

## 前置要求

| 环境              | 需要什么                                                                                                        |
| ----------------- | ----------------------------------------------------------------------------------------------------------- |
| **Rust**          | [esp-rs 工具链](https://docs.espressif.com/projects/rust-book/en/latest/introduction.html)，运行 `espup install` 安装 |
| **烧录工具**      | [espflash](https://github.com/esp-rs/espflash)，运行 `cargo install espflash` 安装                                    |
| **macOS / Linux** | 没啥特别要求，第一次缺工具时 `build.sh` 会提示你装 espup/ldproxy                                              |
| **Windows**       | Visual Studio（勾选「使用 C++ 的桌面开发」+ Windows 10/11 SDK）                                                 |

---

## 快速开始

### macOS / Linux

```bash
./build.sh                    # 只编译
./build.sh --flash            # 编译完直接烧录（会提示你擦不擦除、选哪个串口）
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

**第一次用**：板子上电后会开一个叫 **Beetle** 的热点，手机连上后打开 **http://192.168.4.1** 填 WiFi 和配对码就行。

---

## 环境与构建

```bash
cargo build --release
```

- **编译目标**：默认是 `xtensa-esp32s3-espidf`，具体板型由 `BOARD` 环境变量和 `board_presets.toml` 决定。
- **功能开关**：`config_api`（默认开）、`telegram`、`feishu`（默认开）、`websocket`、`cli`、`ota`。
  比如：`cargo build --release --features cli,ota`

**烧录和串口**：加 `--flash` 才会烧录；`./build.sh clean` 清理编译产物；`--no-monitor` 不打开串口监控。指定串口用 `ESPFLASH_PORT=/dev/cu.usbserial-xxx` 或 `COM3`。连不上的话检查 USB 线、换个口试试，或者按住 BOOT 键再短按 RESET 进下载模式。脚本会在擦除/烧录失败时给你诊断提示。

---

## 支持板型

| BOARD           | Flash | PSRAM | 说明                                     |
| --------------- | ----- | ----- | ---------------------------------------- |
| `esp32-s3-8mb`  | 8MB   | 8MB   | N8R8，用 `BOARD=esp32-s3-8mb ./build.sh`    |
| `esp32-s3-16mb` | 16MB  | 8MB   | N16R8，不设 BOARD 就是这个                 |
| `esp32-s3-32mb` | 32MB  | 16MB  | N32R16，用 `BOARD=esp32-s3-32mb ./build.sh` |

分区表和 Flash 大小由 `board_presets.toml` 和构建脚本生成的 `sdkconfig.defaults.esp32s3.board` 决定，**必须用项目自带的分区表**，不然会报 `spiffs partition could not be found` 错误。

---

## 配置

- **编译时**：编译前设置 `BEETLE_*` 环境变量；如果 NVS 里有对应的 key，运行时会覆盖。
- **运行时**：通过配置页面写入 NVS；密钥不会打印也不会写到 SPIFFS。

| 类别        | 配置项                                                                                                                                              |
| ----------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| WiFi        | `WIFI_SSID`、`WIFI_PASS`                                                                                                                            |
| Telegram    | `TG_TOKEN`、`TG_ALLOWED_CHAT_IDS`                                                                                                                   |
| 飞书        | `FEISHU_APP_ID`、`FEISHU_APP_SECRET`、`FEISHU_ALLOWED_CHAT_IDS`                                                                                     |
| 钉钉        | `DINGTALK_WEBHOOK_URL`                                                                                                                              |
| 企微        | `WECOM_CORP_ID`、`WECOM_CORP_SECRET`、`WECOM_AGENT_ID`、`WECOM_DEFAULT_TOUSER`                                                                      |
| QQ 频道     | `QQ_CHANNEL_APP_ID`、`QQ_CHANNEL_SECRET`                                                                                                            |
| LLM         | 多个来源：`config/llm.json`（SPIFFS）；编译时环境变量作默认值。字段包括：provider、api_key、model、api_url、stream、max_tokens；支持路由模式。支持的提供商：`openai`、`anthropic`、`gemini`、`glm`、`qwen`、`deepseek`、`moonshot`、`ollama`。详见 [LLM 提供商配置指南](docs/zh-cn/llm-providers.md)。 |
| 代理 / 搜索 | `PROXY_URL`、`SEARCH_KEY`、`TAVILY_KEY`                                                                                                             |

完整的配置项和校验规则看 `src/config.rs`。运行时配置分段（LLM、通道、系统）和 API 文档看 [配置 API 契约](docs/zh-cn/config-api.md)。配网教程看 [配置与使用](docs/zh-cn/configuration.md)。

---

## 功能与特性

| 维度         | 说明                                                                                                                                                                                                                                                                                                      |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 板子上跑 Agent | ReAct、工具调用、记忆管理都在 ESP32 里完成                                                                                                                                                                                                                                                                        |
| 多平台统一   | 飞书、钉钉、企微、QQ 频道、Telegram、WebSocket 共用一个队列和一个 Agent                                                                                                                                                                                                                                    |
| 浏览器配网   | 热点 Beetle → http://192.168.4.1；连上 WiFi 后用路由器分配的 IP，配对码保护写操作                                                                                                                                                                                                             |
| Rust 全栈    | 类型安全、统一的错误处理和资源上限；新平台/工具/LLM 只需实现 trait 就能注册                                                                                                                                                                                                                           |
| 记忆与工具   | 长期记忆、会话摘要、定时提醒；内置工具：GetTime、Cron、Files、WebSearch、AnalyzeImage、FetchUrl、HttpPost、RemindAt、KvStore、UpdateSessionSummary；**board_info** 查看设备状态（芯片、内存、运行时间、压力、WiFi、存储）；**device_control** 根据 config/hardware.json 控制 GPIO/PWM/ADC/蜂鸣器等；Skills 可以注入到系统提示 |
| 资源与健康   | 编排器：内存/队列压力监控、HTTP 准入控制、通道熔断；健康状态和资源快照通过 API 暴露                                                                                                                                                                                                                     |

---

## 存储与安全

- **SPIFFS**：`spiffs_data/` 目录会打包烧录到 spiffs 分区，用来存记忆、会话、skills。
- **OTA**（需要开启 `ota` feature）：从配置的 URL 拉固件写到备用分区，失败了不会动当前分区。
- **安全**：密钥不打印、不写盘；队列/消息/响应体都有大小上限；配置页的写操作需要配对码。

### 配置页安全须知

**配对码保护**
- 第一次用的时候设置配对码，用来保护所有配置修改
- 配对码保存在浏览器本地，**1 小时后自动过期**，需要重新输入
- 建议用 6 位以上的随机字符，别用太简单的密码

**跨域访问**
- 可以通过公网域名（比如 http://beetle.uno）访问局域网设备
- 所有修改操作都有 CSRF 保护，防止恶意网站伪造请求
- 第一次访问时会自动获取安全令牌，不用手动操作

**使用建议**
- 只在可信的网络环境下用配置页面
- 别在公共场所或不安全的 WiFi 下配置
- 定期换配对码提高安全性
- 配置完可以断开设备热点，只通过路由器 IP 访问

---

## 文档

| 文档                                                                | 说明                                                                |
| ------------------------------------------------------------------- | ------------------------------------------------------------------- |
| [文档索引](docs/README.md)                                          | 按读者分类的中英文入口与维护约定                                         |
| [Linux 发布包说明](docs/zh-cn/linux-release-rollback.md)             | musl 包目录约定、手工部署说明（尚无一键安装）；tar 内含 systemd 示例 |
| [配置与使用](docs/zh-cn/configuration.md)                           | 怎么配网、用配置页、常用配置项                                              |
| [配置 API 契约](docs/zh-cn/config-api.md)                           | HTTP API：配对、配置分段、健康检查、OTA、webhook                        |
| [Agent 工具说明](docs/zh-cn/tools.md)                               | 固件 `build_default_registry` 注册的工具说明                         |
| [硬件与资源](docs/zh-cn/hardware.md)                                | 板型、内存、PSRAM、看门狗、编译选项、排错                           |
| [硬件设备配置](docs/zh-cn/hardware-device-config.md)                | `hardware.json` 与 `device_control`（GPIO/PWM/ADC/蜂鸣器）        |
| [架构概要](docs/zh-cn/architecture.md)                              | 模块怎么划分的、数据怎么流转、怎么扩展                                          |

---

## 故障排除

- **`spiffs partition could not be found`**：必须用项目自带的分区表（看 [硬件与资源](docs/zh-cn/hardware.md)）。
- **烧录/连接失败**：检查 USB 线和接口；让板子进入下载模式（按住 BOOT 键再短按 RESET）；或者指定 `ESPFLASH_PORT`。
- **任务看门狗 / DNS 等问题**：看 [硬件与资源 - 已知问题与排错](docs/zh-cn/hardware.md#已知问题与排错)。

---

## 参考与许可

- [Rust on ESP Book](https://docs.espressif.com/projects/rust-book/)
- [esp-idf-svc](https://github.com/esp-rs/esp-idf-svc)

本项目采用 **MIT OR Apache-2.0** 双许可，详见 [LICENSE](LICENSE)。
