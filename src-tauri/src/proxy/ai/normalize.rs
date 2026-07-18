//! AI 对话的统一中间表示（IR）。
//! provider 无关，OpenAI / Anthropic 归一化后都产出这套结构。
//! serde `camelCase` 与前端 `src/types/ai.ts` 对齐，前端可直接消费。

use serde::{Deserialize, Serialize};

/// 内容块：文本 / 工具调用 / 工具结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum AiContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        /// 工具入参；能解析为 JSON 则为对象，否则为原始字符串。
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: Vec<AiContentBlock>,
    },
}

impl AiContentBlock {
    pub(crate) fn text(text: impl Into<String>) -> Self {
        AiContentBlock::Text { text: text.into() }
    }
}

/// 对话中的一轮。role 与前端一致：system/user/assistant/tool/tools_def。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct AiTurn {
    pub role: String,
    pub content: Vec<AiContentBlock>,
}

impl AiTurn {
    pub(crate) fn new(role: impl Into<String>, content: Vec<AiContentBlock>) -> Self {
        AiTurn {
            role: role.into(),
            content,
        }
    }
}

/// token 用量。字段可选，因不同 provider / 流式阶段提供的信息不同。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiUsage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<u64>,
}

impl AiUsage {
    /// 简单累加：各字段相加（None 视为 0，任一有值则结果为 Some）。真实计费口径。
    pub(crate) fn accumulate(&mut self, other: &AiUsage) {
        fn add(a: Option<u64>, b: Option<u64>) -> Option<u64> {
            match (a, b) {
                (None, None) => None,
                (x, y) => Some(x.unwrap_or(0) + y.unwrap_or(0)),
            }
        }
        self.prompt_tokens = add(self.prompt_tokens, other.prompt_tokens);
        self.completion_tokens = add(self.completion_tokens, other.completion_tokens);
        self.total_tokens = add(self.total_tokens, other.total_tokens);
        self.cached_tokens = add(self.cached_tokens, other.cached_tokens);
    }
}

/// 归一化后的完整对话（含 provider / 轮次 / 流式状态 / 元信息）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiConversation {
    pub provider: String,
    pub turns: Vec<AiTurn>,
    pub streaming: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<AiUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

/// 从响应 conversation 提取会话标题：
/// 第一条 assistant turn 的文本若为 `{"title": "..."}` JSON，返回 title。
/// 供会话表在首请求响应定稿时命名会话。
pub(crate) fn extract_title(conv: &AiConversation) -> Option<String> {
    let turn = conv.turns.iter().find(|t| t.role == "assistant")?;
    let mut text = String::new();
    for block in &turn.content {
        if let AiContentBlock::Text { text: t } = block {
            text.push_str(t);
        }
    }
    let value: serde_json::Value = serde_json::from_str(strip_code_fence(text.trim())).ok()?;
    let title = value.as_object()?.get("title")?.as_str()?.trim();
    (!title.is_empty()).then(|| title.to_string())
}

/// 剥掉包裹全文的 ``` / ```json 代码栅栏（部分模型会包一层）；不匹配时原样返回。
fn strip_code_fence(s: &str) -> &str {
    let Some(rest) = s.strip_prefix("```").and_then(|r| r.strip_suffix("```")) else {
        return s;
    };
    // 首行可能是语言标记（json 等），跳过到首个换行
    match rest.find('\n') {
        Some(i) => rest[i + 1..].trim(),
        None => rest.trim(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conv(turns: Vec<AiTurn>) -> AiConversation {
        AiConversation {
            provider: "openai".to_string(),
            turns,
            streaming: false,
            model: None,
            usage: None,
            finish_reason: None,
        }
    }

    fn assistant_text(text: &str) -> AiTurn {
        AiTurn::new("assistant", vec![AiContentBlock::text(text)])
    }

    #[test]
    fn extracts_plain_title_json() {
        let c = conv(vec![assistant_text(r#"{"title": "调整气泡背景颜色搭配"}"#)]);
        assert_eq!(extract_title(&c).as_deref(), Some("调整气泡背景颜色搭配"));
    }

    #[test]
    fn extracts_fenced_title_json() {
        let c = conv(vec![assistant_text("```json\n{\"title\": \"标题\"}\n```")]);
        assert_eq!(extract_title(&c).as_deref(), Some("标题"));
        let c = conv(vec![assistant_text("```\n{\"title\": \"标题\"}\n```")]);
        assert_eq!(extract_title(&c).as_deref(), Some("标题"));
    }

    #[test]
    fn tolerates_extra_keys_and_whitespace() {
        let c = conv(vec![assistant_text(
            "  {\"title\": \" 标题 \", \"emoji\": \"🎨\"}  ",
        )]);
        assert_eq!(extract_title(&c).as_deref(), Some("标题"));
    }

    #[test]
    fn rejects_non_title_responses() {
        // 普通文本
        assert_eq!(extract_title(&conv(vec![assistant_text("你好！有什么可以帮你？")])), None);
        // title 为空串
        assert_eq!(extract_title(&conv(vec![assistant_text(r#"{"title": ""}"#)])), None);
        // title 非字符串
        assert_eq!(extract_title(&conv(vec![assistant_text(r#"{"title": 42}"#)])), None);
        // 顶层非对象
        assert_eq!(extract_title(&conv(vec![assistant_text(r#"["title"]"#)])), None);
        // 无 assistant turn
        assert_eq!(extract_title(&conv(vec![])), None);
    }

    #[test]
    fn only_first_assistant_turn_is_considered() {
        let c = conv(vec![
            assistant_text("普通回复"),
            assistant_text(r#"{"title": "后来的标题"}"#),
        ]);
        assert_eq!(extract_title(&c), None);
    }
}
