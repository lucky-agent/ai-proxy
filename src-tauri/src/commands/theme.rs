use tauri::Manager;

use crate::AppState;
use crate::config::Settings;

#[tauri::command]
pub fn get_theme(state: tauri::State<'_, AppState>) -> Result<String, String> {
    Ok(state.settings().ui.theme)
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
        // 拖拽窗口时 webview 重绘可能滞后于窗口边框，Native 背景色暴露为白色。
        // 与 CSS 背景色同步避免拖拽边缘闪白（Windows 忽略 alpha 通道）。
        let bg = match theme.as_str() {
            "dark" => tauri::webview::Color(0x25, 0x25, 0x25, 0xff),
            _ => tauri::webview::Color(0xfa, 0xfb, 0xfb, 0xff),
        };
        window.set_background_color(Some(bg)).map_err(|e| e.to_string())?;
    }

    Ok(theme)
}
