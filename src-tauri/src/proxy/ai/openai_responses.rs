//! OpenAI Responses API 归一化（请求 input[] + 非流式响应 output[] + 流式 SSE）。
//!
//! Chat Completions（/v1/chat/completions）使用 messages[]，Responses（/v1/responses）
//! 使用 input[]；两者在请求体结构、响应结构、SSE 事件类型上完全不同。
//! 参考 claude-tap SSEReassembler._accumulate() 的 Responses API 流式重组逻辑。

use std::collections::BTreeMap;

use serde_json::Value;

use super::normalize::{AiContentBlock, AiConversation, AiTurn, AiUsage};
use super::{AiProtocol, StreamState};

// ══════════════════════════════════════════════════════════════════════════════
// 协议实现
// ══════════════════════════════════════════════════════════════════════════════

pub(crate) struct OpenAiResponsesProtocol;

impl AiProtocol for OpenAiResponsesProtocol {
    fn name(&self) -> &'static str {
        "openai"
    }

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

        if let Some(tools) = p.get("tools").and_then(Value::as_array) {
            if !tools.is_empty() {
                let tools_json = serde_json::to_string(tools).unwrap_or_default();
                turns.push(AiTurn::new("tools_def", vec![AiContentBlock::text(tools_json)]));
            }
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
                _ => {}
            }
        }

        if blocks.is_empty() {
            blocks.push(AiContentBlock::text(""));
        }

        Some(AiConversation {
            provider: "openai".to_string(),
            turns: vec![AiTurn::new("assistant", blocks)],
            streaming: false,
            model: p.get("model").and_then(Value::as_str).map(String::from),
            usage: p.get("usage").map(normalize_usage),
            finish_reason: p.get("status").and_then(Value::as_str).map(String::from),
        })
    }

    fn create_stream_state(&self) -> Box<dyn StreamState> {
        Box::new(OpenAiResponsesStreamState::default())
    }
}

/// 通用 usage 归一化：兼容 OpenAI Chat Completions (prompt/completion)、
/// Anthropic (input/output)、Responses API (input/output)、Gemini (promptTokenCount/…) 四种命名。
pub(crate) fn normalize_usage(usage: &Value) -> AiUsage {
    fn get_u64(v: &Value, keys: &[&str]) -> Option<u64> {
        for k in keys {
            if let Some(val) = v.get(k).and_then(Value::as_u64) {
                return Some(val);
            }
        }
        None
    }

    let input_tokens = get_u64(usage, &["input_tokens", "prompt_tokens", "promptTokenCount", "inputTokens"]);
    let output_tokens = get_u64(usage, &["output_tokens", "completion_tokens", "candidatesTokenCount", "outputTokens"]);
    let total_tokens = get_u64(usage, &["total_tokens", "totalTokens", "totalTokenCount"]);

    let cached_tokens = get_u64(usage, &["cache_read_input_tokens"])
        .or_else(|| {
            get_u64(usage, &["cached_tokens", "cachedContentTokenCount", "cacheReadInputTokens"])
        })
        .or_else(|| {
            usage
                .get("input_tokens_details")
                .or_else(|| usage.get("prompt_tokens_details"))
                .and_then(|d| d.get("cached_tokens"))
                .and_then(Value::as_u64)
        });

    let _cache_create = usage.get("cacheWriteInputTokens").and_then(Value::as_u64);

    AiUsage {
        prompt_tokens: input_tokens,
        completion_tokens: output_tokens,
        total_tokens: total_tokens.or_else(|| {
            match (input_tokens, output_tokens) {
                (None, None) => None,
                (i, o) => Some(i.unwrap_or(0) + o.unwrap_or(0)),
            }
        }),
        cached_tokens,
        ..Default::default()
    }
}

fn parse_tool_input(raw: &str) -> Value {
    if raw.is_empty() {
        return Value::String(String::new());
    }
    serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_string()))
}

fn _blocks_to_text(blocks: &[Value], text_type: &str) -> String {
    blocks
        .iter()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some(text_type))
        .filter_map(|b| b.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("")
}

