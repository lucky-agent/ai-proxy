use std::convert::Infallible;
use std::time::Duration;

use rama::extensions::ExtensionsRef;
use rama::http::client::EasyHttpWebClient;
use rama::http::layer::decompression::DecompressionLayer;
use rama::http::layer::map_response_body::MapResponseBodyLayer;
use rama::http::layer::timeout::{ResponseBodyTimeoutLayer, TimeoutLayer};
use rama::http::request::HttpRequestParts;
use rama::http::{Body, Request, Response, StatusCode, Version};
use rama::layer::Layer;
use rama::rt::Executor;
use rama::service::BoxService;
use rama::service::Service;
use rama::tls::client::{ServerVerifyMode, TlsClientConfig};

use super::events::ProxyEvent;
use super::parser;
use super::state::State;
use super::state::ViaConnectTunnel;
use crate::proxy::state::ProxyCtx;
use crate::script;

pub(crate) async fn http_mitm_proxy(req: Request) -> Result<Response, Infallible> {
    let method = req.method().clone();
    let uri = req.uri().clone();
    // version 用于判别客户端与 MITM 之间是 HTTP/1.1 还是 h2（排查用）
    log::info!("MITM request: {method} {uri} ({:?})", req.version());

    let state = req
        .extensions()
        .get_ref::<State>()
        .cloned()
        .expect("State not found in request extensions");
    let from_connect_tunnel = req.extensions().get_ref::<ViaConnectTunnel>().is_some();
    let host = req.uri().host_str().unwrap_or_default().to_string();

    let (parts, body) = req.into_parts();
    let scripts = state.get_scripts(&host);
    let ctx = ProxyCtx::new(
        parts.method.clone(),
        parts.uri().clone(),
        state.event_channel(),
        state.settings().clone(),
    )
    .with_sessions(state.sessions());

    // ---- request: run scripts ----
    let req = match apply_request_scripts(&state, &scripts, parts, body).await {
        Ok(req) => req,
        Err(blocked) => return Ok(blocked),
    };

    // ---- log request event (after script modification) ----
    let (parts, body) = req.into_parts();
    let body_bytes = match crate::utils::buf_pool::collect_body(body).await {
        Ok(bytes) => bytes,
        Err(err) => {
            return Ok(parser::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                err.to_string(),
            ));
        }
    };
    parser::log_request(&ctx, &parts, &body_bytes);
    log::info!(
        "[probe] {} request logged, body={} bytes",
        ctx.request_id(),
        body_bytes.len()
    );
    let req = Request::from_parts(parts, Body::from(body_bytes));

    // ---- direct request (non-tunnel) → reply inline ----
    if !from_connect_tunnel {
        let (req_parts, body) = req.into_parts();

        return Ok(parser::direct_response(&ctx, req_parts, body));
    }

    // ---- forward to upstream ----
    let client = build_upstream_service(
        state.settings().proxy.upstream_proxy,
        true,
    );

    forward_and_log(ctx, client, req, &scripts).await
}

///  请求执行脚本，返回修改后的请求或直接响应（如被脚本阻止）
async fn apply_request_scripts(
    state: &State,
    scripts: &[String],
    parts: rama::http::request::Parts,
    body: Body,
) -> Result<Request, Response> {
    if scripts.is_empty() {
        return Ok(Request::from_parts(parts, body));
    }

    let body_str = script::collect_body_str(body).await;
    let req_data = script::RequestData::from_rama_parts(&parts, &body_str);

    match script::run_request_hooks(scripts, req_data) {
        Some(modified) => Ok(modified.apply(parts)),
        None => {
            log::info!("[script] request blocked");
            if let Some(ref ch) = state.event_channel() {
                ch.send(ProxyEvent::Error {
                    id: "blocked".into(),
                    error: "Request blocked by script".into(),
                })
                .ok();
            }
            Err(parser::error_response(
                StatusCode::FORBIDDEN,
                "Blocked by script",
            ))
        }
    }
}

