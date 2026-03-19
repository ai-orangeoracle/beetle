# 配置 API 契约 / Config API Contract

[English](../en-us/config-api.md) | **中文**

本文档面向**对接设备 HTTP API 的开发者**（如自建配置页、脚本或第三方集成）。设备固件仅提供 HTTP API，**不**内嵌配置页；配置页由外置前端（如本仓库 `configure-ui` 或 GitHub Pages 部署）实现。用户连接设备热点或与设备同网后，在配置页中填写**设备地址**（连接设备热点时填 **http://192.168.4.1**，同网时填路由器分配的 IP）即可调用下述接口。

## 网络与访问

- **SoftAP**：设备上电后开启热点，SSID 固定为 **Beetle**（无密码）。连接该热点后使用 **http://192.168.4.1**（与固件一致）。
- **STA**：若已配置 WiFi 并连接用户路由器，同一 HTTP 服务在 STA 网段也可访问，使用路由器分配的 LAN 地址。
- **CORS**：所有 `/api/*` 及 `GET /` 的响应应带 `Access-Control-Allow-Origin: *`；OPTIONS 预检对下列路径返回 200，并带 `Access-Control-Allow-Methods: GET, POST, DELETE, OPTIONS`、`Access-Control-Allow-Headers: Content-Type, X-Pairing-Code`，以便外置配置页跨域调用。

## 配对码与鉴权

- **未激活**：NVS 中无有效 6 位配对码。仅 **GET /config**、**GET /pairing**、**GET /api/pairing_code**、**POST /api/pairing_code**（仅用于首次设置）及所有 **OPTIONS** 可访问，其余返回 401 `{"error":"请先设置配对码"}`。前端应引导用户访问 `/pairing` 设置配对码。
- **已激活**：用户已通过 **POST /api/pairing_code** 设置过 6 位码。**GET 类**接口（GET /、GET /api/config、health、diagnose、sessions、memory/status、skills、soul、user）**不需**带配对码。**写操作**（POST /api/config/wifi、/api/config/llm、/api/config/channels、/api/config/system、/api/config/hardware、/api/restart、/api/config_reset、/api/soul、/api/user、/api/skills、/api/skills/import、/api/webhook、/api/ota，DELETE /api/skills）必须在请求中携带正确码，否则 401 `{"error":"配对码错误"}`。
- **携带方式**：Query 参数 `?code=<6位码>` 或 Header `X-Pairing-Code: <6位码>`。
- **恢复出厂**：**POST /api/config_reset** 须带正确配对码；成功后清除配置并清除配对码，设备回到未激活状态。

## 根路径

### GET /

- **用途**：探测设备在线与 API 列表。
- **响应**：200，`Content-Type: application/json`（若实现支持）。
- **Body 示例**：
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
  - 启用 feature `ota` 时，`endpoints` 中会额外包含 `"POST /api/ota"`。

### GET /api/pairing_code

- **用途**：查询是否已设置配对码（不返回明文码）。
- **响应**：200，`{"code_set": true}` 或 `{"code_set": false}`。
- **说明**：本接口为白名单，不需带配对码。

### POST /api/pairing_code

- **用途**：首次设置 6 位配对码（仅未激活时可调用）。
- **请求**：`Content-Type: application/json`，Body `{"code": "123456"}`（6 位数字）。
- **响应**：成功 200 `{"ok": true}`；已设置过则 400；格式错误 400。
- **说明**：本接口为白名单，不需带配对码。

### GET /pairing

- **用途**：配对码设置页 HTML（未激活时由前端重定向至此）。
- **响应**：200，`Content-Type: text/html; charset=utf-8`。

## 配置读写

**存储策略**：NVS 仅存 6 个小键（WiFi SSID/密码、代理、会话条数、群组触发、界面语言）；LLM 多源与通道配置存 SPIFFS（`config/llm.json`、`config/channels.json`），硬件设备配置存 `config/hardware.json`，技能启用/顺序存 `config/skills_meta.json`。GET /api/config 合并 NVS 与 SPIFFS 后返回完整配置。

