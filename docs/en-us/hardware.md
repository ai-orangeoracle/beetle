# Hardware and resources

**English** | [中文](../zh-cn/hardware.md) | [Doc index](../README.md)

For **board selection and troubleshooting**: supported boards, memory and build options, where to observe runtime health, and pointers to configurable hardware docs (`hardware.json` details are not duplicated here).

---

## Boards and resources

| Board (BOARD=) | Flash | PSRAM | CPU | Notes |
|----------------|-------|-------|-----|-------|
| `esp32-s3-8mb` | 8MB | 8MB Octal | 240MHz | N8R8; use `BOARD=esp32-s3-8mb ./build.sh` |
| `esp32-s3-16mb` | 16MB | 8MB Octal | 240MHz | N16R8; default when BOARD unset |
| `esp32-s3-32mb` | 32MB | 16MB Octal | 240MHz | N32R16; use `BOARD=esp32-s3-32mb ./build.sh` |

- Board is selected via `BOARD=esp32-s3-8mb` | `esp32-s3-16mb` | `esp32-s3-32mb` (optional; default `esp32-s3-16mb`); `board_presets.toml` defines target and partition table.
- **Only ESP32-S3 with PSRAM is supported**; C3/S2 are no longer supported.

---

## Memory and watchdog

- **ESP32-S3**: Large allocations use PSRAM first; HTTP response bodies and other large buffers use PSRAM to reduce internal heap pressure. The orchestrator module manages resource admission: HTTP requests go through priority-based TLS permit with real-time heap checks; agent inbound messages and LLM/tool calls are gated by pressure level (Normal/Cautious/Critical).
- **Watchdog**: Task watchdog ~60 s; LLM/HTTP long requests feed the dog before running; if request timeout is ≥60s, keep this in mind.

---

## Build and performance

- Default `cargo build --release` uses `opt-level = "s"` (size and speed balance).
- For more LLM/JSON performance when Flash allows:  
  `cargo build --profile release-speed`  
  (inherits release and sets `opt-level` to 3.)

---

## Observability

- **HTTP**: `GET /api/health` and `GET /api/resource` are documented in [config-api](config-api.md) (orchestrator-aligned snapshots).
- **Logs**: Heartbeat emits periodic baselines for trends.
- **CLI** (feature `cli`): Serial `heap_info` and related commands for heap/PSRAM summaries.

---

## Configurable hardware devices

See [Hardware device config](hardware-device-config.md) (design constraints) and [config-api /api/config/hardware](config-api.md) (HTTP contract). The Agent tool name is **`device_control`** (registration rules in [tools](tools.md)).

---

## Known issues and troubleshooting

- **`esp_task_wdt_reset: task not found`**: Any thread that performs HTTP must be registered with the task watchdog (TWDT). The main Agent thread, Feishu/QQ WSS threads, and Telegram poll thread call `register_current_task_to_task_wdt()` at startup. If you still see this, check for use of `EspHttpClient` in a new thread without registration.
- **`couldn't get hostname for :xxx: getaddrinfo() returns 202`**: In ESP-IDF log format `:%s:` the colons are delimiters, not part of the hostname. 202 is DNS resolution failure. Common causes: WSS client auto-reconnect consuming socket/DNS (addressed with `disable_auto_reconnect: true`), or WiFi/DNS not ready.
