//! OpenAI Chat Completions 归一化（请求 messages + 非流式响应 + 流式 SSE）。
//! 语义镜像前端 `src/lib/ai/providers/openai.ts`。

use std::collections::BTreeMap;

use serde_json::Value;

use super::normalize::{AiContentBlock, AiConversation, AiTurn, AiUsage};
use super::{AiProtocol, StreamState};

// ══════════════════════════════════════════════════════════════════════════════
// 协议实现
// ══════════════════════════════════════════════════════════════════════════════

/// OpenAI Chat Completions 协议。
pub(crate) struct OpenAiChatProtocol;

impl AiProtocol for OpenAiChatProtocol {
    fn name(&self) -> &'static str {
        "openai"
    }

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
                if let Some(tools) = p.get("tools").and_then(Value::as_array) {
                    if !tools.is_empty() {
                        let tools_json = serde_json::to_string(tools).unwrap_or_default();
                        turns.push(AiTurn::new("tools_def", vec![AiContentBlock::text(tools_json)]));
                    }
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
                        .map(|b| match (b.get("type").and_then(Value::as_str), b.get("text").and_then(Value::as_str)) {
                            (Some("text"), Some(t)) => AiContentBlock::text(t),
                            _ => AiContentBlock::text(b.to_string()),
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
                                b.get("text").and_then(Value::as_str).map(AiContentBlock::text)
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
                        let id = tc.get("id").and_then(Value::as_str).unwrap_or("").to_string();
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
        if let Some(tcs) = msg.and_then(|m| m.get("tool_calls")).and_then(Value::as_array) {
            for tc in tcs {
                let id = tc.get("id").and_then(Value::as_str).unwrap_or("").to_string();
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

        Some(AiConversation {
            provider: "openai".to_string(),
            turns: vec![AiTurn::new("assistant", blocks)],
            streaming: false,
            model: p.get("model").and_then(Value::as_str).map(String::from),
            usage: p.get("usage").map(usage_from_json),
            finish_reason: first
                .get("finish_reason")
                .and_then(Value::as_str)
                .map(String::from),
        })
    }

    fn create_stream_state(&self) -> Box<dyn StreamState> {
        Box::new(OpenAiStreamState::default())
    }
}

/// 从 OpenAI `usage` JSON 对象提取 [`AiUsage`]。
pub(super) fn usage_from_json(usage: &Value) -> AiUsage {
    AiUsage {
        prompt_tokens: usage.get("prompt_tokens").and_then(Value::as_u64),
        completion_tokens: usage.get("completion_tokens").and_then(Value::as_u64),
        total_tokens: usage.get("total_tokens").and_then(Value::as_u64),
        cached_tokens: usage
            .get("prompt_tokens_details")
            .and_then(|d| d.get("cached_tokens"))
            .and_then(Value::as_u64),
    }
}

/// 把 tool_call 的 arguments 字符串尝试解析为 JSON，失败保留原始字符串。
pub(super) fn parse_tool_input(raw: &str) -> Value {
    if raw.is_empty() {
        return Value::String(String::new());
    }
    serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_string()))
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
        let Ok(p) = serde_json::from_str::<Value>(data) else { return };
        let choice0 = p.get("choices").and_then(|c| c.as_array()).and_then(|a| a.first());
        let delta = choice0.and_then(|c| c.get("delta"));

        if let Some(content) = delta.and_then(|d| d.get("content")).and_then(Value::as_str) {
            self.text.push_str(content);
        }
        if let Some(usage) = p.get("usage").filter(|u| !u.is_null()) {
            self.usage = Some(usage_from_json(usage));
        }
        if let Some(fr) = choice0.and_then(|c| c.get("finish_reason")).and_then(Value::as_str) {
            self.finish_reason = Some(fr.to_string());
        }
        if self.model.is_none() {
            if let Some(m) = p.get("model").and_then(Value::as_str) {
                self.model = Some(m.to_string());
            }
        }
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

    fn snapshot(&self) -> AiConversation {
        let mut blocks: Vec<AiContentBlock> = Vec::new();
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
        AiConversation {
            provider: "openai".to_string(),
            turns: vec![AiTurn::new("assistant", blocks)],
            streaming: !self.done && self.finish_reason.is_none(),
            model: self.model.clone(),
            usage: self.usage.clone(),
            finish_reason: self.finish_reason.clone(),
        }
    }

    fn finalize(&mut self) {
        self.done = true;
    }
}
