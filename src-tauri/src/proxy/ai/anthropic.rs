//! Anthropic Messages 归一化（请求 messages+system + 非流式响应 + 流式 SSE）。
//! 语义镜像前端 `src/lib/ai/providers/anthropic.ts`。

use std::collections::BTreeMap;

use serde_json::Value;

use super::normalize::{AiContentBlock, AiConversation, AiTurn, AiUsage};
use super::{AiProtocol, StreamState};

// ══════════════════════════════════════════════════════════════════════════════
// 协议实现
// ══════════════════════════════════════════════════════════════════════════════

pub(crate) struct AnthropicProtocol;

impl AiProtocol for AnthropicProtocol {
    fn name(&self) -> &'static str {
        "anthropic"
    }

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
        if let Some(tools) = p.get("tools").and_then(Value::as_array) {
            if !tools.is_empty() {
                let tools_json = serde_json::to_string(tools).unwrap_or_default();
                turns.push(AiTurn::new("tools_def", vec![AiContentBlock::text(tools_json)]));
            }
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

        let input = p.get("usage").and_then(|u| u.get("input_tokens")).and_then(Value::as_u64);
        let output = p.get("usage").and_then(|u| u.get("output_tokens")).and_then(Value::as_u64);

        Some(AiConversation {
            provider: "anthropic".to_string(),
            turns: vec![AiTurn::new("assistant", blocks)],
            streaming: false,
            model: p.get("model").and_then(Value::as_str).map(String::from),
            usage: usage_from_parts(input, output),
            finish_reason: p.get("stop_reason").and_then(Value::as_str).map(String::from),
        })
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

/// 从 Anthropic `usage` 计算 [`AiUsage`]（input→prompt, output→completion, total=和）。
pub(super) fn usage_from_parts(input: Option<u64>, output: Option<u64>) -> Option<AiUsage> {
    if input.is_none() && output.is_none() {
        return None;
    }
    Some(AiUsage {
        prompt_tokens: input,
        completion_tokens: output,
        total_tokens: Some(input.unwrap_or(0) + output.unwrap_or(0)),
        cached_tokens: None,
    })
}

// ══════════════════════════════════════════════════════════════════════════════
// 流式 SSE 状态机
// ══════════════════════════════════════════════════════════════════════════════

struct AnthropicBlock {
    is_tool: bool,
    text: String,
    tool_id: String,
    tool_name: String,
    partial_json: String,
}

#[derive(Default)]
struct AnthropicStreamState {
    model: Option<String>,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    stop_reason: Option<String>,
    saw_message_stop: bool,
    blocks: BTreeMap<i64, AnthropicBlock>,
}

impl StreamState for AnthropicStreamState {
    fn apply(&mut self, event: &str, data: &str) {
        let Ok(p) = serde_json::from_str::<Value>(data) else { return };
        match event {
            "message_start" => {
                let msg = p.get("message");
                self.model = msg.and_then(|m| m.get("model")).and_then(Value::as_str).map(String::from);
                self.input_tokens = msg.and_then(|m| m.get("usage")).and_then(|u| u.get("input_tokens")).and_then(Value::as_u64);
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
                            self.blocks.entry(idx).or_insert_with(|| AnthropicBlock {
                                is_tool: false, text: String::new(), tool_id: String::new(),
                                tool_name: String::new(), partial_json: String::new(),
                            }).text.push_str(t);
                        }
                    }
                    Some("input_json_delta") => {
                        if let Some(j) = delta.and_then(|d| d.get("partial_json")).and_then(Value::as_str) {
                            self.blocks.entry(idx).or_insert_with(|| AnthropicBlock {
                                is_tool: true, text: String::new(), tool_id: String::new(),
                                tool_name: String::new(), partial_json: String::new(),
                            }).partial_json.push_str(j);
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
            if b.is_tool {
                let input = if b.partial_json.is_empty() {
                    serde_json::Value::Object(Default::default())
                } else {
                    serde_json::from_str(&b.partial_json).unwrap_or_else(|_| Value::String(b.partial_json.clone()))
                };
                blocks.push(AiContentBlock::ToolUse { id: b.tool_id.clone(), name: b.tool_name.clone(), input });
            } else if !b.text.is_empty() {
                blocks.push(AiContentBlock::text(b.text.clone()));
            }
        }
        if blocks.is_empty() { blocks.push(AiContentBlock::text("")); }
        AiConversation {
            provider: "anthropic".to_string(),
            turns: vec![AiTurn::new("assistant", blocks)],
            streaming: !self.saw_message_stop,
            model: self.model.clone(),
            usage: usage_from_parts(self.input_tokens, self.output_tokens),
            finish_reason: self.stop_reason.clone(),
        }
    }

    fn finalize(&mut self) {
        self.saw_message_stop = true;
    }
}
