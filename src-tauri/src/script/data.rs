use std::collections::HashMap;

use rama::http::body::CollectOptions;
use rama::http::body::util::BodyExt;
use rama::http::{Body, Method, Request, Response, StatusCode, header};
use rama::net::uri::Uri;
use serde::{Deserialize, Serialize};

use crate::utils::buf_pool::BODY_CAPTURE_LIMIT;

use super::engine;

/// HTTP 请求数据，供脚本 onRequest 钩子读写。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestData {
    pub method: String,
    pub uri: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

/// HTTP 响应数据，供脚本 onResponse 钩子读写。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseData {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
}

impl RequestData {
    /// 从 rama request Parts 和 body 字符串构造。
    pub fn from_rama_parts(parts: &rama::http::request::Parts, body: String) -> Self {
        Self {
            method: parts.method.to_string(),
            uri: parts.uri.to_string(),
            headers: headers_to_hashmap(&parts.headers),
            body,
        }
    }

    /// 将修改后的数据应用到 rama Parts 上，返回重建的 Request。
    pub fn apply(self, mut parts: rama::http::request::Parts) -> Request {
        parts.method = Method::from_bytes(self.method.as_bytes()).unwrap_or(parts.method);
        if let Ok(uri) = self.uri.parse::<Uri>() {
            parts.uri = uri;
        }
        parts.headers = hashmap_to_headers(&self.headers);
        // body 可能被脚本修改，移除旧 content-length 交由框架按实际长度重设
        parts.headers.remove(header::CONTENT_LENGTH);
        Request::from_parts(parts, Body::from(self.body))
    }
}

impl ResponseData {
    /// 从 rama response Parts 和 body 字符串构造。
    pub fn from_rama_parts(parts: &rama::http::response::Parts, body: String) -> Self {
        Self {
            status: parts.status.as_u16(),
            headers: headers_to_hashmap(&parts.headers),
            body,
        }
    }

    /// 将修改后的数据应用到 rama Parts 上，返回重建的 Response。
    pub fn apply(self, mut parts: rama::http::response::Parts) -> Response {
        parts.status = StatusCode::from_u16(self.status).unwrap_or(parts.status);
        parts.headers = hashmap_to_headers(&self.headers);
        // body 可能被脚本修改，移除旧 content-length 交由框架按实际长度重设
        parts.headers.remove(header::CONTENT_LENGTH);
        Response::from_parts(parts, Body::from(self.body))
    }
}

/// 收集 rama Body 的所有 chunk，返回 UTF-8 字符串。
/// body 超过收集上限时返回 `Err(重组后的原始 body)`，调用方应跳过脚本原样转发。
pub async fn collect_body_str(body: Body) -> Result<String, Body> {
    match body
        .collect_with(CollectOptions::new().with_max_size(BODY_CAPTURE_LIMIT))
        .await
    {
        Ok(collected) => Ok(String::from_utf8_lossy(&collected.to_bytes()).into_owned()),
        Err(err) if err.is_cap_reached() => Err(err
            .into_full_body()
            .expect("cap reached implies forwardable remainder")),
        // 流错误：保留已读部分（与旧行为一致——忽略出错的 chunk）
        Err(err) => Ok(String::from_utf8_lossy(&err.bytes_read()).into_owned()),
    }
}

/// 按顺序运行所有脚本的 onRequest 钩子，如果有脚本返回 None 则表示阻止请求，
/// 最终返回修改后的 RequestData 或 None。
pub fn run_request_hooks(scripts: &[String], data: RequestData) -> Option<RequestData> {
    let mut current = data;
    for script in scripts {
        match engine::exec_request_hook(script, &current) {
            Ok(Some(modified)) => current = modified,
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
            Ok(modified) => current = modified,
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
