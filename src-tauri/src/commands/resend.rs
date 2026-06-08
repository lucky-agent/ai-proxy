use std::collections::HashMap;
use std::time::Instant;
use rama::http::{Body, Request};
use rama::futures::StreamExt;
use rama::service::Service;
use uuid::Uuid;
use crate::proxy::client;
use crate::proxy::events::ProxyEvent;
use crate::AppState;
#[tauri::command]
pub async fn resend_request(
    state: tauri::State<'_, AppState>,
    method: String,
    url: String,
    headers: HashMap<String, String>,
    body: Option<String>,
) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    let event_channel = state.event_channel();
    if let Some(ref ch) = event_channel {
        ch.send(ProxyEvent::Request {
            id: id.clone(),
            method: method.clone(),
            uri: url.clone(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            headers: headers.clone(),
            query_params: HashMap::new(),
        })
        .map_err(|e| format!("event channel: {e:?}"))?;
    }
    let mut req_builder = Request::builder().method(method.as_str()).uri(&url);
    for (k, v) in &headers {
        let lk = k.to_lowercase();
        if lk == "host" || lk == "content-length" || lk == "transfer-encoding" { continue; }
        req_builder = req_builder.header(k.as_str(), v.as_str());
    }
    let req = match body {
        Some(b) => req_builder.body(Body::from(b)).map_err(|e| format!("build: {e:?}"))?,
        None => req_builder.body(Body::empty()).map_err(|e| format!("build: {e:?}"))?,
    };
    let svc = client::build_upstream_service(rama::rt::Executor::default(), false, false);
    let start = Instant::now();
    match svc.serve(req).await {
        Ok(resp) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            let (parts, resp_body) = resp.into_parts();
            let is_sse = parts.headers.get("content-type")
                .and_then(|v| v.to_str().ok())
                .map(|ct| ct.to_lowercase().contains("text/event-stream"))
                .unwrap_or(false);
            let mut stream = resp_body.into_data_stream();
            let mut acc = String::new();
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        let s = String::from_utf8_lossy(&bytes).to_string();
                        if is_sse {
                            if let Some(ref ch) = event_channel {
                                ch.send(ProxyEvent::ResponseChunk { id: id.clone(), chunk: s.clone() }).ok();
                            }
                        }
                        acc.push_str(&s);
                    }
                    Err(e) => { log::warn!("chunk: {e:?}"); break; }
                }
            }
            if !is_sse && !acc.is_empty() {
                if let Some(ref ch) = event_channel {
                    ch.send(ProxyEvent::ResponseChunk { id: id.clone(), chunk: acc }).ok();
                }
            }
            let resp_headers: HashMap<String, String> = parts.headers.iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();
            if let Some(ref ch) = event_channel {
                ch.send(ProxyEvent::Response {
                    id: id.clone(), status: parts.status.as_u16(),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    duration_ms, headers: resp_headers,
                }).ok();
            }
            Ok(id)
        }
        
        Err(err) => {
            if let Some(ref ch) = event_channel {
                ch.send(ProxyEvent::Error { id: id.clone(), error: format!("{err:?}") }).ok();
            }
            Err(format!("resend: {err:?}"))
        }
    }
}
