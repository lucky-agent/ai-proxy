use crate::proxy::events::ProxyEvent;
use rama::extensions::Extension;
use rama::rt::Executor;
use rama::tls::rustls::server::TlsAcceptorData;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use tauri::ipc::Channel;
use tokio::sync::oneshot;

use crate::config::{Settings, Store};
use crate::config::db::Db;

/// MITM 解密白名单：仅列表中的域名会走 MITM 解密，其余一律隧道透传。
#[derive(Debug, Clone, Default)]
pub(crate) struct MitmWhitelist {
    pub(crate) hosts: HashSet<String>,
}

impl MitmWhitelist {
    /// 判断 host 是否命中白名单（命中则走 MITM 解密）。
    pub(crate) fn contains_host(&self, host: &str) -> bool {
        let host = host.to_lowercase();
        self.hosts.iter().any(|p| {
            let p = p.to_lowercase();
            if p == "*" {
                return true;
            }
            if let Some(suffix) = p.strip_prefix("*.") {
                return host.ends_with(suffix) && host != suffix;
            }
            p == host
        })
    }
}

#[derive(Debug, Clone, Extension)]
pub(crate) struct State {
    mitm_tls_service_data: TlsAcceptorData,
    exec: Executor,
    app_handle: AppHandle,
    upstream_proxy: bool,
    scripts: Vec<String>,
    pub(crate) mitm_whitelist: Arc<tokio::sync::RwLock<MitmWhitelist>>,
    pub(crate) whitelist_path: std::path::PathBuf,
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
        mitm_whitelist: MitmWhitelist,
        whitelist_path: std::path::PathBuf,
    ) -> Self {
        Self {
            mitm_tls_service_data,
            exec,
            app_handle,
            upstream_proxy,
            scripts,
            mitm_whitelist: Arc::new(tokio::sync::RwLock::new(mitm_whitelist)),
            whitelist_path,
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
    db: Arc<Mutex<Db>>,
}

impl AppState {
    pub(crate) fn new(store: Store, settings: Settings, db: Db) -> Self {
        Self {
            settings: Arc::new(Mutex::new(settings)),
            running: Arc::new(Mutex::new(false)),
            store,
            shutdown_signal: Arc::new(Mutex::new(None)),
            proxy_event_channel: Arc::new(Mutex::new(None)),
            pending_open_settings: Arc::new(AtomicBool::new(false)),
            db: Arc::new(Mutex::new(db)),
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

    pub(crate) fn set_settings(&self, settings: Settings) {
        *self
            .settings
            .lock()
            .expect("Failed to acquire settings lock") = settings;
    }

    pub(crate) fn db(&self) -> Arc<Mutex<Db>> {
        self.db.clone()
    }

    pub(crate) fn settings(&self) -> Settings {
        self.settings
            .lock()
            .expect("Failed to acquire settings lock")
            .clone()
    }
}

// ── MitmWhitelist 持久化 ──

#[derive(Debug, Serialize, Deserialize)]
struct MitmWhitelistFile {
    hosts: Vec<String>,
}

pub(crate) fn load_mitm_whitelist(path: &std::path::Path) -> MitmWhitelist {
    let Ok(file) = std::fs::File::open(path) else {
        return MitmWhitelist::default();
    };
    let Ok(data) = serde_json::from_reader::<_, MitmWhitelistFile>(file) else {
        return MitmWhitelist::default();
    };
    MitmWhitelist {
        hosts: data.hosts.into_iter().collect(),
    }
}

pub(crate) fn save_mitm_whitelist(whitelist: &MitmWhitelist, path: &std::path::Path) {
    let data = MitmWhitelistFile {
        hosts: whitelist.hosts.iter().cloned().collect(),
    };
    let json = serde_json::to_string_pretty(&data).unwrap_or_default();
    if let Err(err) = std::fs::write(path, json) {
        log::warn!("failed to save MITM whitelist: {:?}", err);
    }
}
