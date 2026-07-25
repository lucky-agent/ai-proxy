use crate::AppState;
use crate::bail;
use crate::proxy::ProxyServer;
use crate::proxy::events::ProxyEvent;
use tauri::ipc::Channel;
use tokio::sync::oneshot;

#[tauri::command]
pub async fn start_proxy(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let data_dir = state.store().data_dir().clone();
    let settings = state.settings();

    if state.running() {
        bail!("Proxy is already running");
    }
    state.set_running(true);

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    state.set_shutdown_signal(shutdown_tx);

    let listen_addr = format!(
        "{}:{}",
        settings.proxy.listen_host, settings.proxy.listen_port
    );
    log::info!("Proxy started on {}", listen_addr);
    let server = ProxyServer::new(settings.proxy, app_handle, shutdown_rx, data_dir);

    tauri::async_runtime::spawn(async move {
        if let Err(err) = server.run().await {
            log::error!("Proxy server error: {:?}", err);
        }
    });

    Ok(format!("Proxy started on {}", listen_addr))
}

#[tauri::command]
pub fn stop_proxy(state: tauri::State<'_, AppState>) -> Result<String, String> {
    if !state.running() {
        return Err("Proxy is not running".to_string());
    }
    state.set_running(false);

    if let Some(tx) = state.take_shutdown_signal() {
        tx.send(()).ok();
    }

    log::info!("Proxy stopped");
    Ok("Proxy stopped".to_string())
}

#[tauri::command]
pub fn get_status(state: tauri::State<'_, AppState>) -> String {
    if state.running() {
        let s = state.settings();
        format!("Running on {}:{}", s.proxy.listen_host, s.proxy.listen_port)
    } else {
        "Stopped".to_string()
    }
}

#[tauri::command]
pub fn subscribe_proxy_events(state: tauri::State<'_, AppState>, channel: Channel<ProxyEvent>) {
    state.set_event_channel(channel);
}
