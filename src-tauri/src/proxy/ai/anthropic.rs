//! Anthropic Messages 归一化（请求 messages+system + 非流式响应 + 流式 SSE）。
//! 语义镜像前端 `src/lib/ai/providers/anthropic.ts`。

use std::collections::BTreeMap;

use serde_json::Value;

use super::normalize::{AiContentBlock, AiConversation, AiTurn, AiUsage, normalize_usage};
use super::{AiProtocol, StreamState};

// ══════════════════════════════════════════════════════════════════════════════
// 协议实现
// ══════════════════════════════════════════════════════════════════════════════

pub(crate) struct AnthropicProtocol;

impl AiProtocol for AnthropicProtocol {
    /// 解析请求体（顶层 `system` + `messages[]`，含 tool_use/tool_result block + tools 定义）。
    fn parse_request(&self, body: &str) -> Option<Vec<AiTurn>> {
        let p: Value = serde_json::from_str(body).ok()?;
        let messages = p.get("messages")?.as_array()?;
        if messages.is_empty() {
            return None;
        }
        let mut turns: Vec<AiTurn> = Vec::new();

        // 顶层 system → system turn（系统提示词排在 tools 定义之前）
        if let Some(system) = p.get("system") {
            let sys_text = match system {
                Value::String(s) => s.clone(),
                Value::Array(arr) => blocks_to_text(arr),
                _ => String::new(),
            };
            if !sys_text.is_empty() {
                turns.push(AiTurn::new("system", vec![AiContentBlock::text(sys_text)]));
            }
        }

        // tools[] 定义 → tools_def turn
        if let Some(t) = p.get("tools").and_then(Value::as_array).and_then(|ts| AiTurn::tools_def(ts)) {
            turns.push(t);
        }

        for m in messages {
            let role_raw = m.get("role").and_then(Value::as_str).unwrap_or("user");
            let role = match role_raw {
                "user" | "assistant" | "tool" => role_raw,
                _ => "user",
            };
            match m.get("content") {
                Some(Value::Array(arr)) => {
                    let mut blocks: Vec<AiContentBlock> = Vec::new();
                    for b in arr {
                        match b.get("type").and_then(Value::as_str) {
                            Some("text") => {
                                if let Some(t) = b.get("text").and_then(Value::as_str) {
                                    blocks.push(AiContentBlock::text(t));
                                }
                            }
                            // 多轮工具链会回放 thinking block，与响应侧同口径捕获；
                            // redacted_thinking（加密）落入 `_` 丢弃
                            Some("thinking") => {
                                if let Some(t) = b.get("thinking").and_then(Value::as_str) {
                                    blocks.push(AiContentBlock::thinking(t));
                                }
                            }
                            Some("tool_use") => {
                                blocks.push(AiContentBlock::ToolUse {
                                    id: b.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
                                    name: b.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
                                    input: b.get("input").cloned().unwrap_or(Value::Object(Default::default())),
                                });
                            }
                            Some("tool_result") => {
                                let inner = match b.get("content") {
                                    Some(Value::String(s)) => vec![AiContentBlock::text(s.clone())],
                                    Some(Value::Array(cb)) => {
                                        let texts: Vec<AiContentBlock> = cb
                                            .iter()
                                            .filter(|c| c.get("type").and_then(Value::as_str) == Some("text"))
                                            .filter_map(|c| c.get("text").and_then(Value::as_str).map(AiContentBlock::text))
                                            .collect();
                                        if texts.is_empty() {
                                            vec![AiContentBlock::text(
                                                b.get("content").map(|c| c.to_string()).unwrap_or_default(),
                                            )]
                                        } else {
                                            texts
                                        }
                                    }
                                    _ => vec![AiContentBlock::text("")],
                                };
                                blocks.push(AiContentBlock::ToolResult {
                                    tool_use_id: b
                                        .get("tool_use_id")
                                        .and_then(Value::as_str)
                                        .unwrap_or("")
                                        .to_string(),
                                    content: inner,
                                });
                            }
                            _ => {}
                        }
                    }
                    if blocks.is_empty() {
                        blocks.push(AiContentBlock::text(""));
                    }
                    turns.push(AiTurn::new(role, blocks));
                }
                Some(Value::String(s)) => {
                    if role == "tool" {
                        let tool_use_id = m
                            .get("tool_call_id")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        turns.push(AiTurn::new(
                            role,
                            vec![AiContentBlock::ToolResult {
                                tool_use_id,
                                content: vec![AiContentBlock::text(s.clone())],
                            }],
                        ));
                    } else {
                        turns.push(AiTurn::new(role, vec![AiContentBlock::text(s.clone())]));
                    }
                }
                _ => {
                    turns.push(AiTurn::new(role, vec![AiContentBlock::text("")]));
                }
            }
        }
        Some(turns)
    }

