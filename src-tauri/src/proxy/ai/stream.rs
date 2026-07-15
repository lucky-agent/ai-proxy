//! 流式 SSE 增量归一化解析器。
//!
//! TCP chunk 边界 ≠ SSE event 边界，故解析器持有内部 `buffer`：`feed` 追加原始
//! 字节后按空行（`\n\n`）切出完整 event 逐个消费，剩余不完整片段留在 buffer
//! 等下次。`snapshot` 返回当前累积的归一化对话，`streaming` 字段表示是否已定稿。

use std::collections::BTreeMap;

use serde_json::Value;

use super::normalize::{AiContentBlock, AiConversation, AiTurn, AiUsage};
use super::{anthropic, openai, Provider};

/// 单个 SSE event（已完成分帧）。
struct SseEvent {
    event: String,
    data: String,
}

/// 有状态流式解析器。每个 AI 流式响应对应一个实例。
pub(crate) struct StreamParser {
    provider: Provider,
    /// 跨 chunk 分帧缓冲：尚未凑齐空行的尾部片段。
    buffer: String,
    state: ProviderState,
}

enum ProviderState {
    OpenAi(OpenAiState),
    Anthropic(AnthropicState),
}

impl StreamParser {
    pub(crate) fn new(provider: Provider) -> Self {
        let state = match provider {
            Provider::OpenAi => ProviderState::OpenAi(OpenAiState::default()),
            Provider::Anthropic => ProviderState::Anthropic(AnthropicState::default()),
        };
        StreamParser {
            provider,
            buffer: String::new(),
            state,
        }
    }

    /// 喂入一段原始响应字节，消费其中的完整 SSE event，更新内部状态。
    /// 返回本次是否有新内容被消费（供调用方决定是否推送快照）。
    pub(crate) fn feed(&mut self, bytes: &[u8]) -> bool {
        // 归一换行；非 UTF-8 字节 lossy 处理（与前端 String::from_utf8_lossy 一致）。
        let chunk = String::from_utf8_lossy(bytes);
        // CRLF → LF 归一化，顺序很重要：先处理 \r\n 再处理孤立的 \r。
        // 不可简化为一次 replace('\r','\n')——会把 \r\n 变成 \n\n，破坏 SSE 分帧。
        self.buffer.push_str(&chunk.replace("\r\n", "\n").replace('\r', "\n"));

        let mut consumed = false;
        // 按空行切完整 event；保留最后一段（可能不完整）在 buffer。
        loop {
            let Some(idx) = self.buffer.find("\n\n") else {
                break;
            };
            let raw = self.buffer[..idx].to_string();
            self.buffer.drain(..idx + 2);
            if let Some(evt) = parse_sse_block(&raw) {
                self.apply(&evt);
                consumed = true;
            }
        }
        consumed
    }

    /// 流结束：处理 buffer 中残留的最后一个 event（无尾随空行的情况）。
    pub(crate) fn finalize(&mut self) {
        let leftover = std::mem::take(&mut self.buffer);
        if !leftover.trim().is_empty() {
            if let Some(evt) = parse_sse_block(&leftover) {
                self.apply(&evt);
            }
        }
        match &mut self.state {
            ProviderState::OpenAi(s) => s.done = true,
            ProviderState::Anthropic(s) => s.saw_message_stop = true,
        }
    }

    fn apply(&mut self, evt: &SseEvent) {
        match &mut self.state {
            ProviderState::OpenAi(s) => s.apply(evt),
            ProviderState::Anthropic(s) => s.apply(evt),
        }
    }

    /// 当前累积的归一化对话快照。
    pub(crate) fn snapshot(&self) -> AiConversation {
        match &self.state {
            ProviderState::OpenAi(s) => s.snapshot(self.provider),
            ProviderState::Anthropic(s) => s.snapshot(self.provider),
        }
    }
}

/// 解析一个 SSE event 块（多行），提取 `event:` 与拼接的 `data:`。
fn parse_sse_block(raw: &str) -> Option<SseEvent> {
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
    Some(SseEvent {
        event: if event.is_empty() { "message".into() } else { event },
        data,
    })
}

