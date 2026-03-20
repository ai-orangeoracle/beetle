//! 编译时/环境变量配置，加载后校验；密钥与敏感字段永不打印、不写 SPIFFS。
//! NVS 仅存 6 个小键；LLM/通道存 SPIFFS，由 ConfigFileStore 读写。
//! Build-time / env config with validation; secrets never logged or written to SPIFFS.

use crate::error::{Error, Result};
use crate::display::{
    default_disabled_display_config, validate_display_config_core, DisplayConfig,
    DISPLAY_CONFIG_VERSION,
};
use crate::platform::ConfigStore;
use serde::{Deserialize, Serialize};

/// SPIFFS 配置文件读写，用于 config/llm.json、config/channels.json。由 Platform 实现。
pub trait ConfigFileStore: Send + Sync {
    fn read_config_file(&self, rel_path: &str) -> Result<Option<Vec<u8>>>;
    fn write_config_file(&self, rel_path: &str, data: &[u8]) -> Result<()>;
    fn remove_config_file(&self, rel_path: &str) -> Result<()>;
}

/// NVS 配置命名空间，与 platform::nvs 一致。若 NVS 中存在某 key 则 load 时覆盖 env 值。
pub const NVS_NAMESPACE: &str = "pc_cfg";

const NVS_KEY_WIFI_SSID: &str = "wifi_ssid";
const NVS_KEY_WIFI_PASS: &str = "wifi_pass";
const NVS_KEY_PROXY_URL: &str = "proxy_url";
/// ≤15 字符以符合 ESP-IDF NVS 键名上限。
const NVS_KEY_TG_GROUP_ACTIVATION: &str = "tg_grp_act";
const NVS_KEY_SESSION_MAX_MESSAGES: &str = "sess_max_msg";
/// 界面语言，单独 NVS 键；zh / en，默认 zh。
pub const NVS_KEY_LOCALE: &str = "locale";

/// 单键 NVS 最大长度（字节）；llm_sources JSON 超此长度时 save_to_nvs 返回错误。
pub const NVS_MAX_VALUE_LEN: usize = 512;

/// 配置字段长度上界（wifi/通道/LLM 单字段）。校验统一引用，避免魔法数。
pub const CONFIG_FIELD_MAX_LEN: usize = 64;
/// URL 类字段长度上界（如 dingtalk_webhook_url）。
pub const CONFIG_URL_MAX_LEN: usize = 512;

fn validate_field_len(s: &str, max: usize, field_name: &str) -> Result<()> {
    if s.len() > max {
        Err(Error::config(
            "config",
            format!("{} length must be <= {}", field_name, max),
        ))
    } else {
        Ok(())
    }
}

fn validate_url_len(s: &str, field_name: &str) -> Result<()> {
    validate_field_len(s, CONFIG_URL_MAX_LEN, field_name)
}
/// 企业微信 default_touser 长度上界。
pub const CONFIG_WECOM_TOUSER_MAX: usize = 128;
/// 会话条数合法范围。
pub const CONFIG_SESSION_MAX_MESSAGES_MIN: u32 = 1;
pub const CONFIG_SESSION_MAX_MESSAGES_MAX: u32 = 128;
/// LLM 源 api_url 长度上界。
pub const CONFIG_LLM_API_URL_MAX: usize = 256;

/// 单个 LLM 源配置；与原有 api_key/model/model_provider/api_url 同语义。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmSource {
    pub provider: String,
    pub api_key: String,
    pub model: String,
    pub api_url: String,
    /// 单次响应最大 token 数；None 时由各客户端使用内置默认值（1024）。
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

/// NVS 仅存 6 个小键；LLM/通道存 SPIFFS config/llm.json、config/channels.json，减少 4361。
pub(crate) const NVS_ALL_KEYS: &[&str] = &[
    NVS_KEY_WIFI_SSID,
    NVS_KEY_WIFI_PASS,
    NVS_KEY_PROXY_URL,
    NVS_KEY_SESSION_MAX_MESSAGES,
    NVS_KEY_TG_GROUP_ACTIVATION,
    NVS_KEY_LOCALE,
];

/// 应用配置。由 main 加载一次并通过参数下传；对外只暴露不可变结构体。
/// App config. Load once in main and pass by reference; immutable.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    // WiFi
    pub wifi_ssid: String,
    pub wifi_pass: String,

    // Telegram
    pub tg_token: String,
    /// 逗号分隔的 chat_id 白名单；空则拒绝所有入站。环境变量 BEETLE_TG_ALLOWED_CHAT_IDS。
    pub tg_allowed_chat_ids: String,

    // Feishu
    pub feishu_app_id: String,
    pub feishu_app_secret: String,
    /// 逗号分隔的 chat_id 白名单；空则拒绝所有入站。环境变量 BEETLE_FEISHU_ALLOWED_CHAT_IDS。
    pub feishu_allowed_chat_ids: String,

    /// 钉钉自定义机器人 Webhook 完整 URL；空则不出站。环境变量 BEETLE_DINGTALK_WEBHOOK_URL。
    pub dingtalk_webhook_url: String,

    /// 企业微信企业 ID；与 wecom_corp_secret、wecom_agent_id 均非空且 agent_id 可解析为 u32 时启用出站。
    pub wecom_corp_id: String,
    /// 企业微信应用凭证密钥。
    pub wecom_corp_secret: String,
    /// 企业微信应用 ID（整型，存为字符串）。
    pub wecom_agent_id: String,
    /// 默认接收人：userid 或 @all；出站时 chat_id 为空则用此值。
    pub wecom_default_touser: String,
    /// 企业微信回调 Token（用于签名校验，可选）。
    pub wecom_token: String,
    /// 企业微信回调 EncodingAESKey（用于消息加解密，可选）。
    pub wecom_encoding_aes_key: String,
    /// 钉钉回调 App Secret（用于验签，可选）。
    pub dingtalk_app_secret: String,

    /// QQ 频道机器人 App ID；与 qq_channel_secret 均非空时启用回调与出站。
    #[serde(default)]
    pub qq_channel_app_id: String,
    /// QQ 频道机器人 Bot Secret（用于 Ed25519 验签与 getAppAccessToken）。
    #[serde(default)]
    pub qq_channel_secret: String,

    // LLM
    pub api_key: String,
    pub model: String,
    pub model_provider: String,
    /// OpenAI 兼容端点 base URL，如 https://api.openai.com/v1；仅 model_provider 为 openai/openai_compatible 时使用，空则默认 OpenAI。
    pub api_url: String,

    /// 代理 URL，如 http://proxy.example.com:8080；留空直连。
    pub proxy_url: String,

    // Search
    pub search_key: String,
    pub tavily_key: String,

    /// 群组触发：mention = 仅被 @ 时回复；always = 每条都处理，无需回复时输出 SILENT。默认 mention。
    #[serde(default = "default_tg_group_activation")]
    pub tg_group_activation: String,
    /// 会话加载最近条数，1..=128，默认 32。
    #[serde(default = "default_session_max_messages")]
    pub session_max_messages: u32,

    /// Webhook 是否启用；与 webhook_token 配合，空 token 或 false 时拒绝 POST /api/webhook。
    #[serde(default)]
    pub webhook_enabled: bool,
    /// Webhook 校验 token；请求头 X-Webhook-Token 或 query token 需与此一致。
    #[serde(default)]
    pub webhook_token: String,

    /// 当前启用的通道（仅一个）："" | "telegram" | "feishu" | "dingtalk" | "wecom" | "qq_channel"。空表示不启用任何通道。
    #[serde(default)]
    pub enabled_channel: String,

    /// 多 LLM 源（回退顺序）；空时由 load 从 api_key/model/model_provider/api_url 构造单源。
    #[serde(default)]
    pub llm_sources: Vec<LlmSource>,

    /// 路由模式：路由用源下标；None 表示不启用路由，仅用 worker 回退链。
    #[serde(default)]
    pub llm_router_source_index: Option<u8>,
    /// 路由模式：worker 用源下标；与 router 同时为 Some 且均在 llm_sources 范围内时启用路由。
    #[serde(default)]
    pub llm_worker_source_index: Option<u8>,

    /// 界面语言 "zh" | "en"；存 NVS 键 locale，GET /api/config 与前端一致。
    #[serde(default)]
    pub locale: Option<String>,

    /// SSE 流式模式（全局）；true 时所有 LLM 客户端使用 SSE 逐块读取响应，降低峰值内存。默认 false。
    #[serde(default)]
    pub llm_stream: bool,

    /// 硬件设备配置（从 SPIFFS config/hardware.json 加载），不序列化到 NVS。
    #[serde(skip, default)]
    pub hardware_devices: Vec<DeviceEntry>,

    /// 显示配置（从 SPIFFS config/display.json 加载），不序列化到 NVS。
    #[serde(skip, default)]
    pub display: Option<DisplayConfig>,

    /// 加载过程中产生的可观测错误（NVS/SPIFFS/JSON 解析），仅 load() 内写入，不序列化。
    #[serde(skip, default)]
    pub load_errors: Option<Vec<String>>,
}