    /// 解析非流式响应体（`type: message`，含 `content[]` + `usage`）。
    fn parse_response_body(&self, body: &str) -> Option<AiConversation> {
        let p: Value = serde_json::from_str(body).ok()?;
        if p.get("type").and_then(Value::as_str) != Some("message") {
            return None;
        }
        let content = p.get("content")?.as_array()?;

        let mut blocks: Vec<AiContentBlock> = Vec::new();
        for b in content {
            match b.get("type").and_then(Value::as_str) {
                Some("text") => {
                    if let Some(t) = b.get("text").and_then(Value::as_str) {
                        blocks.push(AiContentBlock::text(t));
                    }
                }
                // extended thinking；redacted_thinking（加密）不采集
                Some("thinking") => {
                    if let Some(t) = b.get("thinking").and_then(Value::as_str) {
                        blocks.push(AiContentBlock::thinking(t));
                    }
                }
                Some("tool_use") => {
                    blocks.push(AiContentBlock::ToolUse {
                        id: b.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
                        name: b.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
                        input: b.get("input").cloned().unwrap_or(Value::Object(Default::default())),
                    });
                }
                _ => {}
            }
        }
        if blocks.is_empty() {
            blocks.push(AiContentBlock::text(""));
        }

        // 完整 usage 过通用归一化：input/output 之外把 cache_read 也捕获
        let usage = p.get("usage").map(normalize_usage).filter(|u| !u.is_empty());

        Some(AiConversation::new(
            "anthropic",
            vec![AiTurn::new("assistant", blocks)],
            false,
            p.get("model").and_then(Value::as_str).map(String::from),
            usage,
            p.get("stop_reason").and_then(Value::as_str).map(String::from),
        ))
    }

    fn create_stream_state(&self) -> Box<dyn StreamState> {
        Box::new(AnthropicStreamState::default())
    }
}

/// 从 content block 数组拼接纯文本。
fn blocks_to_text(blocks: &[Value]) -> String {
    blocks
        .iter()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|b| b.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("")
}

// ══════════════════════════════════════════════════════════════════════════════
// 流式 SSE 状态机
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Default, Clone, Copy, PartialEq)]
enum AnthropicBlockKind {
    #[default]
    Text,
    Thinking,
    Tool,
}

#[derive(Default)]
struct AnthropicBlock {
    kind: AnthropicBlockKind,
    /// text / thinking 的累积正文。
    text: String,
    tool_id: String,
    tool_name: String,
    partial_json: String,
}

#[derive(Default)]
struct AnthropicStreamState {
    model: Option<String>,
    /// `message_start` 的完整 usage（input + cache_read 口径）；
    /// output 走 `message_delta` 增量，snapshot 时经 [`Self::current_usage`] 合并。
    base_usage: Option<AiUsage>,
    output_tokens: Option<u64>,
    stop_reason: Option<String>,
    saw_message_stop: bool,
    blocks: BTreeMap<i64, AnthropicBlock>,
}

impl AnthropicStreamState {
    /// base usage + 累计 output 合并出当前 usage 快照。
    fn current_usage(&self) -> Option<AiUsage> {
        let mut usage = self.base_usage.clone();
        if let Some(ot) = self.output_tokens {
            let u = usage.get_or_insert_default();
            u.completion_tokens = Some(ot);
            u.total_tokens = Some(u.prompt_tokens.unwrap_or(0) + ot);
        }
        usage
    }
}

