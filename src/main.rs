//! 甲虫 (beetle) - ESP32-S3 firmware entry.
//! Firmware version is embedded for OTA and ops.
//! Startup order: NVS → SPIFFS → config → WiFi → memory/session stores → MessageBus → self-check → cron/heartbeat/sinks/dispatch/CLI → agent_loop.
//! ESP32: no graceful shutdown; process runs until power off.
use beetle::config;
use beetle::memory::{MemoryStore, SessionStore};
#[cfg(all(
    feature = "feishu",
    any(target_arch = "xtensa", target_arch = "riscv32")
))]
use beetle::run_feishu_ws_loop;
use beetle::Platform;
use beetle::PlatformHttpClient;
use beetle::{
    get_bot_username, parse_allowed_chat_ids, poll_telegram_once, run_agent_loop,
    run_dingtalk_sender_loop, run_dispatch, run_feishu_sender_loop, run_qq_sender_loop,
    run_telegram_sender_loop, run_wecom_sender_loop, send_chat_action, AnthropicClient, AppConfig,
    ChannelSinks, CronTool, Esp32Platform, EspHttpClient, FallbackLlmClient, FetchUrlTool,
    FilesTool, GetTimeTool, MessageBus, OpenAiCompatibleClient, PcMsg, QueuedSink, RemindAtTool,
    ToolRegistry, UpdateSessionSummaryTool, WebSearchTool, WebSocketSink, DEFAULT_CAPACITY,
};

use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

const TAG: &str = "beetle";
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 各通道的 rx 及 flush 所需凭证，由 build_channel_sinks 填充；未启用通道为 None。
struct ChannelRxSet {
    telegram: Option<mpsc::Receiver<(String, String)>>,
    feishu: Option<FeishuRxConfig>,
    dingtalk: Option<DingtalkRxConfig>,
    wecom: Option<WecomRxConfig>,
    qq_channel: Option<QqChannelRxConfig>,
}
struct FeishuRxConfig {
    rx: mpsc::Receiver<(String, String)>,
    app_id: String,
    app_secret: String,
}
struct DingtalkRxConfig {
    rx: mpsc::Receiver<(String, String)>,
    webhook_url: String,
}
struct WecomRxConfig {
    rx: mpsc::Receiver<(String, String)>,
    corp_id: String,
    corp_secret: String,
    agent_id: String,
    default_touser: String,
}
struct QqChannelRxConfig {
    rx: mpsc::Receiver<(String, String)>,
    app_id: String,
    app_secret: String,
    msg_id_cache: beetle::channels::QqMsgIdCache,
}

