use crate::AppState;
use crate::config::Settings;
use crate::tray::update_tray_menu;

#[tauri::command]
pub fn get_locale(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let data_dir = state.store.data_dir();
    let settings = Settings::load_from_path(&data_dir).map_err(|e| e.to_string())?;
    Ok(settings.ui.language)
}

#[tauri::command]
pub fn set_locale(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    language: String,
) -> Result<String, String> {
    if !["en", "zh", "system"].contains(&language.as_str()) {
        return Err(format!(
            "Invalid language: {}. Must be en, zh, or system.",
            language
        ));
    }

    let data_dir = state.store.data_dir();
    let mut settings = Settings::load_from_path(&data_dir).map_err(|e| e.to_string())?;
    settings.ui.language = language.clone();
    settings
        .save_to_path(&data_dir)
        .map_err(|e| e.to_string())?;

    {
        let mut s = state.settings.lock().unwrap();
        if s.is_some() {
            *s = Some(settings);
        }
    }

    update_tray_menu(&app_handle, &language).map_err(|e| e.to_string())?;

    Ok(language)
}

#[tauri::command]
pub fn sync_tray_locale(app_handle: tauri::AppHandle, locale: String) -> Result<(), String> {
    if !["en", "zh"].contains(&locale.as_str()) {
        return Err(format!("Invalid locale: {}. Must be en or zh.", locale));
    }
    update_tray_menu(&app_handle, &locale).map_err(|e| e.to_string())
}
