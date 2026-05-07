use std::convert::Infallible;
use std::sync::Arc;

use rama::Layer;
use rama::Service;
use rama::error::ErrorContext;
use rama::extensions::ExtensionsRef;
use rama::http::layer::upgrade::Upgraded;
use rama::http::layer::{
    map_response_body::MapResponseBodyLayer,
    remove_header::{RemoveRequestHeaderLayer, RemoveResponseHeaderLayer},
    trace::TraceLayer,
};
use rama::http::server::HttpServer;
use rama::layer::ConsumeErrLayer;
use rama::net::tls::server::SelfSignedData;
use rama::service::service_fn;
use rama::tls::rustls::server::{TlsAcceptorData, TlsAcceptorDataBuilder, TlsAcceptorLayer};

use super::client::http_mitm_proxy;
use super::state::State;

pub(crate) async fn http_connect_proxy(upgraded: Upgraded) -> Result<(), Infallible> {
    let http_service = new_http_mitm_proxy();

    let state = upgraded.extensions().get_ref::<State>().unwrap();
    let executor = state.exec.clone();
    let http_transport_service = HttpServer::auto(executor).service(http_service);

    let https_service = TlsAcceptorLayer::new(state.mitm_tls_service_data.clone())
        .with_store_client_hello(true)
        .into_layer(http_transport_service);

    if let Err(err) = https_service.serve(upgraded).await {
        tracing::warn!("MITM TLS connection failed: {:?}", err);
    }
    Ok(())
}

pub(crate) fn new_http_mitm_proxy() -> impl Service<rama::http::Request, Output = rama::http::Response, Error = Infallible> + Clone {
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

pub(crate) fn try_new_mitm_tls_service_data() -> Result<TlsAcceptorData, rama::error::BoxError> {
    let data = TlsAcceptorDataBuilder::try_new_self_signed(SelfSignedData {
        organisation_name: Some("AI Proxy MITM".to_owned()),
        ..Default::default()
    })
    .context("self signed builder")?
    .with_alpn_protocols_http_auto()
    .try_with_env_key_logger()
    .context("with env key logger")?
    .build();

    Ok(data)
}