/// Forward the request upstream, then apply response scripts and log.
async fn forward_and_log(
    ctx: ProxyCtx,
    client: BoxService<Request, Response, rama::error::BoxError>,
    req: Request,
    scripts: &[String],
) -> Result<Response, Infallible> {
    log::info!("[probe] {} forwarding to upstream...", ctx.request_id());
    let forward_start = std::time::Instant::now();
    match client.serve(req).await {
        Ok(resp) => {
            log::info!(
                "[probe] {} upstream head: {} ({:?})",
                ctx.request_id(),
                resp.status(),
                forward_start.elapsed()
            );
            let resp = if !scripts.is_empty() {
                let (parts, body) = resp.into_parts();
                let body_str = script::collect_body_str(body).await;
                let resp_data = script::ResponseData::from_rama_parts(&parts, &body_str);
                let modified = script::run_response_hooks(scripts, resp_data);
                modified.apply(parts)
            } else {
                resp
            };
            let resp = parser::log_response(&ctx, resp);
            Ok(resp)
        }
        Err(err) => {
            log::error!(
                "error proxying request [{} {}] after {:?}: {err:?}",
                ctx.method(),
                ctx.uri(),
                forward_start.elapsed()
            );
            ctx.send(ProxyEvent::Error {
                id: ctx.request_id().to_string(),
                error: format!("{err:?}"),
            });
            Ok(parser::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("{err:?}"),
            ))
        }
    }
}
pub(crate) fn build_upstream_service(
    upstream_proxy: bool,
    skip_tls_verify: bool,
) -> BoxService<Request, Response, rama::error::BoxError> {
    use std::collections::HashMap;
    use std::sync::{LazyLock, Mutex};

    static CACHE: LazyLock<
        Mutex<HashMap<(bool, bool), BoxService<Request, Response, rama::error::BoxError>>>,
    > = LazyLock::new(|| Mutex::new(HashMap::new()));

    let key = (upstream_proxy, skip_tls_verify);
    if let Some(svc) = CACHE.lock().unwrap().get(&key) {
        return svc.clone();
    }

    // 跳过 TLS 验证（不安全，仅用于测试）
    let tls_config = if skip_tls_verify {
        TlsClientConfig::default_http().with_server_verify(ServerVerifyMode::Disable)
    } else {
        TlsClientConfig::default_http()
    };

    let client = if upstream_proxy {
        EasyHttpWebClient::connector_builder()
            .with_default_transport_connector()
            .with_default_dns_connector()
            .with_tls_proxy_support_using_rustls()
            .with_proxy_support()
            .with_tls_support_using_rustls_and_default_http_version(
                tls_config,
                Version::HTTP_11,
            )
            .with_default_http_connector(Executor::default())
            .build_client()
    } else {
        EasyHttpWebClient::connector_builder()
            .with_default_transport_connector()
            .with_default_dns_connector()
            .with_tls_proxy_support_using_rustls()
            .without_proxy_support()
            .with_tls_support_using_rustls_and_default_http_version(
                tls_config,
                Version::HTTP_11,
            )
            .with_default_http_connector(Executor::default())
            .build_client()
    };

    let svc = (
        MapResponseBodyLayer::new_boxed_streaming_body(),
        DecompressionLayer::new().with_insert_accept_encoding_header(false),
        // 300s overall timeout as safety net against hung requests
        TimeoutLayer::with_status_code(StatusCode::GATEWAY_TIMEOUT, Duration::from_secs(300)),
        // 60s per-chunk timeout: kills dead connections quickly while
        // streaming AI responses can run arbitrarily long under the 300s cap
        ResponseBodyTimeoutLayer::new(Duration::from_secs(60)),
    )
        .into_layer(client)
        .boxed();

    CACHE.lock().unwrap().insert(key, svc.clone());
    svc
}
