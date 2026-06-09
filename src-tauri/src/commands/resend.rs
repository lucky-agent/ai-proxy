use std::collections::HashMap;
use std::time::Instant;
use rama::http::{Body, Method, Request, Uri};
use rama::service::Service;

use crate::proxy::client;
use crate::proxy::parser;
use crate::proxy::events::ProxyEvent;
use crate::script;
use crate::AppState;

#[tauri::command]
pub async fn resend_request(
    state: tauri::State<'_, AppState>,
    method: String,
    url: String,
    headers: HashMap<String, String>,
    body: Option<String>,
) -> Result<String, String> {
    let event_channel = state.event_channel();


    // 1. build request
    let req_method: Method = method.parse().map_err(|_| "invalid method")?;
    let req_uri: Uri = url.parse().map_err(|_| "invalid url")?;
    let mut req_builder = Request::builder().method(&req_method).uri(&req_uri);
    for (k, v) in &headers {
        let lk = k.to_lowercase();
        if lk == "host" || lk == "content-length" || lk == "transfer-encoding" { continue; }
        req_builder = req_builder.header(k.as_str(), v.as_str());
    }
    let req = match body {
        Some(b) => req_builder.body(Body::from(b)).map_err(|e| format!("build: {e:?}"))?,
        None => req_builder.body(Body::empty()).map_err(|e| format!("build: {e:?}"))?,
    };

    // 2. emit request event via parser (consistent with proxy flow)
    let (mut parts, body) = req.into_parts();
    let body_str = script::collect_body_str(body).await;
    // strip authority for display (proxy flow only carries path+query)
    let full_uri = parts.uri.clone();
    parts.uri = full_uri
        .path_and_query()
        .map(|pq| pq.as_str().parse::<Uri>().expect("valid path+query"))
        .unwrap_or_else(|| parts.uri.clone());
    let (method, uri, request_id) = parser::log_request(&parts, &body_str, &event_channel);

    // persist request to DB
    if let Ok(db) = state.db().lock() {
        let q: HashMap<String, String> = HashMap::new();
        db.upsert_request(
            &request_id, &method.to_string(), &full_uri.to_string(),
            chrono::Utc::now().timestamp_millis(),
            &serde_json::to_string(&headers).unwrap_or_default(),
            &serde_json::to_string(&q).unwrap_or_default(),
            if body_str.is_empty() { None } else { Some(&body_str) },
            true,
        ).ok();
    }
    // restore full URI for upstream forwarding
    parts.uri = full_uri;
    let req = Request::from_parts(parts, Body::from(body_str));

    // 3. execute via rama client
    let svc = client::build_upstream_service(rama::rt::Executor::default(), false, false);
    let start = Instant::now();
    match svc.serve(req).await {
        Ok(resp) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            let resp = parser::log_response(resp, method, uri, &request_id, duration_ms, &event_channel);
            // persist response metadata
            if let Ok(db) = state.db().lock() {
                let h: HashMap<String, String> = resp.headers().iter()
                    .filter_map(|(k, v)| Some((k.to_string(), v.to_str().ok()?.to_string())))
                    .collect();
                db.update_response(&request_id, resp.status().as_u16(),
                    chrono::Utc::now().timestamp_millis(), duration_ms,
                    &serde_json::to_string(&h).unwrap_or_default()).ok();
            }
            // consume body to trigger chunk events and persist full body
            let resp_body = script::collect_body_str(resp.into_body()).await;
            if let Ok(db) = state.db().lock() {
                db.update_response_body(&request_id, &resp_body).ok();
            }
            Ok(request_id)
        }
        Err(err) => {
            if let Some(ref ch) = event_channel {
                ch.send(ProxyEvent::Error { id: request_id.clone(), error: format!("{err:?}") }).ok();
            }
            if let Ok(db) = state.db().lock() {
                db.set_error(&request_id, &format!("{err:?}")).ok();
            }
            Err(format!("resend failed: {err:?}"))
        }
    }
}
