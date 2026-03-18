# beetle Rust 编码约束与架构约定

## 编码前必做

- **完整审查**：每次编码前必须完整审查并了解当前项目状态——现有模块、已实现调用链、本阶段目标与依赖关系。
- **明确调用链**：编码前厘清「谁创建、谁调用、谁消费」；只实现当前调用链上的节点，不提前抽象未使用路径。
- **分步实现**：按「定义类型 → 实现构造/发送/接收 → main 或组装点接线 → 单条链路验证」顺序推进；每步可编译、可验证后再进入下一步。
- **禁止冗余**：不引入本阶段未使用的类型、trait 或模块；不复制已有逻辑（如错误处理复用 `beetle::Error`）；同一职责只保留一处实现。

## 项目命名

- **中文名**：甲虫
- **英文名**：beetle
- **Crate 名**：`beetle`
- 对外类型与日志可带 "beetle" 或 "甲虫"；注释与文档保持中英双语。

## 架构原则

- **依赖倒置**：核心模块（agent、bus、llm、tools、memory）只依赖 trait，不依赖 `platform` 或 `channels`。具体实现由 `main` / `run_app` 注入。
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

## 代码风格

- 公共 API 必须有 rustdoc（中英均可）；新增模块在 `lib.rs` 或对应 `mod.rs` 中导出稳定接口。
- 遵循 `rustfmt` 与 `clippy`（`cargo clippy` 无警告）；嵌入式注意栈与堆使用，大 buffer 使用 PSRAM。

## Git

- **Commit messages**：必须使用英文撰写（subject 与 body 均英文），便于国际协作与历史检索。

## 扩展点

- 新通道：实现 `MessageSink` trait 并注册，不修改 bus/agent。
- 新工具：实现 `Tool` trait 并注册到 `ToolRegistry`。
- 新 LLM：实现 `LlmClient` trait 并注入。
- 新流式编辑通道：实现 `StreamEditor` trait（`send_initial` + `edit`），在 `main.rs` 根据 `enabled_channel` 创建并注入 `AgentLoopConfig.stream_editor`。实现方自行创建 HTTP 连接，不依赖 agent 的 LLM 连接。

---

## 已实现模块的封装与约束

### 网络与 HTTP

- **WiFi**：`connect_wifi(config: &AppConfig) -> Result<()>`。调用前对 config 做 `validate_for_wifi()`；连接超时 15s；错误 stage 为 `wifi_connect`。
- **HTTP 客户端**：由 **main 构造一次**，LLM、工具、通道均**注入**同一 `EspHttpClient`，不在各处再 `EspHttpClient::new()`。直连用 `new()`，走 proxy 用 `new_with_config(&config)`。响应体上限 512KB，请求超时 30s。**例外**：`StreamEditor` 实现和 sender 线程需自行创建独立 HTTP 连接（主连接被 LLM 流占用或运行在独立线程）。
- **Error stage**：网络/HTTP 错误带 stage（`wifi_connect`、`http_client_new`、`http_get_request`、`http_get_submit`、`proxy_connect`）；不新增未使用 Error 变体。
- **platform 边界**：platform 是唯一依赖 esp-idf-svc 的模块；核心域（llm、agent、tools、channels）不直接依赖 platform，通过 main 注入的客户端或 trait 使用。

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
- **ToolRegistry**：main 构造一次并注册所有工具；Agent 注入同一 Registry；`tool_specs_for_api(max_len)`、`format_descriptions_for_system_prompt(max_chars)` 总长度不超过参数。
- **新增工具**：实现 `Tool` 并在 main 中 `registry.register(Box::new(XxxTool::new(...)))`；不修改 platform。
