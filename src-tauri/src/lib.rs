mod commands;
mod config;
mod proxy;
mod script;
mod tray;
pub mod utils;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{Emitter, Manager, RunEvent};
use tokio::sync::oneshot;

use crate::commands::{
    get_locale, get_settings, get_status, get_theme, save_settings, set_locale, set_theme,
    start_proxy, stop_proxy, sync_tray_locale,
};
use crate::config::{Settings, Store, UiConfig};

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub(crate) fn open_settings_from_tray(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        state.pending_open_settings.store(true, Ordering::SeqCst);
    }

    show_main_window(app);

    if let Some(window) = app.get_webview_window("main") {
        if window.is_focused().unwrap_or(false) {
            if let Some(state) = app.try_state::<AppState>() {
                state.pending_open_settings.store(false, Ordering::SeqCst);
            }
            let _ = window.emit("open-settings", ());
        }
    }
}

pub(crate) struct AppState {
    settings: Arc<Mutex<Option<Settings>>>,
    running: Arc<Mutex<bool>>,
    store: Store,
    shutdown_signal: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    pending_open_settings: Arc<AtomicBool>,
}

fn app_setup(app: &mut tauri::App, ui: &UiConfig) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(window) = app.get_webview_window("main") {
        let tauri_theme = match ui.theme.as_str() {
            "dark" => Some(tauri::Theme::Dark),
            "light" => Some(tauri::Theme::Light),
            _ => None,
        };
        window.set_theme(tauri_theme).ok();
    }
    tray::setup_tray(app, ui.tray_locale())?;
    Ok(())
}

fn handle_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if let tauri::WindowEvent::Focused(true) = event {
        let app = window.app_handle();
        let should_open = app
            .try_state::<AppState>()
            .is_some_and(|state| state.pending_open_settings.swap(false, Ordering::SeqCst));
        if should_open {
            let _ = window.emit("open-settings", ());
        }
    }

    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        log::info!("Window close requested, exiting app");
        window.app_handle().exit(0);
    }
}

pub fn run() {
    log::info!("Starting AI Proxy");
    let store = Store::new();
    let settings =
        Settings::load_from_path(&store.data_dir()).expect("Failed to load configuration");
    let ui = settings.ui.clone();

    let app = tauri::Builder::default()
        .plugin(Store::build_log_plugin(&settings.log).build())
        .manage(AppState {
            settings: Arc::new(Mutex::new(None)),
            running: Arc::new(Mutex::new(false)),
            store,
            shutdown_signal: Arc::new(Mutex::new(None)),
            pending_open_settings: Arc::new(AtomicBool::new(false)),
        })
        .setup(move |app| app_setup(app, &ui))
        .on_window_event(handle_window_event)
        .invoke_handler(tauri::generate_handler![
            start_proxy,
            stop_proxy,
            get_status,
            get_theme,
            set_theme,
            get_settings,
            save_settings,
            get_locale,
            set_locale,
            sync_tray_locale,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    log::info!("Configuration loaded, proxy ready");

    app.run(|app_handle, event| {
        if let RunEvent::ExitRequested { .. } = &event {
            if let Some(state) = app_handle.try_state::<AppState>() {
                let mut r = state.running.lock().unwrap();
                if *r {
                    *r = false;
                    log::info!("Proxy stopped (app exiting)");
                }
                let signal = state.shutdown_signal.lock().unwrap().take();
                if let Some(tx) = signal {
                    tx.send(()).ok();
                }
            }
        }
    });
}
