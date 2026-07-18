pub(crate) mod db;
mod settings;
mod store;

pub use settings::{AiConfig, AiRuleSource, LogConfig, ProxyConfig, ScriptConfig, Settings, SslConfig, sync_ssl_for_ai};
/// 单测构造规则用；生产代码经 AiConfig 间接持有，无需直接引用。
#[cfg(test)]
pub use settings::AiUrlRule;
pub use store::Store;
