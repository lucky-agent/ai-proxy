//! 流式 SSE 分帧 + 分发。
//!
//! TCP chunk 边界 ≠ SSE event 边界，故持有内部字节 `buffer`：`feed` 追加原始
//! 字节后按空行（`\n\n`）切出完整 event 逐个消费，剩余不完整片段留在 buffer
//! 等下次。行尾归一化与分帧都在字节层进行，UTF-8 转换推迟到完整 event 切出
//! 之后——多字节字符或 `\r\n` 被 TCP 切割在两个 chunk 时不会被破坏。
//! 具体协议的状态机通过 `StreamState` trait 回调。

use super::{AiConversation, Provider, StreamState};

/// 单个 event 的 buffer 上限（4 MiB）：正常 SSE event 远小于此，超限说明上游
/// 并非按 SSE 分帧（如误标 content-type 的长流），丢弃防止无界增长——
/// 仅影响旁路观测，不影响字节透传。
const MAX_EVENT_BUFFER: usize = 4 * 1024 * 1024;

/// 有状态流式解析器。每个 AI 流式响应对应一个实例。
pub(crate) struct StreamParser {
    /// 原始字节缓冲，行尾已在追加时归一化为 `\n`（见 [`Self::feed`]）。
    buffer: Vec<u8>,
    /// 已扫描且确认无空行的前缀长度（下轮从此处继续，避免重复扫描）。
    scan_from: usize,
    /// 上一字节是否为 `\r`——`\r\n` 跨 chunk 切割时用于吞掉后半的 `\n`。
    last_was_cr: bool,
    state: Box<dyn StreamState>,
}

impl StreamParser {
    pub(crate) fn new(provider: Provider) -> Self {
        StreamParser {
            buffer: Vec::new(),
            scan_from: 0,
            last_was_cr: false,
            state: provider.create_stream_state(),
        }
    }

    /// 喂入一段原始响应字节，消费其中的完整 SSE event，更新内部状态。
    /// 返回本次是否有新内容被消费。
    pub(crate) fn feed(&mut self, bytes: &[u8]) -> bool {
        // 行尾归一化（字节层，状态跨 chunk 存活）：\r\n → \n，孤立 \r → \n。
        // UTF-8 多字节序列的所有字节 ≥ 0x80，不含 \r/\n，逐字节处理不会切坏字符。
        for &b in bytes {
            match b {
                b'\r' => {
                    self.buffer.push(b'\n');
                    self.last_was_cr = true;
                }
                // \r\n 的后半：\r 时已写入 \n，此处吞掉
                b'\n' if self.last_was_cr => self.last_was_cr = false,
                _ => {
                    self.buffer.push(b);
                    self.last_was_cr = false;
                }
            }
        }

        let mut consumed = false;
        // 按空行切出完整 event，此时才做 UTF-8 转换（字符必然完整）
        while let Some(rel) = find_double_newline(&self.buffer[self.scan_from..]) {
            let idx = self.scan_from + rel;
            let event_bytes: Vec<u8> = self.buffer.drain(..idx + 2).collect();
            self.scan_from = 0;
            let raw = String::from_utf8_lossy(&event_bytes[..idx]);
            if let Some((event, data)) = parse_sse_block(&raw) {
                self.state.apply(&event, &data);
                consumed = true;
            }
        }
        // 尾字节可能与下个 chunk 的 \n 配对成空行，回退 1 字节重扫
        self.scan_from = self.buffer.len().saturating_sub(1);

        if self.buffer.len() > MAX_EVENT_BUFFER {
            log::warn!("[ai-stream] event buffer exceeded {MAX_EVENT_BUFFER} bytes, dropped");
            self.buffer.clear();
            self.scan_from = 0;
        }
        consumed
    }

    /// 流结束：处理 buffer 中残留的最后一个 event（无尾随空行的情况），标记定稿。
    pub(crate) fn finalize(&mut self) {
        let leftover = std::mem::take(&mut self.buffer);
        self.scan_from = 0;
        let raw = String::from_utf8_lossy(&leftover);
        if !raw.trim().is_empty() {
            if let Some((event, data)) = parse_sse_block(&raw) {
                self.state.apply(&event, &data);
            }
        }
        self.state.finalize();
    }

    /// 当前累积的归一化对话快照。
    pub(crate) fn snapshot(&self) -> AiConversation {
        self.state.snapshot()
    }
}

/// 找最早的 `\n\n`（event 分隔空行）起始下标。
fn find_double_newline(buf: &[u8]) -> Option<usize> {
    buf.windows(2).position(|w| w == b"\n\n")
}

