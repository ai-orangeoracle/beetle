<p align="center">
  <strong>beetle</strong><br>
  <sub>甲虫</sub>
</p>

<p align="center">
  <code>一块板子 · 多端对话 · 零服务器 · 插电即用</code>
</p>

<p align="center">
  <strong>ESP32 边缘 AI Agent 固件</strong> · Rust · ReAct · 多通道同源
</p>

---

## ▸ 定位

> **单板即 Agent。** 飞书、钉钉、企微、QQ 频道、Telegram、WebSocket 全部汇聚到同一块 ESP32，无 Gateway、无常开 PC；配网用热点 + 浏览器，板型用 `BOARD=xxx` 切换，OTA 失败不覆盖当前分区。  
> 受 [OpenClaw](https://github.com/openclaw/openclaw) 启发，用 Rust 在 MCU 上跑类型安全的全栈 Agent。

---

## ▸ 架构一图

```
  FEISHU  DINGTALK  WECOM  QQ  TG  WS
      \      |      |    /   |   /
       \     |      |   /    |  /
        \    |      |  /     | /
         \   ▼      ▼ ▼      ▼
          ╔═══════════════════════╗
          ║  ◉ ESP32  · 单板     ║
          ║  一只甲虫 · 一 Agent ║
          ║  ReAct │ 工具 │ 记忆  ║
          ╚═══════════════════════╝
```

---

## ▸ 特性速览

| 维度 | 说明 |
|------|------|
| **板子即 Agent** | ReAct、工具、记忆均在 ESP32 内完成，无云端推理依赖 |
| **多通道统一** | 飞书 / 钉钉 / 企微 / QQ 频道 / Telegram / WebSocket 同队列、同一 Agent |
| **浏览器配网** | 热点 **Beetle** → **192.168.4.1**；已连 WiFi → **http://beetle.local**（mDNS），配对码保护写操作 |
| **Rust 全栈** | 类型安全、统一错误与资源上界，新通道/工具/LLM 实现 trait 即注册 |
| **记忆与工具** | 长期记忆、会话摘要、到点提醒；FetchUrl、WebSearch、Cron、Files；Skills 注入系统提示 |

---

## ▸ 快速开始

### macOS / Linux

```bash
./build.sh              # 仅构建
./build.sh --flash      # 构建后烧录（会提示擦除、选串口）
BOARD=esp32-s3-16mb ./build.sh --flash   # 指定板型并烧录（默认 S3）
ESPFLASH_PORT=/dev/cu.usbserial-xxx ./build.sh --flash   # 指定串口（也支持 cu.usbmodem*，如板载 USB）
```

### Windows

在项目根目录用 **PowerShell** 或 **cmd**（`build.cmd` 会调用 `build.ps1`）：

```powershell
.\build.ps1              # 仅构建
.\build.ps1 --flash       # 构建后烧录（会提示擦除、选串口）
$env:BOARD="esp32-s3-16mb"; .\build.ps1 --flash   # 指定板型并烧录（默认 S3）
$env:ESPFLASH_PORT="COM3"; .\build.ps1 --flash   # 指定串口（如 COM3）
```

Windows 需安装 **Visual Studio**（带「使用 C++ 的桌面开发」及 Windows 10/11 SDK）；若路径过长可先 `.\build.ps1 clean` 再构建。

---

首次缺工具链会自动安装 espup/ldproxy。未配网时设备开热点 **Beetle**（无密码），浏览器打开 **192.168.4.1** 填 WiFi 与配对码。

---

## ▸ 环境与构建

- **Rust**：`rust-toolchain.toml` 使用 `esp` channel，需先安装 [esp-rs 工具链](https://docs.espressif.com/projects/rust-book/en/latest/introduction.html)（`espup install`）
- **烧录**：[espflash](https://github.com/esp-rs/espflash)（`cargo install espflash`）

```bash
cargo build --release
```

- **Features**：`config_api`（默认）、`telegram`、`feishu`（默认）、`websocket`、`cli`、`ota`、`gpio`，例：`cargo build --release --features cli,ota`
- **Target**：默认 `xtensa-esp32s3-espidf`；其他板型见下节

---

## ▸ 支持板型（BOARD）

| BOARD | 说明 |
|-------|------|
| `esp32-s3-16mb` | ESP32-S3，16MB + PSRAM（默认，唯一支持板型） |

分区表由 `board_presets.toml` 与 sdkconfig.defaults.esp32s3 决定，**须使用项目自带分区表**，否则会报 `spiffs partition could not be found`。

烧录：`--flash` 才烧录；`./build.sh clean` 清理；`--no-monitor` 不打开串口监控。指定串口：`ESPFLASH_PORT=/dev/cu.usbserial-xxx` 或 `ESPFLASH_PORT=/dev/cu.usbmodem*`（板载 USB/CH340）。若连接失败：检查 USB 线/口、板子进入下载模式（按住 BOOT 短按 RESET）、或换另一串口；脚本会在 erase/flash 失败时打印诊断提示。

---

## ▸ 配置概要

- **编译时**：构建前环境变量 `BEETLE_*`；NVS 有对应 key 则运行时覆盖
- **运行时**：配置页写入 NVS；密钥不打印、不写 SPIFFS

| 类别 | 键 |
|------|-----|
| WiFi | `WIFI_SSID`、`WIFI_PASS` |
| Telegram | `TG_TOKEN`、`TG_ALLOWED_CHAT_IDS` |
| 飞书 | `FEISHU_APP_ID`、`FEISHU_APP_SECRET`、`FEISHU_ALLOWED_CHAT_IDS` |
| 钉钉 | `DINGTALK_WEBHOOK_URL` |
| 企微 | `WECOM_CORP_ID`、`WECOM_CORP_SECRET`、`WECOM_AGENT_ID`、`WECOM_DEFAULT_TOUSER` |
| QQ 频道 | `QQ_CHANNEL_APP_ID`、`QQ_CHANNEL_SECRET` |
| LLM | `API_KEY`、`MODEL`、`MODEL_PROVIDER`、`API_URL`（兼容 Ollama 等） |
| 代理 / 搜索 | `PROXY_URL`、`SEARCH_KEY`、`TAVILY_KEY` |

完整键名与校验见 `src/config.rs`。配网与配置页详见 [配置与使用](docs/configuration.md)。

---

## ▸ 其他

- **SPIFFS**：`spiffs_data/` 打包烧录到 spiffs 分区，存记忆、会话、skills
- **OTA**（feature `ota`）：从配置 URL 拉固件写备用分区，失败不改写当前分区
- **安全**：密钥不打印、不写盘；队列/消息/响应体上界集中配置

---

## ▸ 文档

| 文档 | 说明 |
|------|------|
| [配置与使用](docs/configuration.md) | 配网、配置页、mDNS、常用配置 |
| [硬件与资源](docs/hardware.md) | 板型、内存、PSRAM、看门狗、编译选项 |
| [架构概要](docs/architecture.md) | 模块划分、数据流、扩展方式 |

---

## ▸ 参考

- [Rust on ESP Book](https://docs.espressif.com/projects/rust-book/)
- [esp-idf-svc](https://github.com/esp-rs/esp-idf-svc)
