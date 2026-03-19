# 配置与使用

[English](../en-us/configuration.md) | **中文**

本文档面向用户，说明如何配网、访问配置页以及常用配置项。

---

## 访问设备

### 未配网（首次使用）

1. 设备上电后开启热点，SSID 为 **Beetle**（无密码）
2. 手机或电脑连接该热点
3. 浏览器打开 **http://192.168.4.1**（端口 80，无需输入端口号）

此时仅设备自身在该热点下，192.168.4.1 为设备地址，不会与家中路由器冲突。

### 已连 WiFi（配网完成后）

设备连接家中路由器后，IP 由路由器 DHCP 分配。只要手机/电脑与设备在同一局域网，可使用：

- **http://beetle.local**（推荐，mDNS，与网段无关）

无需记忆或查询设备 IP。

---

## 配对码

- 首次访问需在配置页设置 **6 位配对码**
- 配对码用于保护写操作（保存配置、重启、OTA 等）；密钥仅经配置页写入 NVS，不打印、不写 SPIFFS
- 忘记配对码可通过恢复出厂清除（需能访问配置页并执行恢复操作）

---

## 配置页功能

配置页的在线版本（与设备上提供的界面一致）可在此访问，便于在未连接设备时预览或使用：

- **https://ai-orangeoracle.github.io/beetle/**（若仓库配置了自定义域名，则以该域名为准）

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

- **GET /api/health**：返回 WiFi 状态、入站/出站队列深度、最近错误摘要及 **metrics** 快照（消息进/出、LLM/tool 调用与错误、WDT feed、按 stage 错误计数含 session 写入失败 err_session 等），无敏感信息，便于运维与基线对比。
- 串口日志中 heartbeat 每 30 秒打一条 metrics 基线（msg_in、llm_calls、err_* 等），可用于长期运行对比。
