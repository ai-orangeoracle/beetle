# Agent tools

[中文](../zh-cn/tools.md) | **English** | [Doc index](../README.md)

This doc is for **users of a Beetle device**: it lists the **tools** the on-device AI Agent can use, what they do, and their limits. You do not call them yourself—the Agent picks tools from the conversation. Failures (network, bad args, etc.) are reported in natural language.

**Source of truth**: registration order in `build_default_registry` ([`src/tools/registry.rs`](../../src/tools/registry.rs)); conditional tools are called out separately.

---

## Always registered

| Tool | Summary | When the Agent might use it |
|------|---------|----------------------------|
| **get_time** | Current UTC time (date, weekday, time). | “What time is it?”, dates. |
| **files** | List or **read** files under storage root; no `..`. | List/read skills, notes, etc. (read-only). |
| **web_search** | Web search with a short summary. | Recent facts, “search for …”. |
| **analyze_image** | Vision model over an image URL. | Describe what’s in a linked image. |
| **remind_at** | Schedule a reminder (ISO8601 or Unix seconds + text); fires on the same channel. | “Remind me at …”. |
| **remind_list** | Upcoming reminders for the current chat (optional limit). | “What reminders did I set?”. |
| **update_session_summary** | Short summary of the chat for later context. | Used by the Agent at natural breaks. |
| **board_info** | Chip, heap/PSRAM, uptime, pressure, WiFi, SPIFFS, etc. | “Device status”, memory, storage. |
| **kv_store** | Persistent KV: `get`/`set`/`delete`/`list_keys`; keys/values/entry caps apply. | “Remember …”, “what keys are stored?”. |
| **memory_manage** | Long-term memory, soul/user text, daily notes: `get_memory`/`set_memory`, soul/user ops, daily note CRUD, etc. | Managing memory and notes (distinct from config-UI SOUL/USER flows; behavior follows tool ops). |
| **http_request** | HTTP **GET/POST/PUT/DELETE/PATCH** with optional headers/body. **Private/internal URLs are blocked** (SSRF). | Public APIs, webhooks, integrations. |
| **session_manage** | Sessions: `list`/`info`/`clear`/`delete`. | Inspect or clear session history. |
| **file_write** | **Write** under storage root (overwrite/append); **protected paths** (e.g. `config/llm.json`, `config/SOUL.md`) cannot be written. | User notes and other non-protected paths. |
| **system_control** | `restart` (needs `confirm=true`), `spiffs_usage`. | Restart, storage usage (dangerous ops need confirmation). |
| **cron_manage** | Persistent scheduled tasks (cron + action); evaluated by the device cron loop. | Recurring automated messages. |
| **proxy_config** | Get/set/clear HTTP proxy in NVS; **effective after reboot**. | Change proxy when allowed. |
| **model_config** | Read/update model-related fields in `config/llm.json` (**api_key not shown**); **effective after reboot**. | Switch model/URL when allowed. |
| **network_scan** | `wifi_scan`, `wifi_status`, `connectivity_check`; scans are **rate-limited**. | WiFi / basic connectivity checks. |

---

## Conditional registration

| Tool | When registered | Summary |
|------|-----------------|---------|
| **device_control** | `hardware.json` loaded with a non-empty device list | GPIO/PWM/ADC/buzzer by configured `device_id`; see [Hardware device config](hardware-device-config.md). |
| **sensor_watch** | Same, and sensor devices (`adc_in`/`gpio_in`) exist | Threshold watches: `add`/`list`/`remove`/`update`; tied to the cron loop. |
| **i2c_device** | `i2c_bus` + `i2c_devices` present in config | I2C register read/write per configured devices (schema from config). |

---

## Limits and behavior

- **Time**: Accurate after NTP/RTC sync; use **get_time** to verify.
- **files**: Read-only; paths must stay under the storage root; list/read limits apply (see code constants).
- **Reminders**: Stored on device, capped count; delivered on the **same channel/session**.
- **Network tools**: May be deferred under resource pressure (orchestrator gating).
- **http_request**: **RFC1918 / local targets are rejected**—do not use for LAN probing.

For JSON-driven onboard hardware and the `device_control` tool, see [Hardware device config](hardware-device-config.md) and the hardware section of the config UI.
