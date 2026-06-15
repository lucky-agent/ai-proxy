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
        decrypted: bool,
        /// 从 Content-Type header 提取的值
        content_type: Option<String>,
        /// 从 Content-Length header 解析的值
        content_length: Option<u64>,
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
}
