use crate::proxy::ai::session::SessionStore;
use crate::proxy::events::ProxyEvent;
use crate::utils::domain_match::domain_match;
use rama::extensions::Extension;
use rama::http::Method;
use rama::net::uri::Uri;
use rama::tls::server::TlsServerConfig;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;
use tauri::ipc::Channel;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::config::db::Db;
use crate::config::{Settings, Store};

#[derive(Clone, Extension)]
pub(crate) struct State {
    mitm_tls_service_data: TlsServerConfig,
    read_settings: Arc<RwLock<Settings>>,
    event_channel: Arc<RwLock<Option<Channel<ProxyEvent>>>>,
    /// 跨请求 AI 会话表（内存，不持久化）。
    sessions: Arc<Mutex<SessionStore>>,
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("mitm_tls_service_data", &self.mitm_tls_service_data)
            .field("settings", &self.read_settings)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, Extension)]
pub(crate) struct ViaConnectTunnel;

impl State {
    pub(crate) fn new(
        mitm_tls_service_data: TlsServerConfig,
        read_settings: Arc<RwLock<Settings>>,
        event_channel: Arc<RwLock<Option<Channel<ProxyEvent>>>>,
    ) -> Self {
        let max_sessions = read_settings
            .read()
            .map(|s| s.ai.session.max_sessions)
            .unwrap_or(500);
        Self {
            mitm_tls_service_data,
            read_settings,
            event_channel,
            sessions: Arc::new(Mutex::new(SessionStore::new(max_sessions))),
        }
    }

    /// 访问共享的 AI 会话表。
    pub(crate) fn sessions(&self) -> Arc<Mutex<SessionStore>> {
        self.sessions.clone()
    }

    pub(crate) fn mitm_tls_service_data(&self) -> &TlsServerConfig {
        &self.mitm_tls_service_data
    }

    /// 按域名匹配加载已启用脚本的内容
    pub(crate) fn get_scripts(&self, host: &str) -> Vec<String> {
        let settings = self.settings();
        let config = &settings.script;
        let Some(ref dir) = config.scripts_dir else {
            return Vec::new();
        };
        config
            .scripts
            .iter()
            .filter(|item| item.enabled && !item.file_name.is_empty() && domain_match(&item.domain, host))
            .filter_map(|item| {
                let path = dir.join(&item.file_name);
                match std::fs::read_to_string(&path) {
                    Ok(content) if !content.trim().is_empty() => Some(content),
                    Ok(_) => {
                        log::warn!("[script] Skipped empty script {}", path.display());
                        None
                    }
                    Err(e) => {
                        log::error!("[script] Failed to read {}: {e}", path.display());
                        None
                    }
                }
            })
            .collect()
    }

    pub(crate) fn settings(&self) -> std::sync::RwLockReadGuard<'_, Settings> {
        self.read_settings.read().expect("settings lock")
    }

    pub(crate) fn event_channel(&self) -> Option<Channel<ProxyEvent>> {
        self.event_channel
            .read()
            .expect("event_channel lock")
            .clone()
    }
}

pub(crate) struct AppState {
    settings: Arc<RwLock<Settings>>,
    running: Arc<AtomicBool>,
    store: Store,
    shutdown_signal: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    proxy_event_channel: Arc<RwLock<Option<Channel<ProxyEvent>>>>,
    pending_open_settings: Arc<AtomicBool>,
}

