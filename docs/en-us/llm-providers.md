# LLM provider configuration

[中文](../zh-cn/llm-providers.md) | **English** | [Doc index](../README.md)

Configure sources in the web UI or via SPIFFS `config/llm.json` (or the HTTP API in [config-api](config-api.md)).

## How the firmware picks a client (vs vendor docs)

[`build_llm_clients`](../../src/llm/mod.rs) routes by `provider`:

- **`anthropic`**: `AnthropicClient` (Claude Messages API).
- **`openai`, `openai_compatible`, `gemini`, `glm`, `qwen`, `deepseek`, `moonshot`, `ollama`**: **`OpenAiCompatibleClient`** (OpenAI-style chat/completions; per-vendor base URL/headers handled inside that client).

Field length validation lives in `config`. **Inclusion in the fallback chain** also follows `build_llm_clients`: **empty `api_url` is OK** for the OpenAI-compatible IDs above; **`anthropic` (and any provider not in that list) is skipped when `api_url` is empty**. For non-empty URLs, `AnthropicClient` expects the **full Messages request URL** (same role as the default `https://api.anthropic.com/v1/messages`), see [`anthropic.rs`](../../src/llm/anthropic.rs).

**Multi-source fallback** ([`FallbackLlmClient`](../../src/llm/fallback.rs)): try `llm_sources` **in order**, return the **first Ok**; if all fail, return the **last** error. Router mode is described under **GET /api/config** / multi-LLM fields in [config-api](config-api.md).

Model names below are **examples**—follow each vendor’s current documentation.

---

## Provider IDs

### OpenAI
- **ID**: `openai`
- **Examples**: `gpt-4o`, `gpt-4`, `gpt-3.5-turbo`
- **Keys**: [platform.openai.com](https://platform.openai.com)

### Anthropic (Claude)
- **ID**: `anthropic`
- **Examples**: see [Anthropic docs](https://docs.anthropic.com)
- **Keys**: [console.anthropic.com](https://console.anthropic.com)

### Google Gemini (via OpenAI-compatible client)
- **ID**: `gemini`
- **Examples**: see [Google AI](https://ai.google.dev)

### Zhipu GLM
- **ID**: `glm`

### Qwen
- **ID**: `qwen`

### DeepSeek
- **ID**: `deepseek`

### Moonshot
- **ID**: `moonshot`

### Ollama (local)
- **ID**: `ollama`
- **api_url**: typically `http://<host>:11434/v1`
- **api_key**: any non-empty placeholder is fine if the server ignores it

---

## Configuration examples

### Single provider
```json
{
  "llm_sources": [
    {
      "provider": "deepseek",
      "api_key": "sk-...",
      "model": "deepseek-chat",
      "api_url": ""
    }
  ]
}
```

### Multiple providers (ordered fallback)
```json
{
  "llm_sources": [
    {
      "provider": "gemini",
      "api_key": "AIza...",
      "model": "gemini-1.5-flash",
      "api_url": ""
    },
    {
      "provider": "glm",
      "api_key": "...",
      "model": "glm-4-flash",
      "api_url": ""
    },
    {
      "provider": "ollama",
      "api_key": "ollama",
      "model": "qwen2",
      "api_url": "http://192.168.1.100:11434/v1"
    }
  ]
}
```

Summary:

- Sources are tried **in array order**; **first success wins**.
- If every source fails, the **last** error is returned.

### Offline-friendly ordering (example)

Put Ollama last so local inference is used when upstream is down:

```json
{
  "llm_sources": [
    {
      "provider": "qwen",
      "api_key": "sk-...",
      "model": "qwen-turbo",
      "api_url": ""
    },
    {
      "provider": "ollama",
      "api_key": "ollama",
      "model": "llama3",
      "api_url": "http://192.168.1.100:11434/v1"
    }
  ]
}
```