fn default_tg_group_activation() -> String {
    "mention".into()
}
fn default_session_max_messages() -> u32 {
    32
}

impl AppConfig {
    /// 从编译时环境变量加载。构建前可设置 e.g. BEETLE_WIFI_SSID。
    /// Load from compile-time env (option_env!). Set e.g. BEETLE_WIFI_SSID before build.
    pub fn load_from_env() -> Self {
        Self {
            wifi_ssid: option_env!("BEETLE_WIFI_SSID").unwrap_or("").into(),
            wifi_pass: option_env!("BEETLE_WIFI_PASS").unwrap_or("").into(),
            tg_token: option_env!("BEETLE_TG_TOKEN").unwrap_or("").into(),
            tg_allowed_chat_ids: option_env!("BEETLE_TG_ALLOWED_CHAT_IDS")
                .unwrap_or("")
                .into(),
            feishu_app_id: option_env!("BEETLE_FEISHU_APP_ID").unwrap_or("").into(),
            feishu_app_secret: option_env!("BEETLE_FEISHU_APP_SECRET").unwrap_or("").into(),
            feishu_allowed_chat_ids: option_env!("BEETLE_FEISHU_ALLOWED_CHAT_IDS")
                .unwrap_or("")
                .into(),
            dingtalk_webhook_url: option_env!("BEETLE_DINGTALK_WEBHOOK_URL")
                .unwrap_or("")
                .into(),
            wecom_corp_id: option_env!("BEETLE_WECOM_CORP_ID").unwrap_or("").into(),
            wecom_corp_secret: option_env!("BEETLE_WECOM_CORP_SECRET").unwrap_or("").into(),
            wecom_agent_id: option_env!("BEETLE_WECOM_AGENT_ID").unwrap_or("").into(),
            wecom_default_touser: option_env!("BEETLE_WECOM_DEFAULT_TOUSER")
                .unwrap_or("")
                .into(),
            wecom_token: option_env!("BEETLE_WECOM_TOKEN").unwrap_or("").into(),
            wecom_encoding_aes_key: option_env!("BEETLE_WECOM_ENCODING_AES_KEY")
                .unwrap_or("")
                .into(),
            dingtalk_app_secret: option_env!("BEETLE_DINGTALK_APP_SECRET")
                .unwrap_or("")
                .into(),
            api_key: option_env!("BEETLE_API_KEY").unwrap_or("").into(),
            model: option_env!("BEETLE_MODEL")
                .unwrap_or("claude-opus-4-5")
                .into(),
            model_provider: option_env!("BEETLE_MODEL_PROVIDER")
                .unwrap_or("anthropic")
                .into(),
            api_url: option_env!("BEETLE_API_URL").unwrap_or("").into(),
            proxy_url: option_env!("BEETLE_PROXY_URL").unwrap_or("").into(),
            search_key: option_env!("BEETLE_SEARCH_KEY").unwrap_or("").into(),
            tavily_key: option_env!("BEETLE_TAVILY_KEY").unwrap_or("").into(),
            tg_group_activation: match option_env!("BEETLE_TG_GROUP_ACTIVATION") {
                Some("always") => "always".into(),
                _ => "mention".into(),
            },
            session_max_messages: option_env!("BEETLE_SESSION_MAX_MESSAGES")
                .unwrap_or("32")
                .parse()
                .unwrap_or(32)
                .clamp(1, 128),
            webhook_enabled: option_env!("BEETLE_WEBHOOK_ENABLED")
                .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            webhook_token: option_env!("BEETLE_WEBHOOK_TOKEN").unwrap_or("").into(),
            enabled_channel: option_env!("BEETLE_ENABLED_CHANNEL").unwrap_or("").into(),
            qq_channel_app_id: option_env!("BEETLE_QQ_CHANNEL_APP_ID").unwrap_or("").into(),
            qq_channel_secret: option_env!("BEETLE_QQ_CHANNEL_SECRET").unwrap_or("").into(),
            llm_sources: vec![],
            llm_router_source_index: None,
            llm_worker_source_index: None,
            locale: option_env!("BEETLE_LOCALE")
                .filter(|s| *s == "zh" || *s == "en")
                .map(String::from),
            llm_stream: option_env!("BEETLE_LLM_STREAM")
                .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            hardware_devices: vec![],
            display: None,
            load_errors: None,
        }
    }

    /// 加载过程中产生的错误码列表（nvs_read_failed / spiffs_*_unavailable / *_json_invalid），供 health/diagnose 或日志可观测。
    pub fn load_errors(&self) -> &[String] {
        self.load_errors.as_deref().unwrap_or(&[])
    }