impl AppState {
    pub(crate) fn new(store: Store, settings: Settings) -> Self {
        Self {
            settings: Arc::new(RwLock::new(settings)),
            running: Arc::new(AtomicBool::new(false)),
            store,
            shutdown_signal: Arc::new(Mutex::new(None)),
            proxy_event_channel: Arc::new(RwLock::new(None)),
            pending_open_settings: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn event_channel(&self) -> Option<Channel<ProxyEvent>> {
        self.proxy_event_channel.read().expect("lock").clone()
    }

    pub(crate) fn event_channel_arc(&self) -> Arc<RwLock<Option<Channel<ProxyEvent>>>> {
        self.proxy_event_channel.clone()
    }

    pub(crate) fn settings_arc(&self) -> Arc<RwLock<Settings>> {
        self.settings.clone()
    }

    pub(crate) fn set_event_channel(&self, channel: Channel<ProxyEvent>) {
        *self.proxy_event_channel.write().expect("lock") = Some(channel);
    }

    pub(crate) fn store(&self) -> &Store {
        &self.store
    }

    pub(crate) fn running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
    pub(crate) fn set_running(&self, v: bool) {
        self.running.store(v, Ordering::SeqCst);
    }

    pub(crate) fn set_shutdown_signal(&self, tx: oneshot::Sender<()>) {
        *self.shutdown_signal.lock().expect("lock") = Some(tx);
    }

    pub(crate) fn take_shutdown_signal(&self) -> Option<oneshot::Sender<()>> {
        self.shutdown_signal.lock().expect("lock").take()
    }

    pub(crate) fn set_pending_open_settings(&self, v: bool) {
        self.pending_open_settings.store(v, Ordering::SeqCst);
    }

    pub(crate) fn take_pending_open_settings(&self) -> bool {
        self.pending_open_settings.swap(false, Ordering::SeqCst)
    }

    pub(crate) fn set_settings(&self, settings: Settings) {
        *self.settings.write().expect("lock") = settings;
    }

    pub(crate) fn db(&self) -> Arc<Db> {
        self.store().db()
    }

    pub(crate) fn settings(&self) -> Settings {
        self.settings.read().expect("lock").clone()
    }
}

/// Shared context for a single proxy request.
pub(crate) struct ProxyCtx {
    request_id: String,
    method: Method,
    uri: Uri,
    start_time: Instant,
    sender: Option<Channel<ProxyEvent>>,
    settings: crate::config::Settings,
    /// AI 会话表（proxy 流量有；resend 等场景为 None）。
    sessions: Option<Arc<Mutex<SessionStore>>>,
    /// 请求侧归一化判定出的 (provider, session_id, 请求 turns)，供响应侧消费。
    ai_req: Arc<Mutex<Option<(crate::proxy::ai::Provider, String, Vec<crate::proxy::ai::AiTurn>)>>>,
}

impl ProxyCtx {
    pub(crate) fn new(
        method: Method,
        uri: Uri,
        sender: Option<Channel<ProxyEvent>>,
        settings: crate::config::Settings,
    ) -> Self {
        Self {
            request_id: Uuid::new_v4().to_string(),
            method,
            uri,
            start_time: Instant::now(),
            sender,
            settings,
            sessions: None,
            ai_req: Arc::new(Mutex::new(None)),
        }
    }

    /// 附加 AI 会话表（proxy 转发路径调用）。
    pub(crate) fn with_sessions(mut self, sessions: Arc<Mutex<SessionStore>>) -> Self {
        self.sessions = Some(sessions);
        self
    }

    pub(crate) fn sessions(&self) -> Option<&Arc<Mutex<SessionStore>>> {
        self.sessions.as_ref()
    }

    /// 请求侧登记归一化判定结果。
    pub(crate) fn set_ai_req(
        &self,
        provider: crate::proxy::ai::Provider,
        session_id: String,
        request_turns: Vec<crate::proxy::ai::AiTurn>,
    ) {
        *self.ai_req.lock().expect("ai_req lock") = Some((provider, session_id, request_turns));
    }

    /// 响应侧读取请求侧判定结果。
    pub(crate) fn ai_req(
        &self,
    ) -> Option<(crate::proxy::ai::Provider, String, Vec<crate::proxy::ai::AiTurn>)> {
        self.ai_req.lock().expect("ai_req lock").clone()
    }

    pub(crate) fn request_id(&self) -> &str {
        &self.request_id
    }

    pub(crate) fn method(&self) -> &Method {
        &self.method
    }

    pub(crate) fn uri(&self) -> &Uri {
        &self.uri
    }

    pub(crate) fn sender(&self) -> &Option<Channel<ProxyEvent>> {
        &self.sender
    }

    pub(crate) fn settings(&self) -> &crate::config::Settings {
        &self.settings
    }

    pub(crate) fn duration_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }

    pub(crate) fn send(&self, event: ProxyEvent) {
        if let Some(ref ch) = self.sender {
            ch.send(event).ok();
        }
    }
}
