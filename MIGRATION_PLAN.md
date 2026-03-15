# 甲虫（beetle）Rust 重构与迁移计划

> **项目名称**：甲虫（中文）/ beetle（英文）；Rust crate 名：`beetle`。  
> 本文档为 Rust 版本的实施计划：按模块、分阶段、可验收。**目标为生产/消费级、扩展性最强、可长期维护的架构，并超越 C 版能力。**  
> C 版仅作功能参考：`../c/docs/ARCHITECTURE.md`。

---

## 一、目标与原则

### 1.1 目标

- **生产/消费级**：可直接用于真实环境长期运行——可观测（日志/指标/健康检查）、可恢复（重试/退避/降级）、安全（密钥不落盘不打印、配置校验、输入约束）、资源可控（内存/栈/队列有上界、超时全覆盖）。
- **功能对等并超越 C 版**：在 C 版能力基础上，增加结构化可观测、弹性策略、配置校验与演进、运行时可扩展点（见 1.3）。
- **Rust 惯用 + 最强扩展性**：核心仅依赖 trait（`LlmClient`、`Tool`、`MessageSink`、`MemoryStore`、`SessionStore`）；新增通道/工具/LLM 通过「实现 trait + 注册」完成，无需改核心循环；支持 feature 裁剪与可选运行时配置。
- **可维护**：单一错误类型、配置显式下传、关键路径可 host 单测；lib 对外 API 稳定、有语义化版本与 rustdoc；架构决策有记录（ADR）。

### 1.2 原则

- **逐步迁移**：按依赖顺序从底层到上层，每步可单独编译、运行或联调；每一步交付物满足本计划中的「生产级」要求再进入下一步。
- **单语计划**：本计划仅中文；代码与对外文档保持中英双语（注释、README、日志）。
- **不破坏 C 版**：Rust 工程独立于 `c/`，可并行维护；架构不复制 C 的目录与调用关系。

### 1.3 超越 C 版的能力（必须在本计划中落地）

| 维度 | C 版现状 | Rust 版目标 |
|------|----------|-------------|
| **可观测** | 分散 ESP_LOG | 结构化日志（带 TAG、可选 request_id/chat_id）；关键路径耗时与错误计数可被采集（为后续指标预留）；健康检查接口或 CLI 命令（WiFi、队列深度、最近错误）。 |
| **弹性** | 基本无重试/退避 | LLM/HTTP 调用：可配置重试次数与指数退避；通道轮询失败不崩溃、带退避；队列满时背压（拒绝或阻塞可配置）。 |
| **安全** | 编译时密钥 | 配置加载时校验必填项与格式；密钥与敏感字段永不打印、不写 SPIFFS 调试文件；工具/会话输入有长度与格式约束，防止溢出与注入。 |
| **资源** | 部分 PSRAM 使用 | 所有队列、缓冲区、会话条数有明确上界；单条消息/单次 LLM 响应有大小限制；栈与堆使用有文档与启动时自检（可选）。 |
| **配置** | 仅编译时 | 支持「编译时默认 + 可选运行时覆盖」（如 NVS/SPIFFS）；配置 schema 可演进（新字段有默认值、旧字段废弃有文档）。 |
| **扩展** | 改代码加通道/工具 | 通道/工具/LLM 均通过 trait + 注册；新增实现不修改 agent/bus 核心；可选：事件钩子（如 on_message_received）便于审计与扩展。 |
| **运维** | CLI + OTA | OTA 失败可回退分区；心跳/健康可被外部监控；关键命令（如 session_clear）有确认或审计日志。 |

---

## 二、目标架构（生产级 + 最强扩展性）

Rust 版按**类型与能力**划分，满足生产/消费级运行与长期扩展：依赖倒置、trait 抽象、配置与资源显式注入、消息传递为主；在此基础上增加可观测、弹性、安全与扩展钩子。

### 2.1 设计原则摘要

| 原则 | 做法 |
|------|------|
| **依赖倒置** | 核心逻辑只依赖 trait（`LlmClient`、`ToolRegistry`、`MessageSink`、`MemoryStore`、`SessionStore`），实现由 main 注入；换 LLM/加通道/加工具均不改 agent/bus。 |
| **单一错误类型** | 统一 `beetle::Error`（enum + thiserror），各层 `From` 收敛；所有公共 API 返回 `Result<T, Error>`；错误带上下文（如 status_code、stage）便于排查。 |
| **配置显式下传** | `AppConfig` 一次加载并校验（必填项、格式、长度上界）；通过参数下传；支持「编译时默认 + 可选 NVS/SPIFFS 覆盖」；密钥与敏感字段永不打印、不落盘。 |
| **通过通信共享** | 入站/出站 channel 传递 `PcMsg`，队列有固定容量与背压策略；尽量少用 `Arc<Mutex<T>>`；任务边界清晰，便于加超时与监控。 |
| **可观测** | 关键路径带结构化信息（TAG、chat_id、可选 request_id）；错误与重试可计数；预留「指标回调」或简单计数器（如消息入/出、LLM 调用次数）；健康状态可查询（WiFi、队列深度、最近错误）。 |
| **弹性** | LLM/HTTP 可配置重试与指数退避；通道轮询失败不 panic、带退避；队列满时行为明确（阻塞或返回错误）；单次请求/响应有超时与大小上限。 |
| **可测试性** | 业务逻辑与硬件/网络解耦，依赖 trait 注入；context、工具解析、消息转换可在 host 上单测；集成测试与 e2e 策略在阶段 10 明确。 |
| **扩展点** | 通道/工具/LLM 均为「实现 trait + 注册」；可选事件钩子（如 `on_message_received`、`on_tool_called`）供审计或扩展，不侵入核心。 |
| **可选功能** | 通道、CLI、OTA、cron、heartbeat 等用 **feature** 控制；lib 对外 API 稳定，遵循语义化版本。 |