    /// 多源加载：先 load_from_env()，再 NVS 6 键覆盖，再可选从 reader 读 SPIFFS llm/channels 合并。
    pub fn load(store: &dyn ConfigStore, reader: Option<&dyn ConfigFileStore>) -> Self {
        let mut c = Self::load_from_env();
        let mut load_errors = Vec::new();
        let values = match store.read_strings(NVS_ALL_KEYS) {
            Ok(v) => v,
            Err(_) => {
                log::warn!("[config] NVS read_strings failed");
                load_errors.push("nvs_read_failed".into());
                Vec::new()
            }
        };
        let opt = |i: usize| values.get(i).and_then(|v| v.as_ref());
        // NVS 6 键：wifi_ssid, wifi_pass, proxy_url, session_max_messages, tg_group_activation, locale
        if let Some(s) = opt(0) {
            if !s.is_empty() {
                c.wifi_ssid = s.clone();
            }
        }
        if let Some(s) = opt(1) {
            if !s.is_empty() {
                c.wifi_pass = s.clone();
            }
        }
        if let Some(s) = opt(2) {
            if !s.is_empty() {
                c.proxy_url = s.clone();
            }
        }
        if let Some(s) = opt(3) {
            if let Ok(n) = s.parse::<u32>() {
                if (1..=128).contains(&n) {
                    c.session_max_messages = n;
                }
            }
        }
        if let Some(s) = opt(4) {
            if s == "mention" || s == "always" {
                c.tg_group_activation = s.clone();
            }
        }
        if let Some(s) = opt(5) {
            if s == "zh" || s == "en" {
                c.locale = Some(s.clone());
            }
        }
        if let Some(r) = reader {
            match r.read_config_file("config/llm.json") {
                Ok(Some(b)) => {
                    let s = String::from_utf8_lossy(&b);
                    c.merge_llm_from_json(&s, &mut load_errors);
                }
                Ok(None) => {
                    // 文件不存在属首次启动正常情况，不记为错误。
                }
                Err(_) => {
                    load_errors.push("spiffs_llm_read_error".into());
                }
            }
            match r.read_config_file("config/channels.json") {
                Ok(Some(b)) => {
                    let s = String::from_utf8_lossy(&b);
                    c.merge_channels_from_json(&s, &mut load_errors);
                }
                Ok(None) => {
                    // 文件不存在属首次启动正常情况，不记为错误。
                }
                Err(_) => {
                    load_errors.push("spiffs_channels_read_error".into());
                }
            }
            match r.read_config_file("config/hardware.json") {
                Ok(Some(b)) => {
                    let s = String::from_utf8_lossy(&b);
                    c.merge_hardware_from_json(&s, &mut load_errors);
                }
                Ok(None) => {}
                Err(_) => {
                    load_errors.push("spiffs_hardware_read_error".into());
                }
            }
            match r.read_config_file("config/display.json") {
                Ok(Some(b)) => {
                    let s = String::from_utf8_lossy(&b);
                    c.merge_display_from_json(&s, &mut load_errors);
                }
                Ok(None) => {}
                Err(_) => {
                    load_errors.push("spiffs_display_read_error".into());
                }
            }
        }
        if c.llm_sources.is_empty() {
            c.llm_sources = vec![LlmSource {
                provider: c.model_provider.clone(),
                api_key: c.api_key.clone(),
                model: c.model.clone(),
                api_url: c.api_url.clone(),
                max_tokens: None,
            }];
        }
        c.load_errors = if load_errors.is_empty() {
            None
        } else {
            Some(load_errors)
        };
        c
    }

    /// 从 SPIFFS 读到的 llm.json 字符串合并到当前 config（仅覆盖 LLM 相关字段）。
    pub fn merge_llm_from_json(&mut self, json: &str, errors: &mut Vec<String>) {
        match serde_json::from_str::<LlmSegment>(json) {
            Ok(seg) => {
                self.llm_stream = seg.llm_stream;
                if !seg.llm_sources.is_empty() {
                    self.llm_sources = seg.llm_sources.clone();
                    self.llm_router_source_index = seg.llm_router_source_index;
                    self.llm_worker_source_index = seg.llm_worker_source_index;
                    let first = &self.llm_sources[0];
                    self.api_key = first.api_key.clone();
                    self.model = first.model.clone();
                    self.model_provider = first.provider.clone();
                    self.api_url = first.api_url.clone();
                }
            }
            Err(e) => {
                log::warn!("[config] merge_llm_from_json parse failed: {}", e);
                errors.push("llm_json_invalid".into());
            }
        }
    }

    /// 从 SPIFFS 读到的 channels.json 字符串合并到当前 config（仅覆盖通道相关字段）。
    pub fn merge_channels_from_json(&mut self, json: &str, errors: &mut Vec<String>) {
        match serde_json::from_str::<ChannelsSegment>(json) {
            Ok(seg) => {
                self.tg_token = seg.tg_token;
                self.tg_allowed_chat_ids = seg.tg_allowed_chat_ids;
                self.feishu_app_id = seg.feishu_app_id;
                self.feishu_app_secret = seg.feishu_app_secret;
                self.feishu_allowed_chat_ids = seg.feishu_allowed_chat_ids;
                self.dingtalk_webhook_url = seg.dingtalk_webhook_url;
                self.wecom_corp_id = seg.wecom_corp_id;
                self.wecom_corp_secret = seg.wecom_corp_secret;
                self.wecom_agent_id = seg.wecom_agent_id;
                self.wecom_default_touser = seg.wecom_default_touser;
                self.wecom_token = seg.wecom_token;
                self.wecom_encoding_aes_key = seg.wecom_encoding_aes_key;
                self.dingtalk_app_secret = seg.dingtalk_app_secret;
                self.qq_channel_app_id = seg.qq_channel_app_id;
                self.qq_channel_secret = seg.qq_channel_secret;
                self.webhook_enabled = seg.webhook_enabled;
                self.webhook_token = seg.webhook_token;
                if is_valid_enabled_channel(seg.enabled_channel.as_str()) {
                    self.enabled_channel = seg.enabled_channel;
                }
            }
            Err(e) => {
                log::warn!("[config] merge_channels_from_json parse failed: {}", e);
                errors.push("channels_json_invalid".into());
            }
        }
    }

    /// 从 SPIFFS 读到的 hardware.json 字符串合并到当前 config（仅覆盖硬件设备列表）。
    /// 解析成功后校验；校验失败则不覆盖、保留空列表，并记录 hardware_validation_failed。
    pub fn merge_hardware_from_json(&mut self, json: &str, errors: &mut Vec<String>) {
        match serde_json::from_str::<HardwareSegment>(json) {
            Ok(seg) => {
                if let Err(e) = validate_hardware_segment(&seg) {
                    log::warn!("[config] merge_hardware_from_json validation failed: {}", e);
                    errors.push("hardware_validation_failed".into());
                    return;
                }
                self.hardware_devices = seg.hardware_devices;
            }
            Err(e) => {
                log::warn!("[config] merge_hardware_from_json parse failed: {}", e);
                errors.push("hardware_json_invalid".into());
            }
        }
    }

