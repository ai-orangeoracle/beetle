# Changelog

## [Unreleased]

### Added
- **LLM 提供商扩展**：新增 6 个 LLM 提供商支持
  - Google Gemini (`gemini`)
  - 智谱 GLM (`glm`)
  - 通义千问 (`qwen`)
  - DeepSeek (`deepseek`)
  - Moonshot/Kimi (`moonshot`)
  - Ollama 本地模型 (`ollama`)
- 所有新提供商通过 OpenAI 兼容 API 接入
- 每个提供商有默认 API 端点，`api_url` 留空时自动使用
- 新增 `docs/llm-providers.md` 配置指南

### Changed
- 更新 `src/llm/mod.rs` 的 `build_llm_clients` 函数以识别新提供商
- 更新 `src/llm/openai_compatible.rs` 的 `from_source` 方法以设置提供商默认端点
- 更新 README.md 和 README.zh-CN.md，添加支持的提供商列表

### Technical Details
- 提供商差距从 2 vs 14+/15+ 缩小到 8 vs 14+/15+
- 支持局域网 Ollama 作为离线回退源
- 所有修改遵循最小化原则，仅扩展现有架构
