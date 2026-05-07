use std::time::Duration;

use rama::Layer;
use rama::error::{BoxError, ErrorContext};
use rama::http::layer::upgrade::{DefaultHttpProxyConnectReplyService, UpgradeLayer};
use rama::http::layer::trace::TraceLayer;
use rama::http::matcher::MethodMatcher;
use rama::http::server::HttpServer;
use rama::layer::{AddInputExtensionLayer, ConsumeErrLayer};
use rama::net::stream::layer::http::BodyLimitLayer;
use rama::service::service_fn;
use rama::tcp::server::TcpListener;
use rama::{graceful::Shutdown, rt::Executor};

use crate::config::Settings;

use mitm::{http_connect_proxy, new_http_mitm_proxy, try_new_mitm_tls_service_data};
use state::State;

pub(crate) mod client;
pub(crate) mod mitm;
pub(crate) mod parser;
pub(crate) mod state;

pub struct ProxyServer {
    settings: Settings,
}

impl ProxyServer {
    pub fn new(settings: Settings) -> Self {
        Self { settings }
    }

    pub async fn run(&self) -> Result<(), BoxError> {
        let mitm_tls_service_data =
            try_new_mitm_tls_service_data().context("generate self-signed mitm tls cert")?;

        let listen_addr = format!("{}:{}", &self.settings.listen_host, self.settings.listen_port);

        let tcp_service = TcpListener::build(Executor::default())
            .bind_address(&listen_addr)
            .await
            .context(format!("bind tcp proxy to {listen_addr} failed"))?;

        tracing::info!("MITM Proxy server listening on http://{}", listen_addr);

        let graceful = Shutdown::default();
        graceful.spawn_task_fn({
            move |guard| async move {
                let exec = Executor::graceful(guard.clone());
                let state = State {
                    mitm_tls_service_data,
                    exec: exec.clone(),
                };

                let http_mitm_service = new_http_mitm_proxy();
                let http_service = HttpServer::auto(exec.clone()).service(std::sync::Arc::new(
                    (
                        TraceLayer::new_for_http(),
                        ConsumeErrLayer::default(),
                        UpgradeLayer::new(
                            exec,
                            MethodMatcher::CONNECT,
                            DefaultHttpProxyConnectReplyService::new(),
                            service_fn(http_connect_proxy),
                        ),
                    )
                        .into_layer(http_mitm_service),
                ));

                tcp_service
                    .serve(
                        (
                            AddInputExtensionLayer::new(state),
                            BodyLimitLayer::symmetric(2 * 1024 * 1024),
                        )
                            .into_layer(http_service),
                    )
                    .await;
            }
        });

        graceful
            .shutdown_with_limit(Duration::from_secs(30))
            .await
            .context("graceful shutdown")?;

        Ok(())
    }
}