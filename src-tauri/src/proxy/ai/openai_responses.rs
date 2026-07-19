//! OpenAI Responses API 归一化（请求 input[] + 非流式响应 output[] + 流式 SSE）。
//!
//! Chat Completions（/v1/chat/completions）使用 messages[]，Responses（/v1/responses）
//! 使用 input[]；两者在请求体结构、响应结构、SSE 事件类型上完全不同。
//! 参考 claude-tap SSEReassembler._accumulate() 的 Responses API 流式重组逻辑。

use std::collections::BTreeMap;

use serde_json::Value;

use super::normalize::{
    AiContentBlock, AiConversation, AiTurn, AiUsage, normalize_usage, parse_tool_input,
};
use super::{AiProtocol, StreamState};

// ══════════════════════════════════════════════════════════════════════════════
// 协议实现
// ══════════════════════════════════════════════════════════════════════════════

pub(crate) struct OpenAiResponsesProtocol;

impl AiProtocol for OpenAiResponsesProtocol {
    /// 解析 Responses API 请求体（`input[]` + 顶层 `instructions` + `tools[]`）。
    fn parse_request(&self, body: &str) -> Option<Vec<AiTurn>> {
        let p: Value = serde_json::from_str(body).ok()?;
        let input = p.get("input")?.as_array()?;
        if input.is_empty() {
            return None;
        }

        let mut turns: Vec<AiTurn> = Vec::new();

        if let Some(instructions) = p.get("instructions").and_then(Value::as_str) {
            if !instructions.is_empty() {
                turns.push(AiTurn::new("system", vec![AiContentBlock::text(instructions)]));
            }
        }

        if let Some(t) = p.get("tools").and_then(Value::as_array).and_then(|ts| AiTurn::tools_def(ts)) {
            turns.push(t);
        }

        for m in input {
            let role_raw = m.get("role").and_then(Value::as_str).unwrap_or("user");
            let role = match role_raw {
                "system" | "user" | "assistant" | "tool" => role_raw,
                _ => "user",
            };
            let content = m.get("content");

            if role == "tool" {
                let inner = match content {
                    Some(Value::String(s)) => vec![AiContentBlock::text(s.clone())],
                    Some(Value::Array(arr)) => {
                        let texts: Vec<AiContentBlock> = arr
                            .iter()
                            .filter(|b| {
                                matches!(b.get("type").and_then(Value::as_str), Some("input_text" | "output_text"))
                            })
                            .filter_map(|b| b.get("text").and_then(Value::as_str).map(AiContentBlock::text))
                            .collect();
                        if texts.is_empty() { vec![AiContentBlock::text("")] } else { texts }
                    }
                    _ => vec![AiContentBlock::text("")],
                };
                let tool_use_id = m.get("tool_call_id").and_then(Value::as_str).unwrap_or("").to_string();
                turns.push(AiTurn::new(role, vec![AiContentBlock::ToolResult { tool_use_id, content: inner }]));
                continue;
            }

            let mut blocks: Vec<AiContentBlock> = match content {
                Some(Value::String(s)) => vec![AiContentBlock::text(s.clone())],
                Some(Value::Array(arr)) => {
                    let texts: Vec<AiContentBlock> = arr
                        .iter()
                        .filter_map(|b| {
                            let t = b.get("type").and_then(Value::as_str)?;
                            if t == "input_text" || t == "output_text" {
                                b.get("text").and_then(Value::as_str).map(AiContentBlock::text)
                            } else { None }
                        })
                        .collect();
                    if texts.is_empty() { vec![AiContentBlock::text("")] } else { texts }
                }
                _ => vec![AiContentBlock::text("")],
            };

            if role == "assistant" {
                if let Some(tcs) = m.get("tool_calls").and_then(Value::as_array) {
                    for tc in tcs {
                        let id = tc.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                        let func = tc.get("function");
                        let name = func.and_then(|f| f.get("name")).and_then(Value::as_str).unwrap_or("").to_string();
                        let args = func.and_then(|f| f.get("arguments")).and_then(Value::as_str).unwrap_or("");
                        blocks.push(AiContentBlock::ToolUse { id, name, input: parse_tool_input(args) });
                    }
                }
            }

            turns.push(AiTurn::new(role, blocks));
        }
        Some(turns)
    }

