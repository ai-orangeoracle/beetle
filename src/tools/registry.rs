//! ToolRegistry：按 name 注册与查找，生成 API 用 tool specs。
//! ToolRegistry: register, get by name, tool_specs for API.

use crate::config::AppConfig;
use crate::error::{Error, Result};
use crate::llm::ToolSpec as LlmToolSpec;
use crate::tools::{Tool, MAX_TOOL_ARGS_LEN, MAX_TOOL_RESULT_LEN};
use crate::util::truncate_to_byte_len;
use indexmap::IndexMap;
use std::sync::Arc;

/// 按 name 注册与派发工具；可生成带总长度上界的 tool specs。IndexMap 保证工具顺序稳定。
pub struct ToolRegistry {
    tools: IndexMap<String, Box<dyn Tool>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: IndexMap::new(),
        }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|b| b.as_ref())
    }

    /// 该工具是否需要网络（从 Tool trait 推导）。未注册工具返回 false。
    /// Whether the named tool requires network (derived from Tool trait). Returns false for unknown tools.
    pub fn is_network_tool(&self, name: &str) -> bool {
        self.tools.get(name).is_some_and(|t| t.requires_network())
    }

    /// 生成供 LLM API 使用的 tool specs，总描述长度不超过 max_total_len（字符数）。
    /// 超限时从尾部丢弃工具。
    pub fn tool_specs_for_api(&self, max_total_len: usize) -> Vec<LlmToolSpec> {
        let mut out = Vec::new();
        let mut len = 0usize;
        for tool in self.tools.values() {
            let spec = LlmToolSpec {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: tool.schema(),
            };
            let add_len =
                spec.name.len() + spec.description.len() + spec.parameters.to_string().len() + 2;
            if len + add_len > max_total_len && !out.is_empty() {
                break;
            }
            len += add_len;
            out.push(spec);
        }
        out
    }

    /// 供阶段 7 注入系统提示：格式化为工具说明文本，总长度不超过 max_chars。
    pub fn format_descriptions_for_system_prompt(&self, max_chars: usize) -> String {
        let mut s = String::new();
        for tool in self.tools.values() {
            let line = format!("- {}: {}\n", tool.name(), tool.description());
            if s.len() + line.len() > max_chars {
                break;
            }
            s.push_str(&line);
        }
        s
    }

    /// 按 name 执行工具；args 超限返回 Error::Config；返回值在 Registry 内截断至 MAX_TOOL_RESULT_LEN。
    pub fn execute(
        &self,
        name: &str,
        args: &str,
        ctx: &mut dyn crate::tools::ToolContext,
    ) -> Result<String> {
        if args.len() > MAX_TOOL_ARGS_LEN {
            return Err(Error::config(
                "tool_execute",
                format!("args length exceeds {}", MAX_TOOL_ARGS_LEN),
            ));
        }
        let tool = self.get(name).ok_or_else(|| Error::Other {
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("tool not found: {}", name),
            )),
            stage: "tool_execute",
        })?;
        let result = tool.execute(args, ctx)?;
        Ok(truncate_to_byte_len(&result, MAX_TOOL_RESULT_LEN))
    }
}

/// 构建包含所有内置工具的注册表。`platform` 用于 `board_info` 等依赖平台能力的工具。
pub fn build_default_registry(
    config: &AppConfig,
    platform: Arc<dyn crate::platform::Platform>,
    remind_at_store: Arc<dyn crate::memory::RemindAtStore + Send + Sync>,
    session_summary_store: Arc<dyn crate::memory::SessionSummaryStore + Send + Sync>,
    session_store: Arc<dyn crate::memory::SessionStore + Send + Sync>,
    memory_store: Arc<dyn crate::memory::MemoryStore + Send + Sync>,
    config_store: Arc<dyn crate::platform::ConfigStore + Send + Sync>,
) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(super::GetTimeTool));
    registry.register(Box::new(super::FilesTool::new(platform.state_fs())));
    registry.register(Box::new(super::WebSearchTool::new(config)));
    registry.register(Box::new(super::AnalyzeImageTool::new(config)));
    let remind_at_store_for_list = Arc::clone(&remind_at_store);
    registry.register(Box::new(super::RemindAtTool::new(remind_at_store)));
    registry.register(Box::new(super::RemindListTool::new(
        remind_at_store_for_list,
    )));
    registry.register(Box::new(super::UpdateSessionSummaryTool::new(
        session_summary_store,
        Arc::clone(&session_store),
    )));
    registry.register(Box::new(super::BoardInfoTool::new(Arc::clone(&platform))));
    registry.register(Box::new(super::KvStoreTool::new(platform.state_fs())));
    if !config.hardware_devices.is_empty() {
        registry.register(Box::new(super::DeviceControlTool::new(
            config.hardware_devices.clone(),
            Arc::clone(&platform),
        )));
    }
    // --- New tools ---
    registry.register(Box::new(super::MemoryManageTool::new(Arc::clone(
        &memory_store,
    ))));
    registry.register(Box::new(super::HttpRequestTool));
    registry.register(Box::new(super::SessionManageTool::new(session_store)));
    registry.register(Box::new(super::FileWriteTool::new(platform.state_fs())));
    registry.register(Box::new(super::SystemControlTool::new(Arc::clone(
        &platform,
    ))));
    registry.register(Box::new(super::CronManageTool::new(Arc::clone(
        &memory_store,
    ))));
    registry.register(Box::new(super::ProxyConfigTool::new(config_store)));
    registry.register(Box::new(super::ModelConfigTool::new(Arc::clone(&platform))));
    registry.register(Box::new(super::NetworkScanTool::new(Arc::clone(&platform))));
    if !config.hardware_devices.is_empty() {
        registry.register(Box::new(super::SensorWatchTool::new(
            Arc::clone(&memory_store),
            config.hardware_devices.clone(),
        )));
    }
    if config.i2c_bus.is_some() && !config.i2c_devices.is_empty() {
        registry.register(Box::new(super::I2cDeviceTool::new(
            Arc::clone(&platform),
            config.i2c_devices.clone(),
        )));
    }
    // --- New tools ---
    registry.register(Box::new(super::MemoryManageTool::new(Arc::clone(
        &memory_store,
    ))));
    registry.register(Box::new(super::HttpRequestTool));
    registry.register(Box::new(super::SessionManageTool::new(session_store)));
    registry.register(Box::new(super::FileWriteTool));
    registry.register(Box::new(super::SystemControlTool::new(Arc::clone(
        &platform,
    ))));
    registry.register(Box::new(super::CronManageTool::new(Arc::clone(
        &memory_store,
    ))));
    registry.register(Box::new(super::ProxyConfigTool::new(config_store)));
    registry.register(Box::new(super::ModelConfigTool::new(Arc::clone(&platform))));
    registry.register(Box::new(super::NetworkScanTool::new(Arc::clone(&platform))));
    if !config.hardware_devices.is_empty() {
        registry.register(Box::new(super::SensorWatchTool::new(
            Arc::clone(&memory_store),
            config.hardware_devices.clone(),
        )));
    }
    if config.i2c_bus.is_some() && !config.i2c_devices.is_empty() {
        registry.register(Box::new(super::I2cDeviceTool::new(
            Arc::clone(&platform),
            config.i2c_devices.clone(),
        )));
    }
    registry
}
