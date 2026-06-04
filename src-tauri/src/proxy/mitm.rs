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
use rama::layer::AddInputExtensionLayer;
use rama::layer::ConsumeErrLayer;
use rama::service::service_fn;
use rama::tls::rustls::server::TlsAcceptorLayer;

use super::client::http_mitm_proxy;
use super::state::State;
use super::state::ViaConnectTunnel;

pub(crate) async fn http_connect_proxy(upgraded: Upgraded) -> Result<(), Infallible> {
    let state = upgraded.extensions().get_ref::<State>().unwrap();
    let executor = state.exec().clone();
    let http_mitm_service =
        AddInputExtensionLayer::new(ViaConnectTunnel).into_layer(new_http_mitm_proxy());
    let http_transport_service = HttpServer::auto(executor).service(http_mitm_service);

    let https_service = TlsAcceptorLayer::new(state.mitm_tls_service_data().clone())
        .with_store_client_hello(true)
        .into_layer(http_transport_service);

    if let Err(err) = https_service.serve(upgraded).await {
        log::warn!("MITM TLS connection failed: {:?}", err);
    }
    Ok(())
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

