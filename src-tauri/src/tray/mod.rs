use tauri::{
    AppHandle,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};

pub const TRAY_ID: &str = "main";

struct TrayLabels {
    show: &'static str,
    settings: &'static str,
    quit: &'static str,
}

fn tray_labels(locale: &str) -> TrayLabels {
    match locale {
        "zh" => TrayLabels {
            show: "显示",
            settings: "设置",
            quit: "退出",
        },
        _ => TrayLabels {
            show: "Show",
            settings: "Settings",
            quit: "Quit",
        },
    }
}

pub fn build_tray_menu(app: &AppHandle, locale: &str) -> tauri::Result<Menu<tauri::Wry>> {
    let labels = tray_labels(locale);
    let show_i = MenuItem::with_id(app, "show", labels.show, true, None::<&str>)?;
    let settings_i = MenuItem::with_id(app, "settings", labels.settings, true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit_i = MenuItem::with_id(app, "quit", labels.quit, true, None::<&str>)?;
    Menu::with_items(app, &[&show_i, &sep1, &settings_i, &sep2, &quit_i])
}

pub fn setup_tray(app: &tauri::App, locale: &str) -> tauri::Result<()> {
    let menu = build_tray_menu(app.handle(), locale)?;

    let icon = app
        .default_window_icon()
        .cloned()
        .expect("default window icon should exist");

    let _tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("AI Proxy")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => super::show_main_window(app),
            "settings" => super::open_settings_from_tray(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

pub fn update_tray_menu(app: &AppHandle, locale: &str) -> tauri::Result<()> {
    let menu = build_tray_menu(app, locale)?;
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_menu(Some(menu))?;
    }
    Ok(())
}