    /// 解析非流式 Responses API 响应体（`object: "response"`，含 `output[]` + `usage`）。
    fn parse_response_body(&self, body: &str) -> Option<AiConversation> {
        let p: Value = serde_json::from_str(body).ok()?;
        if p.get("object").and_then(Value::as_str) != Some("response") {
            return None;
        }

        let output = p.get("output").and_then(Value::as_array)?;
        let mut blocks: Vec<AiContentBlock> = Vec::new();
        for item in output {
            let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
            match item_type {
                "message" => {
                    if let Some(arr) = item.get("content").and_then(Value::as_array) {
                        for c in arr {
                            if matches!(c.get("type").and_then(Value::as_str), Some("output_text" | "input_text")) {
                                if let Some(t) = c.get("text").and_then(Value::as_str) {
                                    blocks.push(AiContentBlock::text(t));
                                }
                            }
                        }
                    }
                }
                "function_call" | "tool_use" => {
                    blocks.push(AiContentBlock::ToolUse {
                        id: item.get("id").or_else(|| item.get("call_id")).and_then(Value::as_str).unwrap_or("").to_string(),
                        name: item.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
                        input: item.get("arguments").or_else(|| item.get("input")).cloned().unwrap_or(Value::Object(Default::default())),
                    });
                }
                "reasoning" => {
                    let summary = reasoning_summary_text(item);
                    if !summary.is_empty() {
                        blocks.push(AiContentBlock::thinking(summary));
                    }
                }
                _ => {}
            }
        }

        if blocks.is_empty() {
            blocks.push(AiContentBlock::text(""));
        }

        Some(AiConversation::new(
            "openai",
            vec![AiTurn::new("assistant", blocks)],
            false,
            p.get("model").and_then(Value::as_str).map(String::from),
            p.get("usage").map(normalize_usage),
            p.get("status").and_then(Value::as_str).map(String::from),
        ))
    }

    fn create_stream_state(&self) -> Box<dyn StreamState> {
        Box::new(OpenAiResponsesStreamState::default())
    }
}

/// reasoning item 的 summary[] 文本拼接（加密的 encrypted_content 不采集）。
fn reasoning_summary_text(item: &Value) -> String {
    item.get("summary")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter(|s| s.get("type").and_then(Value::as_str) == Some("summary_text"))
                .filter_map(|s| s.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

// ══════════════════════════════════════════════════════════════════════════════
// 流式 SSE 状态机
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Default)]
struct OpenAiOutputItem {
    item_type: String,
    /// message 正文 / reasoning summary 的累积文本。
    text: String,
    tool_id: String,
    tool_name: String,
    tool_arguments: String,
}

#[derive(Default)]
struct OpenAiResponsesStreamState {
    model: Option<String>,
    status: Option<String>,
    usage: Option<AiUsage>,
    done: bool,
    output_items: BTreeMap<i64, OpenAiOutputItem>,
}

