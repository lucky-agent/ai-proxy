use std::collections::HashMap;

use rama::http::{Body, Method, Request, Response, StatusCode, Uri};
use rama::futures::StreamExt;
use log::info;
use serde_json::json;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

/// Log request and emit Tauri event. Returns (modified request, method, uri, request_id).
pub(crate) fn log_request(req: Request, app_handle: &AppHandle) -> (Request, Method, Uri, String) {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let request_id = Uuid::new_v4().to_string();

    let request_timestamp = chrono::Utc::now().timestamp_millis();

    let (parts, body) = req.into_parts();

    let req_headers: HashMap<String, String> = parts
        .headers
        .iter()
        .filter_map(|(name, value)| Some((name.to_string(), value.to_str().ok()?.to_string())))
        .collect();

    let _ = app_handle.emit(
        "proxy:request",
        json!({
            "id": request_id,
            "method": method.to_string(),
            "uri": uri.to_string(),
            "timestamp": request_timestamp,
            "headers": req_headers,
        }),
    );

    let log_method = method.clone();
    let log_uri = uri.clone();
    let rid = request_id.clone();
    let ah = app_handle.clone();
    let logged_body = Body::from_stream(body.into_data_stream().map(move |result| {
        if let Ok(ref bytes) = result {
            let chunk_str = String::from_utf8_lossy(bytes);
            info!("Request chunk [{} {}]: {}", log_method, log_uri, chunk_str);
            let _ = ah.emit(
                "proxy:request-chunk",
                json!({
                    "id": rid,
                    "chunk": chunk_str.into_owned(),
                }),
            );
        }
        result
    }));

    let req = Request::from_parts(parts, logged_body);
    (req, method, uri, request_id)
}

/// Log response and emit Tauri event.
/// Headers/status are emitted immediately; each body chunk is emitted
/// as a separate `proxy:response-chunk` event, preserving streaming behavior.
pub(crate) fn log_response(
    resp: Response,
    method: Method,
    uri: Uri,
    request_id: &str,
    duration_ms: u64,
    app_handle: &AppHandle,
) -> Response {
    let status = resp.status();
    info!("Response [{} {}] {}", method, uri, status);

    let response_timestamp = chrono::Utc::now().timestamp_millis();

    let (parts, body) = resp.into_parts();

    let resp_headers: HashMap<String, String> = parts
        .headers
        .iter()
        .filter_map(|(name, value)| Some((name.to_string(), value.to_str().ok()?.to_string())))
        .collect();

    let _ = app_handle.emit(
        "proxy:response",
        json!({
            "id": request_id,
            "status": status.as_u16(),
            "timestamp": response_timestamp,
            "duration_ms": duration_ms,
            "headers": resp_headers,
        }),
    );

    let log_method = method.clone();
    let log_uri = uri.clone();
    let rid = request_id.to_string();
    let ah = app_handle.clone();

    let logged_body = Body::from_stream(body.into_data_stream().map(move |result| {
        if let Ok(ref bytes) = result {
            let chunk_str = String::from_utf8_lossy(bytes);
            info!("Response chunk [{} {}]: {}", log_method, log_uri, chunk_str);
            let _ = ah.emit(
                "proxy:response-chunk",
                json!({
                    "id": rid,
                    "chunk": chunk_str.into_owned(),
                }),
            );
        }
        result
    }));

    Response::from_parts(parts, logged_body)
}

/// Build an error response for proxy forwarding failures.
pub(crate) fn error_response() -> Response {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(Body::empty())
        .unwrap()
}