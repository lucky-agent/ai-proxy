use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;

use rama::Layer;
use rama::Service;
use rama::extensions::ExtensionsRef;
use rama::http::layer::upgrade::Upgraded;
use rama::http::layer::{
    map_response_body::MapResponseBodyLayer,
    remove_header::{RemoveRequestHeaderLayer, RemoveResponseHeaderLayer},
    trace::TraceLayer,
};
use rama::http::server::HttpServer;
use rama::io::Io;
use rama::layer::AddInputExtensionLayer;
use rama::layer::ConsumeErrLayer;
use rama::layer::timeout::TimeoutLayer;
use rama::net::proxy::IoForwardService;
use rama::tls::server::peek_client_hello_from_input;
use rama::rt::Executor;
use rama::service::service_fn;
use rama::tcp::proxy::IoToProxyBridgeIoLayer;
use rama::tls::rustls::server::TlsAcceptorLayer;

use super::client::http_mitm_proxy;
use super::events::{AiHint, ProxyEvent};
use super::state::{State, ViaConnectTunnel};

pub(crate) async fn http_connect_proxy(upgraded: Upgraded) -> Result<(), Infallible> {
    let state =
        upgraded.extensions().get_ref::<State>().cloned().expect(
            "State should be set via AddInputExtensionLayer before request reaches handler",
        );

    let peek_timeout = Some(std::time::Duration::from_secs(30));
    // 通过检查 TLS ClientHello 来决定是否进行 MITM
    let (prefixed, maybe_client_hello) =
        match peek_client_hello_from_input(upgraded, peek_timeout).await {
            Ok(result) => result,
            Err(err) => {
                log::warn!("peek TLS ClientHello failed: {:?}", err);
                return Ok(());
            }
        };

    let sni = maybe_client_hello
        .as_ref()
        .and_then(|ch| ch.ext_server_name().map(|s| s.to_string()));

    // SNI 命中已启用的 MITM 白名单 → 解密；否则（无 SNI 或未命中）→ 隧道透传。
    match sni {
        Some(host) if state.settings().ssl.should_mitm(&host) => {
            log::info!(
                "CONNECT TLS, SNI={}, in MITM whitelist, routing to MITM",
                host
            );
            // 把隧道解密后的流量重新解析成 HTTP 语义
            let http_mitm_service =
                AddInputExtensionLayer::new(ViaConnectTunnel).into_layer(new_http_mitm_proxy());
            let http_transport = HttpServer::auto(Executor::default()).service(http_mitm_service);
            let https_service = TlsAcceptorLayer::new(state.mitm_tls_service_data().clone())
                .with_store_client_hello(true)
                .into_layer(http_transport);

            if let Err(err) = https_service.serve(prefixed).await {
                if is_timeout_err(&err) {
                    // TLS 握手已成功，仅是连接空闲/预连接超过 header_read_timeout(30s)
                    // 未发请求头，属正常连接回收，非故障。
                    log::debug!(
                        "MITM connection to {} idle-closed (no request): {:?}",
                        host,
                        err
                    );
                } else {
                    log::warn!("MITM session failed for {}: {:?}", host, err);
                }
            }
        }
        maybe_host => {
            let (host, port, is_tls) = if let Some(h) = maybe_host {
                let port = prefixed
                    .extensions()
                    .get_ref::<rama::net::client::ConnectorTarget>()
                    .map(|t| t.0.port)
                    .unwrap_or(443);
                (h, port, true)
            } else {
                let proxy_target = prefixed
                    .extensions()
                    .get_ref::<rama::net::client::ConnectorTarget>()
                    .map(|t| (t.0.host.to_string(), t.0.port));
                let (host, port) = proxy_target.unwrap_or_else(|| ("unknown".to_string(), 0));
                (host, port, false)
            };
            log::info!("CONNECT {}: not in MITM whitelist, routing to tunnel", host);
            tunnel_connect_proxy(state, prefixed, host, port, is_tls).await;
        }
    }
    Ok(())
}

/// 隧道路径：不解密 TLS，直接双向字节桥接。
async fn tunnel_connect_proxy<P>(state: State, prefixed: P, host: String, port: u16, is_tls: bool)
where
    P: Io + ExtensionsRef + std::marker::Unpin,
{
    let request_id = uuid::Uuid::new_v4().to_string();
    let start = std::time::Instant::now();
    let event_channel = state.event_channel();
    let scheme = if is_tls { "https" } else { "http" };
    let uri = format!("{scheme}://{host}:{port}");

    let mut headers = HashMap::new();
    headers.insert("Host".into(), format!("{host}:{port}"));

    // Emit Request event for tunnel connection
    if let Some(ref ch) = event_channel {
        ch.send(ProxyEvent::Request {
            id: request_id.clone(),
            method: "CONNECT".into(),
            uri: uri.clone(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            headers,
            query_params: HashMap::new(),
            decrypted: false,
            content_type: None,
            content_length: None,
            ai_hint: AiHint::None,
        })
        .ok();
    }

    let executor = Executor::default();
    let tunnel_svc = (TimeoutLayer::new(std::time::Duration::from_secs(30)),).into_layer(
        IoToProxyBridgeIoLayer::extension_connector_target()
            .into_layer(IoForwardService::new(executor)),
    );

    let tunnel_result = tunnel_svc.serve(prefixed).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match tunnel_result {
        Ok(()) => {
            if let Some(ref ch) = event_channel {
                ch.send(ProxyEvent::Response {
                    id: request_id.clone(),
                    status: 200,
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    duration_ms,
                    headers: HashMap::new(),
                    content_type: None,
                    content_length: None,
                })
                .ok();
            }
        }
        Err(err) => {
            log::warn!("tunnel forwarding failed: {:?}", err);
            if let Some(ref ch) = event_channel {
                ch.send(ProxyEvent::Error {
                    id: request_id.clone(),
                    error: format!("{err:?}"),
                })
                .ok();
            }
        }
    }
}

/// 错误链中是否为超时（如 HTTP header read timeout）。
/// 用于区分空闲连接回收（无害）与真正的 TLS/HTTP 失败。
fn is_timeout_err(err: &rama::error::BoxError) -> bool {
    let mut cur: Option<&(dyn std::error::Error + 'static)> = err.source();
    while let Some(e) = cur {
        if let Some(http_err) = e.downcast_ref::<rama::http::core::Error>() {
            if http_err.is_timeout() {
                return true;
            }
        }
        cur = e.source();
    }
    false
}

pub(crate) fn new_http_mitm_proxy()
-> impl Service<rama::http::Request, Output = rama::http::Response, Error = Infallible> + Clone {
    Arc::new(
        (
            MapResponseBodyLayer::new_boxed_streaming_body(),
            TraceLayer::new_for_http(),
            ConsumeErrLayer::default(),
            RemoveResponseHeaderLayer::hop_by_hop(),
            RemoveRequestHeaderLayer::hop_by_hop(),
        )
            .into_layer(service_fn(http_mitm_proxy)),
    )
}
