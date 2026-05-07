use rama::error::{BoxError, ErrorContext};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{info, warn};
use tracing_subscriber::{EnvFilter, fmt::time::ChronoLocal};

use crate::bail;

const DEFAULT_ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com";
const CONFIG_FILE_NAME: &str = "setting.json";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Settings {
    /// 上游 API 基础地址
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anthropic_base_url: Option<String>,
    /// API 密钥
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anthropic_api_key: Option<String>,
    /// 监听地址（如 "127.0.0.1"）
    #[serde(default)]
    pub listen_host: String,
    /// 监听端口
    #[serde(default)]
    pub listen_port: u16,
    /// 日志模式：chat / verbose / quiet
    #[serde(default)]
    pub level: String,
    /// 是否使用 HTTPS 连接上游
    #[serde(default = "default_true")]
    pub upstream_https: bool,
    /// 上游端口（默认 443）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream_port: Option<u16>,
    /// 上游主机名（仅用于显示，实际请求 URL 由 base_url 决定）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream_host: Option<String>,
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            anthropic_base_url: None,
            anthropic_api_key: None,
            listen_host: "127.0.0.1".to_string(),
            listen_port: 5201,
            level: "info".to_string(),
            upstream_https: true,
            upstream_port: None,
            upstream_host: None,
        }
    }
}

impl Settings {
    pub(crate) fn load() -> Result<Settings, BoxError> {
        let config_path = Path::new(CONFIG_FILE_NAME);

        if config_path.exists() {
            info!("Loading configuration from {}", CONFIG_FILE_NAME);
            let content =
                std::fs::read_to_string(config_path).context("Failed to read config file")?;

            let mut settings: Settings =
                serde_json::from_str(&content).context("json parse error")?;

            settings.resolve_env_vars();
            info!("Configuration loaded successfully from file");
            return Ok(settings);
        }

        warn!(
            "{} not found, falling back to environment variables",
            CONFIG_FILE_NAME
        );

        let mut settings = Settings::default();
        settings.resolve_env_vars();

        if settings.anthropic_base_url.is_none() {
            bail!(
                "Configuration error: Neither {} file nor ANTHROPIC_BASE_URL environment variable is set",
                CONFIG_FILE_NAME
            );
        }

        info!("Configuration loaded from environment variables");
        Ok(settings)
    }

    pub(crate) fn init_logger(&self) -> Result<(), BoxError> {
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| EnvFilter::new(format!("{}", self.level))),
            )
            .with_timer(ChronoLocal::new("%Y-%m-%d %H:%M:%S%.3f".to_string()))
            .finish();

        tracing::subscriber::set_global_default(subscriber)
            .context("Failed to set tracing subscriber")?;
        Ok(())
    }

    fn resolve_env_vars(&mut self) {
        // 环境变量 ANTHROPIC_BASE_URL
        if self.anthropic_base_url.is_none() {
            if let Ok(url) = std::env::var("ANTHROPIC_BASE_URL") {
                if !url.is_empty() {
                    self.anthropic_base_url = Some(url);
                }
            }
        }

        // 环境变量 ANTHROPIC_API_KEY
        if self.anthropic_api_key.is_none() {
            if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
                if !key.is_empty() {
                    self.anthropic_api_key = Some(key);
                }
            }
        }

        // 默认 base_url
        if self.anthropic_base_url.is_none() {
            self.anthropic_base_url = Some(DEFAULT_ANTHROPIC_BASE_URL.to_string());
        }
    }
}