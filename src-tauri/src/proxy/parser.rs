use std::collections::HashMap;

use log::info;
use rama::futures::StreamExt;
use rama::http::{Body, Method, Response, StatusCode, Uri};
use tauri::ipc::Channel;
use uuid::Uuid;

use super::events::ProxyEvent;
use crate::config::db::{spawn_insert_chunk, spawn_update_response_body};

/// 简易 URL 百分号解码
fn url_decode(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2 {
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    result.push(byte as char);
                    continue;
                }
            }
            // 无效的百分号编码，保留原样
            result.push('%');
            result.push_str(&hex);
        } else if ch == '+' {
            result.push(' ');
        } else {
            result.push(ch);
        }
    }
    result
}

/// Log request and emit via channel. Returns (modified request, method, uri, request_id).
pub(crate) fn log_request(
    parts: &rama::http::request::Parts,
    body_str: &str,
    event_channel: &Option<Channel<ProxyEvent>>,
) -> (Method, Uri, String) {
    let method = parts.method.clone();
    let uri = parts.uri.clone();
    let request_id = Uuid::new_v4().to_string();
    let query_params: HashMap<String, String> = uri
        .query()
        .map(|q| {
            q.split('&')
                .filter_map(|pair| {
                    let mut parts = pair.splitn(2, '=');
                    let key = parts.next()?.to_string();
                    let value = parts.next().unwrap_or("").to_string();
                    let decoded_key = url_decode(&key);
                    let decoded_value = url_decode(&value);
                    Some((decoded_key, decoded_value))
                })
                .collect()
        })
    .unwrap_or_default();

    let req_headers: HashMap<String, String> = parts
        .headers
        .iter()
        .filter_map(|(name, value)| {
            let key = name.to_string();
            let val = value.to_str().ok()?.to_string();
            // 合并重复的 header（如 Cookie 可能在请求头中出现多次）
            Some((key, val))
        })
        .fold(HashMap::new(), |mut acc, (key, val)| {
            if let Some(existing) = acc.get_mut(&key) {
                existing.push_str("; ");
                existing.push_str(&val);
            } else {
                acc.insert(key, val);
            }
            acc
        });

    let req_content_type = parts.headers.get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let req_content_length = parts.headers.get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    if let Some(ch) = event_channel {
       ch.send(ProxyEvent::Request {
           id: request_id.clone(),
           method: method.to_string(),
           uri: uri.to_string(),
           timestamp: chrono::Utc::now().timestamp_millis(),
           headers: req_headers,
           query_params,
            decrypted: true,
            content_type: req_content_type,
            content_length: req_content_length,
       })
        .ok();
        if !body_str.is_empty() {
            ch.send(ProxyEvent::RequestChunk {
                id: request_id.clone(),
                chunk: body_str.to_string(),
            })
            .ok();
        }
    }

    (method, uri, request_id)
}

/// Handle direct (non-tunnel) requests to the proxy itself.
/// Returns a 200 OK response with a success JSON body, and logs it as a response event.
pub(crate) fn direct_response(
    method: Method,
    uri: Uri,
    request_id: &str,
    event_channel: &Option<Channel<ProxyEvent>>,
) -> Response {
    info!("Direct request to proxy: {}, returning directly", uri);
    let resp = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"code":0,"msg":"success"}"#))
        .expect("valid status code and body for direct response");
    log_response(resp, method, uri, request_id, 0, event_channel, None)
}

/// Log response and emit via channel.
pub(crate) fn log_response(
    resp: Response,
    method: Method,
    uri: Uri,
    request_id: &str,
    duration_ms: u64,
    event_channel: &Option<Channel<ProxyEvent>>,
    db_path: Option<String>,
) -> Response {
    let status = resp.status();
    info!("Response [{} {}] {}", method, uri, status);

    let (parts, body) = resp.into_parts();

    let resp_headers: HashMap<String, String> = parts
        .headers
        .iter()
        .filter_map(|(name, value)| {
            let key = name.to_string();
            let val = value.to_str().ok()?.to_string();
            Some((key, val))
        })
        .fold(HashMap::new(), |mut acc, (key, val)| {
            // Set-Cookie 可能多次出现，合并为新行分隔
            if key.to_lowercase() == "set-cookie" {
                if let Some(existing) = acc.get_mut(&key) {
                    existing.push('\n');
                    existing.push_str(&val);
                } else {
                    acc.insert(key, val);
                }
            } else if let Some(existing) = acc.get_mut(&key) {
                existing.push_str(", ");
                existing.push_str(&val);
            } else {
                acc.insert(key, val);
            }
            acc
        });

    if let Some(ch) = event_channel {
        let resp_content_type = parts.headers.get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let resp_content_length = parts.headers.get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());

        ch.send(ProxyEvent::Response {
            id: request_id.to_string(),
            status: status.as_u16(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            duration_ms,
            headers: resp_headers,
            content_type: resp_content_type,
            content_length: resp_content_length,
        })
        .ok();
    }

    let logged_body = log_body_chunks(
        body,
        "Response".into(),
        method,
        uri,
        request_id.into(),
        event_channel.clone(),
        db_path,
    );

    Response::from_parts(parts, logged_body)
}

/// Accumulates response body chunks and writes the full body to DB on drop.
struct ChunkAccumulator {
    db_path: Option<String>,
    request_id: String,
    body: String,
}

impl Drop for ChunkAccumulator {
    fn drop(&mut self) {
        if let Some(ref path) = self.db_path {
            if !self.body.is_empty() {
                spawn_update_response_body(path, &self.request_id, &self.body);
            }
        }
    }
}

fn log_body_chunks(
    body: Body,
    label: String,
    method: Method,
    uri: Uri,
    request_id: String,
    event_channel: Option<Channel<ProxyEvent>>,
    db_path: Option<String>,
) -> Body {
    let mut acc = ChunkAccumulator {
        db_path: db_path.clone(),
        request_id: request_id.clone(),
        body: String::new(),
    };
    let mut seq: i64 = 0;

    Body::from_stream(body.into_data_stream().map(move |result| {
        if let Ok(ref bytes) = result {
            let chunk_str = String::from_utf8_lossy(bytes).into_owned();
            info!("{label} chunk [{} {}]: {chunk_str}", method, uri);
            if let Some(ref ch) = event_channel {
                let event = ProxyEvent::ResponseChunk {
                    id: request_id.clone(),
                    chunk: chunk_str.clone(),
                };
                ch.send(event).ok();
            }
            seq += 1;
            acc.body.push_str(&chunk_str);
            if let Some(ref path) = db_path {
                spawn_insert_chunk(path, &request_id, &chunk_str, seq, chrono::Utc::now().timestamp_millis());
            }
        }
        result
    }))
}

/// Build an error response for proxy forwarding failures.
pub(crate) fn error_response() -> Response {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(Body::empty())
        .expect("valid status code and empty body for error response")
}
