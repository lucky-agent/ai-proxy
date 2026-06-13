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
use crate::config::db::Db;
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
    let (parts, body) = req.into_parts();
    let body_str = script::collect_body_str(body).await;

    let (method, uri, request_id) = parser::log_request(&parts, &body_str, &event_channel);

    // persist request to DB (async, errors won't affect normal flow)
    {
        let app_state = state.app_handle().state::<AppState>();
        if let Ok(db) = app_state.db().lock() {
            db.upsert_request_async(
                &request_id,
                &method.to_string(),
                &uri.to_string(),
                chrono::Utc::now().timestamp_millis(),
                &Db::headers_to_json(&parts.headers),
                &Db::query_to_json(&uri),
                if body_str.is_empty() {
                    None
                } else {
                    Some(&body_str)
                },
                false,
            );
        }
    }
    let req = if has_scripts {
        let req_data = script::RequestData::from_rama_parts(&parts, &body_str);

        match script::run_request_hooks(state.scripts(), &req_data) {
            Some(modified) => modified.apply(parts),
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
                    .expect("valid status code and body for blocked response"));
            }
        }
    } else {
        Request::from_parts(parts, Body::from(body_str))
    };

    // ---- direct request detection ----
    // Non-tunnel request → direct to proxy, return directly
    if !from_connect_tunnel {
        return Ok(parser::direct_response(
            method,
            uri,
            &request_id,
            &event_channel,
        ));
    }
    // ---- forward to upstream ----
    let client = build_upstream_service(state.exec().clone(), state.upstream_proxy(), true);
    let start_time = Instant::now();

    match client.serve(req).await {
        Ok(resp) if has_scripts => {
            let duration_ms = start_time.elapsed().as_millis() as u64;
            let (parts, body) = resp.into_parts();
            let body_str = script::collect_body_str(body).await;
            let resp_data = script::ResponseData::from_rama_parts(&parts, &body_str);
            let modified = script::run_response_hooks(state.scripts(), &resp_data);
            let resp = modified.apply(parts);
            let db_path_for_body = {
                let app_state = state.app_handle().state::<AppState>();
                app_state.db().lock().ok().and_then(|db| db.async_path())
            };
            let resp =
                parser::log_response(resp, method, uri, &request_id, duration_ms, &event_channel, db_path_for_body);
            {
                let app_state = state.app_handle().state::<AppState>();
                if let Ok(db) = app_state.db().lock() {
                    db.update_response_async(
                        &request_id,
                        resp.status().as_u16(),
                        chrono::Utc::now().timestamp_millis(),
                        duration_ms,
                        &Db::headers_to_json(resp.headers()),
                    );
                }
            }
            Ok(resp)
        }
        Ok(resp) => {
            let duration_ms = start_time.elapsed().as_millis() as u64;
            let db_path_for_body = {
                let app_state = state.app_handle().state::<AppState>();
                app_state.db().lock().ok().and_then(|db| db.async_path())
            };
            let resp =
                parser::log_response(resp, method, uri, &request_id, duration_ms, &event_channel, db_path_for_body);
            {
                let app_state = state.app_handle().state::<AppState>();
                if let Ok(db) = app_state.db().lock() {
                    db.update_response_async(
                        &request_id,
                        resp.status().as_u16(),
                        chrono::Utc::now().timestamp_millis(),
                        duration_ms,
                        &Db::headers_to_json(resp.headers()),
                    );
                }
            }
            Ok(resp)
        }
        Err(err) => {
            log::error!("error proxying request [{} {}]: {err:?}", method, uri);
            if let Some(ref ch) = event_channel {
                ch.send(ProxyEvent::Error {
                    id: request_id.clone(),
                    error: format!("{err:?}"),
                })
                .ok();
            }
            {
                let app_state = state.app_handle().state::<AppState>();
                if let Ok(db) = app_state.db().lock() {
                    db.set_error_async(&request_id, &format!("{err:?}"));
                }
            }
            Ok(parser::error_response())
        }
    }
}
pub(crate) fn build_upstream_service(
    executor: rama::rt::Executor,
    upstream_proxy: bool,
    skip_tls_verify: bool,
) -> BoxService<Request, Response, rama::error::BoxError> {
    let tls_builder = TlsConnectorDataBuilder::new()
        .with_alpn_protocols_http_auto()
        .try_with_env_key_logger()
        .expect("with env key logger");

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
