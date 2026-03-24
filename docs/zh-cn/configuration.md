# 配置与使用

[English](../en-us/configuration.md) | **中文**

本文档面向**最终用户**，说明如何访问设备、配网、使用配置页（含在线版）、设置配对码，以及常用配置键与健康接口的用途。配置 API 的完整契约见 [配置 API 契约](config-api.md)。

---

## 访问设备

**推荐**：连接设备热点时在浏览器打开 **http://192.168.1.4/**；同局域网时使用路由器分配给设备的 IP。

### 未配网（首次使用）

1. 设备上电后开启热点，SSID 为 **Beetle**（无密码）
2. 手机或电脑连接该热点
3. 浏览器打开 **http://192.168.1.4**（端口 80，无需输入端口号）

此时仅设备自身在该热点下；固件 SoftAP 地址为 192.168.1.4，与家中路由器不冲突。

### 已连 WiFi（配网完成后）

设备连接家中路由器后，只要手机/电脑与设备在同一局域网，使用路由器分配给设备的 IP 访问。

---

## 配对码

- 首次访问需在配置页设置 **6 位配对码**
- 配对码用于保护写操作（保存配置、重启、OTA 等）；密钥仅经配置页写入 NVS，不打印、不写 SPIFFS
- 忘记配对码可通过恢复出厂清除（需能访问配置页并执行恢复操作）

---

## 配置页功能

配置页（与设备上提供的界面一致）有两种打开方式：

1. **从设备打开**：连接设备热点或与设备同网后，在浏览器访问 **http://192.168.1.4**（连接设备热点时）或路由器分配的设备 IP（同局域网时），由设备提供或跳转到配置页。
2. **在线打开**：访问 **https://ai-orangeoracle.github.io/beetle/**（若仓库配置了自定义域名则以该域名为准）。在线版仍需要已烧录固件的设备且浏览器与设备在同一网络；打开后需在页面中填写设备地址（**http://192.168.1.4** 或路由器分配的设备 IP），才能读写配置。

配置页提供：

- 配对码设置与修改
- WiFi 扫描与连接配置
- 各通道（Telegram、飞书、钉钉、企微、QQ 频道、Webhook）密钥与开关
- LLM 配置（API Key、模型、兼容 URL 等）
- 代理、搜索 Key 等
- 系统信息、重启、OTA（若固件启用）、恢复出厂等

所有写操作需在请求中携带正确配对码（配置页会代为携带）。

---

## 常用配置键

与 README 中表格一致，便于查阅：

| 类别 | 键 | 说明 |
|------|-----|------|
| WiFi | `WIFI_SSID`、`WIFI_PASS` | 路由器 SSID 与密码 |
| Telegram | `TG_TOKEN`、`TG_ALLOWED_CHAT_IDS` | Bot Token；允许的 Chat ID，逗号分隔，空则拒绝 |
| 飞书 | `FEISHU_APP_ID`、`FEISHU_APP_SECRET`、`FEISHU_ALLOWED_CHAT_IDS` | 应用凭证与允许的会话 |
| 钉钉 | `DINGTALK_WEBHOOK_URL` | 钉钉机器人 Webhook |
| 企微 | `WECOM_CORP_ID`、`WECOM_CORP_SECRET`、`WECOM_AGENT_ID`、`WECOM_DEFAULT_TOUSER` | 企业微信应用与默认接收人 |
| QQ 频道 | `QQ_CHANNEL_APP_ID`、`QQ_CHANNEL_SECRET` | QQ 频道机器人凭证 |
| LLM | `API_KEY`、`MODEL`、`MODEL_PROVIDER`、`API_URL` | 默认模型如 `claude-opus-4-5`；Provider：`anthropic`/`openai`/`openai_compatible`；兼容接口可填 base URL（如 Ollama） |
| 代理 | `PROXY_URL` | 如 `http://host:8080` |
| 搜索 | `SEARCH_KEY`、`TAVILY_KEY` | 搜索与 Tavily API Key |

编译时可通过环境变量 `BEETLE_*` 预填；运行时以配置页（NVS）为准，存在则覆盖编译时值。启动时会对当前启用的通道做凭证与长度校验（`validate_for_channels`），不符合时打警告日志，不阻塞启动。

---

## 健康与可观测

- **GET /api/health**：无需配对码，返回 WiFi 状态、入站/出站队列深度、最近错误摘要及 **metrics** 快照（消息进/出、LLM/tool 调用与错误、WDT feed、按 stage 错误计数等，无敏感信息）。调用方式：在浏览器或脚本中请求 **http://192.168.1.4/api/health**（连接设备热点时）或设备在局域网中的 IP。便于运维巡检与优化前后对比。
- **串口**：heartbeat 每 30 秒打一条 metrics 基线（msg_in、llm_calls、err_* 等），可用于长期运行对比。