### 2.2 模块依赖规则（禁止反向依赖）

- **核心域**（agent、bus、llm、tools、memory）不依赖 **platform** 和 **channels**；只依赖 trait 与类型定义。
- **channels** 仅依赖 bus（入站 Sender）、platform（HTTP/WS 等）、config；不依赖 agent、llm、tools。
- **platform** 仅依赖 config、error；不依赖业务逻辑。
- **main** 是唯一入口：完成 NVS/SPIFFS/配置加载/WiFi/mDNS 后调用 **run_app(platform, config, wifi_connected)**；run_app 为启动编排（存储与总线、自检、后台任务与通道、agent 循环与 flush），与 main 解耦便于维护。

**实现说明（审计后）**：通道出站统一使用 `channels::QueuedSink`（各通道仅 stage 不同）；`ChannelHttpClient` 位于 `channels/http_client.rs`；LLM 段/通道段写入 SPIFFS 的接口为 `save_llm_segment(writer, body)`、`save_channels_segment(writer, body)`（无 store 参数）；CLI 在 agent 循环前 spawn，与 agent 并行；metrics 含 session 写入失败（stage `session_append`）。

这样保证：换平台（如从 ESP 迁到其他 target）、加通道、换 LLM 均只需改 main 或对应实现，核心可复用、可单测。

### 2.3 Crate 与模块布局建议

```
rust/
├── Cargo.toml              # features: telegram, feishu, websocket, cli, ota, metrics(可选)
├── src/
│   ├── lib.rs              # 稳定对外 API：Error, PcMsg, 各 trait、核心类型；语义化版本
│   ├── main.rs             # 入口：NVS/SPIFFS/配置/校验/WiFi/mDNS，然后 run_app；run_app 为编排（存储、总线、自检、任务、agent、flush）
│   ├── config.rs           # AppConfig 加载、校验（必填、格式、长度）、可选运行时覆盖
│   ├── error.rs            # Error 枚举 + thiserror，带上下文
│   ├── bus.rs              # PcMsg + 入站/出站 channel，固定容量、背压策略
│   ├── agent/
│   │   ├── mod.rs
│   │   ├── context.rs      # 系统提示 + 会话聚合，纯逻辑可单测
│   │   └── loop.rs         # ReAct 循环，依赖 trait；含超时、重试、大小限制
│   ├── llm/
│   │   ├── mod.rs          # LlmClient trait
│   │   ├── types.rs        # 请求/响应 DTO（serde）
│   │   └── anthropic.rs    # 实现 + 重试/退避
│   ├── tools/
│   │   ├── mod.rs          # Tool trait + ToolRegistry，输入长度/格式约束
│   │   ├── web_search.rs
│   │   └── ...
│   ├── memory/
│   │   ├── mod.rs          # MemoryStore + SessionStore trait，条数/大小上界
│   │   └── ...
│   ├── channels/
│   │   ├── mod.rs          # MessageSink trait，注册与 dispatch 分发
│   │   ├── dispatch.rs     # 出站分发；单通道熔断（连续失败冷却）、重试间隔
│   │   ├── telegram.rs     # 含退避与错误计数
│   │   ├── feishu.rs
│   │   └── websocket.rs
│   ├── metrics.rs          # 运行指标与错误按 stage 聚合；health 与 heartbeat 暴露
│   ├── platform/.../health.rs  # 返回 WiFi、队列深度、最近错误、metrics 快照
│   └── platform/
│       ├── mod.rs
│       ├── wifi.rs
│       ├── http.rs         # 超时、重试、响应大小上限
│       ├── spiffs.rs
│       └── nvs.rs
└── docs/
    ├── ARCHITECTURE_ZH.md
    └── adr/                # 架构决策记录（可选）
```

- **lib.rs**：只导出稳定、文档完整的类型与 trait；破坏性变更随主版本号提升。
- **main.rs**：入口仅做平台初始化与配置加载；**run_app** 负责组装、启动、健康检查端点/CLI、OTA 回退策略；无业务细节。

### 2.4 核心抽象（trait）与数据流

