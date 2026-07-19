//! AI 对话的统一中间表示（IR）。
//! provider 无关，OpenAI / Anthropic 归一化后都产出这套结构。
//! serde `camelCase` 与前端 `src/types/ai.ts` 对齐，前端可直接消费。

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 内容块：文本 / 思考 / 工具调用 / 工具结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum AiContentBlock {
    Text {
        text: String,
    },
    /// 模型思考过程（Anthropic thinking / Gemini thought / DeepSeek reasoning_content
    /// / Responses reasoning summary）。加密的思考内容（redacted/encrypted）不采集。
    Thinking {
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

    pub(crate) fn thinking(text: impl Into<String>) -> Self {
        AiContentBlock::Thinking { text: text.into() }
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

    /// tools[] 定义 turn（序列化为 JSON 文本）；空数组返回 None。
    pub(crate) fn tools_def(tools: &[Value]) -> Option<AiTurn> {
        if tools.is_empty() {
            return None;
        }
        let json = serde_json::to_string(tools).unwrap_or_default();
        Some(AiTurn::new("tools_def", vec![AiContentBlock::text(json)]))
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
    /// 缓存写入量（Anthropic cache_creation_input_tokens / Bedrock cacheWriteInputTokens）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_tokens: Option<u64>,
}

impl AiUsage {
    /// 是否所有字段都缺失。usage JSON 存在但无可识别字段时用于过滤，
    /// 避免序列化出空对象 `{}` 误导前端的有值判断。
    pub(crate) fn is_empty(&self) -> bool {
        self.prompt_tokens.is_none()
            && self.completion_tokens.is_none()
            && self.total_tokens.is_none()
            && self.cached_tokens.is_none()
            && self.cache_creation_tokens.is_none()
    }

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
        self.cache_creation_tokens = add(self.cache_creation_tokens, other.cache_creation_tokens);
    }
}

/// 通用 usage 归一化：兼容 OpenAI Chat Completions (prompt/completion)、
/// Anthropic (input/output)、Responses API (input/output)、Gemini (promptTokenCount/…) 四种命名。
/// 缓存命中走 cache_read 系列 fallback 链，缓存写入走 cache_creation 系列。
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

    let cache_creation_tokens =
        get_u64(usage, &["cache_creation_input_tokens", "cacheWriteInputTokens"]);

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
        cache_creation_tokens,
    }
}

/// 把 tool_call 的 arguments 字符串尝试解析为 JSON，失败保留原始字符串。
pub(crate) fn parse_tool_input(raw: &str) -> Value {
    if raw.is_empty() {
        return Value::String(String::new());
    }
    serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_string()))
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
    /// 停止原因，provider 原生值原样透传（OpenAI `stop`/`length`/`tool_calls`、
    /// Anthropic `end_turn`、Gemini `STOP` 等）；流式响应中出现即表示生成定稿。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    /// 首字用时（请求发出 → 首个流式 chunk），仅流式请求有值。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_chunk_ms: Option<u64>,
    /// 总耗时（请求发出 → 流结束），定稿快照注入。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// 请求开始时刻（Unix ms），气泡时间戳展示用；每次快照恒注入。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_ms: Option<i64>,
}

impl AiConversation {
    /// 构造器：timing 字段（`first_chunk_ms` / `duration_ms`）恒为 `None`，
    /// 由 parser 的 `BodyObserver` 注入——provider 解析器不感知时间。
    pub(crate) fn new(
        provider: impl Into<String>,
        turns: Vec<AiTurn>,
        streaming: bool,
        model: Option<String>,
        usage: Option<AiUsage>,
        finish_reason: Option<String>,
    ) -> Self {
        AiConversation {
            provider: provider.into(),
            turns,
            streaming,
            model,
            usage,
            finish_reason,
            first_chunk_ms: None,
            duration_ms: None,
            start_ms: None,
        }
    }
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

/// 从请求 messages 的第一条 user turn 提取会话标题（兜底）。
/// 策略：从纯文本块中取第一条有效片段，
/// 而不是识别并过滤注入标签——注入方随时新增标签，黑名单永远追不上。
/// <USER_REQUEST> 内容优先，否则取首个非空文本块。
pub(crate) fn extract_title_from_request(messages: &[AiTurn]) -> Option<String> {
    let turn = messages.iter().find(|t| t.role == "user")?;

    // 拼接所有 text 块
    let mut full_text = String::new();
    for block in &turn.content {
        if let AiContentBlock::Text { text: t } = block {
            full_text.push_str(t);
        }
    }
    let full_text = full_text.trim().to_string();
    if full_text.is_empty() {
        return None;
    }

    // <USER_REQUEST>...</USER_REQUEST> 内容优先
    if let Some(inner) = extract_tag_content(&full_text, "USER_REQUEST") {
        let cleaned = inner.trim().to_string();
        if !cleaned.is_empty() {
            return Some(truncate_at_char_boundary(&cleaned, 50).to_string());
        }
    }

    // 否则取按行拆分后的第一条有效文本
    for line in full_text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // 跳过 XML 标签行（包括 <tag> / </tag> / <tag attr> 等）
        if trimmed.starts_with('<') && trimmed.ends_with('>') {
            continue;
        }
        // 跳过 XML 开标签（可能在后续行有闭合）
        if trimmed.starts_with('<') && trimmed.contains('>') {
            // 保守策略：以 < 开头且包含 > 的都视为 XML 行跳过；
            // 真正的用户文本不会以 < 开头（除 <USER_REQUEST> 已处理）
            continue;
        }
        // 跳过常见的注入前缀行
        if trimmed.starts_with('#') || trimmed.starts_with('[') || trimmed.starts_with('`') {
            continue;
        }
        return Some(truncate_at_char_boundary(trimmed, 50).to_string());
    }

