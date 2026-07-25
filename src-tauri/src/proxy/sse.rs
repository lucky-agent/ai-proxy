//! SSE 字节分帧器（纯传输层）。
//!
//! TCP chunk 边界 ≠ SSE event 边界，故持有内部字节 `buffer`：`feed` 追加原始
//! 字节后按空行（`\n\n`）切出完整 event 逐个产出，剩余不完整片段留在 buffer
//! 等下次。行尾归一化与分帧都在字节层进行，UTF-8 转换推迟到完整 event 切出
//! 之后——多字节字符或 `\r\n` 被 TCP 切割在两个 chunk 时不会被破坏。
//!
//! 本模块**不解析 JSON，不关心 AI 协议**。产出 [`SseEvent`] 结构体，
//! JSON 解析和 `StreamState::apply` 由调用方（`ResponseObserver`）负责。

/// 单个 event 的 buffer 上限（4 MiB）：正常 SSE event 远小于此，超限说明上游
/// 并非按 SSE 分帧（如误标 content-type 的长流），丢弃防止无界增长——
/// 仅影响旁路观测，不影响字节透传。
const MAX_EVENT_BUFFER: usize = 4 * 1024 * 1024;

/// 解析后的 SSE event，包含 WHATWG 规范定义的全部标准字段。
///
/// 参考：[HTML Standard §9.2 Server-sent events](https://html.spec.whatwg.org/multipage/server-sent-events.html)
#[derive(Debug, Clone)]
pub(crate) struct SseEvent {
    /// Event type（`event:` 字段）。无 `event:` 行时默认 `"message"`。
    pub(crate) event: String,
    /// Data payload（`data:` 字段）。多行 `data:` 以 `\n` 拼接，尾随 `\n` 已在
    /// 重建时移除。`None` 表示纯注释/keepalive 块（无 data 行）；AI 管线跳过。
    pub(crate) data: Option<String>,
    /// 事件 ID（`id:` 字段）。含 U+0000 的 id 整行忽略（WHATWG）。bare `id`
    /// 设为空字符串。
    pub(crate) id: Option<String>,
    /// 重连超时毫秒数（`retry:` 字段）。非纯 ASCII 数字的值被忽略（WHATWG）。
    pub(crate) retry: Option<u64>,
    /// 注释行（`:` 开头）。保留原始内容供录制重建，AI 管线忽略。
    pub(crate) comments: Vec<String>,
}

/// SSE 字节分帧器。纯传输层：字节流 → [`SseEvent`]。
pub(crate) struct SseFramer {
    /// 原始字节缓冲，行尾已在追加时归一化为 `\n`（见 [`Self::feed`]）。
    buffer: Vec<u8>,
    /// 已扫描且确认无空行的前缀长度（下轮从此处继续，避免重复扫描）。
    scan_from: usize,
    /// 上一字节是否为 `\r`——`\r\n` 跨 chunk 切割时用于吞掉后半的 `\n`。
    last_was_cr: bool,
}

impl SseFramer {
    pub(crate) fn new() -> Self {
        SseFramer {
            buffer: Vec::new(),
            scan_from: 0,
            last_was_cr: false,
        }
    }

    /// 喂入一段原始响应字节，消费其中的完整 SSE event。
    /// 返回本批新切出的 [`SseEvent`]（可能为空）。
    pub(crate) fn feed(&mut self, bytes: &[u8]) -> Vec<SseEvent> {
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

        let mut events = Vec::new();
        // 按空行切出完整 event，此时才做 UTF-8 转换（字符必然完整）。
        while let Some(rel) = find_double_newline(&self.buffer[self.scan_from..]) {
            let idx = self.scan_from + rel;
            let raw = String::from_utf8_lossy(&self.buffer[..idx]);
            events.push(parse_sse_block(&raw));
            self.buffer.drain(..idx + 2);
            self.scan_from = 0;
        }
        // 尾字节可能与下个 chunk 的 \n 配对成空行，回退 1 字节重扫
        self.scan_from = self.buffer.len().saturating_sub(1);

        if self.buffer.len() > MAX_EVENT_BUFFER {
            log::warn!("[sse] event buffer exceeded {MAX_EVENT_BUFFER} bytes, dropped");
            self.buffer.clear();
            self.scan_from = 0;
        }

        events
    }

    /// 流结束：取出 buffer 中残留的最后一个 event（无尾随空行的情况）。
    pub(crate) fn finalize(&mut self) -> Option<SseEvent> {
        let leftover = std::mem::take(&mut self.buffer);
        self.scan_from = 0;
        let raw = String::from_utf8_lossy(&leftover);
        if raw.trim().is_empty() {
            return None;
        }
        Some(parse_sse_block(&raw))
    }
}

/// 找最早的 `\n\n`（event 分隔空行）起始下标。
fn find_double_newline(buf: &[u8]) -> Option<usize> {
    buf.windows(2).position(|w| w == b"\n\n")
}

