# beetle Rust 编码约束与架构约定

## 编码前必做

- **完整审查**：每次编码前必须完整审查并了解当前项目状态——现有模块、已实现调用链、本阶段目标与依赖关系。
- **明确调用链**：编码前厘清「谁创建、谁调用、谁消费」；只实现当前调用链上的节点，不提前抽象未使用路径。
- **分步实现**：按「定义类型 → 实现构造/发送/接收 → main 或组装点接线 → 单条链路验证」顺序推进；每步可编译、可验证后再进入下一步。
- **禁止冗余**：不引入本阶段未使用的类型、trait 或模块；不复制已有逻辑（如错误处理复用 `beetle::Error`）；同一职责只保留一处实现。

## 项目命名

- **中文名**：甲壳虫
- **英文名**：beetle
- **Crate 名**：`beetle`
- 对外类型与日志可带 "beetle" 或 "甲壳虫"；注释与文档保持中英双语。

## 架构原则

- **依赖倒置**：核心模块（agent、bus、llm、tools、memory）只依赖 trait，不依赖 `platform` 或 `channels`。具体实现由 `main` / `run_app` 注入。
- **平台隔离**：`esp_idf_svc` 及任何硬件专有调用**仅**出现在 `platform/` 目录或带 `#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]` 守卫的文件/块中。业务层（agent、bus、llm、tools、channels、config、error、memory、doctor、skills、orchestrator、heartbeat、cron、state、cli）**禁止**直接引用 `esp_idf_svc`。新增功能涉及硬件时，必须在 `Platform` trait 上扩展方法（默认 no-op 或返回错误），由 `Esp32Platform` 实现，业务层通过 `platform.xxx()` 调用。
- **单一错误类型**：公共 API 返回 `Result<T, beetle::Error>`，使用 `?` 传播；错误带上下文（stage、status_code 等），禁止在生产路径使用 `unwrap()`/`expect()`。
- **配置显式下传**：通过参数传递 `AppConfig`，禁止全局可变静态保存配置或密钥。
- **通过通信共享**：任务间用 channel 传递 `PcMsg`，避免不必要的 `Arc<Mutex<T>>`；队列与缓冲区有固定容量与背压约定。

## 安全

- 密钥与敏感字段**永不**打印、不写 SPIFFS 调试文件；配置加载后做必填与长度校验；启动期可调用 `config.validate_for_channels()` 校验当前 enabled_channel 对应凭证。
- 日志涉敏处使用 `util::redact_secret()` 脱敏；工具输入、会话与消息 content 有长度与格式上界，防止溢出与注入。
- **不搞伪需求**：在威胁模型已由现有手段覆盖时，不再叠加冗余安全措施；避免安全剧场式需求。

## 资源与弹性

- 队列、缓冲区、会话条数、单条消息/响应均有明确上界（见 `constants.rs`）；超时与退避常量集中（INBOUND_RECV_TIMEOUT_SECS、AGENT_RETRY_*、PENDING_RETRY_MAX_REPLAY、CHANNEL_FAIL_*）。
- 对外调用（LLM、HTTP、通道）需有超时与可配置重试/退避；失败返回 `Error` 而非 panic。dispatch 层单通道连续失败会熔断冷却，避免单通道拖垮全局。
- **资源可观测（orchestrator 为唯一权威）**：`GET /api/resource` 与 `orchestrator::snapshot()` / `format_resource_baseline_line()` 对齐；心跳在同周期内输出 orchestrator 单行基线 + `metrics::to_baseline_log_line`。**出站 Cautious**：`should_accept_outbound` 在 Cautious 下短延迟 `OUTBOUND_DEFER_DELAY_MS_CAUTIOUS`（500ms），Critical 仍用 `OUTBOUND_DEFER_DELAY_MS`；`GET /api/health` 嵌套 `metrics` 与 `resource` 快照（JSON 字段名与 serde 结构体一致）。

