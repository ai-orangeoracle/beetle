//! 甲壳虫 (beetle) - ESP32-S3 firmware entry.
//! Firmware version is embedded for OTA and ops.
//! Startup order: NVS → SPIFFS → config → WiFi → memory/session stores → MessageBus → self-check → cron/heartbeat/sinks/dispatch/CLI → agent_loop.
//! ESP32: no graceful shutdown; process runs until power off.
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
use beetle::channels::connect_esp_wss;
use beetle::config;
use beetle::memory::{MemoryStore, SessionStore};
#[cfg(feature = "feishu")]
use beetle::run_feishu_ws_loop;
use beetle::Platform;
use beetle::PlatformHttpClient;
use beetle::{
    parse_allowed_chat_ids, run_agent_loop, run_dispatch, send_chat_action, AppConfig,
    DisplayChannelStatus, DisplayCommand, DisplayPressureLevel, DisplaySystemState, Esp32Platform,
    MessageBus, DEFAULT_CAPACITY,
};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const TAG: &str = "beetle";
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 从 orchestrator snapshot 的 internal 堆空闲字节数估算已用百分比。
/// ESP32-S3 internal DRAM 约 390KB；取 400KB 作为近似总量。非 ESP 返回 0。
fn heap_used_percent(snapshot: &beetle::orchestrator::ResourceSnapshot) -> u8 {
    const INTERNAL_TOTAL_APPROX: u32 = 400 * 1024;
    let free = snapshot.heap_free_internal;
    if free >= INTERNAL_TOTAL_APPROX {
        return 0;
    }
    let used = INTERNAL_TOTAL_APPROX.saturating_sub(free);
    ((used as u64 * 100) / INTERNAL_TOTAL_APPROX as u64).min(100) as u8
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

/// HTTP 工厂：与 `Platform::create_http_client` 一致（含代理），供流式编辑等独立连接使用。
type HttpFactory = Box<dyn Fn() -> beetle::Result<Box<dyn PlatformHttpClient>> + Send + Sync>;

/// Telegram 流式编辑器：LLM 流式输出期间，按需创建独立 HTTP 连接发送/编辑消息。
struct TelegramStreamEditor {
    token: String,
    create_http: HttpFactory,
}

impl beetle::StreamEditor for TelegramStreamEditor {
    fn send_initial(&self, chat_id: &str, content: &str) -> beetle::Result<Option<String>> {
        let mut http = (self.create_http)()?;
        beetle::tg_send_and_get_id(&mut http, &self.token, chat_id, content)
    }
    fn edit(&self, chat_id: &str, message_id: &str, content: &str) -> beetle::Result<()> {
        let mut http = (self.create_http)()?;
        beetle::tg_edit_message_text(&mut http, &self.token, chat_id, message_id, content)
    }
}

/// 飞书流式编辑器：按需获取 tenant_access_token 并发送/编辑消息。
struct FeishuStreamEditor {
    app_id: String,
    app_secret: String,
    create_http: HttpFactory,
}

impl beetle::StreamEditor for FeishuStreamEditor {
    fn send_initial(&self, chat_id: &str, content: &str) -> beetle::Result<Option<String>> {
        let mut http = (self.create_http)()?;
        let token = beetle::feishu_acquire_token(&mut http, &self.app_id, &self.app_secret)
            .ok_or_else(|| {
                beetle::Error::config("feishu_stream", "failed to acquire tenant_token")
            })?;
        beetle::feishu_send_and_get_id(&mut http, &token, chat_id, content)
    }
    fn edit(&self, _chat_id: &str, message_id: &str, content: &str) -> beetle::Result<()> {
        let mut http = (self.create_http)()?;
        let token = beetle::feishu_acquire_token(&mut http, &self.app_id, &self.app_secret)
            .ok_or_else(|| {
                beetle::Error::config("feishu_stream", "failed to acquire tenant_token")
            })?;
        beetle::feishu_edit_message(&mut http, &token, message_id, content)
    }
}

fn main() {
    let platform: Arc<dyn Platform> = Arc::new(Esp32Platform::new());
    if let Err(e) = platform.init() {
        // init 失败时日志可能未初始化，尝试 eprintln 兜底
        eprintln!("[{}] platform init failed: {}", TAG, e);
        log::error!("[{}] platform init failed: {}", TAG, e);
        return;
    }
    log::info!("========================================");
    log::info!("  甲壳虫 beetle v{}", VERSION);
    log::info!("========================================");

    let config_store = platform.config_store();
    let config_file_store = config::PlatformConfigFileStore(Arc::clone(&platform));

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
            platform.init_sntp();
            true
        }
        Err(e) => {
            log::warn!("[{}] WiFi failed: {}", TAG, e);
            false
        }
    };

    if let Some(display_cfg) = config.display.as_ref() {
        if display_cfg.enabled {
            if let Err(e) = platform.init_display(display_cfg) {
                log::warn!("[{}] display init failed (degraded): {}", TAG, e);
            } else {
                log::info!("[{}] display initialized", TAG);
                let _ = platform.display_command(DisplayCommand::RefreshDashboard {
                    state: DisplaySystemState::Booting,
                    wifi_connected: false,
                    ip_address: None,
                    channels: [
                        DisplayChannelStatus { name: "telegram", enabled: config.enabled_channel == "telegram", healthy: false },
                        DisplayChannelStatus { name: "feishu", enabled: config.enabled_channel == "feishu", healthy: false },
                        DisplayChannelStatus { name: "dingtalk", enabled: config.enabled_channel == "dingtalk", healthy: false },
                        DisplayChannelStatus { name: "wecom", enabled: config.enabled_channel == "wecom", healthy: false },
                        DisplayChannelStatus { name: "qq_channel", enabled: config.enabled_channel == "qq_channel", healthy: false },
                    ],
                    pressure: DisplayPressureLevel::Normal,
                    heap_percent: 0,
                });
            }
        }
    }
    if wifi_connected && platform.display_available() {
        let ip = platform
            .wifi_sta_ip()
            .unwrap_or_else(|| "192.168.4.1".to_string());
        let _ = platform.display_command(DisplayCommand::UpdateIp { ip });
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
    beetle::orchestrator::log_baseline();

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
        beetle::util::spawn_guarded("http_server", move || {
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
    beetle::heartbeat::run_heartbeat_loop_with_tasks(
        VERSION,
        30,
        inbound_tx.clone(),
        || beetle::platform::read_heartbeat_file().unwrap_or_default(),
        Arc::clone(&inbound_depth),
        Arc::clone(&outbound_depth),
        Arc::clone(&session_store),
    );

    if platform.display_available() {
        let display_platform = Arc::clone(&platform);
        let display_config = Arc::clone(&config);
        std::thread::Builder::new()
            .name("display".into())
            .stack_size(6144)
            .spawn(move || {
                let enabled = display_config.enabled_channel.as_str();
                // Dirty-region cache: skip SPI when nothing changed.
                let mut last_state: Option<DisplaySystemState> = None;
                let mut last_ip = String::new();
                let mut last_channels: [(bool, bool); 5] = [(false, false); 5];
                let mut last_pressure: Option<DisplayPressureLevel> = None;
                let mut last_heap: u8 = 255; // 255 forces first-round refresh
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    let snapshot = beetle::orchestrator::snapshot();
                    let pressure = match snapshot.pressure {
                        beetle::orchestrator::PressureLevel::Normal => DisplayPressureLevel::Normal,
                        beetle::orchestrator::PressureLevel::Cautious => {
                            DisplayPressureLevel::Cautious
                        }
                        beetle::orchestrator::PressureLevel::Critical => {
                            DisplayPressureLevel::Critical
                        }
                    };
                    let wifi_connected = beetle::platform::is_wifi_sta_connected();
                    let busy = snapshot.inbound_depth > 0
                        || snapshot.outbound_depth > 0
                        || snapshot.active_http_count > 0;
                    let state =
                        if snapshot.pressure == beetle::orchestrator::PressureLevel::Critical {
                            DisplaySystemState::Fault
                        } else if !wifi_connected {
                            DisplaySystemState::NoWifi
                        } else if busy {
                            DisplaySystemState::Busy
                        } else {
                            DisplaySystemState::Idle
                        };
                    let ip = display_platform
                        .wifi_sta_ip()
                        .unwrap_or_else(|| "192.168.4.1".to_string());
                    let channels = [
                        DisplayChannelStatus {
                            name: "telegram",
                            enabled: enabled == "telegram",
                            healthy: snapshot.channels.telegram.healthy,
                        },
                        DisplayChannelStatus {
                            name: "feishu",
                            enabled: enabled == "feishu",
                            healthy: snapshot.channels.feishu.healthy,
                        },
                        DisplayChannelStatus {
                            name: "dingtalk",
                            enabled: enabled == "dingtalk",
                            healthy: snapshot.channels.dingtalk.healthy,
                        },
                        DisplayChannelStatus {
                            name: "wecom",
                            enabled: enabled == "wecom",
                            healthy: snapshot.channels.wecom.healthy,
                        },
                        DisplayChannelStatus {
                            name: "qq_channel",
                            enabled: enabled == "qq_channel",
                            healthy: snapshot.channels.qq_channel.healthy,
                        },
                    ];
                    let heap_percent = heap_used_percent(&snapshot);

                    // State changed (icon + title area) → full refresh + update all cache
                    if last_state != Some(state) {
                        let cmd = DisplayCommand::RefreshDashboard {
                            state,
                            wifi_connected,
                            ip_address: Some(ip.clone()),
                            channels: channels.clone(),
                            pressure: pressure.clone(),
                            heap_percent,
                        };
                        if let Err(e) = display_platform.display_command(cmd) {
                            log::warn!("[{}] display refresh failed: {}", TAG, e);
                        }
                        last_state = Some(state);
                        last_ip.clear();
                        last_ip.push_str(&ip);
                        for (i, ch) in channels.iter().enumerate() {
                            last_channels[i] = (ch.enabled, ch.healthy);
                        }
                        last_pressure = Some(pressure);
                        last_heap = heap_percent;
                        continue;
                    }

                    // State unchanged → partial updates per dirty region
                    if last_ip.as_str() != ip.as_str() {
                        let _ = display_platform
                            .display_command(DisplayCommand::UpdateIp { ip: ip.clone() });
                        last_ip.clear();
                        last_ip.push_str(&ip);
                    }
                    let channels_changed = channels.iter().enumerate().any(|(i, ch)| {
                        last_channels[i] != (ch.enabled, ch.healthy)
                    });
                    if channels_changed {
                        let _ = display_platform.display_command(
                            DisplayCommand::UpdateChannels {
                                channels: channels.clone(),
                            },
                        );
                        for (i, ch) in channels.iter().enumerate() {
                            last_channels[i] = (ch.enabled, ch.healthy);
                        }
                    }
                    // 2% hysteresis on heap to avoid progress bar flicker
                    let pressure_changed = last_pressure.as_ref() != Some(&pressure);
                    let heap_changed = last_heap.abs_diff(heap_percent) >= 2;
                    if pressure_changed || heap_changed {
                        let _ = display_platform.display_command(
                            DisplayCommand::UpdatePressure {
                                level: pressure.clone(),
                                heap_percent,
                            },
                        );
                        last_pressure = Some(pressure);
                        last_heap = heap_percent;
                    }
                }
            })
            .ok();
    }

    beetle::memory::run_remind_loop(Arc::clone(&remind_at_store), inbound_tx.clone(), 60);

    let (sinks, mut channel_rx_set) =
        beetle::channels::build_channel_sinks(config.as_ref(), &qq_msg_id_cache);
    let sinks = Arc::new(sinks);
    let enabled_channel = config.enabled_channel.as_str();
    log::info!(
        "[{}] enabled_channel='{}'",
        TAG,
        if enabled_channel.is_empty() {
            "(none)"
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
        let pf = Arc::clone(&platform);
        let cfg = Arc::clone(&config);
        beetle::util::spawn_guarded("feishu_ws", move || {
            run_feishu_ws_loop(
                id,
                sec,
                allowed,
                tx,
                move || pf.create_http_client(cfg.as_ref()),
                connect_esp_wss,
            )
        });
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
                let qq_cache_ws = std::sync::Arc::clone(&qq_msg_id_cache);
                let pf = Arc::clone(&platform);
                let cfg = Arc::clone(&config);
                beetle::util::spawn_guarded("qq_ws", move || {
                    beetle::run_qq_ws_loop(
                        qq_id,
                        qq_sec,
                        qq_tx,
                        qq_cache_ws,
                        move || pf.create_http_client(cfg.as_ref()),
                        connect_esp_wss,
                    )
                });
                log::info!("[{}] QQ WS loop started", TAG);
            }
        }
    }

    // Agent / flush 与各通道工厂均经 `create_http_client`，与代理配置一致。
    if let Ok(mut http_client) = platform.create_http_client(config.as_ref()) {
        #[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
        beetle::platform::task_wdt::register_current_task_to_task_wdt();
        // 总控唯一入口：出站网络未就绪时不启动 WSS/通道/Agent 等对外请求，阻塞直到 STA 就绪（轮询+喂狗）
        beetle::platform::wait_for_network_ready();
        beetle::orchestrator::init();
        let outbound_rx_for_dispatch = outbound_rx;
        let sinks_clone = Arc::clone(&sinks);
        beetle::util::spawn_guarded("dispatch", move || run_dispatch(outbound_rx_for_dispatch, sinks_clone));

        if enabled_channel == "telegram" && !config.tg_token.trim().is_empty() {
            let tg_token = config.tg_token.clone();
            let tg_allowed = parse_allowed_chat_ids(&config.tg_allowed_chat_ids);
            let tg_group_activation = config.tg_group_activation.clone();
            let tg_inbound_tx = inbound_tx.clone();
            let tg_outbound_tx = outbound_tx.clone();
            let tg_session_store = Arc::clone(&session_store);
            let tg_wifi = wifi_connected;
            let tg_inbound_depth = Arc::clone(&inbound_depth);
            let tg_outbound_depth = Arc::clone(&outbound_depth);
            let tg_config_store = Arc::clone(&config_store);
            let pf = Arc::clone(&platform);
            let cfg = Arc::clone(&config);
            beetle::util::spawn_guarded("tg_poll", move || {
                beetle::run_telegram_poll_loop(
                    tg_token,
                    tg_allowed,
                    tg_group_activation,
                    tg_inbound_tx,
                    tg_outbound_tx,
                    tg_session_store,
                    tg_wifi,
                    tg_inbound_depth,
                    tg_outbound_depth,
                    tg_config_store,
                    move || pf.create_http_client(cfg.as_ref()),
                )
            });
            log::info!("[{}] Telegram poll loop started", TAG);
        }

        let (router_client, worker_llm_box) = beetle::build_llm_clients(&config);
        let registry = beetle::build_default_registry(
            &config,
            Arc::clone(&platform),
            Arc::clone(&remind_at_store),
            Arc::clone(&session_summary_store),
            Arc::clone(&session_store),
        );
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
        // 流式编辑器：根据 enabled_channel 选择对应通道的 StreamEditor 实现。
        enum StreamEditorImpl {
            Telegram(TelegramStreamEditor),
            Feishu(FeishuStreamEditor),
        }
        let stream_editor_impl: Option<StreamEditorImpl> = if config.llm_stream {
            let pf = Arc::clone(&platform);
            let cfg = Arc::clone(&config);
            let make_http: HttpFactory = Box::new(move || pf.create_http_client(cfg.as_ref()));
            match config.enabled_channel.as_str() {
                "telegram" if !config.tg_token.trim().is_empty() => {
                    Some(StreamEditorImpl::Telegram(TelegramStreamEditor {
                        token: config.tg_token.clone(),
                        create_http: make_http,
                    }))
                }
                "feishu" if !config.feishu_app_id.trim().is_empty() => {
                    Some(StreamEditorImpl::Feishu(FeishuStreamEditor {
                        app_id: config.feishu_app_id.clone(),
                        app_secret: config.feishu_app_secret.clone(),
                        create_http: make_http,
                    }))
                }
                _ => None,
            }
        } else {
            None
        };
        let stream_editor_ref: Option<&dyn beetle::StreamEditor> =
            stream_editor_impl.as_ref().map(|e| match e {
                StreamEditorImpl::Telegram(t) => t as &dyn beetle::StreamEditor,
                StreamEditorImpl::Feishu(f) => f as &dyn beetle::StreamEditor,
            });
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
            llm_stream: config.llm_stream,
            stream_editor: stream_editor_ref,
        };
        #[cfg(feature = "cli")]
        {
            let cli_ctx = beetle::cli::CliContext::new(
                Arc::clone(&config),
                Arc::clone(&config_store),
                Arc::clone(&memory_store),
                Arc::clone(&session_store),
                Arc::clone(&platform),
                wifi_connected,
                Some(Arc::clone(&inbound_depth)),
                Some(Arc::clone(&outbound_depth)),
            );
            beetle::util::spawn_guarded("cli_repl", move || {
                let reader = std::io::BufReader::new(std::io::stdin());
                beetle::cli::run_repl(cli_ctx, reader);
            });
            log::info!("[{}] CLI REPL started (stdin)", TAG);
        }

        let create_http: Arc<
            dyn Fn() -> beetle::Result<Box<dyn PlatformHttpClient>> + Send + Sync,
        > = Arc::new({
            let pf = Arc::clone(&platform);
            let cfg = Arc::clone(&config);
            move || pf.create_http_client(cfg.as_ref())
        });
        beetle::channels::spawn_sender_threads(&mut channel_rx_set, &config.tg_token, create_http);

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
