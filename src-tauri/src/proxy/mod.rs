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
use tauri::AppHandle;
use tokio::sync::oneshot;

use crate::config::ProxyConfig;

use mitm::{http_connect_proxy, new_http_mitm_proxy};
use state::State;

pub(crate) mod cert;
pub(crate) mod client;
pub(crate) mod mitm;
pub(crate) mod parser;
pub(crate) mod state;

pub struct ProxyServer {
    config: ProxyConfig,
    app_handle: AppHandle,
    shutdown_rx: oneshot::Receiver<()>,
    data_dir: std::path::PathBuf,
    scripts: Vec<String>,
}

impl ProxyServer {
    pub fn new(
        config: ProxyConfig,
        app_handle: AppHandle,
        shutdown_rx: oneshot::Receiver<()>,
        data_dir: std::path::PathBuf,
        scripts: Vec<String>,
    ) -> Self {
        Self { config, app_handle, shutdown_rx, data_dir, scripts }
    }

    pub async fn run(self) -> Result<(), BoxError> {
        let provider =
            cert::MitmCertProvider::try_new(&self.data_dir).context("MITM cert provider")?;
        let mitm_tls_service_data = provider.into_tls_acceptor_data();

        let scripts = self.scripts.clone();
        let listen_addr = format!("{}:{}", &self.config.listen_host, &self.config.listen_port);

        let app_handle = self.app_handle.clone();
        let shutdown_rx = self.shutdown_rx;

        let graceful = Shutdown::new(async move {
            shutdown_rx.await.ok();
            log::info!("Shutdown signal received");
        });

        // Build TcpListener with a graceful executor so serve() responds to shutdown
        let tcp_exec = Executor::graceful(graceful.guard());
        let tcp_service = TcpListener::build(tcp_exec)
            .bind_address(&listen_addr)
            .await
            .context(format!("bind tcp proxy to {listen_addr} failed"))?;

        log::info!("MITM Proxy server listening on http://{}", listen_addr);

        graceful.spawn_task_fn({
            move |guard| async move {
                let exec = Executor::graceful(guard.clone());
                let state = State {
                    mitm_tls_service_data,
                    exec: exec.clone(),
                    app_handle,
                    upstream_proxy: self.config.upstream_proxy,
                    scripts,
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