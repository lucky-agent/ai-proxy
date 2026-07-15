//! 后端 AI 语义归一化引擎。
//!
//! 把 OpenAI Chat Completions 与 Anthropic Messages 的请求体、非流式响应、
//! 流式 SSE 解析为统一中间表示（[`normalize::AiConversation`]），并在归一化时
//! 就地完成跨请求会话分组（[`session`]）。与代理转发层解耦，便于单测。

pub(crate) mod anthropic;
pub(crate) mod normalize;
pub(crate) mod openai;
pub(crate) mod session;
pub(crate) mod stream;

pub(crate) use normalize::{AiConversation, AiTurn, AiUsage};

/// AI provider 类别。由 URL 命中的 provider 或 body/响应签名判定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Provider {
    OpenAi,
    Anthropic,
}

impl Provider {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Provider::OpenAi => "openai",
            Provider::Anthropic => "anthropic",
        }
    }

    pub(crate) fn from_str(s: &str) -> Option<Provider> {
        match s {
            "openai" => Some(Provider::OpenAi),
            "anthropic" => Some(Provider::Anthropic),
            _ => None,
        }
    }
}

/// 解析请求体为 turns（provider 已知）。畸形 body 返回空 vec。
pub(crate) fn parse_request(provider: Provider, body: &str) -> Vec<AiTurn> {
    match provider {
        Provider::OpenAi => openai::parse_request(body),
        Provider::Anthropic => anthropic::parse_request(body),
    }
    .unwrap_or_default()
}

/// 解析非流式响应体为 conversation（provider 已知）。畸形 body 返回 None。
pub(crate) fn parse_response_body(provider: Provider, body: &str) -> Option<AiConversation> {
    match provider {
        Provider::OpenAi => openai::parse_response_body(body),
        Provider::Anthropic => anthropic::parse_response_body(body),
    }
}

/// 从请求 body 嗅探 provider（镜像前端 detectProvider 的 body-schema 分支）。
/// Anthropic 特征：顶层 `system`，或 message content 含 tool_use/tool_result/image block。
pub(crate) fn provider_from_body(body: &str) -> Option<Provider> {
    let p: serde_json::Value = serde_json::from_str(body).ok()?;
    let messages = p.get("messages")?.as_array()?;
    if messages.is_empty() {
        return None;
    }
    let system = p.get("system");
    if matches!(system, Some(serde_json::Value::String(_)) | Some(serde_json::Value::Array(_))) {
        return Some(Provider::Anthropic);
    }
    let has_anthropic_block = messages.iter().any(|m| {
        m.get("content")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter().any(|b| {
                    matches!(
                        b.get("type").and_then(serde_json::Value::as_str),
                        Some("tool_use") | Some("tool_result") | Some("image")
                    )
                })
            })
            .unwrap_or(false)
    });
    if has_anthropic_block {
        return Some(Provider::Anthropic);
    }
    Some(Provider::OpenAi)
}

/// 综合 ai_hint 的 provider 提示与 body 嗅探，判定请求 provider。
pub(crate) fn provider_for_request(hint_provider: Option<&str>, body: &str) -> Option<Provider> {
    if let Some(p) = hint_provider.and_then(Provider::from_str) {
        return Some(p);
    }
    provider_from_body(body)
}
