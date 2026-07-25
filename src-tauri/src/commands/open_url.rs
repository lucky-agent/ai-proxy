use rama::net::Protocol;
use rama::net::uri::Uri;
use tauri::command;

/// 用系统默认浏览器打开 URL（仅 http/https）
#[command]
pub fn open_url(url: &str) {
    let allowed = Uri::parse(url)
        .ok()
        .and_then(|u| u.scheme().cloned())
        .is_some_and(|s| s == Protocol::HTTP || s == Protocol::HTTPS);
    if allowed {
        let _ = open::that(url);
    }
}