impl StreamState for AnthropicStreamState {
    fn apply(&mut self, event: &str, data: &str) {
        let Ok(p) = serde_json::from_str::<Value>(data) else { return };
        match event {
            "message_start" => {
                let msg = p.get("message");
                self.model = msg.and_then(|m| m.get("model")).and_then(Value::as_str).map(String::from);
                self.base_usage = msg
                    .and_then(|m| m.get("usage"))
                    .map(normalize_usage)
                    .filter(|u| !u.is_empty());
            }
            "content_block_start" => {
                let idx = p.get("index").and_then(Value::as_i64).unwrap_or(0);
                let cb = p.get("content_block");
                match cb.and_then(|c| c.get("type")).and_then(Value::as_str) {
                    Some("tool_use") => {
                        self.blocks.insert(idx, AnthropicBlock {
                            kind: AnthropicBlockKind::Tool,
                            tool_id: cb.and_then(|c| c.get("id")).and_then(Value::as_str).unwrap_or("").to_string(),
                            tool_name: cb.and_then(|c| c.get("name")).and_then(Value::as_str).unwrap_or("").to_string(),
                            ..Default::default()
                        });
                    }
                    Some("text") => {
                        self.blocks.insert(idx, AnthropicBlock {
                            kind: AnthropicBlockKind::Text,
                            text: cb.and_then(|c| c.get("text")).and_then(Value::as_str).unwrap_or("").to_string(),
                            ..Default::default()
                        });
                    }
                    Some("thinking") => {
                        self.blocks.insert(idx, AnthropicBlock {
                            kind: AnthropicBlockKind::Thinking,
                            text: cb.and_then(|c| c.get("thinking")).and_then(Value::as_str).unwrap_or("").to_string(),
                            ..Default::default()
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
                            // or_default → kind 默认 Text
                            self.blocks.entry(idx).or_default().text.push_str(t);
                        }
                    }
                    Some("thinking_delta") => {
                        if let Some(t) = delta.and_then(|d| d.get("thinking")).and_then(Value::as_str) {
                            self.blocks
                                .entry(idx)
                                .or_insert_with(|| AnthropicBlock {
                                    kind: AnthropicBlockKind::Thinking,
                                    ..Default::default()
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
                                    kind: AnthropicBlockKind::Tool,
                                    ..Default::default()
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

    fn snapshot(&self) -> AiConversation {
        let mut blocks: Vec<AiContentBlock> = Vec::new();
        for b in self.blocks.values() {
            match b.kind {
                AnthropicBlockKind::Tool => {
                    let input = if b.partial_json.is_empty() {
                        serde_json::Value::Object(Default::default())
                    } else {
                        serde_json::from_str(&b.partial_json).unwrap_or_else(|_| Value::String(b.partial_json.clone()))
                    };
                    blocks.push(AiContentBlock::ToolUse { id: b.tool_id.clone(), name: b.tool_name.clone(), input });
                }
                AnthropicBlockKind::Thinking => {
                    if !b.text.is_empty() {
                        blocks.push(AiContentBlock::thinking(b.text.clone()));
                    }
                }
                AnthropicBlockKind::Text => {
                    if !b.text.is_empty() {
                        blocks.push(AiContentBlock::text(b.text.clone()));
                    }
                }
            }
        }
        if blocks.is_empty() { blocks.push(AiContentBlock::text("")); }
        AiConversation::new(
            "anthropic",
            vec![AiTurn::new("assistant", blocks)],
            !self.saw_message_stop,
            self.model.clone(),
            self.current_usage(),
            self.stop_reason.clone(),
        )
    }

    fn finalize(&mut self) {
        self.saw_message_stop = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_request ──────────────────────────────────────────────────────────

    #[test]
    fn parse_request_system_tools_and_blocks() {
        let body = r#"{
            "system": "You are helpful.",
            "tools": [{"name": "get_weather", "input_schema": {"type": "object"}}],
            "messages": [
                {"role": "user", "content": "hi"},
                {"role": "assistant", "content": [
                    {"type": "text", "text": "let me check"},
                    {"type": "tool_use", "id": "tu_1", "name": "get_weather", "input": {"city": "NY"}}
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "tu_1", "content": "sunny"}
                ]}
            ]
        }"#;
        let turns = AnthropicProtocol.parse_request(body).unwrap();
        assert_eq!(turns.len(), 5);
        assert_eq!(turns[0].role, "system");
        assert_eq!(turns[1].role, "tools_def");
        assert_eq!(turns[2].role, "user");
        assert!(turns[3].content.iter().any(|b| matches!(b, AiContentBlock::ToolUse { .. })));
        assert!(turns[4].content.iter().any(|b| matches!(b, AiContentBlock::ToolResult { .. })));
    }

    // ── parse_response_body ────────────────────────────────────────────────────

    #[test]
    fn parse_response_text_and_tool_use() {
        let body = r#"{
            "type": "message", "model": "claude-sonnet-4-5", "stop_reason": "tool_use",
            "content": [
                {"type": "text", "text": "checking"},
                {"type": "tool_use", "id": "tu_1", "name": "get_weather", "input": {"city": "NY"}}
            ],
            "usage": {"input_tokens": 5, "output_tokens": 7}
        }"#;
        let conv = AnthropicProtocol.parse_response_body(body).unwrap();
        assert_eq!(conv.provider, "anthropic");
        assert_eq!(conv.model.as_deref(), Some("claude-sonnet-4-5"));
        assert_eq!(conv.finish_reason.as_deref(), Some("tool_use"));
        assert_eq!(conv.turns[0].content.len(), 2);
        let u = conv.usage.unwrap();
        assert_eq!(u.prompt_tokens, Some(5));
        assert_eq!(u.completion_tokens, Some(7));
        assert_eq!(u.total_tokens, Some(12));
    }

    /// usage 的 cache_read_input_tokens 必须进入 cached_tokens（缓存命中占比高的
    /// 客户端如 Claude Code，丢弃该字段会让 usage 统计严重失真）。
    #[test]
    fn parse_response_cache_tokens() {
        let body = r#"{
            "type": "message", "model": "claude-sonnet-4-5", "stop_reason": "end_turn",
            "content": [{"type": "text", "text": "hi"}],
            "usage": {"input_tokens": 12, "cache_read_input_tokens": 8000,
                      "cache_creation_input_tokens": 200, "output_tokens": 50}
        }"#;
        let conv = AnthropicProtocol.parse_response_body(body).unwrap();
        let u = conv.usage.unwrap();
        assert_eq!(u.prompt_tokens, Some(12));
        assert_eq!(u.completion_tokens, Some(50));
        assert_eq!(u.cached_tokens, Some(8000));
    }

    #[test]
    fn rejects_error_response() {
        let body = r#"{"type":"error","error":{"type":"overloaded_error","message":"overloaded"}}"#;
        assert!(AnthropicProtocol.parse_response_body(body).is_none());
    }

    /// extended thinking：响应 content 里的 thinking block → Thinking。
    #[test]
    fn parse_response_thinking_block() {
        let body = r#"{
            "type": "message", "model": "claude-sonnet-4-5", "stop_reason": "end_turn",
            "content": [
                {"type": "thinking", "thinking": "Let me reason...", "signature": "sig"},
                {"type": "text", "text": "answer"}
            ],
            "usage": {"input_tokens": 5, "output_tokens": 7}
        }"#;
        let conv = AnthropicProtocol.parse_response_body(body).unwrap();
        assert_eq!(conv.turns[0].content.len(), 2);
        match &conv.turns[0].content[0] {
            AiContentBlock::Thinking { text } => assert_eq!(text, "Let me reason..."),
            other => panic!("expected thinking block, got {other:?}"),
        }
    }

    /// 多轮工具链请求会回放 thinking block，请求侧同样捕获（与响应口径一致，
    /// 保证前端时间线 LCP 合并时同一 turn 前后形状可比）。
    #[test]
    fn parse_request_replayed_thinking() {
        let body = r#"{
            "messages": [
                {"role": "assistant", "content": [
                    {"type": "thinking", "thinking": "prior reasoning", "signature": "sig"},
                    {"type": "text", "text": "prior answer"}
                ]},
                {"role": "user", "content": "next"}
            ]
        }"#;
        let turns = AnthropicProtocol.parse_request(body).unwrap();
        assert!(
            turns[0].content.iter().any(|b| matches!(b, AiContentBlock::Thinking { .. })),
            "thinking block should be captured from replayed history"
        );
    }

    // ── 流式状态机 ────────────────────────────────────────────────────────────

    #[test]
    fn stream_accumulates_text_and_usage_with_cache() {
        let mut st = AnthropicStreamState::default();
        st.apply(
            "message_start",
            r#"{"type":"message_start","message":{"model":"claude-sonnet-4-5","usage":{"input_tokens":10,"cache_read_input_tokens":900,"output_tokens":1}}}"#,
        );
        st.apply("content_block_start", r#"{"index":0,"content_block":{"type":"text","text":""}}"#);
        st.apply("content_block_delta", r#"{"index":0,"delta":{"type":"text_delta","text":"Hello"}}"#);
        st.apply("content_block_delta", r#"{"index":0,"delta":{"type":"text_delta","text":" world"}}"#);
        st.apply("message_delta", r#"{"delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":42}}"#);
        st.apply("message_stop", r#"{}"#);

        let conv = st.snapshot();
        assert!(!conv.streaming);
        assert_eq!(conv.model.as_deref(), Some("claude-sonnet-4-5"));
        assert_eq!(conv.finish_reason.as_deref(), Some("end_turn"));
        match &conv.turns[0].content[0] {
            AiContentBlock::Text { text } => assert_eq!(text, "Hello world"),
            _ => panic!("expected text block"),
        }
        let u = conv.usage.unwrap();
        assert_eq!(u.prompt_tokens, Some(10));
        assert_eq!(u.completion_tokens, Some(42));
        assert_eq!(u.cached_tokens, Some(900));
        assert_eq!(u.total_tokens, Some(52));
    }

    #[test]
    fn stream_tool_use_partial_json() {
        let mut st = AnthropicStreamState::default();
        st.apply(
            "content_block_start",
            r#"{"index":0,"content_block":{"type":"tool_use","id":"tu_1","name":"get_weather"}}"#,
        );
        st.apply(
            "content_block_delta",
            r#"{"index":0,"delta":{"type":"input_json_delta","partial_json":"{\"city\":"}}"#,
        );
        st.apply(
            "content_block_delta",
            r#"{"index":0,"delta":{"type":"input_json_delta","partial_json":"\"NY\"}"}}"#,
        );
        let conv = st.snapshot();
        assert!(conv.streaming); // 未收到 message_stop
        match &conv.turns[0].content[0] {
            AiContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "tu_1");
                assert_eq!(name, "get_weather");
                assert_eq!(input.get("city").and_then(Value::as_str), Some("NY"));
            }
            _ => panic!("expected tool_use block"),
        }
    }

    /// 流式 thinking：content_block_start(type=thinking) + thinking_delta 累积。
    #[test]
    fn stream_thinking_delta() {
        let mut st = AnthropicStreamState::default();
        st.apply(
            "content_block_start",
            r#"{"index":0,"content_block":{"type":"thinking","thinking":""}}"#,
        );
        st.apply(
            "content_block_delta",
            r#"{"index":0,"delta":{"type":"thinking_delta","thinking":"step 1"}}"#,
        );
        st.apply(
            "content_block_delta",
            r#"{"index":0,"delta":{"type":"thinking_delta","thinking":", step 2"}}"#,
        );
        st.apply("content_block_start", r#"{"index":1,"content_block":{"type":"text","text":""}}"#);
        st.apply("content_block_delta", r#"{"index":1,"delta":{"type":"text_delta","text":"answer"}}"#);

        let conv = st.snapshot();
        assert_eq!(conv.turns[0].content.len(), 2);
        match &conv.turns[0].content[0] {
            AiContentBlock::Thinking { text } => assert_eq!(text, "step 1, step 2"),
            other => panic!("expected thinking block, got {other:?}"),
        }
        match &conv.turns[0].content[1] {
            AiContentBlock::Text { text } => assert_eq!(text, "answer"),
            other => panic!("expected text block, got {other:?}"),
        }
    }
}
