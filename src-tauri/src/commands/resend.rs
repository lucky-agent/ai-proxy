use bytes::Bytes;
use rama::http::{Body, Method, Request, Uri};
use rama::service::Service;
use std::collections::HashMap;

use crate::AppState;
use crate::proxy::client;
use crate::proxy::events::ProxyEvent;
use crate::proxy::parser;
use crate::proxy::state::ProxyCtx;

#[tauri::command]
pub async fn resend_request(
    state: tauri::State<'_, AppState>,
    method: String,
    url: String,
    headers: HashMap<String, String>,
    body: Option<String>,
) -> Result<String, String> {
    let method = Method::from_bytes(method.as_bytes()).unwrap_or(Method::GET);
    let full_uri: Uri = url.parse().map_err(|_| "invalid url")?;
    let ctx = ProxyCtx::new(method.clone(), full_uri.clone(), state.event_channel());

    // 1. 用 Body::empty() 获取 Parts
    let mut req_builder = Request::builder()
        .method(method.clone())
        .uri(full_uri.clone());
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

    // 2. DB 持久化 — 获取自增 ID
    let db = state.db();
    let db_id: i64 = {
        let guard = db.lock().map_err(|e| format!("db lock: {e}"))?;
        let q: HashMap<String, String> = HashMap::new();
        guard
            .upsert_request(
                &method.to_string(),
                &full_uri.to_string(),
                chrono::Utc::now().timestamp_millis(),
                &serde_json::to_string(&headers).unwrap_or_default(),
                &serde_json::to_string(&q).unwrap_or_default(),
                body.as_deref(),
                true,
                "traffic",
                None,
                "[]",
                "",
                "",
                "",
            )
            .map_err(|e| format!("db upsert: {e}"))?
    };

    // 3. body → Bytes
    let body_bytes: Bytes = body
        .map(|s| Bytes::from(s.into_bytes()))
        .unwrap_or_default();

    // 4. 日志事件
    parser::log_request(&ctx, &parts, &body_bytes);

    // 5. 拼装 Request 发给上游
    let req = Request::from_parts(parts, Body::from(body_bytes));

    // 6. execute via rama client
    let up = state.settings().proxy.upstream_proxy;
    let svc = client::build_upstream_service(rama::rt::Executor::default(), up, false);
    match svc.serve(req).await {
        Ok(resp) => {
            let (parts, body) = resp.into_parts();
            let headers_json = crate::config::db::Db::headers_to_json(&parts.headers);

            let resp_bytes = crate::utils::buf_pool::collect_body(body)
                .await
                .map_err(|e| format!("resend failed: {e:?}"))?;

            let resp_body = String::from_utf8_lossy(&resp_bytes);

            // 发送响应体 chunk 事件（用 ctx.request_id() 保持 ID 一致）
            ctx.send(ProxyEvent::ResponseChunk {
                id: ctx.request_id().to_string(),
                chunk: resp_body.to_string(),
            });

            // persist response metadata
            if let Ok(db) = state.db().lock() {
                db.update_response(
                    db_id,
                    parts.status.as_u16(),
                    chrono::Utc::now().timestamp_millis(),
                    ctx.duration_ms(),
                    &headers_json,
                )
                .ok();
                db.update_response_body(db_id, &resp_body).ok();
            }

            // 发送响应事件（用 ctx.request_id() 保持 ID 一致）
            ctx.send(ProxyEvent::Response {
                id: ctx.request_id().to_string(),
                status: parts.status.as_u16(),
                timestamp: chrono::Utc::now().timestamp_millis(),
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

            Ok(ctx.request_id().to_string())
        }
        Err(err) => {
            let msg = format!("{err}");
            ctx.send(ProxyEvent::Error {
                id: ctx.request_id().to_string(),
                error: msg.clone(),
            });
            if let Ok(db) = state.db().lock() {
                db.set_error(db_id, &msg).ok();
            }
            Ok(ctx.request_id().to_string())
        }
    }
}