- **消息总线**：`Inbound: Sender<PcMsg>`、`Outbound: Sender<PcMsg>`（或封装为 `MessageBus` 结构体）。通道侧只 push 入站；单独一个 dispatch 任务 pop 出站并按 `channel` 分发给各 `MessageSink`。
- **出站发送**：定义 `trait MessageSink { fn send(&self, chat_id: &str, content: &str) -> Result<()>; }`，Telegram/飞书/WebSocket 各自实现；dispatch 根据 `PcMsg.channel` 选择对应 sink；单通道连续失败达阈值后熔断冷却，重试间有间隔；失败记入 metrics 与错误 stage。
- **LLM**：`trait LlmClient { fn chat(&self, system: &str, messages: &[Message], tools: Option<&[ToolSpec]>) -> Result<LlmResponse>; }`，Anthropic 为默认实现；后续可加 OpenAI 等实现。
- **工具**：`trait Tool { fn name(&self) -> &str; fn description(&self) -> &str; fn schema(&self) -> Value; fn execute(&self, args: &str) -> Result<String>; }`，`ToolRegistry` 持有多态集合，按 name 派发并生成 API 所需的 tools 数组。
- **记忆/会话**：可抽象为 `MemoryStore`（读/写 MEMORY、每日笔记）与 `SessionStore`（按 chat_id 追加/加载/清空 JSONL），具体实现依赖 SPIFFS；agent 只依赖 trait，便于测试时注入内存实现。

数据流（与 C 版功能等价，结构按 Rust 习惯）：

```
通道（Telegram/飞书/WS）收到用户消息
  → Inbound.push(PcMsg)
  → Agent 任务：pop 入站
     → context_builder 使用 MemoryStore + SessionStore 构建 (system, messages)
     → LlmClient.chat(system, messages, tools)
     → 若 tool_use：ToolRegistry.execute → 追加 tool_result，再 chat
     → 直到 end_turn
     → SessionStore 追加本轮；Outbound.push(回复 PcMsg)
  → Dispatch 任务：pop 出站 → 按 channel 调用对应 MessageSink.send
```

### 2.5 与 C 版的差异（仅架构层面）

- **不按 C 的目录一一对应**：如不设「platform 大杂烩」，而是按领域拆成 `llm`、`tools`、`memory`、`channels`，platform 仅做硬件与 ESP-IDF 封装。
- **配置**：C 版头文件宏 + 编译时；Rust 版用 `AppConfig` 结构体一次加载，通过参数下传，便于测试与后续从 NVS/SPIFFS 读配置。
- **错误**：统一 `Error` + `?`，避免分散的 errno 或返回值检查。
- **并发**：用 channel 与所有权传递消息，减少全局队列与手写锁；任务边界清晰（入站消费、出站分发、各通道轮询）。

### 2.6 生产级检查清单（各阶段交付前自检）

- **安全**：无密钥/敏感信息打印或写调试文件；配置与输入有校验与长度上限。
- **资源**：队列、缓冲区、会话条数有上界；单次请求/响应有超时与大小限制；栈/堆使用有文档或自检。
- **弹性**：外部调用（LLM、HTTP、通道）有重试与退避；失败不 panic，可降级或返回明确错误。
- **可观测**：关键路径有 TAG 与必要上下文；错误可区分、可计数；健康状态可查询（阶段 10 前落地）。
- **扩展**：新通道/工具/LLM 仅通过实现 trait + 注册完成，核心无改动。

---

## 三、阶段与步骤（可实施顺序）

以下步骤按顺序执行，每步完成后再进入下一步；**交付物**与**验收标准**必须满足后再继续。

---

### 阶段 0：工程与构建（第 0 步） ✅ 已完成

**目的**：在 `rust/` 下得到可烧录到 ESP32-S3 的空白固件，为后续模块打基础。

| 步骤 | 动作 | 交付物 | 验收标准 | 状态 |
|------|------|--------|----------|------|
| **0.1** | 使用 esp-idf-template 或官方推荐方式，在 `rust/` 创建 Cargo 工程，target 为 `xtensa-esp32s3-espidf` | `rust/Cargo.toml`、`rust/.cargo/config.toml`、`rust/build.rs`（若需要）、`rust/src/main.rs`（仅 `fn main()` 打印日志并 loop） | `cargo build` 通过，`cargo run` 或 idf.py 烧录后串口有日志输出 | ✅ |
| **0.2** | 配置 sdkconfig（或等价方式）：WiFi、PSRAM、分区表与 C 版对齐（如 2MB×2 OTA + 12MB SPIFFS） | `rust/sdkconfig.defaults`、`rust/sdkconfig.defaults.esp32s3`、`rust/partitions.csv` | 分区布局与 C 版一致，PSRAM 使能 | ✅ |
| **0.3** | 增加 SPIFFS 镜像预烧录（与 C 版相同的目录与示例文件） | `rust/spiffs_data/`（config、memory、skills 目录与示例文件）；README 说明生成与烧录方式 | 启动后挂载 SPIFFS，可列目录、读文件（阶段 1 实现挂载） | ✅ |
| **0.4** | 编写 `rust/README.md`（中英双语）：工具链、构建、烧录、与 C 版关系；约定固件版本号写入二进制（如 `env!("CARGO_PKG_VERSION")`）便于 OTA 与运维 | `rust/README.md` | 他人可完成一次完整构建与烧录；版本可查 | ✅ |

