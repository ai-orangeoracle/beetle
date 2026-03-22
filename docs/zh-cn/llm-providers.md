# LLM 提供商配置

甲壳虫支持多个 LLM 提供商。在网页配置界面或通过 `config/llm.json` 文件配置。

## 支持的提供商

### OpenAI
- **提供商标识**: `openai`
- **模型示例**: `gpt-4`, `gpt-3.5-turbo`
- **API 密钥**: 从 [platform.openai.com](https://platform.openai.com) 获取

### Anthropic (Claude)
- **提供商标识**: `anthropic`
- **模型示例**: `claude-3-5-sonnet-20241022`, `claude-3-haiku-20240307`
- **API 密钥**: 从 [console.anthropic.com](https://console.anthropic.com) 获取

### Google Gemini
- **提供商标识**: `gemini`
- **模型示例**: `gemini-pro`, `gemini-1.5-flash`
- **API 密钥**: 从 [ai.google.dev](https://ai.google.dev) 获取

### 智谱 GLM
- **提供商标识**: `glm`
- **模型示例**: `glm-4`, `glm-4-flash`
- **API 密钥**: 从 [open.bigmodel.cn](https://open.bigmodel.cn) 获取

### 通义千问
- **提供商标识**: `qwen`
- **模型示例**: `qwen-turbo`, `qwen-plus`, `qwen-max`
- **API 密钥**: 从 [dashscope.aliyun.com](https://dashscope.aliyun.com) 获取

### DeepSeek
- **提供商标识**: `deepseek`
- **模型示例**: `deepseek-chat`, `deepseek-coder`
- **API 密钥**: 从 [platform.deepseek.com](https://platform.deepseek.com) 获取

### Moonshot
- **提供商标识**: `moonshot`
- **模型示例**: `moonshot-v1-8k`, `moonshot-v1-32k`, `moonshot-v1-128k`
- **API 密钥**: 从 [platform.moonshot.cn](https://platform.moonshot.cn) 获取

### Ollama（本地模型）
- **提供商标识**: `ollama`
- **模型示例**: `llama3`, `qwen2`, `gemma2`
- **设置方法**: 在局域网安装 Ollama，API 地址设为 `http://你的IP:11434/v1`
- **API 密钥**: 填任意值（不验证）

## 配置示例

### 单个提供商
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

### 多个提供商（自动回退）
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

设备会按顺序尝试提供商。如果第一个失败，会自动切换到下一个。

**工作原理：**
- 按 `llm_sources` 列表顺序依次尝试
- 第一个成功的响应会立即返回
- 如果全部失败，返回最后一个错误

## 离线模式

将 Ollama 作为最后的回退提供商。当互联网断开时，设备会使用你的本地模型：

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