### GET /api/wifi/scan

- **用途**：由设备扫描周边 WiFi，返回 SSID 列表供配置页下拉选择。
- **鉴权**：无需配对码。
- **响应**：200，JSON 数组 `[{ "ssid": "MyWiFi", "rssi": -50 }, ...]`，按信号强度（rssi）降序。扫描不可用（非 ESP 或 WiFi 未就绪）时 503。

### GET /api/config

- **用途**：获取当前完整配置（含各字段真实值，含密钥类字段）。
- **响应**：200，JSON 为 `AppConfig` 序列化，各字段为实际存储值。
- **多 LLM 源**：`llm_sources` 为数组，每项含 `provider`、`api_key`、`model`、`api_url`、`stream`（可选 bool，默认 false，true 时使用 SSE 流式读取）、`max_tokens`（可选 u32，null 时各客户端使用内置默认值 1024）；空时 load 从旧字段构造单源。`llm_router_source_index`、`llm_worker_source_index` 为可选 0～255，两者均设且 < `llm_sources.len()` 时启用路由模式。

### POST /api/config/llm

- **用途**：仅写入 LLM 段（多源与路由/worker 下标）到 SPIFFS（`config/llm.json`）；请求体为 segment 全量，后端按 body 校验并写入。
- **请求**：`Content-Type: application/json`，Body 为 `{ "llm_sources": [...], "llm_router_source_index": 0, "llm_worker_source_index": 0 }`（后两者可选）。`llm_sources` 非空；每项 `api_key` 必填。每项可含：
  - `provider`（必填，如 `"anthropic"`、`"openai"`、`"openai_compatible"`）
  - `api_key`（必填）
  - `model`（必填）
  - `api_url`（必填，openai/openai_compatible 下可为空表示默认端点）
  - `stream`（可选 bool，默认 false；true 时使用 SSE 流式读取，降低峰值内存）
  - `max_tokens`（可选 u32，默认 null；null 时各客户端使用内置默认值 1024）
- **校验**：仅本段——`llm_sources` 非空，各字段长度（provider/api_key/model ≤ 64，api_url ≤ 256）；router/worker 下标须 < `llm_sources.len()`。
- **响应**：成功 200 `{"ok": true}`；校验失败 400。

### POST /api/config/channels

- **用途**：仅写入通道段（Telegram、飞书、钉钉、企微、QQ 频道、Webhook）到 SPIFFS（`config/channels.json`）；请求体为 segment 全量，后端按 body 校验并写入。
- **请求**：`Content-Type: application/json`，Body 含 `tg_token`、`tg_allowed_chat_ids`、`feishu_app_id`、`feishu_app_secret`、`feishu_allowed_chat_ids`、`dingtalk_webhook_url`、`wecom_corp_id`、`wecom_corp_secret`、`wecom_agent_id`、`wecom_default_touser`、`qq_channel_app_id`、`qq_channel_secret`、`webhook_enabled`、`webhook_token`。
- **校验**：仅本段字段长度（tg/feishu/wecom/qq 等 ≤ 64，dingtalk_webhook_url ≤ 512，wecom_default_touser ≤ 128）。
- **响应**：成功 200 `{"ok": true}`；校验失败 400。

### POST /api/config/system

- **用途**：仅写入系统段（WiFi、代理、会话条数、群组触发、界面语言）；请求体为 segment 全量，后端按 body 校验并写入。
- **请求**：`Content-Type: application/json`，Body 含 `wifi_ssid`、`wifi_pass`、`proxy_url`、`session_max_messages`（1～128）、`tg_group_activation`（`"mention"` 或 `"always"`）、`locale`（可选，`"zh"` 或 `"en"`）。
- **校验**：仅本段——wifi 字段长度 ≤ 64；`proxy_url` 为空或形如 `http://host:port`；`session_max_messages`、`tg_group_activation` 同上。
- **响应**：成功 200 `{"ok": true}`；校验失败 400。
- **说明**：WiFi 写入后需重启生效。

