//! OpenAI Chat Completions 归一化（请求 messages + 非流式响应 + 流式 SSE）。
//! 语义镜像前端 `src/lib/ai/providers/openai.ts`。

use std::collections::BTreeMap;

use serde_json::Value;

use super::normalize::{
    AiContentBlock, AiConversation, AiTurn, AiUsage, normalize_usage, parse_tool_input,
};
use super::{AiProtocol, StreamState};

// ══════════════════════════════════════════════════════════════════════════════
// 协议实现
// ══════════════════════════════════════════════════════════════════════════════

/// OpenAI Chat Completions 协议。
pub(crate) struct OpenAiChatProtocol;

impl AiProtocol for OpenAiChatProtocol {
    /// 解析请求体 `messages[]`（含 system/user/assistant/tool + tool_calls + tools 定义）。
    fn parse_request(&self, body: &str) -> Option<Vec<AiTurn>> {
        let p: Value = serde_json::from_str(body).ok()?;
        let messages = p.get("messages")?.as_array()?;
        if messages.is_empty() {
            return None;
        }
        // 系统提示词排在 tools 定义之前：找到开头连续 system 消息的边界，
        // tools_def 在边界处插入（无 system 消息时边界为 0，等价于置最前）。
        let tools_split = messages
            .iter()
            .position(|m| m.get("role").and_then(Value::as_str) != Some("system"))
            .unwrap_or(messages.len());

        let mut turns: Vec<AiTurn> = Vec::new();

        for (i, m) in messages.iter().enumerate() {
            // 在第一条非 system 消息前插入 tools[] 定义
            if i == tools_split {
                if let Some(t) = p.get("tools").and_then(Value::as_array).and_then(|ts| AiTurn::tools_def(ts)) {
                    turns.push(t);
                }
            }

            let role_raw = m.get("role").and_then(Value::as_str).unwrap_or("user");
            let role = match role_raw {
                "system" | "user" | "assistant" | "tool" => role_raw,
                _ => "user",
            };
            let content = m.get("content");

            // tool 角色 → tool_result block
            if role == "tool" {
                let inner = match content {
                    Some(Value::String(s)) => vec![AiContentBlock::text(s.clone())],
                    Some(Value::Array(arr)) => arr
                        .iter()
                        .map(|b| {
                            match (
                                b.get("type").and_then(Value::as_str),
                                b.get("text").and_then(Value::as_str),
                            ) {
                                (Some("text"), Some(t)) => AiContentBlock::text(t),
                                _ => AiContentBlock::text(b.to_string()),
                            }
                        })
                        .collect(),
                    _ => vec![AiContentBlock::text("")],
                };
                let tool_use_id = m
                    .get("tool_call_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                turns.push(AiTurn::new(
                    role,
                    vec![AiContentBlock::ToolResult {
                        tool_use_id,
                        content: inner,
                    }],
                ));
                continue;
            }

            // 普通文本 content
            let mut blocks: Vec<AiContentBlock> = match content {
                Some(Value::String(s)) => vec![AiContentBlock::text(s.clone())],
                Some(Value::Array(arr)) => {
                    let texts: Vec<AiContentBlock> = arr
                        .iter()
                        .filter_map(|b| {
                            if b.get("type").and_then(Value::as_str) == Some("text") {
                                b.get("text")
                                    .and_then(Value::as_str)
                                    .map(AiContentBlock::text)
                            } else {
                                None
                            }
                        })
                        .collect();
                    if texts.is_empty() {
                        vec![AiContentBlock::text("")]
                    } else {
                        texts
                    }
                }
                _ => vec![AiContentBlock::text("")],
            };

            // assistant 消息中的 tool_calls → tool_use blocks
            if role == "assistant" {
                if let Some(tcs) = m.get("tool_calls").and_then(Value::as_array) {
                    for tc in tcs {
                        let id = tc
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        let func = tc.get("function");
                        let name = func
                            .and_then(|f| f.get("name"))
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        let args = func
                            .and_then(|f| f.get("arguments"))
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        blocks.push(AiContentBlock::ToolUse {
                            id,
                            name,
                            input: parse_tool_input(args),
                        });
                    }
                }
            }

            turns.push(AiTurn::new(role, blocks));
        }
        Some(turns)
    }

    /// 解析非流式响应体（`object: chat.completion`，含 `choices[].message` + `usage`）。
    fn parse_response_body(&self, body: &str) -> Option<AiConversation> {
        let p: Value = serde_json::from_str(body).ok()?;
        let choices = p.get("choices")?.as_array()?;
        let first = choices.first()?;
        let msg = first.get("message");

        let mut blocks: Vec<AiContentBlock> = Vec::new();
        // reasoning_content（DeepSeek R1）/ reasoning（部分网关）→ Thinking，排在正文之前
        if let Some(r) = msg
            .and_then(|m| m.get("reasoning_content").or_else(|| m.get("reasoning")))
            .and_then(Value::as_str)
        {
            if !r.is_empty() {
                blocks.push(AiContentBlock::thinking(r));
            }
        }
        // 文本 content
        let text = match msg.and_then(|m| m.get("content")) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Array(arr)) => arr
                .iter()
                .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(""),
            _ => String::new(),
        };
        if !text.is_empty() {
            blocks.push(AiContentBlock::text(text));
        }
        // tool_calls
        if let Some(tcs) = msg
            .and_then(|m| m.get("tool_calls"))
            .and_then(Value::as_array)
        {
            for tc in tcs {
                let id = tc
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let func = tc.get("function");
                let name = func
                    .and_then(|f| f.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let args = func
                    .and_then(|f| f.get("arguments"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                blocks.push(AiContentBlock::ToolUse {
                    id,
                    name,
                    input: parse_tool_input(args),
                });
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
            p.get("usage").map(normalize_usage).filter(|u| !u.is_empty()),
            first
                .get("finish_reason")
                .and_then(Value::as_str)
                .map(String::from),
        ))
    }

    fn create_stream_state(&self) -> Box<dyn StreamState> {
        Box::new(OpenAiStreamState::default())
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// 流式 SSE 状态机
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Default)]
struct OpenAiToolCall {
    id: String,
    name: String,
    arguments: String,
}

#[derive(Default)]
struct OpenAiStreamState {
    /// delta.reasoning_content 的累积思考文本。
    reasoning: String,
    text: String,
    model: Option<String>,
    usage: Option<AiUsage>,
    finish_reason: Option<String>,
    done: bool,
    tool_calls: BTreeMap<i64, OpenAiToolCall>,
}

impl StreamState for OpenAiStreamState {
    fn apply(&mut self, _event: &str, data: &str) {
        if data.trim() == "[DONE]" {
            self.done = true;
            return;
        }
        let Ok(p) = serde_json::from_str::<Value>(data) else {
            return;
        };
        let choice0 = p
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|a| a.first());
        let delta = choice0.and_then(|c| c.get("delta"));

        if let Some(content) = delta.and_then(|d| d.get("content")).and_then(Value::as_str) {
            self.text.push_str(content);
        }
        if let Some(r) = delta
            .and_then(|d| d.get("reasoning_content").or_else(|| d.get("reasoning")))
            .and_then(Value::as_str)
        {
            self.reasoning.push_str(r);
        }
        if let Some(u) = p.get("usage").map(normalize_usage).filter(|u| !u.is_empty()) {
            self.usage = Some(u);
        }
        if let Some(fr) = choice0
            .and_then(|c| c.get("finish_reason"))
            .and_then(Value::as_str)
        {
            self.finish_reason = Some(fr.to_string());
        }
        if self.model.is_none() {
            if let Some(m) = p.get("model").and_then(Value::as_str) {
                self.model = Some(m.to_string());
            }
        }
        if let Some(tcs) = delta
            .and_then(|d| d.get("tool_calls"))
            .and_then(Value::as_array)
        {
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
                if let Some(args) = func
                    .and_then(|f| f.get("arguments"))
                    .and_then(Value::as_str)
                {
                    entry.arguments.push_str(args);
                }
            }
        }
    }

    fn snapshot(&self) -> AiConversation {
        let mut blocks: Vec<AiContentBlock> = Vec::new();
        // 思考先于正文（生成顺序）
        if !self.reasoning.is_empty() {
            blocks.push(AiContentBlock::thinking(self.reasoning.clone()));
        }
        if !self.text.is_empty() {
            blocks.push(AiContentBlock::text(self.text.clone()));
        }
        for tc in self.tool_calls.values() {
            blocks.push(AiContentBlock::ToolUse {
                id: tc.id.clone(),
                name: tc.name.clone(),
                input: parse_tool_input(&tc.arguments),
            });
        }
        if blocks.is_empty() {
            blocks.push(AiContentBlock::text(""));
        }
        AiConversation::new(
            "openai",
            vec![AiTurn::new("assistant", blocks)],
            !self.done && self.finish_reason.is_none(),
            self.model.clone(),
            self.usage.clone(),
            self.finish_reason.clone(),
        )
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
    fn parse_request_tools_def_after_system() {
        let body = r#"{
            "model": "gpt-4o",
            "tools": [{"type": "function", "function": {"name": "get_weather"}}],
            "messages": [
                {"role": "system", "content": "You are helpful."},
                {"role": "user", "content": "weather?"}
            ]
        }"#;
        let turns = OpenAiChatProtocol.parse_request(body).unwrap();
        assert_eq!(turns[0].role, "system");
        assert_eq!(turns[1].role, "tools_def");
        assert_eq!(turns[2].role, "user");
    }

    #[test]
    fn parse_request_tool_calls_and_result() {
        let body = r#"{
            "messages": [
                {"role": "assistant", "content": null, "tool_calls": [
                    {"id": "call_1", "type": "function",
                     "function": {"name": "get_weather", "arguments": "{\"city\":\"NY\"}"}}
                ]},
                {"role": "tool", "tool_call_id": "call_1", "content": "sunny"}
            ]
        }"#;
        let turns = OpenAiChatProtocol.parse_request(body).unwrap();
        assert!(turns[0].content.iter().any(|b| matches!(b, AiContentBlock::ToolUse { .. })));
        assert_eq!(turns[1].role, "tool");
        assert!(turns[1].content.iter().any(|b| matches!(b, AiContentBlock::ToolResult { .. })));
    }

    // ── parse_response_body ────────────────────────────────────────────────────

    #[test]
    fn parse_response_content_tool_calls_and_cached_usage() {
        let body = r#"{
            "model": "gpt-4o",
            "choices": [{
                "message": {"role": "assistant", "content": "hi",
                    "tool_calls": [{"id": "call_1", "function": {"name": "f", "arguments": "{}"}}]},
                "finish_reason": "tool_calls"
            }],
            "usage": {"prompt_tokens": 100, "completion_tokens": 20, "total_tokens": 120,
                      "prompt_tokens_details": {"cached_tokens": 60}}
        }"#;
        let conv = OpenAiChatProtocol.parse_response_body(body).unwrap();
        assert_eq!(conv.provider, "openai");
        assert_eq!(conv.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(conv.turns[0].content.len(), 2);
        let u = conv.usage.unwrap();
        assert_eq!(u.prompt_tokens, Some(100));
        assert_eq!(u.completion_tokens, Some(20));
        assert_eq!(u.total_tokens, Some(120));
        assert_eq!(u.cached_tokens, Some(60));
    }

    /// DeepSeek R1 风格 reasoning_content → Thinking block，排在正文之前。
    #[test]
    fn parse_response_reasoning_content() {
        let body = r#"{
            "model": "deepseek-r1",
            "choices": [{
                "message": {"role": "assistant", "content": "4", "reasoning_content": "2+2 = 4"},
                "finish_reason": "stop"
            }]
        }"#;
        let conv = OpenAiChatProtocol.parse_response_body(body).unwrap();
        assert_eq!(conv.turns[0].content.len(), 2);
        match &conv.turns[0].content[0] {
            AiContentBlock::Thinking { text } => assert_eq!(text, "2+2 = 4"),
            other => panic!("expected thinking block, got {other:?}"),
        }
        match &conv.turns[0].content[1] {
            AiContentBlock::Text { text } => assert_eq!(text, "4"),
            other => panic!("expected text block, got {other:?}"),
        }
    }

    // ── 流式状态机 ────────────────────────────────────────────────────────────

    /// 流式 delta.reasoning_content 累积为 Thinking，排在正文之前。
    #[test]
    fn stream_reasoning_content_delta() {
        let mut st = OpenAiStreamState::default();
        st.apply("", r#"{"choices":[{"delta":{"reasoning_content":"think"}}]}"#);
        st.apply("", r#"{"choices":[{"delta":{"reasoning_content":"ing"}}]}"#);
        st.apply("", r#"{"choices":[{"delta":{"content":"answer"}}]}"#);
        let conv = st.snapshot();
        assert_eq!(conv.turns[0].content.len(), 2);
        match &conv.turns[0].content[0] {
            AiContentBlock::Thinking { text } => assert_eq!(text, "thinking"),
            other => panic!("expected thinking block, got {other:?}"),
        }
        match &conv.turns[0].content[1] {
            AiContentBlock::Text { text } => assert_eq!(text, "answer"),
            other => panic!("expected text block, got {other:?}"),
        }
    }

    #[test]
    fn stream_accumulates_text_tools_and_usage() {
        let mut st = OpenAiStreamState::default();
        st.apply("", r#"{"model":"gpt-4o","choices":[{"delta":{"content":"Hel"}}]}"#);
        st.apply("", r#"{"choices":[{"delta":{"content":"lo"}}]}"#);
        st.apply(
            "",
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"f","arguments":"{\"a\":"}}]}}]}"#,
        );
        st.apply(
            "",
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"1}"}}]}}]}"#,
        );
        st.apply("", r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#);
        st.apply(
            "",
            r#"{"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#,
        );
        st.apply("", "[DONE]");

        let conv = st.snapshot();
        assert!(!conv.streaming);
        assert_eq!(conv.model.as_deref(), Some("gpt-4o"));
        assert_eq!(conv.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(conv.turns[0].content.len(), 2);
        match &conv.turns[0].content[0] {
            AiContentBlock::Text { text } => assert_eq!(text, "Hello"),
            _ => panic!("expected text block"),
        }
        match &conv.turns[0].content[1] {
            AiContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "f");
                assert_eq!(input.get("a").and_then(Value::as_u64), Some(1));
            }
            _ => panic!("expected tool_use block"),
        }
        let u = conv.usage.unwrap();
        assert_eq!(u.total_tokens, Some(15));
    }
}
