pub mod open_url;
pub use open_url::open_url;
mod resend;
mod locale;
mod proxy;
mod settings;
mod collection;
pub use collection::{
    get_collections, create_collection, create_folder, create_request,
    delete_node, rename_node, move_node, save_request, duplicate_request,
};
mod theme;

pub use locale::{get_locale, set_locale, sync_tray_locale};
pub use proxy::{get_status, start_proxy, stop_proxy, subscribe_proxy_events};
pub use resend::resend_request;
pub use settings::{get_settings, save_settings};
pub use settings::{get_ssl_config, save_ssl_config, set_ssl_enabled};
pub use settings::{get_ai_config, save_ai_config};
pub use settings::{get_script_config, save_script_config, set_script_enabled, get_script_content, save_script_content};
pub use settings::test_rule_match;
pub use theme::{get_theme, set_theme};
mod traffic;
pub use traffic::load_traffic_history;