### GET /api/config/hardware

- **用途**：获取当前硬件设备配置段（`config/hardware.json` 内容）。
- **响应**：200，JSON 为 `HardwareSegment`：`{ "hardware_devices": [...] }`。文件不存在时返回 `{ "hardware_devices": [] }`。
- **说明**：GET 返回文件原始内容。若启动时该校验未通过，运行时使用空设备列表，且 `load_errors` 会包含 `hardware_validation_failed`。

### POST /api/config/hardware

- **用途**：写入硬件设备配置段到 SPIFFS（`config/hardware.json`）；请求体为 segment 全量，后端校验并写入。重启后生效。重启后加载时若校验失败，`load_errors` 将含 `hardware_validation_failed`；完整校验规则见 [硬件设备配置与 LLM 驱动设计](hardware-device-config.md)。
- **请求**：`Content-Type: application/json`，Body 为 `HardwareSegment`：
  ```json
  {
    "hardware_devices": [
      {
        "id": "板载LED",
        "device_type": "gpio_out",
        "pins": { "pin": 2 },
        "what": "板载指示灯，可开关",
        "how": "传 value：1=亮，0=灭"
      }
    ]
  }
  ```
  每项 `DeviceEntry` 含 `id`、`device_type`、`pins`、`what`、`how`、可选 `options`。
- **校验**（仅本段）：
  - 设备总数 ≤ 8
  - `id` 非空且 ≤ 32 字节，不得重复
  - `device_type` 须为 `gpio_out` / `gpio_in` / `pwm_out` / `adc_in` / `buzzer` 之一
  - `what` ≤ 128 字节，`how` ≤ 256 字节
  - `pins` 须含 `"pin"` 键；引脚值 1–48，不得为 strapping 引脚（0, 3, 45, 46），不得跨设备冲突
  - `adc_in` 引脚须在 ADC1 范围（GPIO 1–10）
  - `pwm_out` 设备总数 ≤ 4；`options.frequency_hz` 若存在须在 1–40000
- **响应**：成功 200 `{"ok": true}`；校验失败 400。

### POST /api/config/wifi

- **用途**：仅将 WiFi SSID/密码写入 NVS，供配置页单独配网场景。
- **请求**：`Content-Type: application/json`，Body `{"wifi_ssid":"...","wifi_pass":"..."}`；字段长度 ≤ 64。
- **响应**：成功 200，`{"ok": true, "restart_required": true}`；校验失败 400。
- **说明**：WiFi 写入 NVS 后需重启设备生效。可选：请求时带 query `?restart=1`，保存成功后将自动重启（与 `POST /api/restart` 相同逻辑）。

### GET /api/soul

- **用途**：获取当前 SOUL（人格）配置内容，供外置配置页回显或编辑。
- **配对**：须携带配对码（见「首次配对」）。
- **响应**：200，`Content-Type: text/plain`，Body 为 SOUL 文件全文（UTF-8）；读失败 500，`{"error":"..."}`。

### POST /api/soul

- **用途**：提交 SOUL 内容并写入 SPIFFS（config/SOUL.md）。
- **配对**：须携带配对码。
- **请求**：Body 为纯文本或 JSON `{"content": "..."}`；长度 ≤ 32KB（MAX_SOUL_USER_LEN）。
- **响应**：成功 200，`{"ok": true}`；超长或非法 UTF-8 返回 400；写入失败 500。

### GET /api/user

- **用途**：获取当前 USER（用户信息）配置内容。
- **配对**：须携带配对码。
- **响应**：200，`Content-Type: text/plain`，Body 为 USER 文件全文；读失败 500。

### POST /api/user