impl StreamState for OpenAiResponsesStreamState {
    fn apply(&mut self, event: &str, data: &str) {
        let Ok(p) = serde_json::from_str::<Value>(data) else { return };
        match event {
            "response.created" => {
                let r = p.get("response");
                if self.model.is_none() {
                    self.model = r.and_then(|r| r.get("model")).and_then(Value::as_str).map(String::from);
                }
                self.status = r.and_then(|r| r.get("status")).and_then(Value::as_str).map(String::from);
            }
            "response.output_item.added" => {
                let idx = p.get("output_index").and_then(Value::as_i64).unwrap_or(0);
                let item_type = p
                    .get("item")
                    .and_then(|i| i.get("type"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                self.output_items
                    .entry(idx)
                    .or_insert(OpenAiOutputItem { item_type, ..Default::default() });
            }
            "response.output_text.delta" => {
                let delta = p.get("delta").and_then(Value::as_str).unwrap_or("");
                let idx = p.get("output_index").and_then(Value::as_i64).unwrap_or(0);
                self.output_items.entry(idx).or_default().text.push_str(delta);
            }
            "response.output_item.done" => {
                let idx = p.get("output_index").and_then(Value::as_i64).unwrap_or(0);
                if let Some(item) = p.get("item") {
                    let entry = self.output_items.entry(idx).or_default();
                    if entry.text.is_empty() {
                        if let Some(content) = item.get("content").and_then(Value::as_array) {
                            entry.text = content
                                .iter()
                                .filter(|c| matches!(c.get("type").and_then(Value::as_str), Some("output_text" | "input_text")))
                                .filter_map(|c| c.get("text").and_then(Value::as_str))
                                .collect::<Vec<_>>()
                                .join("");
                        }
                    }
                    if let Some(t) = item.get("type").and_then(Value::as_str) { entry.item_type = t.to_string(); }
                    if entry.item_type == "function_call" {
                        entry.tool_id = item.get("id").or_else(|| item.get("call_id")).and_then(Value::as_str).unwrap_or("").to_string();
                        entry.tool_name = item.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                        entry.tool_arguments = item.get("arguments").and_then(Value::as_str).unwrap_or("").to_string();
                    }
                    if entry.item_type == "reasoning" {
                        entry.text = reasoning_summary_text(item);
                    }
                }
            }
            "response.completed" | "response.done" | "response.incomplete" | "response.failed" => {
                let r = p.get("response");
                if let Some(r) = r {
                    if let Some(s) = r.get("status").and_then(Value::as_str) { self.status = Some(s.to_string()); }
                    if let Some(m) = r.get("model").and_then(Value::as_str) { self.model = Some(m.to_string()); }
                    if let Some(u) = r.get("usage") { self.usage = Some(normalize_usage(u)); }
                    if let Some(output) = r.get("output").and_then(Value::as_array) {
                        if !output.is_empty() {
                            for (i, o) in output.iter().enumerate() {
                                let idx = i as i64;
                                let entry = self.output_items.entry(idx).or_default();
                                merge_terminal_output_item(entry, o);
                            }
                        }
                    }
                }
                if self.usage.is_none() {
                    if let Some(u) = p.get("usage") { self.usage = Some(normalize_usage(u)); }
                }
                self.done = true;
            }
            "response.error" => {
                self.status = Some("failed".to_string());
                self.done = true;
            }
            _ => {}
        }
    }

    fn snapshot(&self) -> AiConversation {
        let mut blocks: Vec<AiContentBlock> = Vec::new();
        for item in self.output_items.values() {
            match item.item_type.as_str() {
                "message" => {
                    if !item.text.is_empty() { blocks.push(AiContentBlock::text(item.text.clone())); }
                }
                "reasoning" => {
                    if !item.text.is_empty() { blocks.push(AiContentBlock::thinking(item.text.clone())); }
                }
                "function_call" => {
                    let input = if item.tool_arguments.is_empty() {
                        Value::Object(Default::default())
                    } else {
                        serde_json::from_str(&item.tool_arguments).unwrap_or_else(|_| Value::String(item.tool_arguments.clone()))
                    };
                    blocks.push(AiContentBlock::ToolUse { id: item.tool_id.clone(), name: item.tool_name.clone(), input });
                }
                _ => {}
            }
        }
        if blocks.is_empty() { blocks.push(AiContentBlock::text("")); }
        AiConversation::new(
            "openai",
            vec![AiTurn::new("assistant", blocks)],
            !self.done,
            self.model.clone(),
            self.usage.clone(),
            self.status.clone(),
        )
    }

    fn finalize(&mut self) {
        self.done = true;
    }
}

fn merge_terminal_output_item(entry: &mut OpenAiOutputItem, item: &Value) {
    if let Some(t) = item.get("type").and_then(Value::as_str) { entry.item_type = t.to_string(); }
    if entry.item_type == "message" {
        if let Some(content) = item.get("content").and_then(Value::as_array) {
            entry.text = content
                .iter()
                .filter(|c| matches!(c.get("type").and_then(Value::as_str), Some("output_text" | "input_text")))
                .filter_map(|c| c.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("");
        }
    }
    if entry.item_type == "reasoning" {
        entry.text = reasoning_summary_text(item);
    }
    if entry.item_type == "function_call" {
        entry.tool_id = item.get("id").or_else(|| item.get("call_id")).and_then(Value::as_str).unwrap_or("").to_string();
        entry.tool_name = item.get("name").and_then(Value::as_str).unwrap_or("").to_string();
        entry.tool_arguments = item.get("arguments").and_then(Value::as_str).unwrap_or("").to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_input() {
        let turns = OpenAiResponsesProtocol.parse_request(r#"{"model":"gpt-4o","input":[{"role":"user","content":"hello"}]}"#).unwrap();
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].role, "user");
        match &turns[0].content[0] {
            AiContentBlock::Text { text } => assert_eq!(text, "hello"),
            _ => panic!("expected text block"),
        }
    }

    #[test]
    fn parse_input_text_blocks() {
        let turns = OpenAiResponsesProtocol.parse_request(r#"{"model":"gpt-4o","input":[{"role":"user","content":[{"type":"input_text","text":"hi there"}]}]}"#).unwrap();
        assert_eq!(turns.len(), 1);
        match &turns[0].content[0] {
            AiContentBlock::Text { text } => assert_eq!(text, "hi there"),
            _ => panic!("expected text block"),
        }
    }

    #[test]
    fn parse_with_instructions() {
        let turns = OpenAiResponsesProtocol.parse_request(r#"{"model":"gpt-4o","instructions":"You are helpful.","input":[{"role":"user","content":"hi"}]}"#).unwrap();
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].role, "system");
        match &turns[0].content[0] {
            AiContentBlock::Text { text } => assert_eq!(text, "You are helpful."),
            _ => panic!("expected text block"),
        }
    }

    #[test]
    fn parse_with_tools() {
        let turns = OpenAiResponsesProtocol.parse_request(r#"{"model":"gpt-4o","tools":[{"type":"function","name":"get_weather"}],"input":[{"role":"user","content":"weather?"}]}"#).unwrap();
        assert_eq!(turns[0].role, "tools_def");
    }

    #[test]
    fn parse_tool_result() {
        let body = r#"{
            "model": "gpt-4o",
            "input": [
                {"role": "assistant", "content": "let me check", "tool_calls": [{"id":"call_1","type":"function","function":{"name":"get_weather","arguments":"{\"city\":\"NY\"}"}}]},
                {"role": "tool", "tool_call_id": "call_1", "content": "sunny, 22°C"}
            ]
        }"#;
        let turns = OpenAiResponsesProtocol.parse_request(body).unwrap();
        assert_eq!(turns.len(), 2);
        assert!(turns[0].content.iter().any(|b| matches!(b, AiContentBlock::ToolUse { .. })));
        assert_eq!(turns[1].role, "tool");
        assert!(turns[1].content.iter().any(|b| matches!(b, AiContentBlock::ToolResult { .. })));
    }

    #[test]
    fn parse_non_streaming_response() {
        let conv = OpenAiResponsesProtocol.parse_response_body(r#"{
            "id": "resp_123", "object": "response", "model": "gpt-4o", "status": "completed",
            "output": [{"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "Hello!"}]}],
            "usage": {"input_tokens": 10, "output_tokens": 5, "total_tokens": 15}
        }"#).unwrap();
        assert_eq!(conv.provider, "openai");
        assert_eq!(conv.finish_reason.as_deref(), Some("completed"));
        match &conv.turns[0].content[0] {
            AiContentBlock::Text { text } => assert_eq!(text, "Hello!"),
            _ => panic!("expected text block"),
        }
    }

    #[test]
    fn parse_response_with_function_call() {
        let conv = OpenAiResponsesProtocol.parse_response_body(r#"{
            "id": "resp_456", "object": "response", "model": "gpt-4o", "status": "completed",
            "output": [{"type": "function_call", "id": "fc_1", "call_id": "call_abc", "name": "get_weather", "arguments": "{\"city\":\"NY\"}"}],
            "usage": {"input_tokens": 5, "output_tokens": 2, "total_tokens": 7}
        }"#).unwrap();
        assert!(conv.turns[0].content.iter().any(|b| matches!(b, AiContentBlock::ToolUse { .. })));
    }

    #[test]
    fn rejects_non_response_object() {
        assert!(OpenAiResponsesProtocol.parse_response_body(r#"{"object":"chat.completion","choices":[{"message":{"role":"assistant","content":"hi"}}]}"#).is_none());
    }

    /// o 系列 reasoning item 的 summary 文本 → Thinking block。
    #[test]
    fn parse_response_reasoning_item() {
        let conv = OpenAiResponsesProtocol.parse_response_body(r#"{
            "id": "resp_1", "object": "response", "model": "o4-mini", "status": "completed",
            "output": [
                {"type": "reasoning", "summary": [{"type": "summary_text", "text": "step by step"}]},
                {"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "done"}]}
            ]
        }"#).unwrap();
        assert_eq!(conv.turns[0].content.len(), 2);
        match &conv.turns[0].content[0] {
            AiContentBlock::Thinking { text } => assert_eq!(text, "step by step"),
            other => panic!("expected thinking block, got {other:?}"),
        }
        match &conv.turns[0].content[1] {
            AiContentBlock::Text { text } => assert_eq!(text, "done"),
            other => panic!("expected text block, got {other:?}"),
        }
    }

    /// 流式 reasoning item：output_item.done 时提取 summary 文本。
    #[test]
    fn stream_reasoning_item_done() {
        let mut st = OpenAiResponsesStreamState::default();
        st.apply("response.output_item.added", r#"{"output_index":0,"item":{"type":"reasoning"}}"#);
        st.apply(
            "response.output_item.done",
            r#"{"output_index":0,"item":{"type":"reasoning","summary":[{"type":"summary_text","text":"thought hard"}]}}"#,
        );
        st.apply("response.output_item.added", r#"{"output_index":1,"item":{"type":"message","role":"assistant"}}"#);
        st.apply("response.output_text.delta", r#"{"output_index":1,"delta":"hi"}"#);
        let conv = st.snapshot();
        assert_eq!(conv.turns[0].content.len(), 2);
        match &conv.turns[0].content[0] {
            AiContentBlock::Thinking { text } => assert_eq!(text, "thought hard"),
            other => panic!("expected thinking block, got {other:?}"),
        }
        match &conv.turns[0].content[1] {
            AiContentBlock::Text { text } => assert_eq!(text, "hi"),
            other => panic!("expected text block, got {other:?}"),
        }
    }
}