**本阶段结束**：Rust 工程可独立构建、烧录，具备与 C 版一致的存储与分区基础，并为生产运维预留版本标识。

**阶段 0 完成记录**：已创建 `beetle` Cargo 工程（Cargo.toml、.cargo/config.toml、build.rs、rust-toolchain.toml）、sdkconfig 与分区表、spiffs_data 目录结构、main.rs 带版本日志、README 中英双语。构建需在安装 `esp` 工具链（espup）后执行 `cargo build --release`。

---

### 阶段 1：配置与平台基础设施（第 1 步） ✅ 已完成

**目的**：统一配置（显式下传）、错误类型（Rust 惯用）、平台存储与日志，为依赖注入打基础。

| 步骤 | 动作 | 交付物 | 验收标准 | 状态 |
|------|------|--------|----------|------|
| **1.1** | 新建 `config`：`AppConfig` 从环境变量或 build-time 文件加载；**加载后校验**（必填项、格式、长度上界）；类型安全、无硬编码；对外仅暴露不可变结构体；**密钥与敏感字段永不打印、不写 SPIFFS** | `src/config.rs` | main 加载一次并下传；校验失败时明确错误；无敏感信息泄露 | ✅ |
| **1.2** | 定义 `beetle::Error`（enum + thiserror），兼容 esp-idf-svc 与 std；各底层错误 `From` 收敛；**错误带上下文**（如 stage、status_code）便于生产排查；约定 `Result<T, Error>` 与 `?` | `src/error.rs` | 后续模块统一 `?`；错误可区分、可日志 | ✅ |
| **1.3** | 封装 NVS（失败时 erase 再 init）、SPIFFS 挂载；「读/写/列目录」最小 API 放在 `platform`；路径与 C 版 SPIFFS 一致；**写操作有大小或次数约束**，避免写满分区 | `platform::nvs`、`platform::spiffs` | 启动后可用；写有上界 | ✅ |
| **1.4** | 统一日志：esp-idf-svc log + level；**约定 TAG 与可选 chat_id/request_id**，便于后续结构化与排查 | `docs/LOGGING.md` | 串口分级日志可读，关键路径可追踪 | ✅ |

**本阶段结束**：配置与错误达到生产级（校验、无敏感泄露、可观测）；后续模块通过参数接收 `AppConfig` 与 `Error`。

**阶段 1 完成记录**：已实现 `error.rs`（Error 枚举 + thiserror，Nvs/Spiffs/Config/Io/Esp/Http/Other，带 stage 上下文）、`config.rs`（AppConfig::load_from_env，validate_for_wifi/validate_proxy，无敏感打印）、`platform/nvs.rs`（init_nvs，失败时 erase 再 init）、`platform/spiffs.rs`（init_spiffs，read_file/write_file/list_dir，MAX_WRITE_SIZE 256KB）、`docs/LOGGING.md`；main 中加载 config、校验 proxy、初始化 NVS/SPIFFS 并打日志。

---

### 阶段 2：消息总线（第 2 步） ✅ 已完成

**目的**：用 Rust 惯用方式实现入站/出站消息传递（channel + 所有权），语义与 C 版对齐，为 Agent 与通道解耦。

| 步骤 | 动作 | 交付物 | 验收标准 | 状态 |
|------|------|--------|----------|------|
| **2.1** | 定义 `PcMsg`：`channel`、`chat_id`、`content`（`String`），**对 content 长度设上界**（如 64KB）并在入队前校验；可派生 Serialize/Deserialize | `src/bus.rs` 中的 `PcMsg` | 与 C 版语义一致；防止单条消息耗尽内存 | ✅ |
| **2.2** | 实现入站/出站 channel，**固定容量**（如 8）；**明确背压行为**：队满时 send 阻塞或返回错误，文档约定；暴露 `MessageBus { inbound_tx, outbound_tx, … }` | `bus` 模块 | push/pop 所有权清晰；队满行为明确、可观测 | ✅ |
| **2.3** | main 中构造 bus，占位 consumer 从出站取并打日志；**可选：入队/出队失败或队满时打日志或计数**，为健康检查预留 | main 中组装与占位任务 | 一条消息可完整传递；队满可观测 | ✅ |

**本阶段结束**：消息总线具备生产级边界（长度与容量上界、背压明确）；通道只持入站 Sender，dispatch 持出站 Receiver 与各 MessageSink。

**阶段 2 完成记录**：已实现 `src/bus.rs`（PcMsg、MAX_CONTENT_LEN 64KB、PcMsg::new 校验、MessageBus::new 返回 inbound/outbound channel，背压为 sync_channel 队满阻塞）；main 中 MessageBus::new(DEFAULT_CAPACITY)、占位 outbound consumer（recv → log）、占位 inbound producer 发一条、main 发一条到 outbound 验证链路；lib 导出 bus、MessageBus、PcMsg、DEFAULT_CAPACITY、MAX_CONTENT_LEN。

