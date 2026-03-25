# beetle 文档索引 / Documentation index

**中文** | [English section below](#english)

面向外部读者：按角色选读，避免在多篇文档里重复维护同一事实。

## 维护约定（给贡献者）

- **HTTP 路由与鉴权**：以 [`src/platform/http_server/router/dispatch.rs`](../src/platform/http_server/router/dispatch.rs) 为准。
- **工具清单**：以 [`src/tools/registry.rs`](../src/tools/registry.rs) 中 `build_default_registry` 为准。
- **LLM 源与回退**：以 [`src/llm/mod.rs`](../src/llm/mod.rs)、[`src/llm/fallback.rs`](../src/llm/fallback.rs) 为准。
- **单一事实来源**：API 契约详写在 **config-api**；健康/metrics 字段详写在 **config-api** 的 `GET /api/health`；用户操作流写在 **configuration**。

## 按读者分类

| 读者 | 中文 | English |
|------|------|---------|
| 终端用户（配网、配置页、常用键） | [zh-cn/configuration.md](zh-cn/configuration.md) | [en-us/configuration.md](en-us/configuration.md) |
| Agent 可用工具（用户向说明） | [zh-cn/tools.md](zh-cn/tools.md) | [en-us/tools.md](en-us/tools.md) |
| LLM 提供商标识与示例 | [zh-cn/llm-providers.md](zh-cn/llm-providers.md) | [en-us/llm-providers.md](en-us/llm-providers.md) |
| HTTP API（配对、CSRF、各路径） | [zh-cn/config-api.md](zh-cn/config-api.md) | [en-us/config-api.md](en-us/config-api.md) |
| 模块与数据流、扩展点 | [zh-cn/architecture.md](zh-cn/architecture.md) | [en-us/architecture.md](en-us/architecture.md) |
| 板型、内存、排错入口 | [zh-cn/hardware.md](zh-cn/hardware.md) | [en-us/hardware.md](en-us/hardware.md) |
| `hardware.json` 与 `device_control` 设计 | [zh-cn/hardware-device-config.md](zh-cn/hardware-device-config.md) | [en-us/hardware-device-config.md](en-us/hardware-device-config.md) |
| Linux 发布 tarball 说明 | [zh-cn/linux-release-rollback.md](zh-cn/linux-release-rollback.md) | [en-us/linux-release-rollback.md](en-us/linux-release-rollback.md) |
| 品牌 Logo 资源说明 | [assets/README.md](assets/README.md) | 同上 |

## 语言内阅读顺序建议

1. configuration →（需要对接 API 时）config-api  
2. tools、llm-providers 按需查阅  
3. architecture / hardware 面向二次开发或排错  

---

## English

For external readers: pick docs by role; each topic has one primary source of truth.

### Conventions (contributors)

- **HTTP routes and auth**: [`src/platform/http_server/router/dispatch.rs`](../src/platform/http_server/router/dispatch.rs).
- **Tool list**: `build_default_registry` in [`src/tools/registry.rs`](../src/tools/registry.rs).
- **LLM sources and fallback**: [`src/llm/mod.rs`](../src/llm/mod.rs), [`src/llm/fallback.rs`](../src/llm/fallback.rs).
- **Single source of truth**: API details in **config-api**; health/metrics shape under `GET /api/health` there; end-user flows in **configuration**.

### By audience

See the table above: **zh-cn** and **en-us** columns list the same topics in each language.

### Suggested reading order

1. configuration → config-api (when integrating)  
2. tools, llm-providers as needed  
3. architecture / hardware for development or troubleshooting  