// ── OpenAI 流式状态 ──────────────────────────────────────────────────────────

#[derive(Default)]
struct OpenAiToolCall {
    id: String,
    name: String,
    arguments: String,
}

#[derive(Default)]
struct OpenAiState {
    text: String,
    model: Option<String>,
    usage: Option<AiUsage>,
    finish_reason: Option<String>,
    done: bool,
    /// tool_call 按 index 累积。
    tool_calls: BTreeMap<i64, OpenAiToolCall>,
}

impl OpenAiState {
    fn apply(&mut self, evt: &SseEvent) {
        let data = evt.data.trim();
        if data == "[DONE]" {
            self.done = true;
            return;
        }
        let Ok(p) = serde_json::from_str::<Value>(data) else {
            return;
        };
        let choice0 = p.get("choices").and_then(|c| c.as_array()).and_then(|a| a.first());
        let delta = choice0.and_then(|c| c.get("delta"));

        if let Some(content) = delta.and_then(|d| d.get("content")).and_then(Value::as_str) {
            self.text.push_str(content);
        }
        if let Some(usage) = p.get("usage").filter(|u| !u.is_null()) {
            self.usage = Some(openai::usage_from_json(usage));
        }
        if let Some(fr) = choice0.and_then(|c| c.get("finish_reason")).and_then(Value::as_str) {
            self.finish_reason = Some(fr.to_string());
        }
        if self.model.is_none() {
            if let Some(m) = p.get("model").and_then(Value::as_str) {
                self.model = Some(m.to_string());
            }
        }
        // tool_calls 增量
        if let Some(tcs) = delta.and_then(|d| d.get("tool_calls")).and_then(Value::as_array) {
            for tc in tcs {
                let idx = tc.get("index").and_then(Value::as_i64).unwrap_or(0);
                let entry = self.tool_calls.entry(idx).or_default();
                if let Some(id) = tc.get("id").and_then(Value::as_str) {
                    if entry.id.is_empty() {
                        entry.id = id.to_string();
                    }
                }
                let func = tc.get("function");
                if let Some(name) = func.and_then(|f| f.get("name")).and_then(Value::as_str) {
                    if entry.name.is_empty() {
                        entry.name = name.to_string();
                    }
                }
                if let Some(args) = func.and_then(|f| f.get("arguments")).and_then(Value::as_str) {
                    entry.arguments.push_str(args);
                }
            }
        }
    }

    fn snapshot(&self, provider: Provider) -> AiConversation {
        let mut blocks: Vec<AiContentBlock> = Vec::new();
        if !self.text.is_empty() {
            blocks.push(AiContentBlock::text(self.text.clone()));
        }
        for tc in self.tool_calls.values() {
            blocks.push(AiContentBlock::ToolUse {
                id: tc.id.clone(),
                name: tc.name.clone(),
                input: openai::parse_tool_input(&tc.arguments),
            });
        }
        if blocks.is_empty() {
            blocks.push(AiContentBlock::text(""));
        }
        AiConversation {
            provider: provider.as_str().to_string(),
            turns: vec![AiTurn::new("assistant", blocks)],
            streaming: !self.done && self.finish_reason.is_none(),
            model: self.model.clone(),
            usage: self.usage.clone(),
            finish_reason: self.finish_reason.clone(),
        }
    }
}

// ── Anthropic 流式状态 ───────────────────────────────────────────────────────

struct AnthropicBlock {
    is_tool: bool,
    text: String,
    tool_id: String,
    tool_name: String,
    partial_json: String,
}

#[derive(Default)]
struct AnthropicState {
    model: Option<String>,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    stop_reason: Option<String>,
    saw_message_stop: bool,
    blocks: BTreeMap<i64, AnthropicBlock>,
}