---

### 阶段 3：存储与记忆（第 3 步） ✅ 已完成

**目的**：实现 MEMORY、每日笔记、会话 JSONL（格式与 C 版兼容以便数据共用）；抽象为 MemoryStore/SessionStore，便于测试时注入 mock。

| 步骤 | 动作 | 交付物 | 验收标准 | 状态 |
|------|------|--------|----------|------|
| **3.1** | 实现 `memory_store`：读/写 MEMORY.md、每日笔记；路径与 C 版一致；**单文件大小或单次写入有上界**，避免写满 SPIFFS | `memory::memory_store` API | 能读写；写有上界 | ✅ |
| **3.2** | 实现 `session_mgr`：按 chat_id 会话文件；追加、按 ring 加载最近 N 条、清空；**N 与单条长度有上界**；JSONL 与 C 版一致 | `memory::session_mgr` API | 追加与 ring 正确；条数/长度有界 | ✅ |
| **3.3** | 系统提示素材聚合：SOUL + USER + MEMORY + 近期每日笔记；**总长度有上界**（或截断策略），供 agent::context 使用 | 聚合接口（可单测） | Agent 能拿到系统提示；不超资源上限 | ✅ |

**本阶段结束**：记忆与会话具备生产级资源约束；Agent 依赖 trait，不直接依赖 SPIFFS 路径。

**阶段 3 完成记录**：已实现 `src/memory/mod.rs`（MemoryStore/SessionStore trait、SessionMessage、路径与上界常量、build_system_prompt 纯函数及单测）；`src/platform/spiffs_memory.rs`（SpiffsMemoryStore）；`src/platform/spiffs_session.rs`（SpiffsSessionStore，有界 ring、chat_id 校验、非法 JSONL 行跳过打日志）；main 中读 memory/soul/user、会话 append/load_recent、build_system_prompt 验证。memory 不依赖 platform；lib 导出 memory 与两 store 实现。

---

### 阶段 4：网络与 HTTP（第 4 步） ✅ 已完成

**目的**：WiFi STA、HTTP(S) 客户端、可选 HTTP 代理（CONNECT 隧道），供后续 LLM、工具、通道使用。

| 步骤 | 动作 | 交付物 | 验收标准 | 状态 |
|------|------|--------|----------|------|
| **4.1** | WiFi STA：从 config 读 SSID/密码；等待连接成功或**超时**；**断线后重试与退避**（不 panic），便于生产环境网络抖动 | `platform::wifi` | 能连上 AP；断线可恢复 | ✅ |
| **4.2** | HTTP(S) 客户端：GET/POST、Header、body；TLS；**每次请求有超时与响应体大小上限**；错误映射到 `Error` 并带上下文；**可配置重试次数与指数退避**（用于 LLM/工具） | `platform::http_client` | 能发 HTTPS；超时与大小受控；重试可配置 | ✅ |
| **4.3** | HTTP 代理：CONNECT 隧道；从 config 读 proxy（可选）；**隧道建立超时与失败映射到 Error** | `platform::proxy` | 代理可选；失败可观测 | ✅ |

**阶段 4 完成记录**：已实现 `platform::wifi::connect`（15s 超时、错误 stage `wifi_connect`）、`platform::http_client::EspHttpClient`（`new()`/`new_with_config()`、`get`/`post`、30s 超时、512KB 响应体上限）；proxy 为占位（配置了 proxy 时 get/post 返回 `proxy_connect` 错误）。main 在 init_spiffs 后连 WiFi、发一条 HTTPS GET 验证。

**阶段 4 新增封装与约束（后续编码必须遵守）**：

- **WiFi**：`connect_wifi(config: &AppConfig) -> Result<()>`。调用前应对 config 做 `validate_for_wifi()`；连接超时 15s；错误统一用 `Error::Config`/`Error::Other`，stage 为 `wifi_connect`。
- **HTTP 客户端**：由 main **构造一次**，阶段 5/6/8 应**注入**同一 `EspHttpClient` 使用，不在各处再 `EspHttpClient::new()`。直连用 `new()`，走 proxy 用 `new_with_config(&config)`（当前 CONNECT 未实现会返回 `proxy_connect` 错误）。API：`get(&mut self, url: &str) -> Result<(u16, Vec<u8>)>`、`post(&mut self, url: &str, body: &[u8]) -> Result<(u16, Vec<u8>)>`；响应体上限 512KB，请求超时 30s。
- **Error stage 约定**：网络/HTTP 相关错误沿用带 `stage` 的 `Error`（如 `wifi_connect`、`http_client_new`、`http_get_request`、`http_get_submit`、`proxy_connect`）；后续扩展 HTTP/LLM/工具时继续带 stage，不新增未使用变体。
- **platform 边界**：platform 是唯一依赖 esp-idf-svc 的模块；核心域（llm、agent、channels）不直接依赖 platform，通过 main 注入的客户端或 trait 使用。

