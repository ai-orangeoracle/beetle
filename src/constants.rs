//! 资源上界单源；仅支持带 PSRAM 的 ESP32-S3，C3/S2 已移除。
//! Single source for resource bounds; only ESP32-S3 with PSRAM supported.

/// 入站/出站队列固定容量（条数）。
pub const DEFAULT_CAPACITY: usize = 8;

/// 单条消息 content 最大长度（字节）。
pub const MAX_CONTENT_LEN: usize = 64 * 1024;

/// HTTP 响应体最大读取字节数。
pub const MAX_RESPONSE_BODY_LEN: usize = 512 * 1024;

/// LLM 请求体最大字节数，与 MAX_RESPONSE_BODY_LEN 一致。
pub const MAX_REQUEST_BODY_LEN: usize = MAX_RESPONSE_BODY_LEN;

/// 系统提示总长度上界（字符）。
pub const DEFAULT_SYSTEM_MAX_LEN: usize = 32 * 1024;

/// 会话 messages 总长度上界（字符）。
pub const DEFAULT_MESSAGES_MAX_LEN: usize = 24 * 1024;

/// 进 agent 轮前要求的最小空闲堆（字节）。仅无 PSRAM 时使用（当前仅 S3 支持，恒有 PSRAM，此常量作防御保留）。
pub const MIN_FREE_HEAP_FOR_AGENT_ROUND: usize = 96 * 1024;

/// 有 PSRAM 时进 agent 轮前要求的最小 internal 堆（字节）。
/// TLS 操作已由 tls_admission 独立准入（56KB + 连续块检查），agent 门槛只需保证自身逻辑不 OOM。
/// S3 稳态 internal ~85KB；设 48KB 留足余量，不再替 TLS 站岗。
pub const MIN_FREE_INTERNAL_WHEN_PSRAM: usize = 48 * 1024;

/// TLS 准入：有 PSRAM 时允许发起单次 TLS（HTTP/WSS）要求的最小 internal 空闲（字节）。
/// S3 稳态 ~63KB，WSS TLS 常驻 ~1.5KB，agent HTTP TLS ~5KB；56KB 在双 TLS 并存时仅余 ~500B。
/// 有 PSRAM 时 mbedTLS 大部分分配走 SPIRAM，internal 仅需 ~15KB 给硬件加密/DMA，50KB 留足安全边际。
pub const TLS_ADMISSION_MIN_INTERNAL_BYTES: usize = 50 * 1024;
/// TLS 准入：要求 internal 最大连续块不低于此值，避免碎片化导致 mbedTLS 分配失败。
pub const TLS_ADMISSION_MIN_LARGEST_BLOCK_BYTES: usize = 24 * 1024;
/// TLS 准入：无 PSRAM 时 internal 堆空闲下限（字节），mbedTLS 全部走 internal 需更多空间。
pub const TLS_ADMISSION_NO_PSRAM_MIN_BYTES: usize = 72 * 1024;

/// 低内存且非 cron 时，重入队后休眠毫秒数，避免忙等、给 internal 恢复时间。
pub const LOW_MEM_DEFER_SLEEP_MS: u64 = 4000;

/// 工具结果拼成一条 user 消息时，user_content 部分的字节数上限（4 KiB）。
pub const MAX_TOOL_RESULTS_USER_MESSAGE_LEN: usize = 4 * 1024;

/// 多轮延续：单任务 last_output 最大长度（字节）。set 时由实现方截断。
pub const TASK_CONTINUATION_MAX_OUTPUT_LEN: usize = 4 * 1024;
/// 多轮延续：回复超过此长度或含 [CONTINUE] 时写回延续。
pub const TASK_CONTINUATION_CONTINUE_THRESHOLD_LEN: usize = 500;

/// Agent 结构化输出：模型回复含此时视为用户要求停止，固件终止当轮并只回确认。
pub const AGENT_MARKER_STOP: &str = "[STOP]";
/// Agent 结构化输出：固件将当轮 user 消息标为截断时优先保留。
pub const AGENT_MARKER_MARK_IMPORTANT: &str = "[MARK_IMPORTANT]";
/// Agent 结构化输出：固件在下轮 build_context 时注入情绪提示，随后清除。
pub const AGENT_MARKER_SIGNAL_COMFORT: &str = "[SIGNAL:comfort]";

/// remind_at 存储条目数上界；超过时实现应拒绝或淘汰最旧。
pub const REMIND_AT_MAX_ENTRIES: usize = 32;
/// remind_at 单条 context 最大字节数；实现应截断或拒绝超长。
pub const REMIND_AT_MAX_CONTEXT_LEN: usize = 512;

/// 会话摘要存贮与注入时截断上限（字符）。
pub const SESSION_SUMMARY_MAX_LEN: usize = 1024;

// ---------- 可靠性：超时与退避（须小于 TWDT 超时，避免静默复位） ----------
/// Agent 入站 recv 超时（秒）；超时后喂狗再继续等待。
pub const INBOUND_RECV_TIMEOUT_SECS: u64 = 30;
/// Agent 同一消息重试时退避基数（毫秒）；第 n 次重试 sleep(base * 2^n)，上限 AGENT_RETRY_MAX_MS。
pub const AGENT_RETRY_BASE_MS: u64 = 100;
/// Agent 重试退避上限（毫秒）。
pub const AGENT_RETRY_MAX_MS: u64 = 500;
/// pending_retry 重放次数上限；超过则清除不再注入，避免重复饥饿。
pub const PENDING_RETRY_MAX_REPLAY: u32 = 3;
/// Dispatch 单通道连续失败后熔断冷却时间（秒）；冷却期内不再向该通道发送。
pub const CHANNEL_FAIL_COOLDOWN_SECS: u64 = 60;
/// Dispatch 熔断阈值：连续失败此次数后进入冷却。
pub const CHANNEL_FAIL_THRESHOLD: u32 = 3;
