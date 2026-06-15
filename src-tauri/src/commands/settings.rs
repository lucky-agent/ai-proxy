use crate::AppState;
use crate::config::{ProxyConfig, ScriptConfig, Settings, SslConfig};

#[tauri::command]
pub fn get_settings(state: tauri::State<'_, AppState>) -> Result<Settings, String> {
    Ok(state.settings())
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

#[tauri::command]
pub fn get_script_config(state: tauri::State<'_, AppState>) -> Result<ScriptConfig, String> {
    let settings = state.settings();
    Ok(settings.script)
}

#[tauri::command]
pub fn save_script_config(
    state: tauri::State<'_, AppState>,
    script: ScriptConfig,
) -> Result<(), String> {
    let data_dir = state.store().data_dir();
    let mut settings = Settings::load_from_path(&data_dir).map_err(|e| e.to_string())?;
    settings.script = script;
    settings.save_to_path(&data_dir).map_err(|e| e.to_string())?;

    state.set_settings(settings);

    Ok(())
}

#[tauri::command]
pub fn get_ssl_config(state: tauri::State<'_, AppState>) -> Result<SslConfig, String> {
    let settings = state.settings();
    Ok(settings.ssl)
}

#[tauri::command]
pub fn save_ssl_config(
    state: tauri::State<'_, AppState>,
    ssl: SslConfig,
) -> Result<(), String> {
    let data_dir = state.store().data_dir();
    let mut settings = Settings::load_from_path(&data_dir).map_err(|e| e.to_string())?;
    settings.ssl = ssl;
    settings.save_to_path(&data_dir).map_err(|e| e.to_string())?;

    state.set_settings(settings);

    Ok(())
}
