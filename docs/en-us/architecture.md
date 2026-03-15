# Architecture overview

**English** | [中文](../zh-cn/architecture.md)

This doc is for external readers: module layout, data flow, and how to extend. No internal implementation detail.

---

## Module layout

| Module | Responsibility |
|--------|----------------|
| **config** | Build-time / env and NVS, SPIFFS config load and validation; secrets not logged or written to disk. |
| **error** | Unified error type (stage is `&'static str`); public APIs return `Result<T, Error>`. |
| **bus** | Inbound/outbound message queues (fixed capacity, backpressure); decouples channels from Agent. |
| **memory** | Long-term memory and session storage; system prompt aggregation. |
| **platform** | Platform abstraction (config store, skill store, HTTP client, etc.) and ESP32 implementation; only module that directly depends on esp-idf-svc. |
| **llm** | LLM client abstraction; supports Anthropic, OpenAI-compatible (e.g. Ollama), etc. |
| **tools** | Tool registry; GetTime, Cron, FetchUrl, WebSearch, RemindAt, Files, etc.; new tools implement `Tool` trait and register. |
| **agent** | Context build, ReAct loop; depends on LlmClient, ToolRegistry, Memory, Session. |
| **channels** | Channel abstraction and dispatch; Telegram, Feishu, DingTalk, WeCom, QQ Channel, WebSocket; inbound pushes to bus, outbound dispatched by channel; single-channel consecutive failures trigger cooldown to avoid dragging down the system. |
| **metrics** | Runtime metrics and error profile: messages in/out, LLM/tool calls and errors, WDT feed, dispatch success/fail, per-stage error aggregation (incl. session write failures); exposed via health API and heartbeat baseline logs. |
| **cli** (optional) | Serial commands: wifi_status, heap_info, session_list, restart, ota, etc. |
| **ota** (optional) | Fetch firmware from URL, write to OTA partition; failure does not corrupt current partition. |
| **cron / heartbeat / skills** | Scheduled tasks, periodic logs (incl. metrics baseline), SPIFFS skill loading. |

---

## Data flow

```
  Channels (Feishu / DingTalk / WeCom / QQ / Telegram / WebSocket)
       ↓ push
  Inbound queue
       ↓
  Agent (build_context → LlmClient → Tools → write session)
       ↓ push
  Outbound queue
       ↓
  Dispatch to each MessageSink by channel
```

- **Inbound**: Channels (or cron) push user/system messages into Inbound; Agent consumes from Inbound.
- **Agent**: Aggregates system prompt and history from Memory/Session, calls LLM; on tool_use runs tools and appends results, loops until end_turn; writes session and pushes reply to Outbound.
- **Outbound**: Dispatch takes from Outbound and calls each channel's send; after consecutive failures per channel exceed a threshold, that channel enters cooldown and is not sent to until cooldown ends.

**Observability and health**: `GET /api/health` returns WiFi, inbound/outbound queue depth, recent error summary, and a **metrics** snapshot (messages in/out, LLM/tool calls and errors, WDT feed, per-stage error counts, etc.; no sensitive data). Heartbeat logs a metrics baseline every 30 seconds for before/after comparison.

---

## How to extend

- **New channel**: Outbound uses `dispatch::QueuedSink` (`QueuedSink::new(tx, "stage")`); register in main's `run_app` into dispatch's sink list. Channel side implements `flush_*_sends` reading from the corresponding rx and sending HTTP. Inbound: send messages to the bus Inbound. For custom send logic, implement `MessageSink` and register.
- **New tool**: Implement `Tool` trait (`name`, `description`, `schema` with parameters, `execute`); in `tools/mod.rs` use `parse_tool_args(args, stage)` for JSON args; register with `ToolRegistry`; return value is truncated to `MAX_TOOL_RESULT_LEN` by the registry.
- **New LLM backend**: Implement `LlmClient` trait; main injects it into the agent.

Core (agent, bus, llm, tools, memory) does not depend on concrete channel or platform; it only depends on abstract traits, which keeps maintenance and extension straightforward.
