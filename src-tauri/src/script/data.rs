use std::collections::HashMap;

use bytes::Bytes;
use rama::http::{Body, Method, Request, Response, StatusCode, header};
use rama::net::uri::Uri;
use serde::{Deserialize, Serialize};

use crate::utils::buf_pool;

use super::engine;

/// HTTP 请求数据，供脚本 onRequest 钩子读写。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestData {
    pub method: String,
    pub uri: String,
    pub headers: HashMap<String, String>,
    pub body: String,
    /// 非 UTF-8（二进制）原始 body：不进入脚本，脚本中 body 为空串，
    /// 脚本未赋值时原样转发，避免 lossy 转码损坏字节。
    #[serde(skip)]
    raw: Option<Bytes>,
}

/// HTTP 响应数据，供脚本 onResponse 钩子读写。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseData {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
    /// 同 [`RequestData::raw`]。
    #[serde(skip)]
    raw: Option<Bytes>,
}

impl RequestData {
    /// 从 rama request Parts 和 body 字节构造。
    pub fn from_rama_parts(parts: &rama::http::request::Parts, body: Bytes) -> Self {
        let (body, raw) = split_body(body);
        Self {
            method: parts.method.to_string(),
            uri: parts.uri.to_string(),
            headers: headers_to_hashmap(&parts.headers),
            body,
            raw,
        }
    }

    /// 脚本处理后的 body 字节。
    pub fn body_bytes(&self) -> Bytes {
        resolve_body(&self.body, &self.raw)
    }

    /// 将修改后的数据应用到 rama Parts 上，返回重建的 Request。
    pub fn apply(self, mut parts: rama::http::request::Parts) -> Request {
        let body = self.body_bytes();
        parts.method = Method::from_bytes(self.method.as_bytes()).unwrap_or(parts.method);
        if let Ok(uri) = self.uri.parse::<Uri>() {
            parts.uri = uri;
        }
        parts.headers = hashmap_to_headers(&self.headers);
        // body 可能被脚本修改，移除旧 content-length 交由框架按实际长度重设
        parts.headers.remove(header::CONTENT_LENGTH);
        Request::from_parts(parts, Body::from(body))
    }
}

impl ResponseData {
    /// 从 rama response Parts 和 body 字节构造。
    pub fn from_rama_parts(parts: &rama::http::response::Parts, body: Bytes) -> Self {
        let (body, raw) = split_body(body);
        Self {
            status: parts.status.as_u16(),
            headers: headers_to_hashmap(&parts.headers),
            body,
            raw,
        }
    }

    /// 将修改后的数据应用到 rama Parts 上，返回重建的 Response。
    pub fn apply(self, mut parts: rama::http::response::Parts) -> Response {
        let body = resolve_body(&self.body, &self.raw);
        parts.status = StatusCode::from_u16(self.status).unwrap_or(parts.status);
        parts.headers = hashmap_to_headers(&self.headers);
        // body 可能被脚本修改，移除旧 content-length 交由框架按实际长度重设
        parts.headers.remove(header::CONTENT_LENGTH);
        Response::from_parts(parts, Body::from(body))
    }
}

/// 合法 UTF-8 → (文本, None)；否则 → (空串, Some(原始字节))。
fn split_body(body: Bytes) -> (String, Option<Bytes>) {
    match std::str::from_utf8(&body) {
        Ok(s) => (s.to_owned(), None),
        Err(_) => (String::new(), Some(body)),
    }
}

/// 二进制 body 且脚本未赋值（仍为空串）→ 原始字节；否则用脚本给出的文本。
fn resolve_body(body: &str, raw: &Option<Bytes>) -> Bytes {
    match raw {
        Some(raw) if body.is_empty() => raw.clone(),
        _ => Bytes::copy_from_slice(body.as_bytes()),
    }
}

/// 收集 rama Body 的所有 chunk，返回原始字节。
/// body 超过收集上限时返回 `Err(重组后的原始 body)`，调用方应跳过脚本原样转发。
///
/// 委托给 [`buf_pool::collect_body`]，复用同一套 capped-collect 逻辑。
pub async fn collect_body_bytes(body: Body) -> Result<Bytes, Body> {
    match buf_pool::collect_body(body).await {
        buf_pool::CollectedBody::Full(bytes) => Ok(bytes),
        buf_pool::CollectedBody::Capped { body, .. } => Err(body),
        // 流错误：保留已读部分（与旧行为一致——忽略出错的 chunk）
        buf_pool::CollectedBody::Error { prefix, .. } => Ok(prefix),
    }
}

/// 同 [`collect_body_bytes`]，返回 lossy UTF-8 字符串。
pub async fn collect_body_str(body: Body) -> Result<String, Body> {
    collect_body_bytes(body)
        .await
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

/// 按顺序运行所有脚本的 onRequest 钩子，如果有脚本返回 None 则表示阻止请求，
/// 最终返回修改后的 RequestData 或 None。
pub fn run_request_hooks(scripts: &[String], data: RequestData) -> Option<RequestData> {
    let mut current = data;
    for script in scripts {
        match engine::exec_request_hook(script, &current) {
            // raw 不经 JS 往返（serde skip），需手动带到下一轮
            Ok(Some(modified)) => {
                current = RequestData {
                    raw: current.raw.take(),
                    ..modified
                }
            }
            Ok(None) => return None,
            Err(e) => log::warn!("[script] onRequest error: {e}"),
        }
    }
    Some(current)
}

/// Run onResponse hooks across all scripts in sequence.
pub fn run_response_hooks(scripts: &[String], data: ResponseData) -> ResponseData {
    let mut current = data;
    for script in scripts {
        match engine::exec_response_hook(script, &current) {
            Ok(modified) => {
                current = ResponseData {
                    raw: current.raw.take(),
                    ..modified
                }
            }
            Err(e) => log::warn!("[script] onResponse error: {e}"),
        }
    }
    current
}

fn headers_to_hashmap(headers: &rama::http::HeaderMap) -> HashMap<String, String> {
    headers
        .iter()
        .filter_map(|(k, v)| Some((k.to_string(), v.to_str().ok()?.to_string())))
        .collect()
}

fn hashmap_to_headers(map: &HashMap<String, String>) -> rama::http::HeaderMap {
    let mut headers = rama::http::HeaderMap::new();
    for (k, v) in map {
        if let (Ok(name), Ok(value)) = (
            header::HeaderName::from_bytes(k.as_bytes()),
            header::HeaderValue::from_str(v),
        ) {
            headers.insert(name, value);
        }
    }
    headers
}
