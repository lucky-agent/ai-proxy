//! 流式 SSE 分帧 + 分发。
//!
//! TCP chunk 边界 ≠ SSE event 边界，故持有内部字节 `buffer`：`feed` 追加原始
//! 字节后按空行（`\n\n`）切出完整 event 逐个消费，剩余不完整片段留在 buffer
//! 等下次。行尾归一化与分帧都在字节层进行，UTF-8 转换推迟到完整 event 切出
//! 之后——多字节字符或 `\r\n` 被 TCP 切割在两个 chunk 时不会被破坏。
//! 具体协议的状态机通过 `StreamState` trait 回调。
//!
//! JSON 解析在本层统一进行：`feed` 将 `data:` 行解析为 `serde_json::Value`，
//! 仅解析一次，结果同时传给 `state.apply()` 和 `raw_keys` 叶子字段名收集。
//! 协议实现不再自行 `serde_json::from_str`。

use std::collections::HashSet;

use serde_json::Value;

use super::normalize::JsonValueExt;
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
    /// SSE 所有 event data JSON 叶子字段名的并集（camelCase 归一化）。
    /// 覆盖率巡检用：流结束时与 IR 字段名做差集，`log::info!` 输出未覆盖字段。
    raw_keys: HashSet<String>,
}

impl StreamParser {
    pub(crate) fn new(provider: Provider) -> Self {
        StreamParser {
            buffer: Vec::new(),
            scan_from: 0,
            last_was_cr: false,
            state: provider.create_stream_state(),
            raw_keys: HashSet::new(),
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
        // 按空行切出完整 event，此时才做 UTF-8 转换（字符必然完整）。
        // 先借用解析（from_utf8_lossy 对纯 ASCII 零分配，AI SSE data 几乎全是 ASCII），
        // 解析后再 drain——避免每 event 一次 Vec<u8> 分配。
        while let Some(rel) = find_double_newline(&self.buffer[self.scan_from..]) {
            let idx = self.scan_from + rel;
            let raw = String::from_utf8_lossy(&self.buffer[..idx]);
            let parsed = parse_sse_block(&raw);
            // Cow<str> 的临时借用在 parse_sse_block 返回后释放，下面 drain 安全。
            drop(raw);
            self.buffer.drain(..idx + 2);
            self.scan_from = 0;
            if let Some((event, data)) = parsed {
                // 本层统一 JSON 解析：只解析一次，结果同时给 state 和 coverage
                if let Ok(value) = serde_json::from_str::<Value>(&data) {
                    self.raw_keys.extend(value.leaf_keys());
                    self.state.apply(&event, &value);
                }
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
    /// 返回流式全程收集的原始 SSE 叶子字段名集合（camelCase，供覆盖率巡检）。
    pub(crate) fn finalize(&mut self) -> HashSet<String> {
        let leftover = std::mem::take(&mut self.buffer);
        self.scan_from = 0;
        let raw = String::from_utf8_lossy(&leftover);
        if !raw.trim().is_empty() {
            if let Some((event, data)) = parse_sse_block(&raw) {
                if let Ok(value) = serde_json::from_str::<Value>(&data) {
                    self.raw_keys.extend(value.leaf_keys());
                    self.state.apply(&event, &value);
                }
            }
        }
        self.state.finalize();
        std::mem::take(&mut self.raw_keys)
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
        if event.is_empty() {
            "message".into()
        } else {
            event
        },
        data,
    ))
}