    /// 从 SPIFFS 读到的 display.json 字符串合并到当前 config。
    pub fn merge_display_from_json(&mut self, json: &str, errors: &mut Vec<String>) {
        match serde_json::from_str::<DisplayConfig>(json) {
            Ok(mut cfg) => {
                if cfg.version == 0 {
                    cfg.version = DISPLAY_CONFIG_VERSION;
                }
                if let Err(e) = validate_display_segment(&cfg, &self.hardware_devices) {
                    log::warn!("[config] merge_display_from_json validation failed: {}", e);
                    errors.push("display_validation_failed".into());
                    return;
                }
                self.display = Some(cfg);
            }
            Err(e) => {
                log::warn!("[config] merge_display_from_json parse failed: {}", e);
                errors.push("display_json_invalid".into());
            }
        }
    }
}

/// 将 Platform 转为 ConfigFileStore，供 load/save 使用。
pub struct PlatformConfigFileStore(pub std::sync::Arc<dyn crate::platform::Platform>);

impl ConfigFileStore for PlatformConfigFileStore {
    fn read_config_file(&self, rel_path: &str) -> Result<Option<Vec<u8>>> {
        self.0.read_config_file(rel_path)
    }
    fn write_config_file(&self, rel_path: &str, data: &[u8]) -> Result<()> {
        self.0.write_config_file(rel_path, data)
    }
    fn remove_config_file(&self, rel_path: &str) -> Result<()> {
        self.0.remove_config_file(rel_path)
    }
}

/// 清空配置区（store 内所有已知 key），重启后 load 仅来自 env。
pub fn reset_to_defaults(store: &dyn ConfigStore) -> Result<()> {
    store.erase_keys(NVS_ALL_KEYS)
}

/// 仅将 tg_group_activation 写入 store（供 Telegram /activation 命令使用）；value 仅允许 "mention" 或 "always"。
pub fn write_tg_group_activation(store: &dyn ConfigStore, value: &str) -> Result<()> {
    if value != "mention" && value != "always" {
        return Err(Error::config(
            "write_tg_group_activation",
            "value must be 'mention' or 'always'",
        ));
    }
    store.write_string(NVS_KEY_TG_GROUP_ACTIVATION, value)
}

impl AppConfig {
    /// 校验：启动 WiFi 前必须提供 SSID（密码可为空用于开放网络）。
    pub fn validate_for_wifi(&self) -> Result<()> {
        if self.wifi_ssid.is_empty() {
            return Err(Error::config("config", "wifi_ssid is required for WiFi"));
        }
        if self.wifi_ssid.len() > CONFIG_FIELD_MAX_LEN {
            return Err(Error::config(
                "config",
                format!("wifi_ssid length must be <= {}", CONFIG_FIELD_MAX_LEN),
            ));
        }
        if self.wifi_pass.len() > CONFIG_FIELD_MAX_LEN {
            return Err(Error::config(
                "config",
                format!("wifi_pass length must be <= {}", CONFIG_FIELD_MAX_LEN),
            ));
        }
        Ok(())
    }

    /// 校验：proxy_url 为空或形如 scheme://host 或 scheme://host:port。
    pub fn validate_proxy(&self) -> Result<()> {
        if self.proxy_url.trim().is_empty() {
            return Ok(());
        }
        parse_proxy_url_to_host_port(self.proxy_url.trim()).ok_or_else(|| {
            Error::config("config", "proxy_url must be empty or like http://host:port")
        })?;
        Ok(())
    }