    None
}

/// 提取 `<TAG>...</TAG>` 的内部内容（仅当整条消息被此标签包裹时）。
/// 大小写不敏感。
fn extract_tag_content(s: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let s_lower = s.to_lowercase();

    if s_lower.starts_with(&open.to_lowercase()) && s_lower.ends_with(&close.to_lowercase()) {
        let inner = s[open.len()..s.len() - close.len()].trim().to_string();
        return (!inner.is_empty()).then_some(inner);
    }

    // 也尝试带属性的情况：<tag ...>
    if starts_with_tag_ci(s, tag) && s_lower.ends_with(&close.to_lowercase()) {
        let after_open = s.find('>')?;
        let inner = s[after_open + 1..s.len() - close.len()].trim().to_string();
        return (!inner.is_empty()).then_some(inner);
    }

    None
}

/// 大小写不敏感检查 `s` 是否以 `<TAG` 开头（不要求紧接 `>`，兼容属性）。
fn starts_with_tag_ci(s: &str, tag: &str) -> bool {
    let s_lower = s.to_lowercase();
    let prefix = format!("<{}", tag.to_lowercase());
    s_lower.starts_with(&prefix)
}

/// UTF-8 字符边界安全截断：`max_len` 字节以内找最近边界截断。
fn truncate_at_char_boundary(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        return s;
    }
    let mut end = max_len;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
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

    /// Anthropic 带 prompt cache 的 usage：cache_read 进 cached_tokens，
    /// cache_creation 暂不采集。
    #[test]
    fn usage_anthropic_with_cache_read() {
        let u = normalize_usage(&serde_json::json!({
            "input_tokens": 12, "cache_read_input_tokens": 8000,
            "cache_creation_input_tokens": 200, "output_tokens": 50
        }));
        assert_eq!(u.prompt_tokens, Some(12));
        assert_eq!(u.completion_tokens, Some(50));
        assert_eq!(u.cached_tokens, Some(8000));
        assert_eq!(u.total_tokens, Some(62));
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

    /// 无可识别字段 → is_empty，调用方以此过滤空 usage。
    #[test]
    fn usage_unknown_shape_is_empty() {
        assert!(normalize_usage(&serde_json::json!({})).is_empty());
        assert!(normalize_usage(&serde_json::json!({"foo": 1})).is_empty());
        assert!(!normalize_usage(&serde_json::json!({"input_tokens": 1})).is_empty());
    }

    /// 缓存写入量：Anthropic snake_case 与 Bedrock camelCase 两种命名。
    #[test]
    fn usage_cache_creation_captured() {
        let u = normalize_usage(&serde_json::json!({
            "input_tokens": 12, "cache_read_input_tokens": 8000,
            "cache_creation_input_tokens": 200, "output_tokens": 50
        }));
        assert_eq!(u.cache_creation_tokens, Some(200));
        let u = normalize_usage(&serde_json::json!({"inputTokens": 12, "cacheWriteInputTokens": 5}));
        assert_eq!(u.cache_creation_tokens, Some(5));
    }

    #[test]
    fn accumulate_includes_cache_fields() {
        let mut a = AiUsage {
            cached_tokens: Some(100),
            cache_creation_tokens: Some(10),
            ..Default::default()
        };
        a.accumulate(&AiUsage {
            cached_tokens: Some(50),
            cache_creation_tokens: Some(5),
            ..Default::default()
        });
        assert_eq!(a.cached_tokens, Some(150));
        assert_eq!(a.cache_creation_tokens, Some(15));
    }

    #[test]
    fn is_empty_considers_cache_creation() {
        let u = AiUsage { cache_creation_tokens: Some(1), ..Default::default() };
        assert!(!u.is_empty());
    }
}
