use bytes::Bytes;
use rama::http::{Body, Method, Request};
use rama::net::uri::Uri;
use rama::service::Service;
use std::collections::HashMap;

use crate::AppState;
use crate::proxy::client;
use crate::proxy::ctx::ProxyCtx;
use crate::proxy::events::ProxyEvent;
use crate::proxy::parser;

#[tauri::command]
pub async fn resend_request(
    state: tauri::State<'_, AppState>,
    method: String,
    url: String,
    headers: HashMap<String, String>,
    body: Option<String>,
) -> Result<u64, String> {
    let method = Method::from_bytes(method.as_bytes()).unwrap_or(Method::GET);
    let full_uri: Uri = url.parse().map_err(|_| "invalid url")?;

    // 1. Build request parts
    let mut req_builder = Request::builder().method(method).uri(full_uri);
    for (k, v) in &headers {
        let lk = k.to_lowercase();
        if lk == "host" || lk == "content-length" || lk == "transfer-encoding" {
            continue;
        }
        req_builder = req_builder.header(k.as_str(), v.as_str());
    }
    let req = req_builder
        .body(Body::empty())
        .map_err(|e| format!("build: {e:?}"))?;
    let (parts, _empty_body) = req.into_parts();

    let ctx = ProxyCtx::new(parts.clone(), state.event_channel(), state.settings(), None);

    // 2. body -> Bytes
    let body_bytes: Bytes = body
        .map(|s| Bytes::from(s.into_bytes()))
        .unwrap_or_default();

    // 3. Log request event
    parser::record_request(&ctx, &body_bytes);

    // 4. Build Request and send upstream
    let req = Request::from_parts(parts, Body::from(body_bytes));

    let up = state.settings().proxy.upstream_proxy;
    let svc = client::build_upstream_service(up, false);
    match svc.serve(req).await {
        Ok(resp) => {
            let (parts, body) = resp.into_parts();

            let resp_bytes = match crate::utils::buf_pool::collect_body(body)
                .await
                .map_err(|e| format!("resend failed: {e:?}"))?
            {
                crate::utils::buf_pool::CollectedBody::Full(bytes) => bytes,
                // 超过收集上限：仅展示前缀，丢弃剩余流
                crate::utils::buf_pool::CollectedBody::Capped { prefix, .. } => prefix,
            };

            let resp_body = String::from_utf8_lossy(&resp_bytes);

            ctx.send(ProxyEvent::ResponseChunk {
                id: ctx.request_id(),
                chunk: resp_body.to_string(),
            });

            ctx.send(ProxyEvent::Response {
                id: ctx.request_id(),
                status: parts.status.as_u16(),
                timestamp: crate::utils::date::now_ms(),
                duration_ms: ctx.duration_ms(),
                headers: parts
                    .headers
                    .iter()
                    .filter_map(|(k, v)| Some((k.to_string(), v.to_str().ok()?.to_string())))
                    .collect(),
                content_type: parts
                    .headers
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string()),
                content_length: parts
                    .headers
                    .get("content-length")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok()),
            });

            Ok(ctx.request_id())
        }
        Err(err) => {
            let msg = format!("{err}");
            ctx.send(ProxyEvent::Error {
                id: ctx.request_id(),
                error: msg,
            });
            Ok(ctx.request_id())
        }
    }
}
