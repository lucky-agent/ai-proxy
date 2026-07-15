use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::proxy::ai::{AiConversation, AiUsage};

/// 后端 URL 检测产出的 AI 提示，挂在 Request 事件上透传前端。
/// externally tagged + lowercase → 序列化为 `"none"` / `"candidate"` / `{"provider":"openai"}`，
/// 与前端 TS 联合 `AiHint` 对齐。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AiHint {
    /// 默认：无 URL 规则命中
    #[default]
    None,
    /// 命中规则但 provider 未知（user 配 provider:null 或非法值）
    Candidate,
    /// 命中规则且 provider 已定
    Provider(String),
}

/// Tagged union sent through the IPC Channel.
/// Frontend dispatches on `type` (serialized as snake_case).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ProxyEvent {
    Request {
        id: String,
        method: String,
        uri: String,
        timestamp: i64,
        headers: HashMap<String, String>,
        query_params: HashMap<String, String>,
        decrypted: bool,
        /// 从 Content-Type header 提取的值
        content_type: Option<String>,
        /// 从 Content-Length header 解析的值
        content_length: Option<u64>,
        #[serde(default)]
        ai_hint: AiHint,
    },
    RequestChunk {
        id: String,
        chunk: String,
    },
    Response {
        id: String,
        status: u16,
        timestamp: i64,
        duration_ms: u64,
        headers: HashMap<String, String>,
        /// 从 Content-Type header 提取的值
        content_type: Option<String>,
        /// 从 Content-Length header 解析的值
        content_length: Option<u64>,
    },
    ResponseChunk {
        id: String,
        chunk: String,
    },
    Error {
        id: String,
        error: String,
    },
    /// 增量归一化快照。AI 流量在各节流窗口推送，末次 `streaming=false` 定稿。
    AiNormalized {
        id: String,
        session_id: String,
        provider: String,
        /// 该次请求的 messages（system/user/assistant 历史），供前端渲染完整对话。
        request_turns: Vec<crate::proxy::ai::AiTurn>,
        /// 该次响应归一化对话（assistant 回复）。
        conversation: AiConversation,
        streaming: bool,
    },
    /// 会话元信息。会话新增请求或 usage 变化时推送。
    AiSession {
        session_id: String,
        scope_host: String,
        request_ids: Vec<String>,
        usage_total: AiUsage,
        turn_count: u32,
        /// 归组依据：`header:<name>` / `prefix` / `new`。
        match_reason: String,
    },
}
