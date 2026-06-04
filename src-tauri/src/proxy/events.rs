use serde::Serialize;
use std::collections::HashMap;

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
    },
    ResponseChunk {
        id: String,
        chunk: String,
    },
    Error {
        id: String,
        error: String,
    },
}