**本阶段结束**：网络层具备生产级弹性（超时、大小限制、重试与退避）；LLM、工具、通道共用该封装。

---

### 阶段 5：LLM 代理（第 5 步） ✅ 已完成

**目的**：实现 `LlmClient` trait 的 Anthropic 版本（非流式），请求/响应用 serde 与 API 对齐；Agent 只依赖 trait，便于日后换实现。

| 步骤 | 动作 | 交付物 | 验收标准 | 状态 |
|------|------|--------|----------|------|
| **5.1** | 定义请求/响应类型（与 Anthropic 一致）：system、messages、tools、max_tokens；response 含 content、stop_reason、tool_use；**请求/响应体有大小上界**（与 PSRAM 预算一致） | `llm::types`（serde） | 类型正确；大小有界 | ✅ |
| **5.2** | 实现 `LlmClient` for Anthropic：依赖注入 `&AppConfig` 与 HTTP 客户端；POST 到 API，serde 解析；**失败时按配置重试与指数退避**；大 buffer 用 PSRAM | `llm::anthropic::AnthropicClient` | 能返回 content 与 stop_reason；重试可配置 | ✅ |
| **5.3** | 错误：网络超时、4xx/5xx、JSON 解析、body 超长统一为 `Error`，**带 status_code/stage 等上下文**；可选：失败计数供健康检查 | 实现与文档 | 错误可区分、可排查、可观测 | ✅ |

**阶段 5 完成记录**：已实现 `llm::types`（Message、LlmResponse、StopReason、请求/响应体上界常量）、`llm::LlmClient` / `LlmHttpClient` trait、`llm::anthropic::AnthropicClient`（重试与退避）；lib 中 `impl LlmHttpClient for EspHttpClient`；platform 新增 `post_with_headers`；main 在 HTTP 客户端就绪后发一条 chat 验证。

**阶段 5 新增封装与约束（后续编码必须遵守）**：

- **LlmClient**：Agent 与 main 只依赖此 trait。`chat(&self, http: &mut dyn LlmHttpClient, system, messages, tools) -> Result<LlmResponse>`；HTTP 由调用方注入，**llm 模块不依赖 platform**。
- **LlmHttpClient**：在 **lib** 中由 `EspHttpClient` 实现（`do_post(url, headers, body)`），供任意 `LlmClient` 实现使用；新增 LLM 厂商时仅新增实现 `LlmClient` 的类型，不修改 platform。
- **类型与常量**：`Message`、`LlmResponse`、`StopReason`、`ToolSpec` 在 `llm::types`；请求体上界 `MAX_REQUEST_BODY_LEN`（512KB）、单条内容建议上界 `MAX_MESSAGE_CONTENT_LEN`（64KB）；序列化后超限用 `Error::Config`。
- **错误 stage**：LLM 相关错误沿用 `llm_request`、`llm_parse`；不新增未使用 Error 变体。
- **扩展新厂商**：新建 `llm::xxx` 实现 `LlmClient`，main 根据 config（如 `model_provider`）构造对应实现并注入；Agent 不感知具体厂商。

**本阶段结束**：LLM 层具备生产级弹性与可观测；Agent 只依赖 `LlmClient` trait。

---

### 阶段 6：工具注册与执行（第 6 步） ✅ 已完成

**目的**：定义 `Tool` trait 与 `ToolRegistry`（按 name 派发、生成 API 的 tools 数组）；实现 web_search、get_time、cron、files 等，行为与 C 版对等。

| 步骤 | 动作 | 交付物 | 验收标准 | 状态 |
|------|------|--------|----------|------|
| **6.1** | 定义 `Tool` trait：name、description、input_schema、`execute(args) -> Result<String>`；**对 args 与返回值长度设上界**；`ToolRegistry`：register、按 name 查找、生成 API 的 tools 数组 | `tools::registry` 与 `Tool` trait | 可注册、可派发；输入/输出有界 | ✅ |
| **6.2** | web_search：调用 Brave API（经 HTTP 客户端）；代理与 key 从 config；**超时与响应大小限制**；失败返回 `Error` 不 panic | `tools::web_search` | 能返回摘要；失败可观测 | ✅ |
| **6.3** | get_time、cron、files（与 C 版行为一致）；**输入校验与长度约束**；全部注册到 Registry | `tools::get_time`、`tools::cron`、`tools::files` | Agent 能按 name 派发；安全约束到位 | ✅ |
| **6.4** | 在 context 中注入工具说明到系统提示；**工具列表总长度有上界**（或截断） | 阶段 7 的 context 中调用 | 系统提示含工具说明且不超限 | ✅ |

**阶段 6 完成记录**：已实现 `tools::Tool`（name/description/schema/execute(args, ctx)）、`ToolContext`（get/get_with_headers）、`ToolRegistry`（register、get、execute、tool_specs_for_api、format_descriptions_for_system_prompt）；get_time/cron/files 占位、web_search 经 Brave API；lib 中 `impl ToolContext for EspHttpClient`；platform 新增 `get_with_headers_inner`；main 注册四工具并 execute("get_time") 验证。

