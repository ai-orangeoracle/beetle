//! 通道出站分片：按字符数或 UTF-8 字节数分片，供各通道 flush 使用。
//! Chunking helpers for channel outbound; shared to avoid code duplication.

/// 按最多 max_chars 个字符分片，不拆开多字节字符。迭代器实现，不中间分配 Vec<char>。
pub fn chunk_str_by_char_count(s: &str, max_chars: usize) -> Vec<String> {
    if max_chars == 0 {
        return vec![];
    }
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut count = 0usize;
    for c in s.chars() {
        if count >= max_chars {
            chunks.push(std::mem::take(&mut current));
            count = 0;
        }
        current.push(c);
        count += 1;
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

/// 按最多 max_bytes 个 UTF-8 字节分片，不拆开多字节字符。
pub fn chunk_str_by_utf8_bytes(s: &str, max_bytes: usize) -> Vec<String> {
    if s.is_empty() {
        return vec![];
    }
    if s.len() <= max_bytes {
        return vec![s.to_string()];
    }
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut current_len = 0usize;
    for c in s.chars() {
        let n = c.len_utf8();
        if current_len + n > max_bytes && !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
            current_len = 0;
        }
        current.push(c);
        current_len += n;
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}