    /// 启动期通道校验：enabled_channel 对应凭证非空且长度在界内；失败返回 Config 错误，不打印凭证。
    pub fn validate_for_channels(&self) -> Result<()> {
        let ch = self.enabled_channel.as_str();
        match ch {
            "telegram" => {
                if self.tg_token.trim().is_empty() {
                    return Err(Error::config(
                        "config",
                        "enabled_channel=telegram requires tg_token",
                    ));
                }
                if self.tg_token.len() > CONFIG_FIELD_MAX_LEN {
                    return Err(Error::config(
                        "config",
                        format!("tg_token length must be <= {}", CONFIG_FIELD_MAX_LEN),
                    ));
                }
            }
            "feishu" => {
                if self.feishu_app_id.trim().is_empty() || self.feishu_app_secret.trim().is_empty()
                {
                    return Err(Error::config(
                        "config",
                        "enabled_channel=feishu requires feishu_app_id and feishu_app_secret",
                    ));
                }
                if self.feishu_app_id.len() > CONFIG_FIELD_MAX_LEN
                    || self.feishu_app_secret.len() > CONFIG_FIELD_MAX_LEN
                {
                    return Err(Error::config(
                        "config",
                        format!("feishu field length must be <= {}", CONFIG_FIELD_MAX_LEN),
                    ));
                }
            }
            "dingtalk" => {
                if self.dingtalk_webhook_url.trim().is_empty() {
                    return Err(Error::config(
                        "config",
                        "enabled_channel=dingtalk requires dingtalk_webhook_url",
                    ));
                }
                if self.dingtalk_webhook_url.len() > CONFIG_URL_MAX_LEN {
                    return Err(Error::config(
                        "config",
                        format!(
                            "dingtalk_webhook_url length must be <= {}",
                            CONFIG_URL_MAX_LEN
                        ),
                    ));
                }
            }
            "wecom" => {
                if self.wecom_corp_id.trim().is_empty()
                    || self.wecom_corp_secret.trim().is_empty()
                    || self.wecom_agent_id.trim().is_empty()
                {
                    return Err(Error::config("config", "enabled_channel=wecom requires wecom_corp_id, wecom_corp_secret, wecom_agent_id"));
                }
                if self.wecom_agent_id.trim().parse::<u32>().is_err() {
                    return Err(Error::config(
                        "config",
                        "wecom_agent_id must be a valid u32",
                    ));
                }
            }
            "qq_channel" => {
                if self.qq_channel_app_id.trim().is_empty()
                    || self.qq_channel_secret.trim().is_empty()
                {
                    return Err(Error::config("config", "enabled_channel=qq_channel requires qq_channel_app_id and qq_channel_secret"));
                }
                if self.qq_channel_app_id.len() > CONFIG_FIELD_MAX_LEN
                    || self.qq_channel_secret.len() > CONFIG_FIELD_MAX_LEN
                {
                    return Err(Error::config(
                        "config",
                        format!(
                            "qq_channel field length must be <= {}",
                            CONFIG_FIELD_MAX_LEN
                        ),
                    ));
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// 从 proxy_url（如 http://host:8080）解析出 (host, port)，供 HTTP 客户端使用。
/// 拒绝 after_scheme 以 ':' 开头（如误配 http://:host）以免得到 host=":host" 导致底层 getaddrinfo 报错。
pub fn parse_proxy_url_to_host_port(url: &str) -> Option<(String, String)> {
    let url = url.trim();
    if url.is_empty() {
        return None;
    }
    let after_scheme = url.find("://").and_then(|i| url.get((i + 3)..))?;
    if after_scheme.is_empty() || after_scheme.starts_with(':') {
        return None;
    }
    let scheme = url.get(..url.find("://")?).unwrap_or("http");
    let default_port = match scheme {
        "https" => "443",
        "socks4" | "socks5" => "1080",
        _ => "80",
    };
    let (host, port) = if let Some(col) = after_scheme.rfind(':') {
        let (h, p) = after_scheme.split_at(col);
        let p = p.trim_start_matches(':');
        if h.is_empty() {
            return None;
        }
        if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) {
            (h, p.to_string())
        } else {
            (after_scheme, default_port.to_string())
        }
    } else {
        (after_scheme, default_port.to_string())
    };
    if host.is_empty() || host.starts_with(':') {
        return None;
    }
    Some((host.to_string(), port))
}

// 以下仍属 impl AppConfig（与 parse_proxy_url_to_host_port 并列的 impl 块继续）
impl AppConfig {
    /// 序列化为 JSON，供 GET /api/config 与 CLI 使用。
    /// NOTE: 含明文密钥，仅限本地 UI 使用，不得用于日志或公开接口。
    /// For local UI only; contains plaintext secrets.
    pub fn to_full_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|e| Error::config("serialize", e.to_string()))
    }

    /// 从 JSON 反序列化并校验（validate_for_wifi、validate_proxy、tg_group_activation、session_max_messages、llm_sources）。
    pub fn from_json_and_validate(body: &[u8]) -> Result<Self> {
        let mut c: AppConfig = serde_json::from_slice(body)
            .map_err(|e| Error::config("deserialize", e.to_string()))?;
        validate_field_len(&c.wifi_ssid, CONFIG_FIELD_MAX_LEN, "wifi_ssid")?;
        validate_field_len(&c.wifi_pass, CONFIG_FIELD_MAX_LEN, "wifi_pass")?;
        validate_field_len(&c.tg_token, CONFIG_FIELD_MAX_LEN, "tg_token")?;
        validate_field_len(&c.feishu_app_id, CONFIG_FIELD_MAX_LEN, "feishu_app_id")?;
        validate_field_len(
            &c.feishu_app_secret,
            CONFIG_FIELD_MAX_LEN,
            "feishu_app_secret",
        )?;
        validate_field_len(&c.api_key, CONFIG_FIELD_MAX_LEN, "api_key")?;
        validate_field_len(&c.search_key, CONFIG_FIELD_MAX_LEN, "search_key")?;
        validate_field_len(&c.tavily_key, CONFIG_FIELD_MAX_LEN, "tavily_key")?;
        validate_field_len(&c.webhook_token, CONFIG_FIELD_MAX_LEN, "webhook_token")?;
        validate_field_len(
            &c.qq_channel_app_id,
            CONFIG_FIELD_MAX_LEN,
            "qq_channel_app_id",
        )?;
        validate_field_len(
            &c.qq_channel_secret,
            CONFIG_FIELD_MAX_LEN,
            "qq_channel_secret",
        )?;
        validate_url_len(&c.dingtalk_webhook_url, "dingtalk_webhook_url")?;
        validate_field_len(&c.wecom_corp_id, CONFIG_FIELD_MAX_LEN, "wecom_corp_id")?;
        validate_field_len(
            &c.wecom_corp_secret,
            CONFIG_FIELD_MAX_LEN,
            "wecom_corp_secret",
        )?;
        validate_field_len(&c.wecom_agent_id, CONFIG_FIELD_MAX_LEN, "wecom_agent_id")?;
        validate_field_len(
            &c.wecom_default_touser,
            CONFIG_WECOM_TOUSER_MAX,
            "wecom_default_touser",
        )?;
        if c.llm_sources.is_empty() {
            c.llm_sources = vec![LlmSource {
                provider: c.model_provider.clone(),
                api_key: c.api_key.clone(),
                model: c.model.clone(),
                api_url: c.api_url.clone(),
                max_tokens: None,
            }];
        }
        validate_llm_sources(
            &c.llm_sources,
            c.llm_router_source_index,
            c.llm_worker_source_index,
        )?;
        if c.tg_group_activation != "mention" && c.tg_group_activation != "always" {
            return Err(Error::config(
                "config",
                "tg_group_activation must be 'mention' or 'always'",
            ));
        }
        if !(CONFIG_SESSION_MAX_MESSAGES_MIN..=CONFIG_SESSION_MAX_MESSAGES_MAX)
            .contains(&c.session_max_messages)
        {
            return Err(Error::config(
                "config",
                format!(
                    "session_max_messages must be {}..={}",
                    CONFIG_SESSION_MAX_MESSAGES_MIN, CONFIG_SESSION_MAX_MESSAGES_MAX
                ),
            ));
        }
        if !c.wifi_ssid.is_empty() {
            c.validate_for_wifi()?;
        }
        c.validate_proxy()?;
        Ok(c)
    }
}

/// 将配置按键名逐字段写入 store；单条 value 超 NVS_MAX_VALUE_LEN 返回错误。
/// 仅写入 NVS 保留的 6 个键；LLM/通道由 save_llm_segment / save_channels_segment 写 SPIFFS。
pub fn save_to_nvs(store: &dyn ConfigStore, config: &AppConfig) -> Result<()> {
    let session_str = config.session_max_messages.to_string();
    let locale = config.locale.as_deref().unwrap_or("zh");
    store.write_strings(&[
        (NVS_KEY_WIFI_SSID, &config.wifi_ssid),
        (NVS_KEY_WIFI_PASS, &config.wifi_pass),
        (NVS_KEY_PROXY_URL, &config.proxy_url),
        (NVS_KEY_SESSION_MAX_MESSAGES, &session_str),
        (NVS_KEY_TG_GROUP_ACTIVATION, &config.tg_group_activation),
        (NVS_KEY_LOCALE, locale),
    ])?;
    Ok(())
}

/// 仅将 WiFi SSID 与密码写入 store；用于配置页仅配 WiFi 场景，不要求 ssid 非空（空表示仅 AP）。
/// 校验：wifi_ssid.len()、wifi_pass.len() 均 ≤ CONFIG_FIELD_MAX_LEN。
pub fn save_wifi_to_nvs(store: &dyn ConfigStore, wifi_ssid: &str, wifi_pass: &str) -> Result<()> {
    if wifi_ssid.len() > CONFIG_FIELD_MAX_LEN {
        return Err(Error::config(
            "wifi",
            format!("wifi_ssid length must be <= {}", CONFIG_FIELD_MAX_LEN),
        ));
    }
    if wifi_pass.len() > CONFIG_FIELD_MAX_LEN {
        return Err(Error::config(
            "wifi",
            format!("wifi_pass length must be <= {}", CONFIG_FIELD_MAX_LEN),
        ));
    }
    store.write_strings(&[
        (NVS_KEY_WIFI_SSID, wifi_ssid),
        (NVS_KEY_WIFI_PASS, wifi_pass),
    ])?;
    Ok(())
}

/// 从 store 读取当前 locale；无或非法则返回 "zh"。
pub fn get_locale(store: &dyn ConfigStore) -> String {
    match store.read_string(NVS_KEY_LOCALE) {
        Ok(Some(s)) if s == "zh" || s == "en" => s,
        _ => "zh".to_string(),
    }
}

/// 写入 locale（仅接受 "zh" 或 "en"）。
pub fn set_locale(store: &dyn ConfigStore, locale: &str) -> Result<()> {
    let locale = locale.trim();
    if locale != "zh" && locale != "en" {
        return Err(Error::config("locale", "must be zh or en"));
    }
    store.write_string(NVS_KEY_LOCALE, locale)?;
    Ok(())
}

/// POST /api/config/llm 请求体。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmSegment {
    pub llm_sources: Vec<LlmSource>,
    #[serde(default)]
    pub llm_router_source_index: Option<u8>,
    #[serde(default)]
    pub llm_worker_source_index: Option<u8>,
    #[serde(default)]
    pub llm_stream: bool,
}

/// 允许的 enabled_channel 取值；空表示不启用任何通道。
pub const ALLOWED_ENABLED_CHANNELS: &[&str] =
    &["", "telegram", "feishu", "dingtalk", "wecom", "qq_channel"];

fn is_valid_enabled_channel(s: &str) -> bool {
    ALLOWED_ENABLED_CHANNELS.contains(&s)
}

/// POST /api/config/channels 请求体；仅通道相关字段。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelsSegment {
    #[serde(default)]
    pub enabled_channel: String,
    #[serde(default)]
    pub tg_token: String,
    #[serde(default)]
    pub tg_allowed_chat_ids: String,
    #[serde(default)]
    pub feishu_app_id: String,
    #[serde(default)]
    pub feishu_app_secret: String,
    #[serde(default)]
    pub feishu_allowed_chat_ids: String,
    #[serde(default)]
    pub dingtalk_webhook_url: String,
    #[serde(default)]
    pub wecom_corp_id: String,
    #[serde(default)]
    pub wecom_corp_secret: String,
    #[serde(default)]
    pub wecom_agent_id: String,
    #[serde(default)]
    pub wecom_default_touser: String,
    #[serde(default)]
    pub wecom_token: String,
    #[serde(default)]
    pub wecom_encoding_aes_key: String,
    #[serde(default)]
    pub dingtalk_app_secret: String,
    #[serde(default)]
    pub qq_channel_app_id: String,
    #[serde(default)]
    pub qq_channel_secret: String,
    #[serde(default)]
    pub webhook_enabled: bool,
    #[serde(default)]
    pub webhook_token: String,
}