**阶段 6 新增封装与约束（后续编码必须遵守）**：

- **Tool**：Agent 按 name 派发；`execute(&self, args: &str, ctx: &mut dyn ToolContext) -> Result<String>`；**tools 模块不依赖 platform**，HTTP 通过 ToolContext 注入。
- **ToolContext**：在 **lib** 中由 `EspHttpClient` 实现（`get` 默认调 `get_with_headers(url, &[])`，`get_with_headers(url, headers)`）；需自定义 header 的工具（如 web_search 的 X-Subscription-Token）用 `get_with_headers`。
- **常量**：`MAX_TOOL_ARGS_LEN`（8KB）、`MAX_TOOL_RESULT_LEN`（16KB）；args 超限在 `registry.execute` 内校验并返回 `Error::Config`；返回值超限由各工具截断或返回 Error。
- **ToolRegistry**：main 构造一次并注册所有工具；阶段 7 Agent 注入同一 Registry；`tool_specs_for_api(max_len)`、`format_descriptions_for_system_prompt(max_chars)` 供 context 使用，总长度不超过参数。
- **新增工具**：实现 `Tool` 并在 main 中 `registry.register(Box::new(XxxTool::new(...)))`；不修改 platform，不新增未使用 Error 变体；错误 stage 如 `tool_web_search`、`tool_execute`。

**本阶段结束**：工具层具备生产级边界与扩展性；新增工具仅实现 `Tool` 并 register。

---

### 阶段 7：Agent 与上下文（第 7 步）

**目的**：实现 `agent::context`（纯逻辑：从 MemoryStore/SessionStore 聚合 system + messages，可 host 单测）与 `agent::loop`（ReAct：依赖 `LlmClient`、`ToolRegistry`、SessionStore、bus）；保存会话并推送到出站。

| 步骤 | 动作 | 交付物 | 验收标准 |
|------|------|--------|----------|
| **7.1** | 实现 `agent::context`：依赖 MemoryStore/SessionStore 与工具说明，聚合 system + messages（含 channel/chat_id）；**总长度受阶段 3 上界约束**；纯逻辑可单测 | `agent::context` | 给定 PcMsg 与 store 返回 (system, messages)；不超资源 |
| **7.2** | 单轮逻辑：`LlmClient::chat`、解析 tool_use、`ToolRegistry::execute` 拼 tool_result；**单轮总耗时或单次 LLM 调用有超时**；execute 失败写入可区分错误内容而非 panic | `agent::loop` 内单轮 | 单轮多 tool_use 正确；超时与失败可观测 |
| **7.3** | ReAct 循环：最多 N 轮（如 10）；end_turn 则写会话、push 出站；tool_use 则追加后继续；**整轮超时可选**（防止单对话卡死） | `agent::loop` | 入站一条→出站一条；含 tool 时正确；可限时 |
| **7.4** | 会话持久化：SessionStore 追加本轮；**错误时只打日志并返回用户可读错误（或重试一次）**，不破坏已有文件 | loop 内调用 SessionStore | 会话正确；失败可恢复 |

**本阶段结束**：Agent 具备生产级超时与错误处理；只依赖 trait，行为与 C 版一致并更稳健。

---

### 阶段 8：通道（第 8 步）

**目的**：定义出站抽象（如 `MessageSink` trait）；实现 Telegram、飞书、WebSocket 三个通道；入站推 bus，出站由单独 dispatch 任务按 channel 调用对应 sink。

| 步骤 | 动作 | 交付物 | 验收标准 |
|------|------|--------|----------|
| **8.1** | 定义 `MessageSink::send(chat_id, content) -> Result<()>`；dispatch 持 channel → sink 映射；**send 失败时记录错误并可选重试**，不丢消息不 panic；入站侧仅持 `Sender<PcMsg>` | `channels` 抽象 | 新通道实现 MessageSink + 入站解析即可；出站失败可观测 |
| **8.2** | Telegram：long polling、解析、推入 Inbound、send_message；**轮询失败或网络错误时退避再试**，不 panic；与 C 版 API 一致 | `channels::telegram` | 能收发；失败可恢复 |
| **8.3** | 飞书：鉴权、WebSocket、事件解析、去重、推入 Inbound、发送；**断线重连与退避**；错误带上下文 | `channels::feishu` | 飞书能收发；弹性到位 |
| **8.4** | WebSocket 网关：端口与协议与 C 版一致；入站推 Inbound、出站经 dispatch；**单条消息大小与连接数有上界** | `channels::websocket` | WS 能收发；资源有界 |
| **8.5** | Outbound Dispatch：循环 pop 出站；按 channel 调用对应 MessageSink；**send 失败打日志并可选重试**；不阻塞其他 channel | main 或 `channels::dispatch` | 所有通道正确路由；单通道失败不影响其他 |

**本阶段结束**：通道层具备生产级弹性与可观测；扩展性最强（新通道仅实现 trait + 注册）。

