# Configuration and usage

**English** | [中文](../zh-cn/configuration.md)

This doc is for users: how to provision the device, access the config page, and common config keys.

---

## Accessing the device

### Unprovisioned (first use)

1. Device powers on and opens a hotspot; SSID is **Beetle** (no password).
2. Connect your phone or PC to that hotspot.
3. In a browser open **http://192.168.4.1** (port 80; no need to type the port).

Only the device is on that hotspot; 192.168.4.1 is the device address and does not conflict with your home router.

### After WiFi is set

Once the device is connected to your router, its IP is assigned by DHCP. As long as your phone/PC and the device are on the same LAN, use:

- **http://beetle.local** (recommended; mDNS; works regardless of subnet)

No need to remember or look up the device IP.

---

## Pairing code

- On first access, set a **6-digit pairing code** on the config page.
- The pairing code protects write operations (save config, restart, OTA, etc.); secrets are written to NVS via the config page only—not logged or written to SPIFFS.
- If you forget the code, you can clear it via factory reset (requires access to the config page to run the reset action).

---

## Config page features

The config page provides:

- Set or change the pairing code
- WiFi scan and connection settings
- Channel credentials and toggles (Telegram, Feishu, DingTalk, WeCom, QQ Channel, Webhook)
- LLM config (API key, model, compatible URL, etc.)
- Proxy, search keys, etc.
- System info, restart, OTA (if enabled in firmware), factory reset

All write operations must include the correct pairing code in the request (the config page sends it for you).

---

## Common config keys

Same as the README table, for quick reference:

| Category | Keys | Description |
|----------|------|-------------|
| WiFi | `WIFI_SSID`, `WIFI_PASS` | Router SSID and password |
| Telegram | `TG_TOKEN`, `TG_ALLOWED_CHAT_IDS` | Bot token; allowed chat IDs, comma-separated; empty = reject |
| Feishu | `FEISHU_APP_ID`, `FEISHU_APP_SECRET`, `FEISHU_ALLOWED_CHAT_IDS` | App credentials and allowed chats |
| DingTalk | `DINGTALK_WEBHOOK_URL` | DingTalk bot webhook |
| WeCom | `WECOM_CORP_ID`, `WECOM_CORP_SECRET`, `WECOM_AGENT_ID`, `WECOM_DEFAULT_TOUSER` | WeCom app and default recipient |
| QQ Channel | `QQ_CHANNEL_APP_ID`, `QQ_CHANNEL_SECRET` | QQ Channel bot credentials |
| LLM | `API_KEY`, `MODEL`, `MODEL_PROVIDER`, `API_URL` | e.g. model `claude-opus-4-5`; provider: `anthropic` / `openai` / `openai_compatible`; compatible API base URL (e.g. Ollama) |
| Proxy | `PROXY_URL` | e.g. `http://host:8080` |
| Search | `SEARCH_KEY`, `TAVILY_KEY` | Search and Tavily API keys |

Build-time env vars `BEETLE_*` can prefill; at runtime the config page (NVS) wins if a key exists. On startup, enabled channels are validated for credentials and length (`validate_for_channels`); failures are logged as warnings and do not block boot.

---

## Health and observability

- **GET /api/health**: Returns WiFi status, inbound/outbound queue depth, recent error summary, and a **metrics** snapshot (messages in/out, LLM/tool calls and errors, WDT feed, per-stage error counts including session write failures `err_session`, etc.); no sensitive data, for ops and baseline comparison.
- Serial heartbeat logs a metrics baseline every 30 seconds (`msg_in`, `llm_calls`, `err_*`, etc.) for long-run comparison.