/// POST /api/config/system 请求体。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemSegment {
    #[serde(default)]
    pub wifi_ssid: String,
    #[serde(default)]
    pub wifi_pass: String,
    #[serde(default)]
    pub proxy_url: String,
    #[serde(default)]
    pub session_max_messages: u32,
    #[serde(default)]
    pub tg_group_activation: String,
    #[serde(default)]
    pub locale: Option<String>,
}

// ── Hardware device config constants ──
const MAX_HARDWARE_DEVICES: usize = 8;
const MAX_PWM_DEVICES: usize = 4;
const HARDWARE_ID_MAX_LEN: usize = 32;
const HARDWARE_WHAT_MAX_LEN: usize = 128;
const HARDWARE_HOW_MAX_LEN: usize = 256;
const HARDWARE_PIN_MIN: i32 = 1;
const HARDWARE_PIN_MAX: i32 = 48;
const HARDWARE_FORBIDDEN_PINS: [i32; 4] = [0, 3, 45, 46]; // ESP32-S3 strapping
const HARDWARE_ADC1_PINS: std::ops::RangeInclusive<i32> = 1..=10;
const HARDWARE_PWM_FREQ_MIN: u32 = 1;
const HARDWARE_PWM_FREQ_MAX: u32 = 40_000;
const KNOWN_DEVICE_TYPES: [&str; 5] = ["gpio_out", "gpio_in", "pwm_out", "adc_in", "buzzer"];

/// 引脚配置：键为引脚角色（如 "pin"），值为 GPIO 编号。
pub type PinConfig = std::collections::HashMap<String, i32>;

/// 单个硬件设备条目。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceEntry {
    pub id: String,
    pub device_type: String,
    pub pins: PinConfig,
    pub what: String,
    pub how: String,
    #[serde(default)]
    pub options: serde_json::Value,
}

/// POST /api/config/hardware 请求体；硬件设备列表。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HardwareSegment {
    #[serde(default)]
    pub hardware_devices: Vec<DeviceEntry>,
}

/// 私有：校验 llm_sources 非空、字段长度、router/worker 下标。供 from_json_and_validate 与 save_llm_segment 复用。
fn validate_llm_sources(
    sources: &[LlmSource],
    router_index: Option<u8>,
    worker_index: Option<u8>,
) -> Result<()> {
    if sources.is_empty() {
        return Err(Error::config("config", "llm_sources must not be empty"));
    }
    for (i, s) in sources.iter().enumerate() {
        if s.api_key.len() > CONFIG_FIELD_MAX_LEN
            || s.provider.len() > CONFIG_FIELD_MAX_LEN
            || s.model.len() > CONFIG_FIELD_MAX_LEN
            || s.api_url.len() > CONFIG_LLM_API_URL_MAX
        {
            return Err(Error::config(
                "config",
                format!("llm_sources[{}] field length over limit", i),
            ));
        }
    }
    let n = sources.len();
    if let (Some(r), Some(w)) = (router_index, worker_index) {
        if (r as usize) >= n || (w as usize) >= n {
            return Err(Error::config(
                "config",
                "llm_router_source_index and llm_worker_source_index must be < llm_sources.len()",
            ));
        }
    }
    Ok(())
}

/// 私有：校验 ChannelsSegment 的 enabled_channel 与各字段长度。供 save_channels_segment 复用。
fn validate_channels_segment_fields(seg: &ChannelsSegment) -> Result<()> {
    if !is_valid_enabled_channel(seg.enabled_channel.as_str()) {
        return Err(Error::config(
            "config",
            "enabled_channel must be one of: empty, telegram, feishu, dingtalk, wecom, qq_channel",
        ));
    }
    if seg.tg_token.len() > CONFIG_FIELD_MAX_LEN
        || seg.feishu_app_secret.len() > CONFIG_FIELD_MAX_LEN
        || seg.feishu_app_id.len() > CONFIG_FIELD_MAX_LEN
        || seg.wecom_corp_id.len() > CONFIG_FIELD_MAX_LEN
        || seg.wecom_corp_secret.len() > CONFIG_FIELD_MAX_LEN
        || seg.wecom_agent_id.len() > CONFIG_FIELD_MAX_LEN
        || seg.qq_channel_app_id.len() > CONFIG_FIELD_MAX_LEN
        || seg.qq_channel_secret.len() > CONFIG_FIELD_MAX_LEN
    {
        return Err(Error::config(
            "config",
            format!("channel field length must be <= {}", CONFIG_FIELD_MAX_LEN),
        ));
    }
    if seg.dingtalk_webhook_url.len() > CONFIG_URL_MAX_LEN {
        return Err(Error::config(
            "config",
            format!(
                "dingtalk_webhook_url length must be <= {}",
                CONFIG_URL_MAX_LEN
            ),
        ));
    }
    if seg.wecom_default_touser.len() > CONFIG_WECOM_TOUSER_MAX {
        return Err(Error::config(
            "config",
            format!(
                "wecom_default_touser length must be <= {}",
                CONFIG_WECOM_TOUSER_MAX
            ),
        ));
    }
    Ok(())
}