- **用途**：提交 USER 内容并写入 SPIFFS（config/USER.md）。
- **配对**：须携带配对码。
- **请求**：同 POST /api/soul（纯文本或 `{"content":"..."}`，≤ 32KB）。
- **响应**：同 POST /api/soul。

### GET /api/sessions

- **用途**：获取当前所有会话的 chat_id 列表（只读），供外置配置页展示。
- **配对**：须携带配对码。
- **响应**：200，JSON 数组 `["chat_id1", "chat_id2", ...]`；失败 500，`{"error":"..."}`。

### GET /api/memory/status

- **用途**：获取 MEMORY、SOUL、USER 的字节数（只读）。
- **配对**：须携带配对码。
- **响应**：200，JSON `{"memory_len": number, "soul_len": number, "user_len": number}`。

## Skills

### GET /api/skills

- **用途**：列表或单条 skill。无 query 时返回列表与顺序；带 `?name=xxx` 时返回该 skill 的纯文本内容（供编辑回显）。
- **配对**：须携带配对码。
- **响应（无 name）**：200，JSON `{"skills": [{"name": "x", "enabled": true}, ...], "order": ["a", "b"]}`。
- **响应（name=xxx）**：200，`Content-Type: text/plain`，Body 为 skill 内容；不存在 404。

### POST /api/skills

- **用途**：更新启用状态、写入内容或更新顺序。Body 形状决定行为。
- **配对**：须携带配对码。
- **请求**：`Content-Type: application/json`。
  - 仅更新启用：`{"name": "x", "enabled": true|false}`。
  - 写入/覆盖 skill：`{"name": "x", "content": "..."}`；content 长度 ≤ 32KB。
  - 仅更新顺序：`{"order": ["a", "b", "c"]}`。
- **响应**：成功 200，`{"ok": true}`；失败 400/500。

### DELETE /api/skills?name=xxx

- **用途**：删除指定 skill 文件。query 必带 `name`。
- **配对**：须携带配对码。
- **响应**：成功 200，`{"ok": true}`；name 非法 400；文件不存在 404。

### POST /api/skills/import

