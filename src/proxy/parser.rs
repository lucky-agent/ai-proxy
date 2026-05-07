use rama::http::{Body, Method, Request, Response, StatusCode, Uri};
use rama::futures::StreamExt;
use tracing::info;

/// Log request body chunks as they flow through, without collecting or modifying.
pub(crate) fn log_request(req: Request) -> (Request, Method, Uri) {
    let method = req.method().clone();
    let uri = req.uri().clone();

    let log_method = method.clone();
    let log_uri = uri.clone();

    let (parts, body) = req.into_parts();
    let logged_body = Body::from_stream(body.into_data_stream().map(move |result| {
        if let Ok(ref bytes) = result {
            let chunk_str = String::from_utf8_lossy(bytes);
            info!("Request chunk [{} {}]: {}", log_method, log_uri, chunk_str);
        }
        result
    }));

    let req = Request::from_parts(parts, logged_body);
    (req, method, uri)
}

/// Log response body chunks as they flow through, without collecting or modifying.
pub(crate) fn log_response(
    resp: Response,
    method: Method,
    uri: Uri,
) -> Response {
    let status = resp.status();
    info!("Response [{} {}] {}", method, uri, status);

    let (parts, body) = resp.into_parts();
    let logged_body = Body::from_stream(body.into_data_stream().map(move |result| {
        if let Ok(ref bytes) = result {
            let chunk_str = String::from_utf8_lossy(bytes);
            info!("Response chunk [{} {}]: {}", method, uri, chunk_str);
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