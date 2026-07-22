use serde::Serialize;
use std::collections::HashMap;

use crate::proxy::ai::session::TimelineEntry;
use crate::proxy::ai::{AiConversation, AiUsage};

/// Tagged union sent through the IPC Channel.
/// Frontend dispatches on `type` (serialized as snake_case).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ProxyEvent {
    Request {
        id: u64,
        method: String,
        uri: String,
        timestamp: i64,
        headers: HashMap<String, String>,
        query_params: HashMap<String, String>,
        decrypted: bool,
        content_type: Option<String>,
    },
    RequestChunk {
        id: u64,
        chunk: String,
    },
    Response {
        id: u64,
        status: u16,
        timestamp: i64,
        duration_ms: u64,
        headers: HashMap<String, String>,
        content_type: Option<String>,
    },
    ResponseChunk {
        id: u64,
        chunk: String,
    },
    Error {
        id: u64,
        error: String,
    },
    /// 增量归一化快照。AI 流量在各节流窗口推送，末次 `streaming=false` 定稿。
    AiNormalized {
        id: u64,
        session_id: String,
        provider: String,
        /// 该次响应归一化对话（assistant 回复 + 元信息）。
        conversation: AiConversation,
        streaming: bool,
    },
    /// 会话时间线增量。请求侧 assign 后发送请求体 delta，
    /// 响应侧 finalize 后发送 assistant delta。前端直接 append 即可渲染。
    AiTimelineDelta {
        session_id: String,
        entries: Vec<TimelineEntry>,
    },
    /// 会话元信息。会话新增请求或 usage 变化时推送。
    AiSession {
        session_id: String,
        scope_host: String,
        request_ids: Vec<u64>,
        usage_total: AiUsage,
        /// 归组依据：`header:<name>` / `prefix` / `new`。
        match_reason: String,
        /// 会话标题：来自首请求响应的 `{"title": "..."}`，无则缺省。
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// 来源归属：规则内 (来源, 合并头) 对的头命中时为对应来源名，无则缺省。
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<String>,
    },
}