- **用途**：从 URL 拉取内容并写入为新 skill。
- **配对**：须携带配对码。
- **请求**：`Content-Type: application/json`，Body `{"url": "https://...", "name": "xxx"}`。url 须为 http(s)；name 合法（无 `..`、`/`、`\`）。
- **响应**：成功 200，`{"ok": true}`；url 拉取失败 502/500；body 非 UTF-8 或超长 400。

### POST /api/webhook

- **用途**：外部 HTTP POST 触发一条入站消息，body 作为 content 推入入站队列，由 agent 处理。
- **配置**：需在配置中设置 `webhook_enabled: true` 且 `webhook_token` 非空；否则返回 403。
- **校验**：请求须携带与配置一致的 token：Header `X-Webhook-Token` 或 query 参数 `token`；校验失败返回 401。
- **请求**：Body 为任意 UTF-8 文本，作为入站消息 content；上限 4KB。
- **响应**：
  - 成功：200，`{"ok": true}`。
  - webhook 未启用或 token 为空：403，`{"error": "webhook disabled"}`。
  - token 不匹配：401，`{"error": "invalid token"}`。
  - Body 非 UTF-8 或超长：400/413。
  - 入站队列满：503，`{"error": "queue full"}`。

## 健康与运维

### GET /api/health

- **用途**：与 CLI `health` 一致的结构化健康信息。
- **响应**：200，JSON 示例：
  ```json
  {
    "wifi": "connected",
    "inbound_depth": 0,
    "outbound_depth": 0,
    "last_error": "none"
  }
  ```
  - `wifi`：`"connected"` | `"disconnected"`。
  - `inbound_depth` / `outbound_depth`：入站/出站队列深度（数字）。
  - `last_error`：最近一次错误摘要（仅 stage/message，无密钥）；无则为 `"none"`。

### GET /api/diagnose

- **用途**：设备自检（Doctor 式），返回结构化结果列表，供外置配置页「设备状态」展示。
- **响应**：200，JSON 数组，每项含 `severity`、`category`、`message`：
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
  - `severity`：`"ok"` | `"warn"` | `"error"`。
  - `category`：`"storage"`（存储可读、SPIFFS）| `"channel"`（队列深度、last_error）| `"config"`（NVS、WiFi）。
  - `message`：人类可读说明；`last_error` 摘要截断至 200 字符。

### POST /api/restart

- **用途**：触发设备重启，使新配置生效。
- **响应**：先返回 200，`{"ok": true}`，随后在约 100～500ms 内设备重启。
- **节流**：60 秒内仅允许一次有效重启。

### GET /api/ota/check

- **用途**：按当前板型与渠道查询是否有可用的固件更新（依赖编译期 `OTA_MANIFEST_URL` 与 manifest 清单）。仅当 feature `ota` 启用时存在。
- **请求**：GET，可选 query `channel`（默认 `stable`）。需已激活（配对码已设置）。
- **响应**：200 JSON。字段：`current_version`（当前固件版本）、`latest_version`（渠道最新版本，有渠道时）、`update_available`（是否有可升级版本）、`url`（有更新时的固件下载 URL）、`release_notes`（可选）、`error`（可选，人话提示，如渠道未配置或拉取失败）。manifest 未配置或拉取/解析失败时仍返回 200，`update_available: false`，可选带 `error` 人话。

### POST /api/ota

- **用途**：从指定 URL 拉取固件并执行 OTA 更新，成功后设备重启。仅当固件以 feature `ota` 编译时该接口存在（GET / 的 endpoints 中会包含 `"POST /api/ota"`）。
- **请求**：`Content-Type: application/json`，Body 为 `{"url": "https://..."}`；url 须非空且为 `http://` 或 `https://` 开头。
- **响应**：成功 200，`{"ok": true}`，随后设备执行 OTA 并在完成后重启；无效或缺失 url 返回 400，`{"error": "invalid url"}`；OTA 下载、校验或写入失败返回 500，`{"error": "可读人话"}`（如「网络或下载失败，请检查网络后重试」「固件校验失败，请更换固件来源」「写入失败，请勿断电并重试」）。响应带 CORS 头。
- **说明**：更新失败时不会写入当前运行分区，设备可继续使用；可重试或更换 URL。

**OTA 渠道 manifest 格式**（由 CI/Release 产出并上传至 `OTA_MANIFEST_URL`）：JSON 根含 `boards`，键为板型 ID（`esp32-s3-8mb`、`esp32-s3-16mb`、`esp32-s3-32mb`），值为渠道对象；各渠道（如 `stable`）含 `version`、`url`（必填）、可选 `release_notes`。例：`{"boards":{"esp32-s3-16mb":{"stable":{"version":"0.2.0","url":"https://...","release_notes":"..."}}}}`。设备构建时通过 `BOARD`、`OTA_MANIFEST_URL` 指定板型与清单地址。

### POST /api/config_reset

- **用途**：恢复出厂（清空 NVS 配置区并删除 SPIFFS 上的 `config/llm.json`、`config/channels.json`、`config/hardware.json`、`config/skills_meta.json`），与 CLI `config_reset yes` 等价。
- **响应**：成功 200，`{"ok": true}`；失败 500，`{"error": "reset failed"}`。
- **说明**：调用后建议用户重启设备，重启后 `AppConfig::load()` 仅来自环境变量；NVS 仅保留 6 个小键（wifi、proxy、session、tg_group、locale 等），其余配置存 SPIFFS。

## 如何获知板子 IP

- 连接设备热点 **Beetle** 时：使用 **http://192.168.4.1**（固件 SoftAP 固定地址）。
- 已连 STA 且与设备在同一 LAN 时：使用路由器分配给设备的 IP。

## 配置页归属

配置页由独立仓库或文档示例维护；本仓库固件不提供 HTML/JS/CSS 静态资源。
