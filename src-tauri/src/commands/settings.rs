use crate::AppState;
use crate::config::{ProxyConfig, Settings};

#[tauri::command]
pub fn get_settings(state: tauri::State<'_, AppState>) -> Result<Settings, String> {
    let data_dir = state.store().data_dir();
    Settings::load_from_path(&data_dir).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_settings(
    state: tauri::State<'_, AppState>,
    proxy: ProxyConfig,
) -> Result<(), String> {
    let data_dir = state.store().data_dir();
    let mut settings = Settings::load_from_path(&data_dir).map_err(|e| e.to_string())?;
    settings.proxy = proxy;
    settings.save_to_path(&data_dir).map_err(|e| e.to_string())?;

    state.set_settings(settings);

    Ok(())
}
