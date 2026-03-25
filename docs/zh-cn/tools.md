# Agent 工具说明

[English](../en-us/tools.md) | **中文** | [文档索引](../README.md)

本文档面向**使用 Beetle 设备的用户**，说明设备上 AI Agent 在对话中可用的**工具**及其作用与限制。你不需要手动调用这些工具——Agent 会根据对话自动选用；若调用失败（如网络错误、参数不合法），Agent 会用自然语言说明结果。

**清单来源**：固件中 `build_default_registry`（[`src/tools/registry.rs`](../../src/tools/registry.rs)）注册顺序；条件注册的工具单独标注。

---

## 始终可用的工具

| 工具名 | 作用概要 | Agent 可能使用的场景 |
|--------|----------|----------------------|
| **get_time** | 返回当前 UTC 时间（日期、星期、时分秒）。 | 「几点了」「今天几号」等。 |
| **files** | 列出或**读取**设备存储（SPIFFS）中的文件；路径须在存储根下，禁止 `..`。 | 列出目录、读取配置/技能/笔记等（只读）。 |
| **web_search** | 按关键词联网搜索并返回摘要。 | 需要较新事实或「搜一下 …」。 |
| **analyze_image** | 根据图片 URL 用视觉模型分析内容。 | 提供图片链接并询问画面内容。 |
| **remind_at** | 按时间（ISO8601 或 Unix 秒）与文案写入提醒；到点在同通道推送消息。 | 「X 点提醒我 …」。 |
| **remind_list** | 列出当前会话未到点的提醒（可限条数）。 | 「我设了哪些提醒」。 |
| **update_session_summary** | 生成本轮对话短摘要供后续参考。 | 长对话断点处由 Agent 自行调用。 |
| **board_info** | 芯片型号、堆/PSRAM、运行时间、资源压力、WiFi 状态、SPIFFS 用量等。 | 「设备状态」「内存」「存储」。 |
| **kv_store** | 持久键值：`get`/`set`/`delete`/`list_keys`；key 字符集与长度、value 与条数有上限。 | 「记住 …」「之前存了什么 key」。 |
| **memory_manage** | 长期记忆、SOUL/USER、每日笔记等：`get_memory`/`set_memory`、`get_soul`/`set_soul`、`get_user`/`set_user`、日记读写等。 | 管理记忆与笔记类内容（与配置页写入的 SOUL/USER 文件域不同，以工具语义为准）。 |
| **http_request** | 统一 HTTP：**GET/POST/PUT/DELETE/PATCH**；可带头与 body。**禁止访问私网地址**（SSRF 防护）。 | 拉取公开 API、Webhook、自动化回调等。 |
| **session_manage** | 会话：`list`/`info`/`clear`/`delete`。 | 查看或清理某会话历史。 |
| **file_write** | 向存储根下**写入**文件（覆写/追加）；关键路径（如 `config/llm.json`、`config/SOUL.md` 等）受保护不可写。 | 用户笔记、非受控路径下的写入需求。 |
| **system_control** | `restart`（须 `confirm=true`）、`spiffs_usage`。 | 重启设备、查看 SPIFFS 用量（高风险操作会要求确认）。 |
| **cron_manage** | 持久化定时任务 CRUD（cron 表达式 + 触发动作）；由设备 cron 循环调度。 | 管理周期性自动发消息类任务。 |
| **proxy_config** | 查看/设置/清除 HTTP 代理（NVS）；**重启后生效**。 | 运行时改代理（若策略允许）。 |
| **model_config** | 查看或更新 `config/llm.json` 中模型相关字段（**不回显 api_key**）；**重启后生效**。 | 切换模型/URL 等（若策略允许）。 |
| **network_scan** | `wifi_scan`、`wifi_status`、`connectivity_check`；扫描有**最小间隔**限制。 | WiFi 与简单连通性诊断。 |

---

## 条件注册（依赖配置）

| 工具名 | 何时注册 | 作用概要 |
|--------|----------|----------|
| **device_control** | `config/hardware.json` 解析成功且设备列表非空 | 按配置的 `device_id` 操作 GPIO/PWM/ADC/蜂鸣器等；详见 [硬件设备配置](hardware-device-config.md)。 |
| **sensor_watch** | 同上，且存在 `adc_in`/`gpio_in` 设备 | 传感器阈值告警：`add`/`list`/`remove`/`update`；与 cron 协同。 |
| **i2c_device** | 配置中存在 `i2c_bus` 与 `i2c_devices` | 按配置的 I2C 设备读写寄存器（具体能力由配置生成的 schema 描述）。 |

---

## 限制与说明

- **时间**：设备时间需 NTP/RTC 同步后才准；可用 **get_time** 自检。
- **files**：只读；路径不得越权；列表条数与单文件读取长度有上限（见实现常量）。
- **提醒**：存于设备，条数有上限；到点向**当前通道/会话**推送。
- **网络类工具**（web_search、analyze_image、http_request、network_scan 等）：在资源紧张时 orchestrator 可能限流或推迟调用。
- **http_request**：**内网/本机地址会被拒绝**，勿用于探测局域网。

如需通过 JSON 描述板载外设并由 Agent 语义操作硬件，见 [硬件设备配置](hardware-device-config.md) 与配置页「硬件」相关项。
