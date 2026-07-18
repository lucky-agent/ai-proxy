//! Google Gemini 归一化（generateContent / streamGenerateContent）。
//!
//! Gemini 请求使用 `contents[]`（而非 `messages[]`），content 为 `parts[]` 数组。
//! 响应使用 `candidates[]`，每 candidate 含 `content.parts[]` + `finishReason`。
//! usage 字段为 `usageMetadata`（promptTokenCount / candidatesTokenCount / …）。
//! 参考 claude-tap SSEReassembler._accumulate_gemini_chunk() 的流式重组逻辑。

use std::collections::BTreeMap;

use serde_json::Value;

use super::normalize::{AiContentBlock, AiConversation, AiTurn, AiUsage};
use super::openai_responses::normalize_usage;
use super::{AiProtocol, StreamState};

// ══════════════════════════════════════════════════════════════════════════════
// 协议实现
// ══════════════════════════════════════════════════════════════════════════════

pub(crate) struct GeminiProtocol;

impl AiProtocol for GeminiProtocol {
    fn name(&self) -> &'static str {
        "gemini"
    }

    /// 解析请求体 `contents[]`（含 systemInstruction + tools）。
    fn parse_request(&self, body: &str) -> Option<Vec<AiTurn>> {
        let p: Value = serde_json::from_str(body).ok()?;
        let contents = p.get("contents")?.as_array()?;
        if contents.is_empty() {
            return None;
        }

        let mut turns: Vec<AiTurn> = Vec::new();

        // systemInstruction → system turn
        if let Some(si) = p.get("systemInstruction") {
            if let Some(parts) = si.get("parts").and_then(Value::as_array) {
                let blocks = parts_to_blocks(parts);
                if !blocks.is_empty() {
                    turns.push(AiTurn::new("system", blocks));
                }
            }
        }

        // tools[] → tools_def turn
        // Gemini tools 格式：[{functionDeclarations: [{name, description, parametersJsonSchema}]}]
        if let Some(tools) = p.get("tools").and_then(Value::as_array) {
            if !tools.is_empty() {
                let tools_json = serde_json::to_string(tools).unwrap_or_default();
                turns.push(AiTurn::new("tools_def", vec![AiContentBlock::text(tools_json)]));
            }
        }

        for m in contents {
            let role_raw = m.get("role").and_then(Value::as_str).unwrap_or("user");
            let role = normalize_role(role_raw);

            let parts = m.get("parts").and_then(Value::as_array);
            let blocks = match parts {
                Some(arr) => parts_to_blocks(arr),
                None => vec![AiContentBlock::text("")],
            };

            if blocks.is_empty() {
                continue;
            }

            // 全部是 tool_result → role = "tool"
            let is_all_tool_result = blocks.iter().all(|b| matches!(b, AiContentBlock::ToolResult { .. }));
            let final_role = if is_all_tool_result { "tool" } else { role };

            turns.push(AiTurn::new(final_role, blocks));
        }

        Some(turns)
    }

    /// 解析非流式响应体（`candidates[]` + `usageMetadata`）。
    fn parse_response_body(&self, body: &str) -> Option<AiConversation> {
        let p: Value = serde_json::from_str(body).ok()?;
        let candidates = p.get("candidates")?.as_array()?;
        if candidates.is_empty() {
            return None;
        }

        let mut blocks: Vec<AiContentBlock> = Vec::new();
        let mut finish_reason: Option<String> = None;

        for candidate in candidates {
            if finish_reason.is_none() {
                finish_reason = candidate
                    .get("finishReason")
                    .and_then(Value::as_str)
                    .map(String::from);
            }
            if let Some(content) = candidate.get("content") {
                if let Some(parts) = content.get("parts").and_then(Value::as_array) {
                    blocks.extend(parts_to_blocks(parts));
                }
            }
        }

        if blocks.is_empty() {
            blocks.push(AiContentBlock::text(""));
        }

        let usage = p
            .get("usageMetadata")
            .map(normalize_usage);

        Some(AiConversation {
            provider: "gemini".to_string(),
            turns: vec![AiTurn::new("assistant", blocks)],
            streaming: false,
            model: p.get("model").and_then(Value::as_str).map(String::from),
            usage,
            finish_reason,
        })
    }

    fn create_stream_state(&self) -> Box<dyn StreamState> {
        Box::new(GeminiStreamState::default())
    }
}

/// Gemini 的 assistant role 为 "model"，归一化为 "assistant"。
fn normalize_role(role: &str) -> &str {
    match role {
        "model" => "assistant",
        "user" | "tool" | "function" => role,
        _ => "user",
    }
}