/// 私有：校验 SystemSegment 的 wifi 长度、session 范围、tg_group_activation、proxy。供 save_system_segment_to_nvs 复用。
fn validate_system_segment_fields(seg: &SystemSegment) -> Result<()> {
    if seg.wifi_ssid.len() > CONFIG_FIELD_MAX_LEN || seg.wifi_pass.len() > CONFIG_FIELD_MAX_LEN {
        return Err(Error::config(
            "config",
            format!(
                "wifi_ssid and wifi_pass length must be <= {}",
                CONFIG_FIELD_MAX_LEN
            ),
        ));
    }
    if seg.tg_group_activation != "mention" && seg.tg_group_activation != "always" {
        return Err(Error::config(
            "config",
            "tg_group_activation must be 'mention' or 'always'",
        ));
    }
    if !(CONFIG_SESSION_MAX_MESSAGES_MIN..=CONFIG_SESSION_MAX_MESSAGES_MAX)
        .contains(&seg.session_max_messages)
    {
        return Err(Error::config(
            "config",
            format!(
                "session_max_messages must be {}..={}",
                CONFIG_SESSION_MAX_MESSAGES_MIN, CONFIG_SESSION_MAX_MESSAGES_MAX
            ),
        ));
    }
    if !seg.proxy_url.trim().is_empty()
        && parse_proxy_url_to_host_port(seg.proxy_url.trim()).is_none()
    {
        return Err(Error::config(
            "config",
            "proxy_url must be empty or like http://host:port",
        ));
    }
    Ok(())
}

/// 私有：校验 HardwareSegment 全部约束（设备数、ID、类型、引脚范围/冲突、PWM 频率等）。
fn validate_hardware_segment(seg: &HardwareSegment) -> Result<()> {
    if seg.hardware_devices.len() > MAX_HARDWARE_DEVICES {
        return Err(Error::config(
            "hardware",
            format!("hardware_devices count must be <= {}", MAX_HARDWARE_DEVICES),
        ));
    }
    let mut seen_ids = std::collections::HashSet::new();
    let mut seen_pins = std::collections::HashSet::new();
    let mut pwm_count: usize = 0;
    for (i, dev) in seg.hardware_devices.iter().enumerate() {
        // id
        if dev.id.is_empty() || dev.id.len() > HARDWARE_ID_MAX_LEN {
            return Err(Error::config(
                "hardware",
                format!(
                    "hardware_devices[{}].id must be 1..={} chars",
                    i, HARDWARE_ID_MAX_LEN
                ),
            ));
        }
        if !seen_ids.insert(&dev.id) {
            return Err(Error::config(
                "hardware",
                format!("hardware_devices[{}].id '{}' is duplicated", i, dev.id),
            ));
        }
        // device_type
        if !KNOWN_DEVICE_TYPES.contains(&dev.device_type.as_str()) {
            return Err(Error::config(
                "hardware",
                format!(
                    "hardware_devices[{}].device_type '{}' is not one of {:?}",
                    i, dev.device_type, KNOWN_DEVICE_TYPES
                ),
            ));
        }
        // what / how
        if dev.what.len() > HARDWARE_WHAT_MAX_LEN {
            return Err(Error::config(
                "hardware",
                format!(
                    "hardware_devices[{}].what length must be <= {}",
                    i, HARDWARE_WHAT_MAX_LEN
                ),
            ));
        }
        if dev.how.len() > HARDWARE_HOW_MAX_LEN {
            return Err(Error::config(
                "hardware",
                format!(
                    "hardware_devices[{}].how length must be <= {}",
                    i, HARDWARE_HOW_MAX_LEN
                ),
            ));
        }
        // pins: must have "pin" key
        let pin_val = dev.pins.get("pin").ok_or_else(|| {
            Error::config(
                "hardware",
                format!("hardware_devices[{}].pins must have a \"pin\" key", i),
            )
        })?;
        // validate all pin values
        for (role, &pv) in &dev.pins {
            if !(HARDWARE_PIN_MIN..=HARDWARE_PIN_MAX).contains(&pv) {
                return Err(Error::config(
                    "hardware",
                    format!(
                        "hardware_devices[{}].pins.{} = {} out of range {}..={}",
                        i, role, pv, HARDWARE_PIN_MIN, HARDWARE_PIN_MAX
                    ),
                ));
            }
            if HARDWARE_FORBIDDEN_PINS.contains(&pv) {
                return Err(Error::config(
                    "hardware",
                    format!(
                        "hardware_devices[{}].pins.{} = {} is a forbidden strapping pin",
                        i, role, pv
                    ),
                ));
            }
            if !seen_pins.insert(pv) {
                return Err(Error::config(
                    "hardware",
                    format!(
                        "pin {} is used by multiple devices (conflict at devices[{}].pins.{})",
                        pv, i, role
                    ),
                ));
            }
        }
        // adc_in: pin must be in ADC1 range
        if dev.device_type == "adc_in" && !HARDWARE_ADC1_PINS.contains(pin_val) {
            return Err(Error::config(
                "hardware",
                format!(
                    "hardware_devices[{}] adc_in pin {} must be in ADC1 range {:?}",
                    i, pin_val, HARDWARE_ADC1_PINS
                ),
            ));
        }
        // pwm_out count + frequency
        if dev.device_type == "pwm_out" {
            pwm_count += 1;
            if let Some(freq) = dev.options.get("frequency_hz").and_then(|v| v.as_u64()) {
                let freq = freq as u32;
                if !(HARDWARE_PWM_FREQ_MIN..=HARDWARE_PWM_FREQ_MAX).contains(&freq) {
                    return Err(Error::config(
                        "hardware",
                        format!(
                            "hardware_devices[{}] pwm_out frequency_hz {} must be {}..={}",
                            i, freq, HARDWARE_PWM_FREQ_MIN, HARDWARE_PWM_FREQ_MAX
                        ),
                    ));
                }
            }
        }
    }
    if pwm_count > MAX_PWM_DEVICES {
        return Err(Error::config(
            "hardware",
            format!(
                "pwm_out device count {} exceeds max {}",
                pwm_count, MAX_PWM_DEVICES
            ),
        ));
    }
    Ok(())
}

fn validate_pin_range(pin: i32) -> Result<()> {
    if !(HARDWARE_PIN_MIN..=HARDWARE_PIN_MAX).contains(&pin) {
        return Err(Error::config(
            "display",
            format!(
                "DISPLAY_CONFIG_INVALID_GPIO: pin {} out of range {}..={}",
                pin, HARDWARE_PIN_MIN, HARDWARE_PIN_MAX
            ),
        ));
    }
    if HARDWARE_FORBIDDEN_PINS.contains(&pin) {
        return Err(Error::config(
            "display",
            format!(
                "DISPLAY_CONFIG_INVALID_GPIO: pin {} is forbidden (strapping pin)",
                pin
            ),
        ));
    }
    Ok(())
}

