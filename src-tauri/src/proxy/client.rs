use std::convert::Infallible;
use std::time::Instant;

use rama::extensions::ExtensionsRef;
use rama::http::client::EasyHttpWebClient;
use rama::http::layer::decompression::DecompressionLayer;
use rama::http::layer::map_response_body::MapResponseBodyLayer;
use rama::http::{Body, Request, Response, StatusCode, Version};
use rama::layer::Layer;
use rama::service::BoxService;
use rama::service::Service;
use rama::tls::rustls::client::TlsConnectorDataBuilder;
use tauri::Manager;

use super::events::ProxyEvent;
use super::parser;
use super::state::State;
use super::state::ViaConnectTunnel;
use crate::AppState;
use crate::script;

pub(crate) async fn http_mitm_proxy(req: Request) -> Result<Response, Infallible> {
    let state = req
        .extensions()
        .get_ref::<State>()
        .cloned()
        .expect("State not found in request extensions");

    let from_connect_tunnel = req.extensions().get_ref::<ViaConnectTunnel>().is_some();

    let event_channel = {
        let app_state = state.app_handle().state::<AppState>();
        app_state.event_channel()
    };

    let has_scripts = !state.scripts().is_empty();

    // ---- request phase ----
    let (req, method, uri, request_id) = if has_scripts {
        let (parts, body) = req.into_parts();
        let body_str = script::collect_body_str(body).await;
        let req_data = script::RequestData::from_rama_parts(&parts, &body_str);

        match script::run_request_hooks(state.scripts(), &req_data) {
            Some(modified) => {
                let req = modified.apply(parts);
                parser::log_request(req, &event_channel)
            }
            None => {
                log::info!("[script] request blocked");
                if let Some(ref ch) = event_channel {
                    ch.send(ProxyEvent::Error {
                        id: "blocked".into(),
                        error: "Request blocked by script".into(),
                    })
                    .ok();
                }
                return Ok(Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .body(Body::from("Blocked by script"))
                    .unwrap());
            }
        }
    } else {
        parser::log_request(req, &event_channel)
    };

    // ---- direct request detection ----
    // 相对 URI（无 host）说明请求目标是代理自身，直接返回，不转发
    if !from_connect_tunnel {
        log::info!("Direct request to proxy: {}, returning directly", req.uri());
        // Consume body to trigger RequestChunk events
        let (_, body) = req.into_parts();
        let _ = script::collect_body_str(body).await;
        let resp = Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"code":0,"msg":"success"}"#))
            .unwrap();
        return Ok(parser::log_response(
            resp,
            method,
            uri,
            &request_id,
            0,
            &event_channel,
        ));
    }
    // ---- forward to upstream ----
    let client = build_upstream_service(state.exec().clone(), state.upstream_proxy());
    let start_time = Instant::now();

    match client.serve(req).await {
        Ok(resp) => {
            let duration_ms = start_time.elapsed().as_millis() as u64;

            if has_scripts {
                let (parts, body) = resp.into_parts();
                let body_str = script::collect_body_str(body).await;
                let resp_data = script::ResponseData::from_rama_parts(&parts, &body_str);
                let modified = script::run_response_hooks(state.scripts(), &resp_data);
                let resp = modified.apply(parts);
                Ok(parser::log_response(
                    resp,
                    method,
                    uri,
                    &request_id,
                    duration_ms,
                    &event_channel,
                ))
            } else {
                Ok(parser::log_response(
                    resp,
                    method,
                    uri,
                    &request_id,
                    duration_ms,
                    &event_channel,
                ))
            }
        }
        Err(err) => {
            log::error!("error proxying request [{} {}]: {err:?}", method, uri);
            if let Some(ref ch) = event_channel {
                ch.send(ProxyEvent::Error {
                    id: request_id,
                    error: format!("{err:?}"),
                })
                .ok();
            }
            Ok(parser::error_response())
        }
    }
}

fn build_upstream_service(
    executor: rama::rt::Executor,
    upstream_proxy: bool,
) -> BoxService<Request, Response, rama::error::BoxError> {
    let tls_config = TlsConnectorDataBuilder::new()
        .with_alpn_protocols_http_auto()
        .try_with_env_key_logger()
        .expect("with env key logger")
        .with_no_cert_verifier()
        .build();

    let client = if upstream_proxy {
        EasyHttpWebClient::connector_builder()
            .with_default_transport_connector()
            .with_tls_proxy_support_using_rustls()
            .with_proxy_support()
            .with_tls_support_using_rustls_and_default_http_version(
                Some(tls_config),
                Version::HTTP_11,
            )
            .with_default_http_connector(executor)
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
            .with_default_http_connector(executor)
            .build_client()
    };

    (
        MapResponseBodyLayer::new_boxed_streaming_body(),
        DecompressionLayer::new().with_insert_accept_encoding_header(false),
    )
        .into_layer(client)
        .boxed()
}