/// 将 functionCall 的 args 或 functionResponse 的 response 尝试解析为 JSON。
fn parse_tool_args(raw: &Value) -> Value {
    match raw {
        Value::Object(_) => raw.clone(),
        Value::String(s) => {
            if s.is_empty() {
                Value::Object(Default::default())
            } else {
                serde_json::from_str(s).unwrap_or_else(|_| Value::String(s.clone()))
            }
        }
        _ => Value::Object(Default::default()),
    }
}

/// 从 parts 数组提取 content blocks（text / thinking / tool_use / tool_result）。
fn parts_to_blocks(parts: &[Value]) -> Vec<AiContentBlock> {
    let mut blocks: Vec<AiContentBlock> = Vec::new();
    for part in parts {
        let Some(part_obj) = part.as_object() else { continue };

        // text / thinking
        if let Some(text) = part_obj.get("text").and_then(Value::as_str) {
            if !text.is_empty() {
                blocks.push(AiContentBlock::text(text));
            }
        }

        // functionCall → tool_use
        if let Some(call) = part_obj.get("functionCall") {
            blocks.push(AiContentBlock::ToolUse {
                id: call.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
                name: call
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("tool_use")
                    .to_string(),
                input: parse_tool_args(call.get("args").unwrap_or(&Value::Object(Default::default()))),
            });
        }

        // functionResponse → tool_result
        if let Some(resp) = part_obj.get("functionResponse") {
            let tool_use_id = resp
                .get("id")
                .or_else(|| resp.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let inner = match resp.get("response") {
                Some(r) => {
                    if let Some(s) = r.as_str() {
                        vec![AiContentBlock::text(s)]
                    } else {
                        vec![AiContentBlock::text(r.to_string())]
                    }
                }
                None => vec![AiContentBlock::text("")],
            };
            blocks.push(AiContentBlock::ToolResult {
                tool_use_id,
                content: inner,
            });
        }
    }
    blocks
}

// ══════════════════════════════════════════════════════════════════════════════
// 流式 SSE 状态机
// ══════════════════════════════════════════════════════════════════════════════
// Gemini 使用裸 `data: {...}` 帧（无 event: header），帧内含 candidates[] + usageMetadata。

struct GeminiToolCall {
    name: String,
    args: Value,
}

#[derive(Default)]
struct GeminiCandidate {
    text: String,
    tool_calls: Vec<GeminiToolCall>,
    finish_reason: Option<String>,
}

#[derive(Default)]
struct GeminiStreamState {
    model: Option<String>,
    usage: Option<AiUsage>,
    done: bool,
    candidates: BTreeMap<i64, GeminiCandidate>,
}

impl StreamState for GeminiStreamState {
    fn apply(&mut self, _event: &str, data: &str) {
        let Ok(p) = serde_json::from_str::<Value>(data) else { return };
        // data 可直接为 Gemini chunk，也可能包裹在 {"response": {...}} 中
        let chunk = p
            .get("response")
            .filter(|r| r.get("candidates").is_some() || r.get("usageMetadata").is_some())
            .unwrap_or(&p);

        if self.model.is_none() {
            if let Some(m) = chunk.get("model").and_then(Value::as_str) {
                self.model = Some(m.to_string());
            }
        }

        if let Some(candidates) = chunk.get("candidates").and_then(Value::as_array) {
            for (pos, candidate) in candidates.iter().enumerate() {
                let idx = candidate.get("index").and_then(Value::as_i64).unwrap_or(pos as i64);
                let entry = self.candidates.entry(idx).or_default();
                if let Some(content) = candidate.get("content") {
                    if let Some(parts) = content.get("parts").and_then(Value::as_array) {
                        for part in parts {
                            if let Some(text) = part.get("text").and_then(Value::as_str) {
                                entry.text.push_str(text);
                            }
                            if let Some(call) = part.get("functionCall") {
                                entry.tool_calls.push(GeminiToolCall {
                                    name: call.get("name").and_then(Value::as_str).unwrap_or("tool_use").to_string(),
                                    args: call.get("args").cloned().unwrap_or(Value::Object(Default::default())),
                                });
                            }
                        }
                    }
                }
                if let Some(fr) = candidate.get("finishReason").and_then(Value::as_str) {
                    entry.finish_reason = Some(fr.to_string());
                }
            }
            let all_done = candidates.iter().all(|c| c.get("finishReason").is_some());
            if all_done {
                self.done = true;
            }
        }

        if let Some(um) = chunk.get("usageMetadata") {
            self.usage = Some(normalize_usage(um));
        }
    }

    fn snapshot(&self) -> AiConversation {
        let mut blocks: Vec<AiContentBlock> = Vec::new();
        for c in self.candidates.values() {
            if !c.text.is_empty() {
                blocks.push(AiContentBlock::text(c.text.clone()));
            }
            for tc in &c.tool_calls {
                blocks.push(AiContentBlock::ToolUse {
                    id: String::new(),
                    name: tc.name.clone(),
                    input: tc.args.clone(),
                });
            }
        }
        if blocks.is_empty() { blocks.push(AiContentBlock::text("")); }
        let finish_reason = self.candidates.values().find_map(|c| c.finish_reason.clone());
        AiConversation {
            provider: "gemini".to_string(),
            turns: vec![AiTurn::new("assistant", blocks)],
            streaming: !self.done,
            model: self.model.clone(),
            usage: self.usage.clone(),
            finish_reason,
        }
    }

    fn finalize(&mut self) {
        self.done = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_request ──────────────────────────────────────────────────────────

    #[test]
    fn parse_simple_user_message() {
        let body = r#"{
            "contents": [{"role": "user", "parts": [{"text": "Hello"}]}]
        }"#;
        let turns = GeminiProtocol.parse_request(body).unwrap();
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].role, "user");
        match &turns[0].content[0] {
            AiContentBlock::Text { text } => assert_eq!(text, "Hello"),
            _ => panic!("expected text block"),
        }
    }

    #[test]
    fn parse_model_role_maps_to_assistant() {
        let body = r#"{
            "contents": [{"role": "model", "parts": [{"text": "Hi!"}]}]
        }"#;
        let turns = GeminiProtocol.parse_request(body).unwrap();
        assert_eq!(turns[0].role, "assistant");
    }

    #[test]
    fn parse_system_instruction() {
        let body = r#"{
            "systemInstruction": {"parts": [{"text": "You are helpful."}]},
            "contents": [{"role": "user", "parts": [{"text": "hi"}]}]
        }"#;
        let turns = GeminiProtocol.parse_request(body).unwrap();
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].role, "system");
        match &turns[0].content[0] {
            AiContentBlock::Text { text } => assert_eq!(text, "You are helpful."),
            _ => panic!("expected system text"),
        }
    }

    #[test]
    fn parse_tools_def() {
        let body = r#"{
            "tools": [{"functionDeclarations": [{"name": "get_weather", "description": "Get weather", "parametersJsonSchema": {"type": "object"}}]}],
            "contents": [{"role": "user", "parts": [{"text": "weather?"}]}]
        }"#;
        let turns = GeminiProtocol.parse_request(body).unwrap();
        assert_eq!(turns[0].role, "tools_def");
    }

    #[test]
    fn parse_function_call_and_response() {
        let body = r#"{
            "contents": [
                {"role": "model", "parts": [{"functionCall": {"name": "get_weather", "args": {"city": "NY"}}}]},
                {"role": "user", "parts": [{"functionResponse": {"id": "call_1", "name": "get_weather", "response": {"output": "sunny"}}}]}
            ]
        }"#;
        let turns = GeminiProtocol.parse_request(body).unwrap();
        assert_eq!(turns.len(), 2);
        // assistant turn with tool_use
        let has_tool_use = turns[0]
            .content
            .iter()
            .any(|b| matches!(b, AiContentBlock::ToolUse { .. }));
        assert!(has_tool_use);
        // tool turn with tool_result
        assert_eq!(turns[1].role, "tool");
        let has_tool_result = turns[1]
            .content
            .iter()
            .any(|b| matches!(b, AiContentBlock::ToolResult { .. }));
        assert!(has_tool_result);
    }

    // ── parse_response_body ────────────────────────────────────────────────────

    #[test]
    fn parse_simple_response() {
        let body = r#"{
            "candidates": [{
                "content": {"role": "model", "parts": [{"text": "Hello!"}]},
                "finishReason": "STOP",
                "index": 0
            }],
            "usageMetadata": {"promptTokenCount": 10, "candidatesTokenCount": 5, "totalTokenCount": 15}
        }"#;
        let conv = GeminiProtocol.parse_response_body(body).unwrap();
        assert_eq!(conv.provider, "gemini");
        assert_eq!(conv.finish_reason.as_deref(), Some("STOP"));
        match &conv.turns[0].content[0] {
            AiContentBlock::Text { text } => assert_eq!(text, "Hello!"),
            _ => panic!("expected text block"),
        }
        let u = conv.usage.unwrap();
        assert_eq!(u.prompt_tokens, Some(10));
        assert_eq!(u.completion_tokens, Some(5));
        assert_eq!(u.total_tokens, Some(15));
    }

    #[test]
    fn parse_response_with_function_call() {
        let body = r#"{
            "candidates": [{
                "content": {"role": "model", "parts": [{"functionCall": {"name": "get_weather", "args": {"city": "NY"}}}]},
                "finishReason": "STOP"
            }],
            "usageMetadata": {"promptTokenCount": 5, "candidatesTokenCount": 2}
        }"#;
        let conv = GeminiProtocol.parse_response_body(body).unwrap();
        let has_tool_use = conv.turns[0]
            .content
            .iter()
            .any(|b| matches!(b, AiContentBlock::ToolUse { .. }));
        assert!(has_tool_use);
    }
}
