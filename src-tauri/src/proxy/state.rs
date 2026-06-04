use crate::proxy::events::ProxyEvent;
use rama::extensions::Extension;
use rama::rt::Executor;
use rama::tls::rustls::server::TlsAcceptorData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use tauri::ipc::Channel;
use tokio::sync::oneshot;

use crate::config::{Settings, Store};

#[derive(Debug, Clone, Extension)]
pub(crate) struct State {
    mitm_tls_service_data: TlsAcceptorData,
    exec: Executor,
    app_handle: AppHandle,
    upstream_proxy: bool,
    scripts: Vec<String>,
}

#[derive(Debug, Clone, Copy, Extension)]
pub(crate) struct ViaConnectTunnel;

impl State {
    pub(crate) fn new(
        mitm_tls_service_data: TlsAcceptorData,
        exec: Executor,
        app_handle: AppHandle,
        upstream_proxy: bool,
        scripts: Vec<String>,
    ) -> Self {
        Self {
            mitm_tls_service_data,
            exec,
            app_handle,
            upstream_proxy,
            scripts,
        }
    }

    pub(crate) fn mitm_tls_service_data(&self) -> &TlsAcceptorData {
        &self.mitm_tls_service_data
    }

    pub(crate) fn exec(&self) -> &Executor {
        &self.exec
    }

    pub(crate) fn app_handle(&self) -> &AppHandle {
        &self.app_handle
    }

    pub(crate) fn upstream_proxy(&self) -> bool {
        self.upstream_proxy
    }

    pub(crate) fn scripts(&self) -> &[String] {
        &self.scripts
    }
}

pub(crate) struct AppState {
    settings: Arc<Mutex<Settings>>,
    running: Arc<Mutex<bool>>,
    store: Store,
    shutdown_signal: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    proxy_event_channel: Arc<Mutex<Option<Channel<ProxyEvent>>>>,
    pending_open_settings: Arc<AtomicBool>,
}

impl AppState {
    pub(crate) fn new(store: Store, settings: Settings) -> Self {
        Self {
            settings: Arc::new(Mutex::new(settings)),
            running: Arc::new(Mutex::new(false)),
            store,
            shutdown_signal: Arc::new(Mutex::new(None)),
            proxy_event_channel: Arc::new(Mutex::new(None)),
            pending_open_settings: Arc::new(AtomicBool::new(false)),
        }
    }
    pub(crate) fn event_channel(&self) -> Option<Channel<ProxyEvent>> {
        self.proxy_event_channel
            .lock()
            .expect("Failed to acquire proxy event channel lock")
            .clone()
    }

    pub(crate) fn set_event_channel(&self, channel: Channel<ProxyEvent>) {
        *self
            .proxy_event_channel
            .lock()
            .expect("Failed to acquire proxy event channel lock") = Some(channel);
    }

    pub(crate) fn store(&self) -> &Store {
        &self.store
    }

    pub(crate) fn running(&self) -> bool {
        *self.running.lock().expect("Failed to acquire running lock")
    }

    pub(crate) fn set_running(&self, value: bool) {
        *self.running.lock().expect("Failed to acquire running lock") = value;
    }

    pub(crate) fn set_shutdown_signal(&self, tx: oneshot::Sender<()>) {
        *self
            .shutdown_signal
            .lock()
            .expect("Failed to acquire shutdown signal lock") = Some(tx);
    }

    pub(crate) fn take_shutdown_signal(&self) -> Option<oneshot::Sender<()>> {
        self.shutdown_signal
            .lock()
            .expect("Failed to acquire shutdown signal lock")
            .take()
    }

    pub(crate) fn set_pending_open_settings(&self, value: bool) {
        self.pending_open_settings.store(value, Ordering::SeqCst);
    }

    pub(crate) fn take_pending_open_settings(&self) -> bool {
        self.pending_open_settings.swap(false, Ordering::SeqCst)
    }

    /// 设置 settings（覆盖写入）
    pub(crate) fn set_settings(&self, settings: Settings) {
        *self
            .settings
            .lock()
            .expect("Failed to acquire settings lock") = settings;
    }

    pub(crate) fn settings(&self) -> Settings {
        self.settings
            .lock()
            .expect("Failed to acquire settings lock")
            .clone()
    }
}