fn collect_display_pins(cfg: &DisplayConfig) -> Vec<(String, i32)> {
    let mut out = vec![
        ("sclk".to_string(), cfg.spi.sclk),
        ("mosi".to_string(), cfg.spi.mosi),
        ("cs".to_string(), cfg.spi.cs),
        ("dc".to_string(), cfg.spi.dc),
    ];
    if let Some(v) = cfg.spi.rst {
        out.push(("rst".to_string(), v));
    }
    if let Some(v) = cfg.spi.bl {
        out.push(("bl".to_string(), v));
    }
    out
}

fn validate_display_segment(cfg: &DisplayConfig, hardware_devices: &[DeviceEntry]) -> Result<()> {
    validate_display_config_core(cfg)?;
    if !cfg.enabled {
        return Ok(());
    }

    let pins = collect_display_pins(cfg);
    let mut seen = std::collections::HashSet::new();
    for (name, pin) in &pins {
        validate_pin_range(*pin)?;
        if !seen.insert(*pin) {
            return Err(Error::config(
                "display",
                format!(
                    "DISPLAY_CONFIG_PIN_CONFLICT_INTERNAL: duplicate pin {} found at {}",
                    pin, name
                ),
            ));
        }
    }

    let mut external = std::collections::HashSet::new();
    for dev in hardware_devices {
        for pin in dev.pins.values() {
            external.insert(*pin);
        }
    }
    for (name, pin) in pins {
        if external.contains(&pin) {
            return Err(Error::config(
                "display",
                format!(
                    "DISPLAY_CONFIG_PIN_CONFLICT_EXTERNAL: display {} pin {} conflicts with hardware_devices",
                    name, pin
                ),
            ));
        }
    }
    Ok(())
}

/// 校验 LlmSegment 并写入 SPIFFS config/llm.json；body 即全量，不做合并。
pub fn save_llm_segment(writer: &dyn ConfigFileStore, body: &str) -> Result<()> {
    let seg: LlmSegment =
        serde_json::from_str(body).map_err(|e| Error::config("deserialize", e.to_string()))?;
    for (i, src) in seg.llm_sources.iter().enumerate() {
        if src.api_key.trim().is_empty() {
            return Err(Error::config(
                "config",
                format!("llm_sources[{}].api_key is required (cannot be empty)", i),
            ));
        }
    }
    validate_llm_sources(
        &seg.llm_sources,
        seg.llm_router_source_index,
        seg.llm_worker_source_index,
    )?;
    let json =
        serde_json::to_string(&seg).map_err(|e| Error::config("serialize", e.to_string()))?;
    writer.write_config_file("config/llm.json", json.as_bytes())?;
    Ok(())
}

/// 校验 ChannelsSegment 并写入 SPIFFS config/channels.json；body 即全量，不做合并。
pub fn save_channels_segment(writer: &dyn ConfigFileStore, body: &str) -> Result<()> {
    let seg: ChannelsSegment =
        serde_json::from_str(body).map_err(|e| Error::config("deserialize", e.to_string()))?;
    validate_channels_segment_fields(&seg)?;
    let json =
        serde_json::to_string(&seg).map_err(|e| Error::config("serialize", e.to_string()))?;
    writer.write_config_file("config/channels.json", json.as_bytes())?;
    Ok(())
}

/// 校验 SystemSegment 并写入对应 NVS 键；body 即全量，不做合并。
pub fn save_system_segment_to_nvs(store: &dyn ConfigStore, body: &str) -> Result<()> {
    let seg: SystemSegment =
        serde_json::from_str(body).map_err(|e| Error::config("deserialize", e.to_string()))?;
    validate_system_segment_fields(&seg)?;
    let session_str = seg.session_max_messages.to_string();
    let mut pairs: Vec<(&str, &str)> = vec![
        (NVS_KEY_WIFI_SSID, &seg.wifi_ssid),
        (NVS_KEY_WIFI_PASS, &seg.wifi_pass),
        (NVS_KEY_PROXY_URL, &seg.proxy_url),
        (NVS_KEY_SESSION_MAX_MESSAGES, &session_str),
        (NVS_KEY_TG_GROUP_ACTIVATION, &seg.tg_group_activation),
    ];
    if let Some(loc) = seg.locale.as_deref() {
        let loc = loc.trim();
        if loc == "zh" || loc == "en" {
            pairs.push((NVS_KEY_LOCALE, loc));
        }
    }
    store.write_strings(&pairs)?;
    Ok(())
}

/// 校验 HardwareSegment 并写入 SPIFFS config/hardware.json；body 即全量，不做合并。
pub fn save_hardware_segment(writer: &dyn ConfigFileStore, body: &str) -> Result<()> {
    let seg: HardwareSegment =
        serde_json::from_str(body).map_err(|e| Error::config("deserialize", e.to_string()))?;
    validate_hardware_segment(&seg)?;
    let json =
        serde_json::to_string(&seg).map_err(|e| Error::config("serialize", e.to_string()))?;
    writer.write_config_file("config/hardware.json", json.as_bytes())?;
    Ok(())
}

/// GET /api/config/display：返回 display.json 内容（不存在时返回 disabled 默认配置）。
pub fn get_display_segment(reader: &dyn ConfigFileStore) -> Result<String> {
    match reader.read_config_file("config/display.json")? {
        Some(b) => Ok(String::from_utf8_lossy(&b).into_owned()),
        None => serde_json::to_string(&default_disabled_display_config())
            .map_err(|e| Error::config("display", e.to_string())),
    }
}

/// POST /api/config/display：校验并写入 SPIFFS config/display.json；body 即全量，不做合并。
pub fn save_display_segment(
    writer: &dyn ConfigFileStore,
    hardware_devices: &[DeviceEntry],
    body: &str,
) -> Result<()> {
    let mut seg: DisplayConfig =
        serde_json::from_str(body).map_err(|e| Error::config("deserialize", e.to_string()))?;
    if seg.version == 0 {
        seg.version = DISPLAY_CONFIG_VERSION;
    }
    validate_display_segment(&seg, hardware_devices)?;
    let json = serde_json::to_string(&seg).map_err(|e| Error::config("serialize", e.to_string()))?;
    writer.write_config_file("config/display.json", json.as_bytes())?;
    Ok(())
}

/// 单条 ID 最大长度、白名单最大条数（避免滥用）。
/// 单条 chat_id 最大长度（飞书 oc_xxx 等可超过 32）。
const MAX_ALLOWED_ID_LEN: usize = 64;
const MAX_ALLOWED_COUNT: usize = 64;

/// 解析逗号分隔的 chat_id 白名单；空字符串返回空 vec；超长或超条数截断。
/// 约定：空列表 = 拒绝所有；非空 = 仅允许列表中的 chat_id。
pub fn parse_allowed_chat_ids(s: &str) -> Vec<String> {
    if s.trim().is_empty() {
        return vec![];
    }
    s.split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .take(MAX_ALLOWED_COUNT)
        .map(|x| {
            if x.len() > MAX_ALLOWED_ID_LEN {
                x.chars().take(MAX_ALLOWED_ID_LEN).collect()
            } else {
                x
            }
        })
        .collect()
}
