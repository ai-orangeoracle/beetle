# 硬件与资源

[English](../en-us/hardware.md) | **中文**

本文档面向用户与开发者，说明支持板型、内存与编译选项，便于选型与排错。

---

## 板型与资源

| 板型 | Flash | PSRAM | CPU | 备注 |
|------|-------|-------|-----|------|
| ESP32-S3 16MB | 16MB | 8MB Octal | 240MHz | 唯一支持板型，大响应体/LLM 请求体优先使用 PSRAM |

- 板型通过 `BOARD=esp32-s3-16mb` 选择（可省略，为默认）；`board_presets.toml` 决定 target。
- **仅支持带 PSRAM 的 ESP32-S3**；C3/S2 已不再支持。

---

## 内存与看门狗

- **ESP32-S3**：大块分配优先 PSRAM，HTTP 响应体等大 buffer 走 PSRAM，减轻内部堆压力；进 Agent 前检查 internal 堆 ≥ 48KB（双 TLS 预留）。
- **看门狗**：任务看门狗约 60 秒；LLM/HTTP 长请求前会喂狗，建议请求超时 ≥60s 时留意配置。

---

## 编译与性能

- 默认 `cargo build --release` 使用 `opt-level = "s"`（体积与速度兼顾）。
- ESP32-S3 若 Flash 充足且希望提升 LLM/JSON 性能，可选用：  
  `cargo build --profile release-speed`  
  即继承 release 并将 `opt-level` 设为 3。

---

## 可观测性

- **Heartbeat**：周期打印内部堆/PSRAM/总空闲堆，便于观察趋势。
- **CLI**（feature `cli`）：串口下可执行 `heap_info` 查看 Internal free、PSRAM free、Total free。

通过日志或 CLI 可验证内存使用是否在预期范围内。

---

## 已知问题与排错

- **`esp_task_wdt_reset: task not found`**：发起 HTTP 的线程必须已注册到任务看门狗（TWDT）。主 Agent 线程、Feishu/QQ WSS 线程、Telegram 轮询线程在启动时均已调用 `register_current_task_to_task_wdt()`；若仍出现此错误，检查是否在新线程中直接使用 `EspHttpClient` 而未注册。
- **`couldn't get hostname for :xxx: getaddrinfo() returns 202`**：ESP-IDF 日志格式为 `:%s:`，冒号是分隔符，不是 hostname 的一部分。202 为 DNS 解析失败。常见原因：WSS 客户端自动重连占用 socket/DNS 资源（已通过 `disable_auto_reconnect: true` 修复），或 WiFi/DNS 未就绪。
