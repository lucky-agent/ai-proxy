use std::convert::Infallible;

use rama::Layer;
use rama::Service;
use rama::extensions::ExtensionsRef;
use rama::http::client::EasyHttpWebClient;
use rama::http::layer::decompression::DecompressionLayer;
use rama::http::layer::map_response_body::MapResponseBodyLayer;
use rama::http::{Request, Response, Version};
use rama::tls::rustls::client::TlsConnectorDataBuilder;

use super::parser::{error_response, log_request, log_response};
use super::state::State;

pub(crate) async fn http_mitm_proxy(req: Request) -> Result<Response, Infallible> {
    let (req, method, uri) = log_request(req);

    let state = req.extensions().get_ref::<State>().unwrap();
    let executor = state.exec.clone();

    let tls_config = TlsConnectorDataBuilder::new()
        .with_alpn_protocols_http_auto()
        .try_with_env_key_logger()
        .expect("with env key logger")
        .with_no_cert_verifier()
        .build();

    let client = (
        MapResponseBodyLayer::new_boxed_streaming_body(),
        DecompressionLayer::new().with_insert_accept_encoding_header(false),
    )
        .into_layer(
            EasyHttpWebClient::connector_builder()
                .with_default_transport_connector()
                .with_tls_proxy_support_using_rustls()
                .with_proxy_support()
                .with_tls_support_using_rustls_and_default_http_version(
                    Some(tls_config),
                    Version::HTTP_11,
                )
                .with_default_http_connector(executor)
                .build_client(),
        );

    match client.serve(req).await {
        Ok(resp) => Ok(log_response(resp, method, uri)),
        Err(err) => {
            tracing::error!("error proxying request [{} {}]: {err:?}", method, uri);
            Ok(error_response())
        }
    }
}