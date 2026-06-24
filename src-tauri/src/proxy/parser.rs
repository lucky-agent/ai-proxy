use std::collections::HashMap;

use log::info;
use rama::futures::StreamExt;
use rama::http::{Body, Response, StatusCode, request};

use crate::proxy::state::ProxyCtx;

use super::events::ProxyEvent;

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

/// Accepts body as raw bytes to avoid unnecessary UTF-8 allocation;
/// only converts lossily when emitting the RequestChunk event.
pub(crate) fn log_request(ctx: &ProxyCtx, parts: &rama::http::request::Parts, body: &[u8]) {
    let query_params: HashMap<String, String> = ctx
        .uri()
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

    let req_content_type = parts
        .headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let req_content_length = parts
        .headers
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    ctx.send(ProxyEvent::Request {
        id: ctx.request_id().to_string(),
        method: ctx.method().to_string(),
        uri: ctx.uri().to_string(),
        timestamp: chrono::Utc::now().timestamp_millis(),
        headers: req_headers,
        query_params,
        decrypted: true,
        content_type: req_content_type,
        content_length: req_content_length,
    });
    if !body.is_empty() {
        ctx.send(ProxyEvent::RequestChunk {
            id: ctx.request_id().to_string(),
            chunk: String::from_utf8_lossy(body).into_owned(),
        });
    }
}

/// Handle direct (non-tunnel) requests to the proxy itself.
/// Constructs a Response directly from the request parts and body.
pub(crate) fn direct_response(ctx: &ProxyCtx, req: request::Parts, body: Body) -> Response {
    let mut resp = Response::builder()
        .status(StatusCode::OK)
        .body(body)
        .expect("valid status code and body for direct response");
    *resp.headers_mut() = req.headers.clone();
    log_response(ctx, resp)
}

/// Log response and emit via channel.
pub(crate) fn log_response(ctx: &ProxyCtx, resp: Response) -> Response {
    let status = resp.status();
    info!("Response [{} {}] {}", ctx.method(), ctx.uri(), status);

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

    let resp_content_type = parts
        .headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let resp_content_length = parts
        .headers
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    ctx.send(ProxyEvent::Response {
        id: ctx.request_id().to_string(),
        status: status.as_u16(),
        timestamp: chrono::Utc::now().timestamp_millis(),
        duration_ms: ctx.duration_ms(),
        headers: resp_headers,
        content_type: resp_content_type,
        content_length: resp_content_length,
    });

    let logged_body = log_body_chunks(body, ctx);

    Response::from_parts(parts, logged_body)
}

fn log_body_chunks(body: Body, ctx: &ProxyCtx) -> Body {
    let request_id = ctx.request_id().to_string();
    let sender = ctx.sender().clone();
    Body::from_stream(body.into_data_stream().map(move |result| {
        if let Ok(ref bytes) = result {
            let chunk_str = String::from_utf8_lossy(bytes).into_owned();
            info!("Response chunk: {chunk_str}");
            if let Some(ref ch) = sender {
                let _ = ch.send(ProxyEvent::ResponseChunk {
                    id: request_id.clone(),
                    chunk: chunk_str.clone(),
                });
            }
        }
        result
    }))
}

/// Build a generic error response returned directly to the client.
pub(crate) fn error_response(status: StatusCode, body: impl Into<Body>) -> Response {
    Response::builder()
        .status(status)
        .body(body.into())
        .expect("valid status code and body for error response")
}