/// 根据 config.enabled_channel 与凭证创建 ChannelSinks 并注册，返回 sinks 与各通道 rx 集合。
fn build_channel_sinks(
    config: &AppConfig,
    qq_msg_id_cache: &beetle::channels::QqMsgIdCache,
) -> (ChannelSinks, ChannelRxSet) {
    let mut sinks = ChannelSinks::new();
    let enabled = config.enabled_channel.as_str();

    let telegram = if enabled == "telegram" && !config.tg_token.trim().is_empty() {
        let (tx, rx) = mpsc::sync_channel(8);
        sinks.register(
            "telegram",
            Box::new(QueuedSink::new(tx, "telegram_send_queue")),
        );
        Some(rx)
    } else {
        None
    };

    let feishu = if enabled == "feishu"
        && !config.feishu_app_id.trim().is_empty()
        && !config.feishu_app_secret.trim().is_empty()
    {
        let (tx, rx) = mpsc::sync_channel(8);
        sinks.register("feishu", Box::new(QueuedSink::new(tx, "feishu_send_queue")));
        Some(FeishuRxConfig {
            rx,
            app_id: config.feishu_app_id.clone(),
            app_secret: config.feishu_app_secret.clone(),
        })
    } else {
        None
    };

    let dingtalk = if enabled == "dingtalk" && !config.dingtalk_webhook_url.trim().is_empty() {
        let (tx, rx) = mpsc::sync_channel(8);
        sinks.register(
            "dingtalk",
            Box::new(QueuedSink::new(tx, "dingtalk_send_queue")),
        );
        Some(DingtalkRxConfig {
            rx,
            webhook_url: config.dingtalk_webhook_url.clone(),
        })
    } else {
        None
    };

    let wecom = if enabled == "wecom"
        && !config.wecom_corp_id.trim().is_empty()
        && !config.wecom_corp_secret.trim().is_empty()
        && config.wecom_agent_id.trim().parse::<u32>().is_ok()
    {
        let (tx, rx) = mpsc::sync_channel(8);
        sinks.register("wecom", Box::new(QueuedSink::new(tx, "wecom_send_queue")));
        Some(WecomRxConfig {
            rx,
            corp_id: config.wecom_corp_id.clone(),
            corp_secret: config.wecom_corp_secret.clone(),
            agent_id: config.wecom_agent_id.clone(),
            default_touser: config.wecom_default_touser.clone(),
        })
    } else {
        None
    };

    let qq_channel = if enabled == "qq_channel"
        && !config.qq_channel_app_id.trim().is_empty()
        && !config.qq_channel_secret.trim().is_empty()
    {
        let (tx, rx) = mpsc::sync_channel(8);
        sinks.register(
            "qq_channel",
            Box::new(QueuedSink::new(tx, "qq_channel_send_queue")),
        );
        Some(QqChannelRxConfig {
            rx,
            app_id: config.qq_channel_app_id.clone(),
            app_secret: config.qq_channel_secret.clone(),
            msg_id_cache: Arc::clone(qq_msg_id_cache),
        })
    } else {
        None
    };

    sinks.register("websocket", Box::new(WebSocketSink::new("ws")));

    let rx_set = ChannelRxSet {
        telegram,
        feishu,
        dingtalk,
        wecom,
        qq_channel,
    };
    (sinks, rx_set)
}

/// 启动自检：存储可读（memory 或 soul 至少其一成功）。失败返回 false，调用方应 log 并 return。
fn startup_self_check(memory_store: &dyn MemoryStore) -> bool {
    memory_store.get_memory().is_ok() || memory_store.get_soul().is_ok()
}

/// 首次启动或空存储：当 get_memory 与 get_soul 均失败时写入占位数据，使后续自检可过、业务可进（如引导配置）。
fn ensure_storage_ready(memory_store: &dyn MemoryStore) {
    let need_defaults = memory_store.get_memory().is_err() && memory_store.get_soul().is_err();
    if !need_defaults {
        return;
    }
    log::info!(
        "[{}] first boot or empty storage: writing default memory/soul",
        TAG
    );
    if let Err(e) = memory_store.set_memory("") {
        log::warn!("[{}] set_memory default failed: {}", TAG, e);
    }
    if let Err(e) = memory_store.set_soul("") {
        log::warn!("[{}] set_soul default failed: {}", TAG, e);
    }
}

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    // 屏蔽 HTTP 服务器每个 URI 注册的 Info 日志，减少刷屏
    let _ = esp_idf_svc::log::set_target_level("esp_idf_svc::http::server", log::LevelFilter::Warn);
    log::info!("========================================");
    log::info!("  甲虫 beetle v{}", VERSION);
    log::info!("========================================");

    let platform: Arc<dyn Platform> = Arc::new(Esp32Platform::new());
    if let Err(e) = platform.init_nvs() {
        log::error!("[{}] nvs init failed: {}", TAG, e);
    } else {
        log::info!("[{}] NVS init ok", TAG);
    }

    let config_store = platform.config_store();
    let config_file_store = config::PlatformConfigFileStore(Arc::clone(&platform));
    if let Err(e) = platform.init_spiffs() {
        log::error!("[{}] SPIFFS init failed: {}", TAG, e);
        log::error!(
            "[{}] 请确认使用项目分区表烧录（partitions.csv 含 spiffs），否则无法启动",
            TAG
        );
        return;
    }
    log::info!("[{}] SPIFFS init ok", TAG);

    let config = AppConfig::load(config_store.as_ref(), Some(&config_file_store));
    if let Err(e) = config.validate_proxy() {
        log::warn!("[{}] config validate_proxy: {}", TAG, e);
    }
    if let Err(e) = config.validate_for_channels() {
        log::warn!("[{}] config validate_for_channels: {}", TAG, e);
    }
    log::info!(
        "[{}] config loaded (wifi_ssid set: {}, proxy set: {})",
        TAG,
        !config.wifi_ssid.is_empty(),
        !config.proxy_url.is_empty()
    );

    if !config.wifi_ssid.is_empty() {
        if let Err(e) = config.validate_for_wifi() {
            log::warn!("[{}] config validate_for_wifi: {}", TAG, e);
        }
    }
    let wifi_connected = match platform.connect_wifi(&config) {
        Ok(()) => {
            log::info!(
                "[{}] WiFi ready (SoftAP up, STA connected if configured)",
                TAG
            );
            beetle::platform::init_sntp();
            true
        }
        Err(e) => {
            log::warn!("[{}] WiFi failed: {}", TAG, e);
            false
        }
    };

    if let Err(e) = platform.init_mdns() {
        log::warn!("[{}] mDNS init failed: {}", TAG, e);
    } else {
        log::info!("[{}] mDNS hostname: beetle.local", TAG);
    }

    run_app(platform, Arc::new(config), wifi_connected);
}

