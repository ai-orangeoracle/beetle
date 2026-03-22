# LLM Provider Configuration

Beetle supports multiple LLM providers. Configure them in the web UI or via `config/llm.json`.

## Supported Providers

### OpenAI
- **Provider**: `openai`
- **Model examples**: `gpt-4`, `gpt-3.5-turbo`
- **API Key**: Get from [platform.openai.com](https://platform.openai.com)

### Anthropic (Claude)
- **Provider**: `anthropic`
- **Model examples**: `claude-3-5-sonnet-20241022`, `claude-3-haiku-20240307`
- **API Key**: Get from [console.anthropic.com](https://console.anthropic.com)

### Google Gemini
- **Provider**: `gemini`
- **Model examples**: `gemini-pro`, `gemini-1.5-flash`
- **API Key**: Get from [ai.google.dev](https://ai.google.dev)

### Zhipu GLM (智谱)
- **Provider**: `glm`
- **Model examples**: `glm-4`, `glm-4-flash`
- **API Key**: Get from [open.bigmodel.cn](https://open.bigmodel.cn)

### Qwen (通义千问)
- **Provider**: `qwen`
- **Model examples**: `qwen-turbo`, `qwen-plus`, `qwen-max`
- **API Key**: Get from [dashscope.aliyun.com](https://dashscope.aliyun.com)

### DeepSeek
- **Provider**: `deepseek`
- **Model examples**: `deepseek-chat`, `deepseek-coder`
- **API Key**: Get from [platform.deepseek.com](https://platform.deepseek.com)

### Moonshot
- **Provider**: `moonshot`
- **Model examples**: `moonshot-v1-8k`, `moonshot-v1-32k`, `moonshot-v1-128k`
- **API Key**: Get from [platform.moonshot.cn](https://platform.moonshot.cn)

### Ollama (Local)
- **Provider**: `ollama`
- **Model examples**: `llama3`, `qwen2`, `gemma2`
- **Setup**: Install Ollama on your local network, set API URL to `http://YOUR_IP:11434/v1`
- **API Key**: Any value (not validated)

## Configuration Examples

### Single Provider
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

### Multiple Providers (Automatic Fallback)
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

The device will try providers in order. If the first fails, it automatically falls back to the next.

**How it works:**
- Providers are tried in the order listed in `llm_sources`
- First successful response is returned immediately
- If all providers fail, the last error is returned

## Offline Mode

Add Ollama as the last fallback provider. When internet is down, the device will use your local model:

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
