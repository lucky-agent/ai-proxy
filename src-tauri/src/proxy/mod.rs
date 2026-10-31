use std::sync::{Arc, Mutex};
use std::time::Duration;

use rama::Layer;
use rama::error::{BoxError, ErrorContext};
use rama::http::layer::trace::TraceLayer;
use rama::http::layer::upgrade::{LazyHttpProxyConnectReplyService, UpgradeLayer};
use rama::http::matcher::MethodMatcher;
use rama::http::server::HttpServer;
use rama::http::{Body, Response, StatusCode};
use rama::layer::{AddInputExtensionLayer, ConsumeErrLayer};
use rama::service::service_fn;
use rama::tcp::server::TcpListener;
use rama::{graceful::Shutdown, rt::Executor};
use tauri::AppHandle;
use tauri::Manager;
use tokio::sync::oneshot;

use crate::config::{ProxyConfig, ReverseProxyRule};
use crate::proxy::ai::session::SessionStore;
use state::{AppState, State};

use mitm::{http_connect_proxy, new_http_mitm_proxy};
use reverse::new_reverse_proxy;

pub(crate) mod ai;
pub(crate) mod cert;
pub(crate) mod client;
pub(crate) mod ctx;
pub(crate) mod events;
pub(crate) mod ext;
pub(crate) mod layer;
pub(crate) mod mitm;
pub(crate) mod record;
pub(crate) mod reverse;
pub(crate) mod sse;
pub(crate) mod state;
pub(crate) mod throttle;

pub struct ProxyServer {
    config: ProxyConfig,
    app_handle: AppHandle,
    shutdown_rx: oneshot::Receiver<()>,
    data_dir: std::path::PathBuf,
}

impl ProxyServer {
    pub fn new(
        config: ProxyConfig,
        app_handle: AppHandle,
        shutdown_rx: oneshot::Receiver<()>,
        data_dir: std::path::PathBuf,
    ) -> Self {
        Self {
            config,
            app_handle,
            shutdown_rx,
            data_dir,
        }
    }

    pub async fn run(self) -> Result<(), BoxError> {
        let provider =
            cert::MitmCertProvider::try_new(&self.data_dir).context("MITM cert provider")?;
        let mitm_tls_service_data = provider.into_tls_server_config();

        let listen_addr = format!("{}:{}", self.config.listen_host, self.config.listen_port);

        let app_handle = self.app_handle.clone();
        let shutdown_rx = self.shutdown_rx;

        let graceful = Shutdown::new(async move {
            shutdown_rx.await.ok();
            log::info!("Shutdown signal received");
        });

        let exec = Executor::graceful(graceful.guard());
        let tcp_service = TcpListener::build(exec.clone())
            .bind_address(&listen_addr)
            .await
            .context(format!("bind tcp proxy to {listen_addr} failed"))?;

        log::info!("MITM Proxy server listening on http://{}", listen_addr);

        let app_state = app_handle.state::<AppState>();
        let db = app_state.db();
        let settings = app_state.settings_arc();
        let scripts_dir = app_state.store().scripts_dir().clone();
        let event_channel = app_state.event_channel_arc();

        let max_sessions = settings
            .read()
            .map(|s| s.ai.session.max_sessions)
            .unwrap_or(500);
        let sessions: Arc<Mutex<SessionStore>> =
            Arc::new(Mutex::new(SessionStore::load(&db, max_sessions)));

        // 反向代理规则：总开关开启时才绑定；仅绑定已启用的规则，端口占用等错误在启动期暴露。
        let reverse_rules: Vec<ReverseProxyRule> = settings
            .read()
            .map(|s| {
                if !s.reverse.enabled {
                    Vec::new()
                } else {
                    s.reverse
                        .rules
                        .iter()
                        .filter(|r| r.enabled)
                        .cloned()
                        .collect()
                }
            })
            .unwrap_or_default();

        let mut reverse_listeners = Vec::with_capacity(reverse_rules.len());
        for rule in &reverse_rules {
            let addr = format!("{}:{}", rule.listen_host, rule.listen_port);
            let listener = TcpListener::build(exec.clone())
                .bind_address(&addr)
                .await
                .context(format!("bind reverse proxy to {addr} failed"))?;
            log::info!(
                "Reverse proxy listening on http://{} -> {}://{}",
                addr,
                rule.upstream_scheme_normalized(),
                rule.upstream_authority()
            );
            reverse_listeners.push((rule.clone(), listener));
        }

        // 共享 State 构建一次，clone 到每个监听器（DB/事件通道/会话均为 Arc 共享）。
        let state = State::with_sessions(
            mitm_tls_service_data,
            settings,
            scripts_dir,
            event_channel,
            db,
            sessions,
        );

        graceful.spawn_task_fn({
            let state = state.clone();
            let exec = exec.clone();
            move |_guard| async move {
                let http_service = HttpServer::auto(exec.clone()).service(
                    (
                        TraceLayer::new_for_http(),
                        ConsumeErrLayer::default(),
                        UpgradeLayer::new_with_services(
                            exec,
                            MethodMatcher::CONNECT,
                            LazyHttpProxyConnectReplyService::new(),
                            service_fn(http_connect_proxy),
                        ),
                    )
                        .into_layer(new_http_mitm_proxy()),
                );

                tcp_service
                    .serve(AddInputExtensionLayer::new(state).into_layer(http_service))
                    .await;
            }
        });

        for (rule, listener) in reverse_listeners {
            graceful.spawn_task_fn({
                let state = state.clone();
                let exec = exec.clone();
                move |_guard| async move {
                    let http_service =
                        HttpServer::auto(exec.clone()).service(new_reverse_proxy(&rule));
                    listener
                        .serve(AddInputExtensionLayer::new(state).into_layer(http_service))
                        .await;
                }
            });
        }

        graceful
            .shutdown_with_limit(Duration::from_secs(30))
            .await
            .context("graceful shutdown")?;
        Ok(())
    }
}

pub(crate) fn error_response(status: StatusCode, body: impl Into<Body>) -> Response {
    Response::builder()
        .status(status)
        .body(body.into())
        .expect("valid status code and body for error response")
}
