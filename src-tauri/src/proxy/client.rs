use std::convert::Infallible;
use std::time::Instant;

use rama::Layer;
use rama::Service;
use rama::extensions::ExtensionsRef;
use rama::http::client::EasyHttpWebClient;
use rama::http::layer::decompression::DecompressionLayer;
use rama::http::layer::map_response_body::MapResponseBodyLayer;
use rama::http::{Request, Response, Version};
use rama::service::BoxService;
use rama::tls::rustls::client::TlsConnectorDataBuilder;
use serde_json::json;
use tauri::Emitter;

use super::parser::{error_response, log_request, log_response};
use super::state::State;

pub(crate) async fn http_mitm_proxy(req: Request) -> Result<Response, Infallible> {
    let app_handle = req
        .extensions()
        .get_ref::<State>()
        .unwrap()
        .app_handle
        .clone();
    let (req, method, uri, request_id) = log_request(req, &app_handle);

    let state = req.extensions().get_ref::<State>().unwrap();
    let executor = state.exec.clone();
    let client = build_upstream_service(executor, state.upstream_proxy);

    let start_time = Instant::now();

    match client.serve(req).await {
        Ok(resp) => {
            let duration_ms = start_time.elapsed().as_millis() as u64;
            Ok(log_response(
                resp,
                method,
                uri,
                &request_id,
                duration_ms,
                &app_handle,
            ))
        }
        Err(err) => {
            log::error!("error proxying request [{} {}]: {err:?}", method, uri);
            let _ = app_handle.emit(
                "proxy:error",
                json!({
                    "id": request_id,
                    "error": format!("{err:?}"),
                }),
            );
            Ok(error_response())
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
