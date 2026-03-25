//! SSE（Server-Sent Events）行解析器；共享于 Anthropic 和 OpenAI 两种 SSE 格式。
//! SSE line parser shared by Anthropic and OpenAI streaming formats.

use crate::constants::SSE_LINE_BUF_SIZE;
use std::collections::VecDeque;

/// 单条 SSE 事件（event + data）。
#[derive(Debug)]
pub struct SseEvent {
    /// event 类型（如 "message_start"、"content_block_delta"）；未指定时为空。
    pub event: String,
    /// data 字段内容（多 data 行以 \n 连接）。
    pub data: String,
}

/// SSE 行解析器：喂入原始字节 chunk，按 `\n\n` 分界提取 event/data。
/// 内部维护固定大小行缓冲，适合嵌入式低内存场景。
pub struct SseLineReader {
    buf: [u8; SSE_LINE_BUF_SIZE],
    pos: usize,
    /// 当前正在累积的 event 类型。
    current_event: String,
    /// 当前正在累积的 data 行。
    current_data: String,
    /// 已完成待取走的事件队列（通常只有 0-1 个）。
    pending: VecDeque<SseEvent>,
}

impl Default for SseLineReader {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for SseLineReader {
    fn default() -> Self {
        Self::new()
    }
}

impl SseLineReader {
    pub fn new() -> Self {
        Self {
            buf: [0u8; SSE_LINE_BUF_SIZE],
            pos: 0,
            current_event: String::new(),
            current_data: String::new(),
            pending: VecDeque::new(),
        }
    }

    /// 喂入一块原始字节；内部按行解析，完整事件放入 pending 队列。
    pub fn feed(&mut self, chunk: &[u8]) {
        for &b in chunk {
            if b == b'\n' {
                self.process_line();
                self.pos = 0;
            } else if b == b'\r' {
                // SSE 规范允许 \r\n 或 \r 作为行终止符；遇 \r 时处理当前行。
                // 后续 \n 会触发一个空行（即事件分隔符）。
                self.process_line();
                self.pos = 0;
            } else if self.pos < SSE_LINE_BUF_SIZE {
                self.buf[self.pos] = b;
                self.pos += 1;
                // 超出缓冲区的字节被丢弃；对正常 SSE 行（< 4KB）不会发生。
            }
        }
    }

    /// 取出下一个已完成的 SSE 事件；无则返回 None。
    pub fn next_event(&mut self) -> Option<SseEvent> {
        self.pending.pop_front()
    }

    /// 处理缓冲区中的一行（不含终止换行符）。
    fn process_line(&mut self) {
        let line = &self.buf[..self.pos];

        // 空行 = 事件分隔符：若有累积 data 则生成事件。
        if line.is_empty() {
            if !self.current_data.is_empty() || !self.current_event.is_empty() {
                self.pending.push_back(SseEvent {
                    event: core::mem::take(&mut self.current_event),
                    data: core::mem::take(&mut self.current_data),
                });
            }
            return;
        }

        // 注释行（以 ':' 开头）：忽略。
        if line[0] == b':' {
            return;
        }

        // 解析 field: value
        let line_str = match core::str::from_utf8(line) {
            Ok(s) => s,
            Err(_) => {
                log::debug!(
                    "[sse] skipping line with invalid UTF-8 ({} bytes)",
                    line.len()
                );
                return;
            }
        };
        if let Some(colon_pos) = line_str.find(':') {
            let field = &line_str[..colon_pos];
            // SSE 规范：冒号后的第一个空格是可选前缀，应跳过。
            let value_start = colon_pos + 1;
            let value = if line_str.as_bytes().get(value_start) == Some(&b' ') {
                &line_str[value_start + 1..]
            } else {
                &line_str[value_start..]
            };

            match field {
                "event" => {
                    self.current_event = value.to_string();
                }
                "data" => {
                    if !self.current_data.is_empty() {
                        self.current_data.push('\n');
                    }
                    self.current_data.push_str(value);
                }
                _ => {
                    // id, retry 等字段：SSE streaming 不使用，忽略。
                }
            }
        } else {
            // 无冒号的行：按 SSE 规范视为 field name = 整行, value = ""。忽略。
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_sse_event() {
        let mut reader = SseLineReader::new();
        reader.feed(b"event: message_start\ndata: {\"type\":\"message_start\"}\n\n");
        let ev = reader.next_event().unwrap();
        assert_eq!(ev.event, "message_start");
        assert_eq!(ev.data, "{\"type\":\"message_start\"}");
        assert!(reader.next_event().is_none());
    }

    #[test]
    fn test_multiple_events() {
        let mut reader = SseLineReader::new();
        reader.feed(b"data: hello\n\ndata: world\n\n");
        let ev1 = reader.next_event().unwrap();
        assert_eq!(ev1.data, "hello");
        let ev2 = reader.next_event().unwrap();
        assert_eq!(ev2.data, "world");
    }

    #[test]
    fn test_multi_data_lines() {
        let mut reader = SseLineReader::new();
        reader.feed(b"data: line1\ndata: line2\n\n");
        let ev = reader.next_event().unwrap();
        assert_eq!(ev.data, "line1\nline2");
    }

    #[test]
    fn test_comment_ignored() {
        let mut reader = SseLineReader::new();
        reader.feed(b": this is a comment\ndata: actual\n\n");
        let ev = reader.next_event().unwrap();
        assert_eq!(ev.data, "actual");
    }

    #[test]
    fn test_partial_chunks() {
        let mut reader = SseLineReader::new();
        reader.feed(b"data: hel");
        assert!(reader.next_event().is_none());
        reader.feed(b"lo\n\n");
        let ev = reader.next_event().unwrap();
        assert_eq!(ev.data, "hello");
    }

    #[test]
    fn test_done_marker() {
        let mut reader = SseLineReader::new();
        reader.feed(b"data: [DONE]\n\n");
        let ev = reader.next_event().unwrap();
        assert_eq!(ev.data, "[DONE]");
    }
}