/// 解析一个 SSE event 块（多行），返回 [`SseEvent`]。
///
/// 字段解析遵循 WHATWG HTML 规范 §9.2：
/// - `event:`  → 事件类型（bare `event` 清空 → dispatch 为 `"message"`）
/// - `data:`   → 多行以 `\n` 拼接；无 data 行时返回 `data: None`
/// - `id:`     → 含 U+0000 整行忽略；bare `id` 设为空字符串
/// - `retry:`  → 严格 ASCII 数字校验
/// - Comment（`:` 开头）→ 保留到 `comments`，供录制重建
/// - 其他字段 → 忽略
///
/// 纯注释/keepalive 块（无 data）也会产出 event，`data` 为 `None`，
/// 与 rama 行为一致。AI 管线按 `data.is_none()` 跳过。
fn parse_sse_block(raw: &str) -> SseEvent {
    let mut event: Option<String> = None;
    let mut data = String::new();
    let mut has_data = false;
    let mut id: Option<String> = None;
    let mut retry: Option<u64> = None;
    let mut comments: Vec<String> = Vec::new();

    for line in raw.split('\n') {
        // 空行（空 event 块内的分隔）
        if line.is_empty() {
            continue;
        }
        // 注释行（以 `:` 开头）
        if line.starts_with(':') {
            // 去掉 `:` 及后面紧跟的单个空格（如有），保留注释内容。
            let comment = if line.as_bytes().get(1) == Some(&b' ') {
                &line[2..]
            } else {
                &line[1..]
            };
            comments.push(comment.to_string());
            continue;
        }

        let (field, value) = match line.find(':') {
            Some(i) => {
                let v = &line[i + 1..];
                // 冒号后首个可选空格
                (&line[..i], v.strip_prefix(' ').unwrap_or(v))
            }
            // 整行无冒号 → field = 整行, value = ""（如 bare `data`、`event`）
            None => (line, ""),
        };

        match field {
            "event" => {
                // WHATWG: bare `event`（无值）清空 event-type buffer，
                // 后续 dispatch 为默认 `"message"` 事件。
                // 有值时存储；空字符串用 None 表示，最终落回 "message"。
                event = Some(if value.is_empty() {
                    String::new()
                } else {
                    value.to_string()
                });
            }
            "data" => {
                if has_data {
                    data.push('\n');
                }
                data.push_str(value);
                has_data = true;
            }
            "id" => {
                // WHATWG: bare `id`（无值）设置 last-event-ID buffer 为空字符串；
                // 含 U+0000 的 id 整行忽略。
                if value.is_empty() {
                    id = Some(String::new());
                } else if !value.contains('\0') {
                    id = Some(value.to_string());
                }
            }
            "retry" => {
                // WHATWG: retry 值必须为纯 ASCII 数字。
                // `u64::parse` 默认接受 `+` 前缀，不符合规范，故手写校验。
                if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) {
                    if let Ok(ms) = value.parse() {
                        retry = Some(ms);
                    }
                }
            }
            // 未知字段 / comment（已在上面跳过）/ 空字段 → 忽略
            _ => {}
        }
    }

    if !has_data {
        return SseEvent {
            event: "message".into(),
            data: None,
            id,
            retry,
            comments,
        };
    }

    SseEvent {
        event: event
            .filter(|e| !e.is_empty())
            .unwrap_or_else(|| "message".into()),
        data: Some(data),
        id,
        retry,
        comments,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------
    // 事件分隔
    // -------------------------------------------------------------------
    #[test]
    fn basic_message_event() {
        let mut f = SseFramer::new();
        let events = f.feed(b"data: hello\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, "message");
        assert_eq!(events[0].data.as_deref(), Some("hello"));
        assert!(events[0].id.is_none());
        assert!(events[0].retry.is_none());
        assert!(events[0].comments.is_empty());
    }

    #[test]
    fn pure_comment_produces_event_without_data() {
        let mut f = SseFramer::new();
        let events = f.feed(b": just a comment\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, None);
        assert_eq!(events[0].comments, vec!["just a comment"]);
    }

    #[test]
    fn multi_line_data() {
        let mut f = SseFramer::new();
        let events = f.feed(b"data: first\ndata: second\ndata: third\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data.as_deref(), Some("first\nsecond\nthird"));
    }

    #[test]
    fn event_type_named() {
        let mut f = SseFramer::new();
        let events = f.feed(b"event: ping\ndata: 42\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, "ping");
        assert_eq!(events[0].data.as_deref(), Some("42"));
    }

    // -------------------------------------------------------------------
    // id 字段
    // -------------------------------------------------------------------
    #[test]
    fn id_field_present() {
        let mut f = SseFramer::new();
        let events = f.feed(b"id: abc123\ndata: x\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id.as_deref(), Some("abc123"));
    }

    #[test]
    fn bare_id_sets_empty_string() {
        // WHATWG: bare `id` → last-event-ID = ""
        let mut f = SseFramer::new();
        let events = f.feed(b"id\ndata: x\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id.as_deref(), Some(""));
    }

    #[test]
    fn id_with_nul_is_ignored() {
        let mut f = SseFramer::new();
        let events = f.feed(b"id: bad\x00id\ndata: x\n\n");
        assert_eq!(events.len(), 1);
        assert!(events[0].id.is_none(), "id containing NUL must be ignored");
    }

    // -------------------------------------------------------------------
    // retry 字段
    // -------------------------------------------------------------------
    #[test]
    fn retry_valid() {
        let mut f = SseFramer::new();
        let events = f.feed(b"retry: 5000\ndata: x\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].retry, Some(5000));
    }

    #[test]
    fn retry_rejects_plus_prefix() {
        let mut f = SseFramer::new();
        let events = f.feed(b"retry: +5\ndata: x\n\n");
        assert_eq!(events.len(), 1);
        assert!(events[0].retry.is_none(), "retry: +5 must be ignored");
    }

    #[test]
    fn retry_rejects_non_digit() {
        let mut f = SseFramer::new();
        let events = f.feed(b"retry: 5ms\ndata: x\n\n");
        assert_eq!(events.len(), 1);
        assert!(events[0].retry.is_none());
    }

    #[test]
    fn retry_empty_ignored() {
        let mut f = SseFramer::new();
        let events = f.feed(b"retry:\ndata: x\n\n");
        assert_eq!(events.len(), 1);
        assert!(events[0].retry.is_none());
    }

    // -------------------------------------------------------------------
    // bare event 清空
    // -------------------------------------------------------------------
    #[test]
    fn bare_event_clears_event_type() {
        // WHATWG: `event` (no value) clears the event-type buffer
        let mut f = SseFramer::new();
        let events = f.feed(b"event: ping\nevent\ndata: 42\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].event, "message",
            "bare event should reset to default 'message'"
        );
    }

    // -------------------------------------------------------------------
    // 完整字段组合
    // -------------------------------------------------------------------
    #[test]
    fn all_fields() {
        let mut f = SseFramer::new();
        let events = f.feed(b"id: 7\nretry: 3000\nevent: update\ndata: {\"k\":1}\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id.as_deref(), Some("7"));
        assert_eq!(events[0].retry, Some(3000));
        assert_eq!(events[0].event, "update");
        assert_eq!(events[0].data.as_deref(), Some("{\"k\":1}"));
    }

    // -------------------------------------------------------------------
    // 跨 chunk / finalize
    // -------------------------------------------------------------------
    #[test]
    fn event_split_across_chunks() {
        let mut f = SseFramer::new();
        let events = f.feed(b"event: add\ndata: 123");
        assert!(events.is_empty()); // incomplete
        let events = f.feed(b"\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, "add");
        assert_eq!(events[0].data.as_deref(), Some("123"));
    }

    #[test]
    fn finalize_returns_leftover() {
        let mut f = SseFramer::new();
        let events = f.feed(b"data: last event");
        assert!(events.is_empty());
        let ev = f.finalize().unwrap();
        assert_eq!(ev.data.as_deref(), Some("last event"));
    }

    // -------------------------------------------------------------------
    // 行尾归一化
    // -------------------------------------------------------------------
    #[test]
    fn crlf_crosses_chunk_boundary() {
        let mut f = SseFramer::new();
        // first chunk ends with \r
        let events = f.feed(b"data: a\r");
        assert!(events.is_empty());
        // second chunk begins with \n — should NOT produce stray empty event
        let events = f.feed(b"\ndata: b\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data.as_deref(), Some("a\nb"));
    }

    #[test]
    fn standalone_cr_as_lf() {
        let mut f = SseFramer::new();
        let events = f.feed(b"data: foo\rdata: bar\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data.as_deref(), Some("foo\nbar"));
    }

    // -------------------------------------------------------------------
    // 注释
    // -------------------------------------------------------------------
    #[test]
    fn comment_lines_are_skipped() {
        let mut f = SseFramer::new();
        let events = f.feed(b": this is a comment\ndata: ok\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data.as_deref(), Some("ok"));
    }

    #[test]
    fn comments_are_preserved() {
        let mut f = SseFramer::new();
        let events = f.feed(b": keepalive\n\n: req #1\ndata: x\n\n");
        // first event: keepalive-only → data: None but still produced
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].data, None);
        assert_eq!(events[0].comments, vec!["keepalive"]);
        assert_eq!(events[1].data.as_deref(), Some("x"));
        assert_eq!(events[1].comments, vec!["req #1"]);
    }

    #[test]
    fn comments_strip_leading_space() {
        let mut f = SseFramer::new();
        let events = f.feed(b":  two spaces\ndata: x\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].comments, vec![" two spaces"]);
    }

    // -------------------------------------------------------------------
    // 已知字段和未知字段混排
    // -------------------------------------------------------------------
    #[test]
    fn unknown_field_ignored() {
        let mut f = SseFramer::new();
        let events = f.feed(b"x-custom: abc\ndata: x\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data.as_deref(), Some("x"));
    }
}
