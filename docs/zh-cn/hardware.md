# 硬件与资源

[English](../en-us/hardware.md) | **中文** | [文档索引](../README.md)

本文档面向**选型与排错的用户与开发者**，说明支持的板型、内存与编译选项、可观测入口，以及可配置硬件的文档引用（`hardware.json` 的细节不在此重复展开）。

---

## 板型与资源

| 板型 (BOARD=) | Flash | PSRAM | CPU | 备注 |
|---------------|-------|-------|-----|------|
| `esp32-s3-8mb` | 8MB | 8MB Octal | 240MHz | N8R8；构建时用 `BOARD=esp32-s3-8mb ./build.sh` |
| `esp32-s3-16mb` | 16MB | 8MB Octal | 240MHz | N16R8；未设 BOARD 时默认 |
| `esp32-s3-32mb` | 32MB | 16MB Octal | 240MHz | N32R16；构建时用 `BOARD=esp32-s3-32mb ./build.sh` |

- 板型通过 `BOARD=esp32-s3-8mb` | `esp32-s3-16mb` | `esp32-s3-32mb` 选择（可省略，默认 `esp32-s3-16mb`）；`board_presets.toml` 决定 target 与分区表。
- **仅支持带 PSRAM 的 ESP32-S3**。

---

## 内存与看门狗

- **ESP32-S3**：大块分配优先 PSRAM，HTTP 响应体等大 buffer 走 PSRAM，减轻内部堆压力。orchestrator 模块统一管理资源准入：HTTP 请求经过带优先级的 TLS 令牌与实时堆检查；agent 入站消息与 LLM/工具调用受压力等级（Normal/Cautious/Critical）门控。
- **看门狗**：任务看门狗约 60 秒；LLM/HTTP 长请求前会喂狗，建议请求超时 ≥60s 时留意配置。

---

## 编译与性能

- 默认 `cargo build --release` 使用 `opt-level = 3`（性能优先）。
- 若更关注固件体积，可使用：  
  `cargo build --profile release-size`  
  即使用 `opt-level = "s"`。

---

## 可观测性

- **HTTP**：`GET /api/health` 与 `GET /api/resource` 的字段说明见 [config-api](config-api.md)（与 orchestrator 快照一致）。
- **日志**：heartbeat 周期性输出基线，便于对照趋势。
- **CLI**（feature `cli`）：串口 `heap_info` 等命令查看堆与 PSRAM 摘要。

---

## 可配置硬件设备

见 [硬件设备配置](hardware-device-config.md)（设计约束）与 [config-api /api/config/hardware](config-api.md)（HTTP 契约）；Agent 侧工具名为 **`device_control`**（注册条件见 [tools](tools.md)）。

---

## 已知问题与排错

- **`esp_task_wdt_reset: task not found`**：发起 HTTP 的线程必须已注册到任务看门狗（TWDT）。主 Agent 线程、Feishu/QQ WSS 线程、Telegram 轮询线程在启动时均已调用 `register_current_task_to_task_wdt()`；若仍出现此错误，检查是否在新线程中直接使用 `EspHttpClient` 而未注册。
- **`couldn't get hostname for :xxx: getaddrinfo() returns 202`**：ESP-IDF 日志格式为 `:%s:`，冒号是分隔符，不是 hostname 的一部分。202 为 DNS 解析失败。常见原因：WSS 客户端自动重连占用 socket/DNS 资源（已通过 `disable_auto_reconnect: true` 修复），或 WiFi/DNS 未就绪。
