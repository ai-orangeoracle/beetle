# Hardware and resources

**English** | [中文](../zh-cn/hardware.md)

This doc is for users and developers: supported boards, memory, and build options for selection and troubleshooting.

---

## Boards and resources

| Board | Flash | PSRAM | CPU | Notes |
|-------|-------|-------|-----|-------|
| ESP32-S3 16MB | 16MB | 8MB Octal | 240MHz | Only supported board; large response/LLM request buffers prefer PSRAM |

- Board is selected via `BOARD=esp32-s3-16mb` (optional; default); `board_presets.toml` defines the target.
- **Only ESP32-S3 with PSRAM is supported**; C3/S2 are no longer supported.

---

## Memory and watchdog

- **ESP32-S3**: Large allocations use PSRAM first; HTTP response bodies and other large buffers use PSRAM to reduce internal heap pressure. Before entering the Agent, internal heap is checked for ≥ 48KB (reserved for dual TLS).
- **Watchdog**: Task watchdog ~60 s; LLM/HTTP long requests feed the dog before running; if request timeout is ≥60s, keep this in mind.

---

## Build and performance

- Default `cargo build --release` uses `opt-level = "s"` (size and speed balance).
- For more LLM/JSON performance when Flash allows:  
  `cargo build --profile release-speed`  
  (inherits release and sets `opt-level` to 3.)

---

## Observability

- **Heartbeat**: Periodically logs internal heap / PSRAM / total free heap for trend observation.
- **CLI** (feature `cli`): On serial you can run `heap_info` to see Internal free, PSRAM free, Total free.

Logs or CLI let you confirm memory usage is within expectations.

---

## Known issues and troubleshooting

- **`esp_task_wdt_reset: task not found`**: Any thread that performs HTTP must be registered with the task watchdog (TWDT). The main Agent thread, Feishu/QQ WSS threads, and Telegram poll thread call `register_current_task_to_task_wdt()` at startup. If you still see this, check for use of `EspHttpClient` in a new thread without registration.
- **`couldn't get hostname for :xxx: getaddrinfo() returns 202`**: In ESP-IDF log format `:%s:` the colons are delimiters, not part of the hostname. 202 is DNS resolution failure. Common causes: WSS client auto-reconnect consuming socket/DNS (addressed with `disable_auto_reconnect: true`), or WiFi/DNS not ready.
