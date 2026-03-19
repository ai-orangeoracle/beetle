# Config API Contract

[中文](../zh-cn/config-api.md) | **English**

This doc is for **developers integrating with the device HTTP API** (e.g. custom config UI, scripts, or third-party apps). The device firmware exposes only HTTP APIs and **does not** ship a built-in config UI; the config UI is implemented by an external frontend (e.g. this repo’s `configure-ui` or a GitHub Pages deployment). After connecting to the device hotspot or the same LAN, the user enters the **device address** in that page (recommended **http://beetle.local/**; if that fails, **http://192.168.4.1** or the router-assigned IP) to call the APIs below.

## Network and access

- **SoftAP**: After power-on the device starts a hotspot; SSID is fixed as **Beetle** (no password). When connected to this hotspot, use **http://beetle.local/** first; if unavailable, use device IP **http://192.168.4.1** (matches firmware).
- **STA**: If WiFi is configured and the device is connected to the user’s router, the same HTTP service is reachable on the STA network. Prefer **http://beetle.local/** (mDNS); if unavailable, use the LAN address assigned by the router.
- **CORS**: All responses for `/api/*` and `GET /` must include `Access-Control-Allow-Origin: *`. OPTIONS preflight for the listed paths returns 200 with `Access-Control-Allow-Methods: GET, POST, DELETE, OPTIONS` and `Access-Control-Allow-Headers: Content-Type, X-Pairing-Code`, so the external config UI can call the API cross-origin.

## Pairing code and auth

- **Not activated**: No valid 6-digit pairing code in NVS. Only **GET /config**, **GET /pairing**, **GET /api/pairing_code**, **POST /api/pairing_code** (for initial setup only), and all **OPTIONS** are allowed; all other requests return 401 `{"error":"Please set pairing code first"}`. The frontend should direct the user to `/pairing` to set the code.
- **Activated**: The user has set a 6-digit code via **POST /api/pairing_code**. **Read-only** endpoints (GET /, GET /api/config, health, diagnose, sessions, memory/status, skills, soul, user) **do not** require the pairing code. **Write** operations (POST /api/config/wifi, /api/config/llm, /api/config/channels, /api/config/system, /api/config/hardware, /api/restart, /api/config_reset, /api/soul, /api/user, /api/skills, /api/skills/import, /api/webhook, /api/ota; DELETE /api/skills) must include the correct code or return 401 `{"error":"Invalid pairing code"}`.
- **How to send**: Query `?code=<6-digit>` or Header `X-Pairing-Code: <6-digit>`.
- **Factory reset**: **POST /api/config_reset** requires the correct pairing code; on success it clears config and the pairing code, and the device returns to the not-activated state.

## Root

### GET /

- **Purpose**: Probe device availability and list APIs.
- **Response**: 200, `Content-Type: application/json` (if supported).
- **Body example**:
  ```json
  {
    "name": "Pocket Crayfish",
    "version": "<CARGO_PKG_VERSION>",
    "endpoints": [
      "GET /pairing",
      "GET /api/pairing_code",
      "POST /api/pairing_code",
      "GET /api/config",
      "GET /api/health",
      "GET /api/diagnose",
      "GET /api/soul",
      "POST /api/soul",
      "GET /api/user",
      "POST /api/user",
      "GET /api/sessions",
      "GET /api/memory/status",
      "GET /api/skills",
      "POST /api/skills",
      "DELETE /api/skills",
      "POST /api/skills/import",
      "POST /api/restart",
      "POST /api/config_reset",
      "POST /api/webhook",
      "GET /api/config/hardware",
      "POST /api/config/hardware"
    ]
  }
  ```
  - When the `ota` feature is enabled, `endpoints` also includes `"POST /api/ota"`.

### GET /api/pairing_code

- **Purpose**: Check whether a pairing code is set (does not return the code).
- **Response**: 200, `{"code_set": true}` or `{"code_set": false}`.
- **Auth**: Whitelisted; no pairing code required.

### POST /api/pairing_code

- **Purpose**: Set the 6-digit pairing code for the first time (only when not yet activated).
- **Request**: `Content-Type: application/json`, Body `{"code": "123456"}` (6 digits).
- **Response**: Success 200 `{"ok": true}`; already set 400; invalid format 400.
- **Auth**: Whitelisted; no pairing code required.

### GET /pairing

- **Purpose**: Pairing setup page HTML (frontend should redirect here when not activated).
- **Response**: 200, `Content-Type: text/html; charset=utf-8`.

## Config read/write

**Storage**: NVS holds only 6 small keys (WiFi SSID/password, proxy, session count, group trigger, UI locale). LLM multi-source and channel config are on SPIFFS (`config/llm.json`, `config/channels.json`); hardware device config on `config/hardware.json`; skill enable/order on `config/skills_meta.json`. GET /api/config merges NVS and SPIFFS and returns the full config.

### GET /api/wifi/scan

- **Purpose**: Device scans nearby WiFi and returns SSID list for the config UI dropdown.
- **Auth**: No pairing code required.
- **Response**: 200, JSON array `[{ "ssid": "MyWiFi", "rssi": -50 }, ...]`, sorted by signal strength (rssi) descending. 503 when scan is unavailable (non-ESP or WiFi not ready).

### GET /api/config

- **Purpose**: Get the current full config (real values for all fields, including secrets).
- **Response**: 200, JSON is `AppConfig` serialization; all fields are stored values.
- **Multi-LLM**: `llm_sources` is an array; each item has `provider`, `api_key`, `model`, `api_url`, optional `stream` (bool, default false; true uses SSE streaming to reduce peak memory), optional `max_tokens` (u32; null means clients use built-in default 1024). When empty, load builds a single source from legacy fields. `llm_router_source_index` and `llm_worker_source_index` are optional 0–255; when both set and &lt; `llm_sources.len()`, router mode is enabled.

### POST /api/config/llm

- **Purpose**: Write only the LLM segment (multi-source and router/worker indices) to SPIFFS (`config/llm.json`); request body is the full segment; backend validates and writes.
- **Request**: `Content-Type: application/json`, Body `{ "llm_sources": [...], "llm_router_source_index": 0, "llm_worker_source_index": 0 }` (last two optional). `llm_sources` must be non-empty; each item must have `api_key`. Each item may include:
  - `provider` (required, e.g. `"anthropic"`, `"openai"`, `"openai_compatible"`)
  - `api_key` (required)
  - `model` (required)
  - `api_url` (required; for openai/openai_compatible may be empty for default endpoint)
  - `stream` (optional bool, default false; true uses SSE streaming to reduce peak memory)
  - `max_tokens` (optional u32, default null; null means clients use built-in default 1024)
- **Validation**: This segment only—`llm_sources` non-empty; field lengths (provider/api_key/model ≤ 64, api_url ≤ 256); router/worker indices must be &lt; `llm_sources.len()`.
- **Response**: Success 200 `{"ok": true}`; validation failure 400.

### POST /api/config/channels

- **Purpose**: Write only the channels segment (Telegram, Feishu, DingTalk, WeCom, QQ Channel, Webhook) to SPIFFS (`config/channels.json`); request body is the full segment; backend validates and writes.
- **Request**: `Content-Type: application/json`, Body includes `tg_token`, `tg_allowed_chat_ids`, `feishu_app_id`, `feishu_app_secret`, `feishu_allowed_chat_ids`, `dingtalk_webhook_url`, `wecom_corp_id`, `wecom_corp_secret`, `wecom_agent_id`, `wecom_default_touser`, `qq_channel_app_id`, `qq_channel_secret`, `webhook_enabled`, `webhook_token`.
- **Validation**: This segment’s field lengths (tg/feishu/wecom/qq etc. ≤ 64, dingtalk_webhook_url ≤ 512, wecom_default_touser ≤ 128).
- **Response**: Success 200 `{"ok": true}`; validation failure 400.

### POST /api/config/system

- **Purpose**: Write only the system segment (WiFi, proxy, session count, group trigger, UI locale); request body is the full segment; backend validates and writes.
- **Request**: `Content-Type: application/json`, Body includes `wifi_ssid`, `wifi_pass`, `proxy_url`, `session_max_messages` (1–128), `tg_group_activation` (`"mention"` or `"always"`), optional `locale` (`"zh"` or `"en"`).
- **Validation**: This segment only—wifi field length ≤ 64; `proxy_url` empty or like `http://host:port`; `session_max_messages`, `tg_group_activation` as above.
- **Response**: Success 200 `{"ok": true}`; validation failure 400.
- **Note**: WiFi changes take effect after reboot.

### GET /api/config/hardware

- **Purpose**: Get current hardware device config segment (`config/hardware.json` content).
- **Response**: 200, JSON is `HardwareSegment`: `{ "hardware_devices": [...] }`. Returns `{ "hardware_devices": [] }` when the file does not exist.
- **Note**: GET returns the raw file content. If validation failed at boot, the runtime uses an empty device list and `load_errors` will include `hardware_validation_failed`.

### POST /api/config/hardware

- **Purpose**: Write the hardware device config segment to SPIFFS (`config/hardware.json`); request body is the full segment; backend validates and writes. Takes effect after reboot. If validation fails when loading after reboot, `load_errors` will include `hardware_validation_failed`; full validation rules in [Hardware device config & LLM-driven control](hardware-device-config.md).
- **Request**: `Content-Type: application/json`, Body is `HardwareSegment`:
  ```json
  {
    "hardware_devices": [
      {
        "id": "onboard_led",
        "device_type": "gpio_out",
        "pins": { "pin": 2 },
        "what": "Onboard LED indicator, toggleable",
        "how": "Pass value: 1=on, 0=off"
      }
    ]
  }
  ```
  Each `DeviceEntry` has `id`, `device_type`, `pins`, `what`, `how`, optional `options`.
- **Validation** (this segment only):
  - Total device count ≤ 8
  - `id` non-empty, ≤ 32 bytes, must be unique
  - `device_type` must be one of `gpio_out` / `gpio_in` / `pwm_out` / `adc_in` / `buzzer`
  - `what` ≤ 128 bytes, `how` ≤ 256 bytes
  - `pins` must have a `"pin"` key; pin value 1–48, must not be strapping pins (0, 3, 45, 46), must not conflict across devices
  - `adc_in` pin must be in ADC1 range (GPIO 1–10)
  - `pwm_out` device count ≤ 4; `options.frequency_hz` if present must be 1–40000
- **Response**: Success 200 `{"ok": true}`; validation failure 400.

### POST /api/config/wifi

- **Purpose**: Write only WiFi SSID/password to NVS for the “WiFi only” config flow.
- **Request**: `Content-Type: application/json`, Body `{"wifi_ssid":"...","wifi_pass":"..."}`; field length ≤ 64.
- **Response**: Success 200, `{"ok": true, "restart_required": true}`; validation failure 400.
- **Note**: After saving WiFi to NVS, device must restart to apply. Optional: send query `?restart=1` to trigger restart after save (same as `POST /api/restart`).

### GET /api/soul

- **Purpose**: Get current SOUL (persona) content for the external config UI to display or edit.
- **Auth**: Pairing code required (see “Pairing”).
- **Response**: 200, `Content-Type: text/plain`, Body is SOUL file content (UTF-8); read failure 500, `{"error":"..."}`.

### POST /api/soul

- **Purpose**: Submit SOUL content and write to SPIFFS (config/SOUL.md).
- **Auth**: Pairing code required.
- **Request**: Body is plain text or JSON `{"content": "..."}`; length ≤ 32KB (MAX_SOUL_USER_LEN).
- **Response**: Success 200, `{"ok": true}`; too long or invalid UTF-8 400; write failure 500.

### GET /api/user

- **Purpose**: Get current USER (user info) config content.
- **Auth**: Pairing code required.
- **Response**: 200, `Content-Type: text/plain`, Body is USER file content; read failure 500.

### POST /api/user

- **Purpose**: Submit USER content and write to SPIFFS (config/USER.md).
- **Auth**: Pairing code required.
- **Request**: Same as POST /api/soul (plain text or `{"content":"..."}`, ≤ 32KB).
- **Response**: Same as POST /api/soul.

### GET /api/sessions

- **Purpose**: Get list of all current session chat_ids (read-only) for the config UI.
- **Auth**: Pairing code required.
- **Response**: 200, JSON array `["chat_id1", "chat_id2", ...]`; failure 500, `{"error":"..."}`.

### GET /api/memory/status

- **Purpose**: Get byte counts for MEMORY, SOUL, USER (read-only).
- **Auth**: Pairing code required.
- **Response**: 200, JSON `{"memory_len": number, "soul_len": number, "user_len": number}`.

## Skills

### GET /api/skills

- **Purpose**: List skills or get a single skill. Without query, returns list and order; with `?name=xxx` returns that skill’s plain text (for edit).
- **Auth**: Pairing code required.
- **Response (no name)**: 200, JSON `{"skills": [{"name": "x", "enabled": true}, ...], "order": ["a", "b"]}`.
- **Response (name=xxx)**: 200, `Content-Type: text/plain`, Body is skill content; not found 404.

### POST /api/skills

- **Purpose**: Update enabled state, write content, or update order. Body shape determines behavior.
- **Auth**: Pairing code required.
- **Request**: `Content-Type: application/json`.
  - Enable only: `{"name": "x", "enabled": true|false}`.
  - Write/overwrite skill: `{"name": "x", "content": "..."}`; content length ≤ 32KB.
  - Order only: `{"order": ["a", "b", "c"]}`.
- **Response**: Success 200, `{"ok": true}`; failure 400/500.

### DELETE /api/skills?name=xxx

- **Purpose**: Delete the given skill file. Query must include `name`.
- **Auth**: Pairing code required.
- **Response**: Success 200, `{"ok": true}`; invalid name 400; file not found 404.

### POST /api/skills/import

- **Purpose**: Fetch content from URL and save as a new skill.
- **Auth**: Pairing code required.
- **Request**: `Content-Type: application/json`, Body `{"url": "https://...", "name": "xxx"}`. url must be http(s); name must be valid (no `..`, `/`, `\`).
- **Response**: Success 200, `{"ok": true}`; url fetch failure 502/500; body not UTF-8 or too long 400.

### POST /api/webhook

- **Purpose**: External HTTP POST to inject one inbound message; body is used as content and pushed to the inbound queue for the agent.
- **Config**: Requires `webhook_enabled: true` and non-empty `webhook_token` in config; otherwise 403.
- **Auth**: Request must include the same token as config: Header `X-Webhook-Token` or query `token`; mismatch returns 401.
- **Request**: Body is arbitrary UTF-8 text as inbound message content; max 4KB.
- **Response**:
  - Success: 200, `{"ok": true}`.
  - Webhook disabled or token empty: 403, `{"error": "webhook disabled"}`.
  - Token mismatch: 401, `{"error": "invalid token"}`.
  - Body not UTF-8 or too long: 400/413.
  - Inbound queue full: 503, `{"error": "queue full"}`.

## Health and ops

### GET /api/health

- **Purpose**: Structured health info, same as CLI `health`.
- **Response**: 200, JSON example:
  ```json
  {
    "wifi": "connected",
    "inbound_depth": 0,
    "outbound_depth": 0,
    "last_error": "none"
  }
  ```
  - `wifi`: `"connected"` | `"disconnected"`.
  - `inbound_depth` / `outbound_depth`: Queue depths (numbers).
  - `last_error`: Last error summary (stage/message only, no secrets); or `"none"`.

### GET /api/diagnose

- **Purpose**: Device self-check (Doctor-style), returns a list of structured results for the config UI “Device status”.
- **Response**: 200, JSON array; each item has `severity`, `category`, `message`:
  ```json
  [
    { "severity": "ok", "category": "storage", "message": "storage readable" },
    { "severity": "ok", "category": "storage", "message": "spiffs total=... used=... free=..." },
    { "severity": "ok", "category": "config", "message": "nvs accessible" },
    { "severity": "warn", "category": "config", "message": "wifi disconnected" },
    { "severity": "ok", "category": "channel", "message": "inbound_depth=0 outbound_depth=0" },
    { "severity": "warn", "category": "channel", "message": "last_error: ..." }
  ]
  ```
  - `severity`: `"ok"` | `"warn"` | `"error"`.
  - `category`: `"storage"` (readable, SPIFFS) | `"channel"` (queue depth, last_error) | `"config"` (NVS, WiFi).
  - `message`: Human-readable; `last_error` summary truncated to 200 chars.

### POST /api/restart

- **Purpose**: Trigger device restart so new config takes effect.
- **Response**: Returns 200, `{"ok": true}`, then device restarts within ~100–500ms.
- **Throttle**: Only one successful restart allowed within 60 seconds.

### GET /api/ota/check

- **Purpose**: Check for available firmware update for current board and channel (uses build-time `OTA_MANIFEST_URL` and manifest). Only present when the `ota` feature is enabled.
- **Request**: GET, optional query `channel` (default `stable`). Requires activated device (pairing code set).
- **Response**: 200 JSON. Fields: `current_version` (current firmware), `latest_version` (latest for channel, if any), `update_available` (whether an update exists), `url` (download URL when update available), optional `release_notes`, optional `error` (human message, e.g. channel not configured or fetch failed). If manifest is missing or fetch/parse fails, still 200 with `update_available: false` and optional `error`.

### POST /api/ota

- **Purpose**: Fetch firmware from the given URL and perform OTA update; on success the device restarts. Only present when the firmware is built with the `ota` feature (GET / endpoints will include `"POST /api/ota"`).
- **Request**: `Content-Type: application/json`, Body `{"url": "https://..."}`; url must be non-empty and start with `http://` or `https://`.
- **Response**: Success 200, `{"ok": true}`, then device runs OTA and restarts; invalid or missing url 400, `{"error": "invalid url"}`; OTA download, verify, or write failure 500, `{"error": "human-readable message"}` (e.g. “Network or download failed, check network and retry”, “Firmware verification failed, try another source”, “Write failed, do not power off and retry”). Response includes CORS headers.
- **Note**: On failure the current running partition is not overwritten; device keeps running; caller can retry or use another URL.

**OTA channel manifest format** (produced by CI/Release and served at `OTA_MANIFEST_URL`): JSON root has `boards`; keys are board IDs (`esp32-s3-8mb`, `esp32-s3-16mb`, `esp32-s3-32mb`), values are channel objects; each channel (e.g. `stable`) has `version`, `url` (required), optional `release_notes`. Example: `{"boards":{"esp32-s3-16mb":{"stable":{"version":"0.2.0","url":"https://...","release_notes":"..."}}}}`. Board and manifest URL are set at build time via `BOARD`, `OTA_MANIFEST_URL`.

### POST /api/config_reset

- **Purpose**: Factory reset (clear NVS config area and remove SPIFFS `config/llm.json`, `config/channels.json`, `config/hardware.json`, `config/skills_meta.json`); same as CLI `config_reset yes`.
- **Response**: Success 200, `{"ok": true}`; failure 500, `{"error": "reset failed"}`.
- **Note**: After calling, user should restart; after restart `AppConfig::load()` uses only env; NVS keeps the 6 small keys (wifi, proxy, session, tg_group, locale, etc.); the rest is on SPIFFS.

## How to get the device IP

- **Recommended**: Use **http://beetle.local/** first (usually works when connected to the device hotspot or same LAN; matches firmware mDNS).
- When connected to hotspot **Beetle** and beetle.local is unavailable: use **http://192.168.4.1** (firmware SoftAP fixed address).
- When on STA and device is on the same LAN and beetle.local is unavailable: use the IP assigned by the router to the device.

## Config UI ownership

The config UI is maintained in a separate repo or as doc examples; this firmware repo does not ship HTML/JS/CSS static assets.
