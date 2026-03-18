//! 按接口域拆分的 handler 逻辑；mod.rs 只做路由注册与配对检查，具体响应体由各子模块生成。

use crate::config::ConfigFileStore;
use crate::platform::{ConfigStore, Platform, SkillMetaStore, SkillStorage};
#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
use std::cell::RefCell;
#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
use crate::platform::EspHttpClient;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

/// ESP 目标上 EspHttpClient 非 Send，且 fn_handler 要求闭包 Send，故 Arc<HandlerContext> 须 Send+Sync；用 RwLock<Option<()>> 占位，fetch_url 内按需 new。
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
type HttpFetchStorage = std::sync::RwLock<Option<()>>;
#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
type HttpFetchStorage = RefCell<Option<EspHttpClient>>;

/// 各 handler 共享的只读上下文，由 run() 构建后以 Arc 传入闭包。配置以 config_store 为唯一源，按需 load。
#[allow(dead_code)]
pub struct HandlerContext {
    pub config_store: Arc<dyn ConfigStore + Send + Sync>,
    pub config_file_store: Arc<dyn ConfigFileStore + Send + Sync>,
    pub platform: Arc<dyn Platform>,
    pub memory_store: Arc<dyn crate::memory::MemoryStore + Send + Sync>,
    pub session_store: Arc<dyn crate::memory::SessionStore + Send + Sync>,
    pub skill_storage: Arc<dyn SkillStorage + Send + Sync>,
    pub skill_meta_store: Arc<dyn SkillMetaStore + Send + Sync>,
    pub inbound_depth: Arc<AtomicUsize>,
    pub outbound_depth: Arc<AtomicUsize>,
    pub wifi_connected: bool,
    pub version: Arc<str>,
    /// 板型 ID，与 board_presets.toml 一致，用于 OTA 渠道查 manifest。
    pub board_id: Arc<str>,
    /// 供 skills/import 等 GET 使用。非 ESP 复用单客户端；ESP 为占位，fetch_url 内按需 load config + create_http_client。
    pub http_for_fetch: HttpFetchStorage,
}

impl HandlerContext {
    /// 使用 http_for_fetch 客户端（或 ESP 上按需 load config + create_http_client）GET url，返回 body 截断至 max_len。
    pub fn fetch_url(&self, url: &str, max_len: usize) -> crate::error::Result<Vec<u8>> {
        #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
        {
            let config = crate::config::AppConfig::load(
                self.config_store.as_ref(),
                Some(self.config_file_store.as_ref()),
            );
            let mut client = self.platform.create_http_client(&config)?;
            let (status, mut body) = client.get(url, &[])?;
            let body_vec: Vec<u8> = body.into_vec();
            crate::platform::response::check_2xx_and_truncate("fetch_url", status, body_vec, max_len)
        }
        #[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
        {
            let mut opt = self.http_for_fetch.borrow_mut();
            let client = opt
                .as_mut()
                .ok_or_else(|| crate::error::Error::config("fetch_url", "no client"))?;
            let (status, mut body) = client.get(url, &[])?;
            let body_vec: Vec<u8> = body.into_vec();
            crate::platform::response::check_2xx_and_truncate("fetch_url", status, body_vec, max_len)
        }
    }
}

pub mod channel_connectivity;
pub mod config;
pub mod dingtalk_webhook;
pub mod feishu_event;
pub mod config_page;
pub mod config_reset;
pub mod diagnose;
pub mod health;
pub mod memory;
pub mod metrics;
pub mod pairing;
pub mod qq_webhook;
pub mod resource;
pub mod root;
pub mod sessions;
pub mod skills;
pub mod soul;
pub mod user;
pub mod restart;
pub mod system_info;
pub mod wecom_webhook;
pub mod webhook;
pub mod wifi_scan;

#[cfg(feature = "ota")]
pub mod ota;