## 代码风格

- 公共 API 必须有 rustdoc（中英均可）；新增模块在 `lib.rs` 或对应 `mod.rs` 中导出稳定接口。
- 遵循 `rustfmt` 与 `clippy`（`cargo clippy` 无警告）；嵌入式注意栈与堆使用，大 buffer 使用 PSRAM。

### 后台线程与资源

- **禁止重复线程**：同一职责（如显示刷新）只允许一个后台线程。若需要不同频率执行不同逻辑，在单一线程内用计数器区分，不得为此多开线程。ESP32 每个线程占 ~4KB 栈，资源宝贵。
- **禁止虚假度量**：显示、日志、API 返回的度量值（heap、CPU、温度等）必须来自真实数据源（如 `orchestrator::snapshot()`、`platform::heap`）。禁止用固定映射或占位值冒充真实度量；暂时不可用的度量应传 0 或在 UI 标注 N/A。

### 嵌入式字节序

- **ESP32 为 little-endian**：处理网络协议数据（IP 地址、端口等）时，注意网络字节序（大端）与平台字节序的转换。ESP-IDF 中 `esp_netif_ip_info_t.ip.addr` 以平台原生字节序存储，拆分字节应使用 `to_ne_bytes()` 而非 `to_be_bytes()`。

### 条件初始化

- **disabled 功能不初始化后端**：当功能被配置为 disabled 时（如 `display.enabled = false`），不应初始化对应的硬件后端或申请硬件资源。避免 disabled 配置的默认值变更导致意外初始化失败。

### 惯用 Rust

- 优先使用 `unwrap_or_else` 而非 `.or_else(|| Some(...))` + 后续 unwrap；避免不必要的 `Option` 包装。当 fallback 值确定时直接 `unwrap_or_else(|| default)` 得到内部类型。

### 显示与嵌入式性能规范（新增）

- **延时语义统一**：业务代码中的“毫秒级等待”统一使用 `std::thread::sleep(Duration::from_millis(...))`，禁止直接把毫秒值传给 `vTaskDelay`（tick 频率可配置，语义不稳定）。
- **热路径禁止无意义拷贝**：高频刷新路径（显示循环、队列消费）禁止 `clone()` 大缓冲（如行缓冲）；优先复用已分配内存并按引用发送。
- **disabled 必须零硬件副作用**：功能关闭时（如 `display.enabled=false`）不得初始化后端、不得申请总线/GPIO 资源。后端句柄使用 `Option`，执行时显式短路。
- **禁留伪检查死代码**：禁止 `_assets_sanity` 这类“只读取不校验”的占位代码；编译期可保证的事实（如 `include_bytes!` 文件存在）不再做运行时伪校验。
- **后台循环复用容器**：周期线程中禁止每轮新建 `Vec<String/...>`；循环外预分配，循环内 `clear + extend` 复用，降低堆碎片与抖动。
- **字节序先证实再改**：涉及网络/IP 字节序时，必须先确认 ESP-IDF/lwIP 字段存储语义再改实现；禁止“凭经验”把 `to_ne_bytes()` 直接改为 `to_be_bytes()`。

## Git

- **Commit messages**：必须使用英文撰写（subject 与 body 均英文），便于国际协作与历史检索。

## 扩展点

- 新通道：实现 `MessageSink` trait 并注册，不修改 bus/agent。
- 新工具：实现 `Tool` trait 并注册到 `ToolRegistry`。
- 新 LLM：实现 `LlmClient` trait 并注入。
- 新流式编辑通道：实现 `StreamEditor` trait（`send_initial` + `edit`），在 `main.rs` 根据 `enabled_channel` 创建并注入 `AgentLoopConfig.stream_editor`。实现方自行创建 HTTP 连接，不依赖 agent 的 LLM 连接。
- 新平台：实现 `Platform` trait（含 `init`、`init_nvs`、`init_spiffs`、`connect_wifi`、`create_http_client`、`request_restart`、`init_sntp`、`ota_from_url` 等）及 `PlatformHttpClient`、`ChannelHttpClient`、`WssConnection` 等 trait，在 main 中替换平台实例与工厂闭包。业务层代码无需改动。

