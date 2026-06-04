use tauri::Manager;

use crate::AppState;
use crate::config::Settings;

#[tauri::command]
pub fn get_theme(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let data_dir = state.store().data_dir();
    let settings = Settings::load_from_path(&data_dir).map_err(|e| e.to_string())?;
    Ok(settings.ui.theme)
}

#[tauri::command]
pub fn set_theme(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    theme: String,
) -> Result<String, String> {
    if !["light", "dark", "system"].contains(&theme.as_str()) {
        return Err(format!(
            "Invalid theme: {theme}. Must be light, dark, or system."
        ));
    }

    let data_dir = state.store().data_dir();
    let mut settings = Settings::load_from_path(&data_dir).map_err(|e| e.to_string())?;
    settings.ui.theme = theme.clone();
    settings.save_to_path(&data_dir).map_err(|e| e.to_string())?;

    state.set_settings(settings);

    let tauri_theme = match theme.as_str() {
        "dark" => Some(tauri::Theme::Dark),
        "light" => Some(tauri::Theme::Light),
        _ => None,
    };
    if let Some(window) = app_handle.get_webview_window("main") {
        window.set_theme(tauri_theme).map_err(|e| e.to_string())?;
    }

    Ok(theme)
}