---

### 阶段 9：运维与周边（第 9 步）

**目的**：CLI、OTA、cron、heartbeat、skills 等与 C 版对等或更好。

| 步骤 | 动作 | 交付物 | 验收标准 |
|------|------|--------|----------|
| **9.1** | Serial CLI：wifi_status、memory_read、memory_write、session_list、session_clear、heap_info、restart、help；**health**：返回 WiFi、入站/出站深度、最近错误摘要及 **metrics 快照**（消息进/出、LLM/tool 调用与错误、WDT feed、按 stage 错误计数，无敏感信息）；**破坏性命令有确认或审计日志** | `cli`、`platform/.../health`、`metrics` | 命令完整；健康与指标可查；关键操作可审计 |
| **9.2** | OTA：从 URL 拉取固件并切换分区；**失败时保留当前分区可启动（回退）**；可选：校验固件签名或版本号 | `ota` 模块 | OTA 成功可启动新版本；失败不 brick |
| **9.3** | Cron：定时向 Inbound 推系统消息或触发技能；**与 C 版行为一致**；错峰或退避避免与通道轮询冲突 | `cron` 模块 | 到点有预期动作 |
| **9.4** | Heartbeat：周期打日志或上报；**可被外部监控**（如 HTTP 或串口约定）；便于运维判断设备存活 | `heartbeat` 模块 | 周期可见；可监控 |
| **9.5** | Skills：从 SPIFFS 加载 skill 描述；**加载失败不阻塞启动**；与 C 版目录格式兼容；总大小或数量有上界 | `skills` 模块 | 能加载 skill；失败可降级 |

**本阶段结束**：运维能力达到生产级（健康、OTA 回退、审计、可监控）；超越 C 版的运维可观测与安全。

---

### 阶段 10：集成、测试与文档（第 10 步）

**目的**：端到端联调、内存与稳定性、CI、以及符合 Rust 习惯的架构文档与 README。

| 步骤 | 动作 | 交付物 | 验收标准 |
|------|------|--------|----------|
| **10.1** | main 按依赖顺序组装；无全局可变状态；Core/栈与 C 版量级接近；**启动时自检**：配置有效、存储可写、队列创建成功；**优雅关闭约定**（若支持）：等待当前消息处理完再退出 | main 与启动说明 | 冷启动无 panic；自检失败明确报错 |
| **10.2** | **资源文档**：PSRAM/栈占用、各队列与缓冲区上界、超时与重试默认值（见 `constants.rs`：INBOUND_RECV_TIMEOUT_SECS、AGENT_RETRY_*、PENDING_RETRY_MAX_REPLAY、CHANNEL_FAIL_*）；长时间运行验证无 OOM/栈溢出；**可观测**：health 返回 metrics 快照，heartbeat 每 30s 打基线日志（msg_in、llm_calls、err_* 等） | 文档或脚本、`constants`、`metrics` | 稳定运行；资源与可观测可查 |
| **10.3** | CI：Rust 固件构建（**feature 矩阵**：至少 default + 最小集）；clippy + fmt；可选烧录与冒烟 | `.github/workflows` | push 即构建；多 feature 覆盖 |
| **10.4** | **架构文档**：`rust/docs/ARCHITECTURE_ZH.md` 含 2 节 + **生产级清单**（安全、资源、弹性、可观测、扩展）；**集成/ e2e 测试策略**（哪些在 host、哪些在设备、如何跑）；README 中英双语，含与 C 版对比及「超越 C 版」能力说明 | 文档与 README | 新人可理解架构与生产要求；测试可复现 |

**本阶段结束**：Rust 版达到**生产/消费级可交付**；架构最强扩展性、可观测与弹性均落地，超越 C 版。

---

## 四、依赖关系简图

```
0 工程/构建
  ↓
1 配置 + 平台(存储/日志/错误)
  ↓
2 消息总线  ←─────────────────── 8 通道(入站/出站)
  ↓
3 存储与记忆
  ↓
4 网络( WiFi + HTTP + 代理 )
  ↓
5 LLM 代理    6 工具
  ↓           ↓
  └─────→ 7 Agent( context_builder + ReAct )
                ↓
           9 运维( CLI / OTA / cron / heartbeat / skills )
                ↓
           10 集成 / 测试 / 文档
```

---

## 五、执行与追踪建议

- **每步收尾**：在本文档或项目 issue 中勾选「已完成」并注明分支/提交；若某步拆成多 PR，可写子步骤。
- **回滚**：任一步发现问题可退回上一步交付物，避免带着技术债进入下一步。
- **并行**：阶段 5（LLM）与 6（工具）可在 4 完成后并行；8 中 Telegram / 飞书 / WebSocket 也可分人并行，但需先定好 bus 与 Channel 抽象（2 与 8.1）。

按上述顺序执行，并在每阶段交付前对照 **2.6 生产级检查清单** 自检，即可得到**真正可生产、可消费、扩展性最强、超越 C 版**的 Rust 架构与固件。