impl AnthropicState {
    fn apply(&mut self, evt: &SseEvent) {
        let Ok(p) = serde_json::from_str::<Value>(&evt.data) else {
            return;
        };
        match evt.event.as_str() {
            "message_start" => {
                let msg = p.get("message");
                self.model = msg
                    .and_then(|m| m.get("model"))
                    .and_then(Value::as_str)
                    .map(String::from);
                self.input_tokens = msg
                    .and_then(|m| m.get("usage"))
                    .and_then(|u| u.get("input_tokens"))
                    .and_then(Value::as_u64);
            }
            "content_block_start" => {
                let idx = p.get("index").and_then(Value::as_i64).unwrap_or(0);
                let cb = p.get("content_block");
                match cb.and_then(|c| c.get("type")).and_then(Value::as_str) {
                    Some("tool_use") => {
                        self.blocks.insert(idx, AnthropicBlock {
                            is_tool: true,
                            text: String::new(),
                            tool_id: cb.and_then(|c| c.get("id")).and_then(Value::as_str).unwrap_or("").to_string(),
                            tool_name: cb.and_then(|c| c.get("name")).and_then(Value::as_str).unwrap_or("").to_string(),
                            partial_json: String::new(),
                        });
                    }
                    Some("text") => {
                        self.blocks.insert(idx, AnthropicBlock {
                            is_tool: false,
                            text: cb.and_then(|c| c.get("text")).and_then(Value::as_str).unwrap_or("").to_string(),
                            tool_id: String::new(),
                            tool_name: String::new(),
                            partial_json: String::new(),
                        });
                    }
                    _ => {}
                }
            }
            "content_block_delta" => {
                let idx = p.get("index").and_then(Value::as_i64).unwrap_or(0);
                let delta = p.get("delta");
                match delta.and_then(|d| d.get("type")).and_then(Value::as_str) {
                    Some("text_delta") => {
                        if let Some(t) = delta.and_then(|d| d.get("text")).and_then(Value::as_str) {
                            self.blocks
                                .entry(idx)
                                .or_insert_with(|| AnthropicBlock {
                                    is_tool: false,
                                    text: String::new(),
                                    tool_id: String::new(),
                                    tool_name: String::new(),
                                    partial_json: String::new(),
                                })
                                .text
                                .push_str(t);
                        }
                    }
                    Some("input_json_delta") => {
                        if let Some(j) = delta.and_then(|d| d.get("partial_json")).and_then(Value::as_str) {
                            self.blocks
                                .entry(idx)
                                .or_insert_with(|| AnthropicBlock {
                                    is_tool: true,
                                    text: String::new(),
                                    tool_id: String::new(),
                                    tool_name: String::new(),
                                    partial_json: String::new(),
                                })
                                .partial_json
                                .push_str(j);
                        }
                    }
                    _ => {}
                }
            }
            "message_delta" => {
                if let Some(ot) = p.get("usage").and_then(|u| u.get("output_tokens")).and_then(Value::as_u64) {
                    self.output_tokens = Some(ot);
                }
                if let Some(sr) = p.get("delta").and_then(|d| d.get("stop_reason")).and_then(Value::as_str) {
                    self.stop_reason = Some(sr.to_string());
                }
            }
            "message_stop" => {
                self.saw_message_stop = true;
            }
            _ => {}
        }
    }

    fn snapshot(&self, provider: Provider) -> AiConversation {
        let mut blocks: Vec<AiContentBlock> = Vec::new();
        for b in self.blocks.values() {
            if b.is_tool {
                let input = if b.partial_json.is_empty() {
                    Value::Object(Default::default())
                } else {
                    serde_json::from_str(&b.partial_json)
                        .unwrap_or_else(|_| Value::String(b.partial_json.clone()))
                };
                blocks.push(AiContentBlock::ToolUse {
                    id: b.tool_id.clone(),
                    name: b.tool_name.clone(),
                    input,
                });
            } else if !b.text.is_empty() {
                blocks.push(AiContentBlock::text(b.text.clone()));
            }
        }
        if blocks.is_empty() {
            blocks.push(AiContentBlock::text(""));
        }
        AiConversation {
            provider: provider.as_str().to_string(),
            turns: vec![AiTurn::new("assistant", blocks)],
            streaming: !self.saw_message_stop,
            model: self.model.clone(),
            usage: anthropic::usage_from_parts(self.input_tokens, self.output_tokens),
            finish_reason: self.stop_reason.clone(),
        }
    }
}