---

## 已实现模块的封装与约束

### 网络与 HTTP

- **WiFi**：`connect_wifi(config: &AppConfig) -> Result<()>`。调用前对 config 做 `validate_for_wifi()`；连接超时 15s；错误 stage 为 `wifi_connect`。
- **HTTP 客户端**：由 **main 构造一次**，LLM、工具、通道均**注入**同一 `EspHttpClient`，不在各处再 `EspHttpClient::new()`。直连用 `new()`，走 proxy 用 `new_with_config(&config)`。响应体上限 512KB，请求超时 30s。**例外**：`StreamEditor` 实现和 sender 线程需自行创建独立 HTTP 连接（主连接被 LLM 流占用或运行在独立线程）。
- **Error stage**：网络/HTTP 错误带 stage（`wifi_connect`、`http_client_new`、`http_get_request`、`http_get_submit`、`proxy_connect`）；不新增未使用 Error 变体。
- **platform 边界**：platform 是唯一依赖 esp-idf-svc 的模块；核心域（llm、agent、tools、channels）不直接依赖 platform，通过 main 注入的客户端或 trait 使用。硬件驱动（GPIO/LEDC/ADC/buzzer）位于 `platform/hardware_drivers.rs`，tools 通过 `crate::platform::hardware_drivers` 调用。`lib.rs` 中 ESP 专有类型的 re-export 均有 `#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]` 守卫。

### LLM

- **LlmClient**：Agent/main 只依赖此 trait。`chat(&self, http: &mut dyn LlmHttpClient, system, messages, tools) -> Result<LlmResponse>`；**llm 不依赖 platform**，HTTP 由调用方注入。
- **chat_with_progress**：可选流式回调方法；默认回退到 `chat`。`StreamProgressFn<'a> = &'a mut dyn FnMut(&str, &str)` 传递 `(delta, accumulated)`。`FallbackLlmClient` 仅第一源走 progress，后续降级普通 chat。
- **LlmHttpClient**：在 **lib** 中由 `EspHttpClient` 实现（`do_post(url, headers, body)`）；新增 LLM 厂商时仅新增实现 `LlmClient` 的类型，不修改 platform。
- **类型与常量**：`Message`、`LlmResponse`、`ToolSpec` 在 `llm::types`；`MAX_REQUEST_BODY_LEN`（512KB）、`MAX_MESSAGE_CONTENT_LEN`（64KB）；序列化超限用 `Error::Config`。
- **错误 stage**：LLM 用 `llm_request`、`llm_parse`。

### 工具

- **Tool**：`execute(&self, args: &str, ctx: &mut dyn ToolContext) -> Result<String>`；**tools 不依赖 platform**，HTTP 通过 ToolContext 注入。
- **ToolContext**：在 **lib** 中由 `EspHttpClient` 实现（`get` / `get_with_headers`）；需自定义 header 时用 `get_with_headers`。
- **常量**：`MAX_TOOL_ARGS_LEN`（8KB）、`MAX_TOOL_RESULT_LEN`（16KB）；args 在 `registry.execute` 内校验；错误 stage 如 `tool_web_search`、`tool_execute`。
- **ToolRegistry**：main 构造一次并注册所有工具；`build_default_registry(config, platform, …)` 需传入 `Arc<dyn Platform>`（如 `board_info`）；Agent 注入同一 Registry；`tool_specs_for_api(max_len)`、`format_descriptions_for_system_prompt(max_chars)` 总长度不超过参数。
- **新增工具**：实现 `Tool` 并在 main 中 `registry.register(Box::new(XxxTool::new(...)))`；不修改 platform。
