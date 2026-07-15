//! Anthropic Messages 归一化（请求 messages+system + 非流式响应）。
//! 语义镜像前端 `src/lib/ai/providers/anthropic.ts`。

use serde_json::Value;

use super::normalize::{AiContentBlock, AiConversation, AiTurn, AiUsage};

/// 从 content block 数组拼接纯文本。
fn blocks_to_text(blocks: &[Value]) -> String {
    blocks
        .iter()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|b| b.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("")
}

/// 解析请求体（顶层 `system` + `messages[]`，含 tool_use/tool_result block + tools 定义）。
pub(crate) fn parse_request(body: &str) -> Option<Vec<AiTurn>> {
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

/// 解析非流式响应体（`type: message`，含 `content[]` + `usage`）。
pub(crate) fn parse_response_body(body: &str) -> Option<AiConversation> {
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