/// 解析一个 SSE event 块（多行），返回 `(event, data)`。
fn parse_sse_block(raw: &str) -> Option<(String, String)> {
    let mut event = String::new();
    let mut data = String::new();
    let mut has_data = false;
    for line in raw.split('\n') {
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        let (field, value) = match line.find(':') {
            Some(i) => {
                let v = &line[i + 1..];
                (&line[..i], v.strip_prefix(' ').unwrap_or(v))
            }
            None => (line, ""),
        };
        match field {
            "event" => event = value.to_string(),
            "data" => {
                if has_data {
                    data.push('\n');
                }
                data.push_str(value);
                has_data = true;
            }
            _ => {}
        }
    }
    if !has_data {
        return None;
    }
    Some((
        if event.is_empty() { "message".into() } else { event },
        data,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy::ai::AiContentBlock;

    /// 取快照 assistant turn 的全部文本。
    fn text_of(parser: &StreamParser) -> String {
        parser.snapshot().turns[0]
            .content
            .iter()
            .filter_map(|b| match b {
                AiContentBlock::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    // ── 基础分帧 ──────────────────────────────────────────────────────────────

    #[test]
    fn feeds_complete_openai_events() {
        let mut p = StreamParser::new(Provider::OpenAiChat);
        assert!(p.feed(b"data: {\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\n\n"));
        assert!(p.feed(b"data: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}\n\n"));
        assert_eq!(text_of(&p), "Hello");
    }

    #[test]
    fn event_split_across_feeds_reassembles() {
        let mut p = StreamParser::new(Provider::OpenAiChat);
        // 同一 event 分两次 feed（切在 ASCII 边界）
        assert!(!p.feed(b"data: {\"choices\":[{\"delta\":{\"con"));
        assert!(p.feed(b"tent\":\"hi\"}}]}\n\n"));
        assert_eq!(text_of(&p), "hi");
    }

    /// 多行 data: 按 SSE 规范以 \n 连接后再解析。
    #[test]
    fn multiline_data_joined_with_newline() {
        let mut p = StreamParser::new(Provider::OpenAiChat);
        p.feed(b"data: {\"choices\":[{\"delta\":{\"content\":\ndata: \"hi\"}}]}\n\n");
        assert_eq!(text_of(&p), "hi");
    }

    /// 注释行 / 无 data 的块不产生消费。
    #[test]
    fn comment_only_block_ignored() {
        let mut p = StreamParser::new(Provider::OpenAiChat);
        assert!(!p.feed(b": ping\n\n"));
    }

    /// 无尾随空行的最后一个 event 由 finalize 消费并定稿。
    #[test]
    fn finalize_consumes_trailing_event() {
        let mut p = StreamParser::new(Provider::OpenAiChat);
        p.feed(b"data: {\"choices\":[{\"delta\":{\"content\":\"tail\"}}]}");
        p.finalize();
        assert_eq!(text_of(&p), "tail");
        assert!(!p.snapshot().streaming);
    }

    /// CRLF 行尾的 Anthropic event（单次 feed 内）正常分发。
    #[test]
    fn crlf_framed_anthropic_event() {
        let mut p = StreamParser::new(Provider::Anthropic);
        p.feed(b"event: content_block_delta\r\ndata: {\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"A\"}}\r\n\r\n");
        assert_eq!(text_of(&p), "A");
    }

    // ── 跨 chunk 边界（TCP 切割点不认字符/行尾边界）──────────────────────────

    /// 多字节 UTF-8 字符被切成两半：重组后不得出现 U+FFFD 乱码。
    #[test]
    fn multibyte_char_split_across_feeds() {
        let mut p = StreamParser::new(Provider::OpenAiChat);
        let event = "data: {\"choices\":[{\"delta\":{\"content\":\"你好\"}}]}\n\n".as_bytes();
        // 切在「你」(E4 BD A0) 的第二个字节之后
        let split = event.iter().position(|&b| b == 0xE4).unwrap() + 2;
        p.feed(&event[..split]);
        p.feed(&event[split..]);
        assert_eq!(text_of(&p), "你好");
    }

    /// \r\n 恰好切成两半：不得拼出假空行把 event 切断丢弃。
    #[test]
    fn crlf_split_across_feeds() {
        let mut p = StreamParser::new(Provider::Anthropic);
        let event: &[u8] = b"event: content_block_delta\r\ndata: {\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"A\"}}\r\n\r\n";
        // 切在第一个 \r 之后（前 chunk 以孤立 \r 结尾）
        let split = event.iter().position(|&b| b == b'\r').unwrap() + 1;
        p.feed(&event[..split]);
        p.feed(&event[split..]);
        assert_eq!(text_of(&p), "A");
    }
}