// ══════════════════════════════════════════════════════════════════════════════
// 流式 SSE 状态机
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Default)]
struct OpenAiOutputItem {
    item_type: String,
    #[allow(dead_code)]
    role: String,
    text: String,
    tool_id: String,
    #[allow(dead_code)]
    tool_call_id: String,
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
                let item = p.get("item");
                let item_type = item.and_then(|i| i.get("type")).and_then(Value::as_str).unwrap_or("").to_string();
                let mut entry = OpenAiOutputItem { item_type, ..Default::default() };
                if let Some(it) = item {
                    if let Some(role) = it.get("role").and_then(Value::as_str) {
                        entry.role = role.to_string();
                    }
                }
                self.output_items.entry(idx).or_insert(entry);
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
                    if let Some(role) = item.get("role").and_then(Value::as_str) { entry.role = role.to_string(); }
                    if entry.item_type == "function_call" {
                        entry.tool_id = item.get("id").or_else(|| item.get("call_id")).and_then(Value::as_str).unwrap_or("").to_string();
                        entry.tool_name = item.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                        entry.tool_arguments = item.get("arguments").and_then(Value::as_str).unwrap_or("").to_string();
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
        AiConversation {
            provider: "openai".to_string(),
            turns: vec![AiTurn::new("assistant", blocks)],
            streaming: !self.done,
            model: self.model.clone(),
            usage: self.usage.clone(),
            finish_reason: self.status.clone(),
        }
    }

    fn finalize(&mut self) {
        self.done = true;
    }
}

fn merge_terminal_output_item(entry: &mut OpenAiOutputItem, item: &Value) {
    if let Some(t) = item.get("type").and_then(Value::as_str) { entry.item_type = t.to_string(); }
    if let Some(role) = item.get("role").and_then(Value::as_str) { entry.role = role.to_string(); }
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
    fn usage_openai_chat_completion() {
        let u = normalize_usage(&serde_json::json!({
            "prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15,
            "prompt_tokens_details": {"cached_tokens": 3}
        }));
        assert_eq!(u.prompt_tokens, Some(10));
        assert_eq!(u.completion_tokens, Some(5));
        assert_eq!(u.total_tokens, Some(15));
        assert_eq!(u.cached_tokens, Some(3));
    }

    #[test]
    fn usage_responses_api() {
        let u = normalize_usage(&serde_json::json!({
            "input_tokens": 100, "output_tokens": 50, "total_tokens": 150,
            "input_tokens_details": {"cached_tokens": 20}
        }));
        assert_eq!(u.prompt_tokens, Some(100));
        assert_eq!(u.completion_tokens, Some(50));
        assert_eq!(u.total_tokens, Some(150));
        assert_eq!(u.cached_tokens, Some(20));
    }

    #[test]
    fn usage_anthropic_fallback() {
        let u = normalize_usage(&serde_json::json!({"input_tokens": 200, "output_tokens": 100}));
        assert_eq!(u.prompt_tokens, Some(200));
        assert_eq!(u.completion_tokens, Some(100));
        assert_eq!(u.total_tokens, Some(300));
    }

    #[test]
    fn usage_google_gemini_fallback() {
        let u = normalize_usage(&serde_json::json!({
            "promptTokenCount": 42, "candidatesTokenCount": 7, "totalTokenCount": 49, "cachedContentTokenCount": 8
        }));
        assert_eq!(u.prompt_tokens, Some(42));
        assert_eq!(u.completion_tokens, Some(7));
        assert_eq!(u.total_tokens, Some(49));
        assert_eq!(u.cached_tokens, Some(8));
    }

    #[test]
    fn usage_bedrock_converse() {
        let u = normalize_usage(&serde_json::json!({
            "inputTokens": 12, "outputTokens": 3, "totalTokens": 15,
            "cacheReadInputTokens": 7, "cacheWriteInputTokens": 5
        }));
        assert_eq!(u.prompt_tokens, Some(12));
        assert_eq!(u.completion_tokens, Some(3));
        assert_eq!(u.total_tokens, Some(15));
        assert_eq!(u.cached_tokens, Some(7));
    }

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
}
