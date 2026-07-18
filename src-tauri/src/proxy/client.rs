use std::convert::Infallible;
use std::time::Duration;

use crate::utils::date;

use rama::http::client::EasyHttpWebClient;
use rama::http::layer::decompression::DecompressionLayer;
use rama::http::layer::map_response_body::MapResponseBodyLayer;
use rama::http::layer::timeout::{ResponseBodyTimeoutLayer, TimeoutLayer};
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
use crate::proxy::ctx::ProxyCtx;
use crate::proxy::state;
use crate::script;
use crate::utils::buf_pool;
use crate::utils::request_ext::RequestExt;

pub(crate) async fn http_mitm_proxy(req: Request) -> Result<Response, Infallible> {
    // version 用于判别客户端与 MITM 之间是 HTTP/1.1 还是 h2（排查用）
    log::info!(
        "MITM request: {} {} ({:?})",
        req.method(),
        req.uri(),
        req.version()
    );

    let state: State = req.ext();
    let from_connect_tunnel = req.has_ext::<ViaConnectTunnel>();
    let start_ms = req.try_ext::<state::StartTime>().map(|st| st.0);
    let host = req.uri().host_str().unwrap_or_default().to_string();

    let (parts, body) = req.into_parts();
    let scripts = state.get_scripts(&host, parts.method.as_str());

    // ---- request: run scripts ----
    let req = match apply_request_scripts(&state, &scripts, parts, body).await {
        Ok(req) => req,
        Err(blocked) => return Ok(blocked),
    };

    // ---- log request event (after script modification) ----
    let (parts, body) = req.into_parts();
    // ctx 在脚本执行后构造：存入的 parts（method/uri/headers）为脚本改写后的定稿。
    let ctx = ProxyCtx::new(
        parts.clone(),
        state.event_channel(),
        state.settings().clone(),
        start_ms,
    )
    .with_sessions(state.sessions())
    .with_db(state.db());

    // capped_body 非 None 表示 body 超过收集上限：仅记录前缀，原样转发完整流。
    let (body_bytes, capped_body) = match crate::utils::buf_pool::collect_body(body).await {
        Ok(buf_pool::CollectedBody::Full(bytes)) => (bytes, None),
        Ok(buf_pool::CollectedBody::Capped { prefix, body }) => {
            log::warn!(
                "[probe] {} request body exceeds capture limit, logging {} bytes prefix only",
                ctx.request_id(),
                prefix.len()
            );
            (prefix, Some(body))
        }
        Err(err) => {
            return Ok(parser::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                err.to_string(),
            ));
        }
    };
    parser::record_request(&ctx, &body_bytes);
    log::info!(
        "[probe] {} request logged, body={} bytes",
        ctx.request_id(),
        body_bytes.len()
    );
    let req = Request::from_parts(
        parts,
        capped_body.unwrap_or_else(|| Body::from(body_bytes)),
    );

    // ---- direct request (non-tunnel, non-proxy) → reply inline ----
    // 仅 origin-form 且非 CONNECT 隧道时才是直接访问本服务。
    // absolute-form（如 `GET http://host/path`）是正向代理请求，应转发上游。
    if !from_connect_tunnel && !req.uri().is_absolute() {
        let (_, body) = req.into_parts();
        return Ok(parser::direct_response(&ctx, body));
    }

    // ---- forward to upstream ----
    let client = build_upstream_service(state.settings().proxy.upstream_proxy, true);

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

    let body_str = match script::collect_body_str(body).await {
        Ok(s) => s,
        // body 超过收集上限：跳过脚本，原样转发
        Err(body) => {
            log::warn!("[script] request body exceeds capture limit, skipping request hooks");
            return Ok(Request::from_parts(parts, body));
        }
    };
    let req_data = script::RequestData::from_rama_parts(&parts, body_str);

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

/// 响应执行脚本，返回修改后的响应。
async fn apply_response_scripts(scripts: &[String], resp: Response) -> Response {
    if scripts.is_empty() {
        return resp;
    }
    let (parts, body) = resp.into_parts();
    let body_str = match script::collect_body_str(body).await {
        Ok(s) => s,
        // body 超过收集上限：跳过脚本，原样转发
        Err(body) => {
            log::warn!("[script] response body exceeds capture limit, skipping response hooks");
            return Response::from_parts(parts, body);
        }
    };
    let resp_data = script::ResponseData::from_rama_parts(&parts, body_str);
    script::run_response_hooks(scripts, resp_data).apply(parts)
}

/// Forward the request upstream, then apply response scripts and log.
async fn forward_and_log(
    ctx: ProxyCtx,
    client: BoxService<Request, Response, rama::error::BoxError>,
    req: Request,
    scripts: &[String],
) -> Result<Response, Infallible> {
    log::info!("[probe] {} forwarding to upstream...", ctx.request_id());
    let forward_start = date::instant_now();
    match client.serve(req).await {
        Ok(resp) => {
            log::info!(
                "[probe] {} upstream head: {} ({:?})",
                ctx.request_id(),
                resp.status(),
                forward_start.elapsed()
            );
            let resp = apply_response_scripts(scripts, resp).await;
            let resp = parser::record_response(&ctx, resp);
            Ok(resp)
        }
        Err(err) => {
            let msg = format!("{err}");
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
            // ── DB 写入错误 ──
            if let (Some(db), Some(db_id)) = (ctx.db_ref(), ctx.db_id()) {
                db.set_traffic_error(db_id, &msg).ok();
            }
            Ok(parser::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                msg,
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
            .with_tls_support_using_rustls_and_default_http_version(tls_config, Version::HTTP_11)
            .with_default_http_connector(Executor::default())
            .build_client()
    } else {
        EasyHttpWebClient::connector_builder()
            .with_default_transport_connector()
            .with_default_dns_connector()
            .with_tls_proxy_support_using_rustls()
            .without_proxy_support()
            .with_tls_support_using_rustls_and_default_http_version(tls_config, Version::HTTP_11)
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
