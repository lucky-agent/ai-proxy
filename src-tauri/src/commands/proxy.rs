use crate::AppState;
use crate::config::Settings;
use crate::proxy::ProxyServer;
use tokio::sync::oneshot;

#[tauri::command]
pub async fn start_proxy(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let data_dir = state.store.data_dir();
    let settings = Settings::load_from_path(&data_dir).map_err(|e| e.to_string())?;

    {
        let mut running = state.running.lock().unwrap();
        if *running {
            return Err("Proxy is already running".to_string());
        }
        *running = true;
    }

    {
        let mut s = state.settings.lock().unwrap();
        *s = Some(settings.clone());
    }

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    {
        let mut signal = state.shutdown_signal.lock().unwrap();
        *signal = Some(shutdown_tx);
    }

    let listen_addr = format!("{}:{}", settings.proxy.listen_host, settings.proxy.listen_port);
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
    let mut running = state.running.lock().unwrap();
    if !*running {
        return Err("Proxy is not running".to_string());
    }
    *running = false;

    let signal = state.shutdown_signal.lock().unwrap().take();
    if let Some(tx) = signal {
        tx.send(()).ok();
    }

    log::info!("Proxy stopped");
    Ok("Proxy stopped".to_string())
}

#[tauri::command]
pub fn get_status(state: tauri::State<'_, AppState>) -> String {
    let running = state.running.lock().unwrap();
    if *running {
        let s = state.settings.lock().unwrap();
        match &*s {
            Some(settings) => format!(
                "Running on {}:{}",
                settings.proxy.listen_host, settings.proxy.listen_port
            ),
            None => "Running (config unavailable)".to_string(),
        }
    } else {
        "Stopped".to_string()
    }
}