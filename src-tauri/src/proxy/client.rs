use std::convert::Infallible;
use std::time::Duration;

use rama::extensions::ExtensionsRef;
use rama::http::client::EasyHttpWebClient;
use rama::http::layer::decompression::DecompressionLayer;
use rama::http::layer::map_response_body::MapResponseBodyLayer;
use rama::http::layer::timeout::TimeoutLayer;
use rama::http::request::HttpRequestParts;
use rama::http::{Body, Request, Response, StatusCode, Version};
use rama::layer::Layer;
use rama::rt::Executor;
use rama::service::BoxService;
use rama::service::Service;
use rama::tls::rustls::client::TlsConnectorDataBuilder;

use super::events::ProxyEvent;
use super::parser;
use super::state::State;
use super::state::ViaConnectTunnel;
use crate::proxy::state::ProxyCtx;
use crate::script;

pub(crate) async fn http_mitm_proxy(req: Request) -> Result<Response, Infallible> {
    let state = req
        .extensions()
        .get_ref::<State>()
        .cloned()
        .expect("State not found in request extensions");
    let from_connect_tunnel = req.extensions().get_ref::<ViaConnectTunnel>().is_some();
    let host = req.uri().host().unwrap_or("").to_string();

    let (parts, body) = req.into_parts();
    let scripts = state.get_scripts(&host);
    let ctx = ProxyCtx::new(
        parts.method.clone(),
        parts.uri().clone(),
        state.event_channel(),
    );

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
    match client.serve(req).await {
        Ok(resp) => {
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
                "error proxying request [{} {}]: {err:?}",
                ctx.method(),
                ctx.uri()
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

    let tls_builder = TlsConnectorDataBuilder::new()
        .with_alpn_protocols_http_auto()
        .try_with_env_key_logger()
        .expect("with env key logger");

    // 跳过 TLS 验证（不安全，仅用于测试）
    let tls_config = if skip_tls_verify {
        tls_builder.with_no_cert_verifier().build()
    } else {
        tls_builder.build()
    };

    let client = if upstream_proxy {
        EasyHttpWebClient::connector_builder()
            .with_default_transport_connector()
            .with_tls_proxy_support_using_rustls()
            .with_proxy_support()
            .with_tls_support_using_rustls_and_default_http_version(
                Some(tls_config),
                Version::HTTP_11,
            )
            .with_default_http_connector(Executor::default())
            .build_client()
    } else {
        EasyHttpWebClient::connector_builder()
            .with_default_transport_connector()
            .with_tls_proxy_support_using_rustls()
            .without_proxy_support()
            .with_tls_support_using_rustls_and_default_http_version(
                Some(tls_config),
                Version::HTTP_11,
            )
            .with_default_http_connector(Executor::default())
            .build_client()
    };

    let svc = (
        MapResponseBodyLayer::new_boxed_streaming_body(),
        DecompressionLayer::new().with_insert_accept_encoding_header(false),
        // 30-second request timeout to prevent hanging when upstream is unreachable
        TimeoutLayer::with_status_code(StatusCode::GATEWAY_TIMEOUT, Duration::from_secs(30)),
    )
        .into_layer(client)
        .boxed();

    CACHE.lock().unwrap().insert(key, svc.clone());
    svc
}
