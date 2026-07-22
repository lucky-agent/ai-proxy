use bytes::Bytes;
use rama::http::{Body, Method, Request};
use rama::net::uri::Uri;
use rama::service::Service;
use std::collections::HashMap;

use crate::AppState;
use crate::proxy::client;
use crate::proxy::ctx::ProxyCtx;
use crate::proxy::events::ProxyEvent;
use crate::proxy::record;

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
    record::record_request(&ctx, &body_bytes);

    // 4. Build Request and send upstream
    let req = Request::from_parts(parts, Body::from(body_bytes));

    let up = state.settings().proxy.upstream_proxy;
    let svc = client::build_upstream_service(up, false);
    match svc.serve(req).await {
        Ok(resp) => {
            let request_id = record::record_and_drain_response(ctx, resp).await;
            Ok(request_id)
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
