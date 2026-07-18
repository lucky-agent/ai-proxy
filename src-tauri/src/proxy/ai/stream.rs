//! 流式 SSE 分帧 + 分发。
//!
//! TCP chunk 边界 ≠ SSE event 边界，故持有内部 `buffer`：`feed` 追加原始
//! 字节后按空行（`\n\n`）切出完整 event 逐个消费，剩余不完整片段留在 buffer
//! 等下次。具体协议的状态机通过 `StreamState` trait 回调。

use super::{AiConversation, Provider, StreamState};

/// 有状态流式解析器。每个 AI 流式响应对应一个实例。
pub(crate) struct StreamParser {
    buffer: String,
    state: Box<dyn StreamState>,
}

impl StreamParser {
    pub(crate) fn new(provider: Provider) -> Self {
        StreamParser {
            buffer: String::new(),
            state: provider.create_stream_state(),
        }
    }

    /// 喂入一段原始响应字节，消费其中的完整 SSE event，更新内部状态。
    /// 返回本次是否有新内容被消费。
    pub(crate) fn feed(&mut self, bytes: &[u8]) -> bool {
        let chunk = String::from_utf8_lossy(bytes);
        // CRLF → LF 归一化，顺序很重要：先处理 \r\n 再处理孤立的 \r。
        self.buffer
            .push_str(&chunk.replace("\r\n", "\n").replace('\r', "\n"));

        let mut consumed = false;
        loop {
            let Some(idx) = self.buffer.find("\n\n") else {
                break;
            };
            let raw = self.buffer[..idx].to_string();
            self.buffer.drain(..idx + 2);
            if let Some((event, data)) = parse_sse_block(&raw) {
                self.state.apply(&event, &data);
                consumed = true;
            }
        }
        consumed
    }

    /// 流结束：处理 buffer 中残留的最后一个 event（无尾随空行的情况），标记定稿。
    pub(crate) fn finalize(&mut self) {
        let leftover = std::mem::take(&mut self.buffer);
        if !leftover.trim().is_empty() {
            if let Some((event, data)) = parse_sse_block(&leftover) {
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
