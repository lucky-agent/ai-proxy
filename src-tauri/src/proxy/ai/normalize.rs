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