/// 启动编排：存储与总线 → 自检 → 后台任务与通道 → agent 循环与 flush。与 main 解耦便于单文件内可读性。
fn run_app(platform: std::sync::Arc<dyn Platform>, config: Arc<AppConfig>, wifi_connected: bool) {
    let config_store = platform.config_store();
    let skill_storage = platform.skill_storage();
    let skill_meta_store = platform.skill_meta_store();
    let memory_store: Arc<dyn MemoryStore + Send + Sync> = platform.memory_store();
    ensure_storage_ready(memory_store.as_ref());
    if let Ok(s) = memory_store.get_memory() {
        log::info!("[{}] memory len={}", TAG, s.len());
    } else {
        log::warn!("[{}] memory read failed or empty", TAG);
    }
    if let Ok(s) = memory_store.get_soul() {
        log::info!("[{}] soul len={}", TAG, s.len());
    } else {
        log::warn!("[{}] soul read failed", TAG);
    }
    if let Ok(s) = memory_store.get_user() {
        log::info!("[{}] user len={}", TAG, s.len());
    } else {
        log::warn!("[{}] user read failed", TAG);
    }

    let session_store: Arc<dyn SessionStore + Send + Sync> = platform.session_store();
    let pending_retry_store: Arc<dyn beetle::memory::PendingRetryStore + Send + Sync> =
        platform.pending_retry_store();
    let task_continuation_store: Arc<dyn beetle::memory::TaskContinuationStore + Send + Sync> =
        platform.task_continuation_store();
    let important_message_store: Arc<dyn beetle::memory::ImportantMessageStore + Send + Sync> =
        platform.important_message_store();
    let remind_at_store: Arc<dyn beetle::memory::RemindAtStore + Send + Sync> =
        platform.remind_at_store();
    let session_summary_store: Arc<dyn beetle::memory::SessionSummaryStore + Send + Sync> =
        platform.session_summary_store();
    let emotion_signal_store = Arc::new(beetle::memory::MemoryEmotionSignalStore::new());

    let (bus, inbound_rx, outbound_rx) = MessageBus::new(DEFAULT_CAPACITY);
    log::info!(
        "[{}] MessageBus created (capacity {})",
        TAG,
        DEFAULT_CAPACITY
    );
    let inbound_depth = Arc::clone(&bus.inbound_depth);
    let outbound_depth = Arc::clone(&bus.outbound_depth);
    let inbound_tx = bus.inbound_tx;
    let outbound_tx = bus.outbound_tx;
    let qq_msg_id_cache: beetle::channels::QqMsgIdCache = Arc::new(Mutex::new(HashMap::new()));

    if !startup_self_check(memory_store.as_ref()) {
        log::error!(
            "[{}] startup self-check failed: storage not readable (get_memory and get_soul both failed)",
            TAG
        );
        return;
    }
    let wifi_status = if wifi_connected {
        "connected"
    } else {
        "disconnected"
    };
    let spiffs_info = platform
        .spiffs_usage()
        .map(|(total, used)| format!("{} free", total.saturating_sub(used)))
        .unwrap_or_else(|| "N/A".to_string());
    log::info!(
        "[{}] startup self-check ok (storage readable, wifi={}, spiffs={})",
        TAG,
        wifi_status,
        spiffs_info
    );
    #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
    beetle::platform::tls_admission::log_baseline();

    #[cfg(feature = "config_api")]
    {
        let platform_http = Arc::clone(&platform);
        let inc = Arc::clone(&inbound_depth);
        let out = Arc::clone(&outbound_depth);
        let memory_http = Arc::clone(&memory_store);
        let session_http = Arc::clone(&session_store);
        let w = wifi_connected;
        let http_inbound_tx = inbound_tx.clone();
        let http_qq_cache = Arc::clone(&qq_msg_id_cache);
        std::thread::spawn(move || {
            if let Err(e) = beetle::platform::http_server::run(
                platform_http,
                w,
                inc,
                out,
                memory_http,
                session_http,
                http_inbound_tx,
                http_qq_cache,
            ) {
                log::warn!("[{}] HTTP config API server error: {}", TAG, e);
            }
        });
        log::info!(
            "[{}] HTTP config API server started (SoftAP: 192.168.4.1)",
            TAG
        );
    }

    beetle::cron::run_cron_loop(inbound_tx.clone(), beetle::cron::DEFAULT_CRON_INTERVAL_SECS);
    beetle::heartbeat::run_heartbeat_loop_with_tasks(VERSION, 30, inbound_tx.clone(), || {
        beetle::platform::read_heartbeat_file().unwrap_or_default()
    });

    // 到点提醒：独立线程轮询 RemindAtStore，到点向 inbound_tx 注入 PcMsg。
    {
        let remind_tx = inbound_tx.clone();
        let remind_store = Arc::clone(&remind_at_store);
        std::thread::spawn(move || {
            const REMIND_POLL_SECS: u64 = 60;
            loop {
                std::thread::sleep(std::time::Duration::from_secs(REMIND_POLL_SECS));
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                while let Ok(Some((channel, chat_id, context))) = remind_store.pop_due(now) {
                    let content = format!("提醒：{}", context);
                    if let Ok(msg) = PcMsg::new(channel, chat_id, content) {
                        let _ = remind_tx.send(msg);
                    }
                }
            }
        });
        log::info!("[{}] remind_at loop started (interval {}s)", TAG, 60);
    }

    let (sinks, mut channel_rx_set) = build_channel_sinks(config.as_ref(), &qq_msg_id_cache);
    let sinks = Arc::new(sinks);
    let enabled_channel = config.enabled_channel.as_str();
    log::info!(
        "[{}] enabled_channel='{}' (use feishu + non-empty app_id/app_secret to start Feishu WS)",
        TAG,
        if enabled_channel.is_empty() {
            "(empty)"
        } else {
            enabled_channel
        }
    );

    #[cfg(all(
        feature = "feishu",
        any(target_arch = "xtensa", target_arch = "riscv32")
    ))]
    if let Some(ref c) = channel_rx_set.feishu {
        let tx = inbound_tx.clone();
        let id = c.app_id.clone();
        let sec = c.app_secret.clone();
        let allowed = parse_allowed_chat_ids(&config.feishu_allowed_chat_ids);
        std::thread::spawn(move || run_feishu_ws_loop(id, sec, allowed, tx));
        log::info!("[{}] Feishu WS loop started", TAG);
    } else if enabled_channel == "feishu" {
        #[cfg(all(
            feature = "feishu",
            any(target_arch = "xtensa", target_arch = "riscv32")
        ))]
        log::warn!(
            "[{}] Feishu WS not started: app_id or app_secret empty (check channels config)",
            TAG
        );
    }

    #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
    if enabled_channel == "qq_channel" {
        if let Some(ref c) = channel_rx_set.qq_channel {
            if !c.app_id.trim().is_empty() && !c.app_secret.trim().is_empty() {
                let qq_tx = inbound_tx.clone();
                let qq_id = c.app_id.clone();
                let qq_sec = c.app_secret.clone();
                std::thread::spawn(move || beetle::run_qq_ws_loop(qq_id, qq_sec, qq_tx));
                log::info!("[{}] QQ WS loop started", TAG);
            }
        }
    }

    // main 使用 platform 提供的 HTTP 客户端供 agent + flush；Telegram 轮询因连接非 Send，需在 spawn 内单独 new。
    if let Ok(mut http_client) = platform.create_http_client(config.as_ref()) {
        #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
        beetle::platform::task_wdt::register_current_task_to_task_wdt();
        beetle::resource::update();
        let outbound_rx_for_dispatch = outbound_rx;
        let sinks_clone = Arc::clone(&sinks);
        std::thread::spawn(move || run_dispatch(outbound_rx_for_dispatch, sinks_clone));

        if enabled_channel == "telegram" && !config.tg_token.trim().is_empty() {
            let tg_inbound_tx = inbound_tx.clone();
            let tg_outbound_tx = outbound_tx.clone();
            let tg_session_store = Arc::clone(&session_store);
            let tg_wifi = wifi_connected;
            let tg_inbound_depth = Arc::clone(&inbound_depth);
            let tg_outbound_depth = Arc::clone(&outbound_depth);
            let tg_token = config.tg_token.clone();
            let tg_allowed = parse_allowed_chat_ids(&config.tg_allowed_chat_ids);
            let tg_group_activation = config.tg_group_activation.clone();
            let tg_config_store = Arc::clone(&config_store);
            std::thread::spawn(move || {
                const TAG_TG: &str = "telegram_poll";
                #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
                beetle::platform::task_wdt::register_current_task_to_task_wdt();
                let cmd_ctx = beetle::channels::TelegramCommandCtx {
                    outbound_tx: tg_outbound_tx,
                    session_store: tg_session_store,
                    wifi_connected: tg_wifi,
                    inbound_depth: tg_inbound_depth,
                    outbound_depth: tg_outbound_depth,
                    set_group_activation: Box::new(move |v| {
                        beetle::config::write_tg_group_activation(tg_config_store.as_ref(), v)
                    }),
                };
                let mut http = match EspHttpClient::new() {
                    Ok(h) => h,
                    Err(e) => {
                        log::warn!("[{}] EspHttpClient::new failed: {}", TAG_TG, e);
                        return;
                    }
                };
                let bot_username = match get_bot_username(&mut http, &tg_token) {
                    Ok(Some(u)) => Some(u),
                    _ => None,
                };
                let mut offset: Option<i64> = None;
                const POLL_INTERVAL_SECS: u64 = 5;
                const BACKOFF_SECS: u64 = 30;
                loop {
                    match poll_telegram_once(
                        &mut http,
                        &tg_token,
                        offset,
                        &tg_inbound_tx,
                        &tg_allowed,
                        &tg_group_activation,
                        bot_username.as_deref(),
                        Some(&cmd_ctx),
                    ) {
                        Ok(next) => offset = next,
                        Err(e) => {
                            log::warn!(
                                "[{}] poll failed: {}, backoff {}s",
                                TAG_TG,
                                e,
                                BACKOFF_SECS
                            );
                            http.reset_connection_for_retry();
                            std::thread::sleep(std::time::Duration::from_secs(BACKOFF_SECS));
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS));
                }
            });
            log::info!("[{}] Telegram poll loop started", TAG);
        }

        let llm_clients: Vec<Box<dyn beetle::LlmClient>> = config
            .llm_sources
            .iter()
            .filter(|s| {
                let has_key = !s.api_key.trim().is_empty();
                let has_model = !s.model.trim().is_empty();
                let has_provider = !s.provider.trim().is_empty();
                let has_url = !s.api_url.trim().is_empty()
                    || s.provider == "openai"
                    || s.provider == "openai_compatible";
                has_key && has_model && has_provider && has_url
            })
            .map(|s| {
                let client: Box<dyn beetle::LlmClient> = match s.provider.as_str() {
                    "openai" | "openai_compatible" => {
                        Box::new(OpenAiCompatibleClient::from_source(s))
                    }
                    _ => Box::new(AnthropicClient::from_source(s)),
                };
                client
            })
            .collect();
        if llm_clients.is_empty() {
            log::warn!(
                "[{}] no valid llm source configured; using NoopLlmClient and skipping external LLM calls",
                TAG
            );
            log::info!(
                "[{}] LLM is in no-op mode: local tools and message processing remain available",
                TAG
            );
        }
        let n_sources = config.llm_sources.len();
        let (router_client, worker_llm_box): (
            Option<Box<dyn beetle::LlmClient>>,
            Box<dyn beetle::LlmClient>,
        ) = if llm_clients.is_empty() {
            (None, Box::new(beetle::llm::NoopLlmClient::new()))
        } else {
            let router_mode = config
                .llm_router_source_index
                .zip(config.llm_worker_source_index)
                .map_or(false, |(r, w)| {
                    (r as usize) < n_sources && (w as usize) < n_sources
                });
            let router_client: Option<Box<dyn beetle::LlmClient>> = if router_mode {
                let idx = config.llm_router_source_index.expect("router_mode true") as usize;
                let s = &config.llm_sources[idx];
                Some(match s.provider.as_str() {
                    "openai" | "openai_compatible" => {
                        Box::new(OpenAiCompatibleClient::from_source(s))
                            as Box<dyn beetle::LlmClient>
                    }
                    _ => Box::new(AnthropicClient::from_source(s)) as Box<dyn beetle::LlmClient>,
                })
            } else {
                None
            };
            (router_client, Box::new(FallbackLlmClient::new(llm_clients)))
        };
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(GetTimeTool));
        registry.register(Box::new(CronTool));
        registry.register(Box::new(FilesTool));
        registry.register(Box::new(WebSearchTool::new(config.as_ref())));
        registry.register(Box::new(FetchUrlTool));
        registry.register(Box::new(RemindAtTool::new(Arc::clone(&remind_at_store))));
        registry.register(Box::new(UpdateSessionSummaryTool::new(Arc::clone(
            &session_summary_store,
        ))));
        registry.register(Box::new(beetle::tools::BoardInfoTool));
        #[cfg(feature = "gpio")]
        {
            registry.register(Box::new(beetle::tools::GpioReadTool));
            registry.register(Box::new(beetle::tools::GpioWriteTool));
        }
        let tool_specs = registry.tool_specs_for_api(4096);
        let skill_meta_store_fn = Arc::clone(&skill_meta_store);
        let skill_storage_fn = Arc::clone(&skill_storage);
        let get_skill_descriptions: Box<dyn Fn() -> String + Send> = Box::new(move || {
            beetle::skills::build_skill_descriptions_for_system_prompt(
                skill_meta_store_fn.as_ref(),
                skill_storage_fn.as_ref(),
                8192,
            )
        });
        let session_max = config.session_max_messages.clamp(1, 128) as usize;
        let agent_inbound_tx = inbound_tx;
        let mut on_typing = |ch: &str, cid: &str, http: &mut _| {
            if ch == "telegram" {
                let _ = send_chat_action(http, &config.tg_token, cid, "typing");
            }
        };
        let agent_config = beetle::AgentLoopConfig {
            memory_store: memory_store.as_ref(),
            session_store: session_store.as_ref(),
            session_summary_store: session_summary_store.as_ref(),
            tool_specs: &tool_specs,
            get_skill_descriptions: &*get_skill_descriptions,
            session_max_messages: session_max,
            tg_group_activation: &config.tg_group_activation,
            task_continuation: task_continuation_store.as_ref(),
            task_continuation_max_rounds: 0u32,
            important_message_store: important_message_store.as_ref(),
            emotion_signal_store: emotion_signal_store.as_ref(),
            pending_retry: pending_retry_store.as_ref(),
        };
        #[cfg(feature = "cli")]
        {
            let cli_ctx = beetle::cli::CliContext::new(
                Arc::clone(&config),
                Arc::clone(&config_store),
                Arc::clone(&memory_store),
                Arc::clone(&session_store),
                wifi_connected,
                Some(Arc::clone(&inbound_depth)),
                Some(Arc::clone(&outbound_depth)),
            );
            std::thread::spawn(move || {
                let reader = std::io::BufReader::new(std::io::stdin());
                beetle::cli::run_repl(cli_ctx, reader);
            });
            log::info!("[{}] CLI REPL started (stdin)", TAG);
        }

        if let Some(tg_rx) = channel_rx_set.telegram.take() {
            let tg_send_token = config.tg_token.clone();
            std::thread::spawn(move || {
                run_telegram_sender_loop(tg_rx, &tg_send_token, || EspHttpClient::new());
            });
            log::info!("[{}] Telegram sender thread started", TAG);
        }
        if let Some(c) = channel_rx_set.feishu.take() {
            let fs_rx = c.rx;
            let fs_id = c.app_id;
            let fs_sec = c.app_secret;
            std::thread::spawn(move || {
                run_feishu_sender_loop(fs_rx, &fs_id, &fs_sec, || EspHttpClient::new());
            });
            log::info!("[{}] Feishu sender thread started", TAG);
        }
        if let Some(c) = channel_rx_set.dingtalk.take() {
            let dt_rx = c.rx;
            let dt_url = c.webhook_url;
            std::thread::spawn(move || {
                run_dingtalk_sender_loop(dt_rx, &dt_url, || EspHttpClient::new());
            });
            log::info!("[{}] DingTalk sender thread started", TAG);
        }
        if let Some(c) = channel_rx_set.wecom.take() {
            let wc_rx = c.rx;
            let wc_cid = c.corp_id;
            let wc_sec = c.corp_secret;
            let wc_aid = c.agent_id;
            let wc_usr = c.default_touser;
            std::thread::spawn(move || {
                run_wecom_sender_loop(wc_rx, &wc_cid, &wc_sec, &wc_aid, &wc_usr, || {
                    EspHttpClient::new()
                });
            });
            log::info!("[{}] WeCom sender thread started", TAG);
        }
        if let Some(c) = channel_rx_set.qq_channel.take() {
            let qq_rx = c.rx;
            let qq_id = c.app_id;
            let qq_sec = c.app_secret;
            let qq_cache = c.msg_id_cache;
            std::thread::spawn(move || {
                run_qq_sender_loop(qq_rx, &qq_id, &qq_sec, qq_cache, || EspHttpClient::new());
            });
            log::info!("[{}] QQ Channel sender thread started", TAG);
        }

        if let Err(e) = run_agent_loop(
            &mut http_client,
            router_client.as_deref(),
            &*worker_llm_box,
            &registry,
            &agent_config,
            agent_inbound_tx,
            inbound_rx,
            outbound_tx,
            Some(&mut on_typing),
        ) {
            log::warn!("[{}] agent loop error: {}", TAG, e);
            beetle::state::set_last_error(&e);
        }
    } else {
        log::warn!("[{}] create_http_client failed, agent not started", TAG);
    }

    loop {
        std::thread::sleep(std::time::Duration::from_secs(10));
        log::info!("[{}] running v{}", TAG, VERSION);
    }
}
