mod locale;
mod proxy;
mod settings;
mod theme;

pub use locale::{get_locale, resolve_locale_for_tray, set_locale, sync_tray_locale};
pub use proxy::{get_status, start_proxy, stop_proxy};
pub use settings::{get_settings, save_settings};
pub use theme::{get_theme, set_theme};