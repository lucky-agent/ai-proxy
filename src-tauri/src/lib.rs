mod commands;
mod config;
mod proxy;
mod script;
mod tray;
pub mod utils;
use proxy::state::AppState;
use tauri::{Emitter, Manager, RunEvent};

use crate::commands::resend_request;
use crate::commands::load_traffic_history;
use crate::commands::{
    get_locale, get_settings, get_status, get_theme, save_settings, set_locale, set_theme,
    start_proxy, stop_proxy, subscribe_proxy_events, sync_tray_locale,
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
        state.set_pending_open_settings(true);
    }

    show_main_window(app);

    if let Some(window) = app.get_webview_window("main") {
        if window.is_focused().unwrap_or(false) {
            if let Some(state) = app.try_state::<AppState>() {
                state.set_pending_open_settings(false);
            }
            let _ = window.emit("open-settings", ());
        }
    }
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
            .is_some_and(|state| state.take_pending_open_settings());
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
    let db = if settings.persistence.unwrap_or(false) {
        crate::config::db::Db::open(&store.db_path()).expect("Failed to open database")
    } else {
        crate::config::db::Db::noop()
    };

    let app = tauri::Builder::default()
        .plugin(Store::build_log_plugin(&settings.log).build())
        .manage(AppState::new(store, settings, db))
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
            subscribe_proxy_events,
            sync_tray_locale,
            load_traffic_history,
            resend_request,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    log::info!("Configuration loaded, proxy ready");

    app.run(|app_handle, event| {
        if let RunEvent::ExitRequested { .. } = &event {
            if let Some(state) = app_handle.try_state::<AppState>() {
                if state.running() {
                    state.set_running(false);
                    log::info!("Proxy stopped (app exiting)");
                }
                if let Some(tx) = state.take_shutdown_signal() {
                    tx.send(()).ok();
                }
            }
        }
    });
}
