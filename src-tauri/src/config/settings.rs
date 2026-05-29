use log::info;
use rama::error::{BoxError, ErrorContext};
use serde::{Deserialize, Serialize};
use std::path::Path;

const CONFIG_FILE_NAME: &str = "setting.json";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProxyConfig {
    /// 监听地址（如 "127.0.0.1"）
    #[serde(default)]
    pub listen_host: String,
    /// 监听端口
    #[serde(default)]
    pub listen_port: u16,
    /// 是否使用系统代理转发上游请求，默认关闭
    #[serde(default)]
    pub upstream_proxy: bool,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            listen_host: "127.0.0.1".to_string(),
            listen_port: 5201,
            upstream_proxy: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogConfig {
    /// 日志级别：trace / debug / info / warn / error
    #[serde(default = "default_level")]
    pub level: String,
    /// 日志存储目录，默认在 data_dir/logs 下
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
    /// 是否输出到控制台
    #[serde(default = "default_true")]
    pub console: bool,
    /// 单个日志文件最大大小（字节），超过后滚动，默认 1GB
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u128,
    /// 日志滚动策略：KeepAll 保留所有 / KeepOne 只保留最新
    #[serde(default = "default_rotation_strategy")]
    pub rotation_strategy: String,
}

fn default_level() -> String {
    "info".to_string()
}

fn default_max_file_size() -> u128 {
    1024 * 1024 * 1024 // 1GB
}

fn default_rotation_strategy() -> String {
    "KeepAll".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: default_level(),
            dir: None,
            console: true,
            max_file_size: default_max_file_size(),
            rotation_strategy: default_rotation_strategy(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UiConfig {
    /// 主题模式: "light" / "dark" / "system"
    #[serde(default = "default_theme")]
    pub theme: String,
    /// 界面语言: "en" / "zh" / "system"
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_theme() -> String {
    "system".to_string()
}

fn default_language() -> String {
    "system".to_string()
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            language: default_language(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Settings {
    /// 代理配置
    #[serde(default)]
    pub proxy: ProxyConfig,
    /// 日志配置
    #[serde(default)]
    pub log: LogConfig,
    /// UI 配置
    #[serde(default)]
    pub ui: UiConfig,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            proxy: ProxyConfig::default(),
            log: LogConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

impl Settings {
    /// Load config from data_dir/setting.json.
    /// Creates the file with defaults if it doesn't exist.
    pub(crate) fn load_from_path(dir: &Path) -> Result<Settings, BoxError> {
        let config_path = dir.join(CONFIG_FILE_NAME);

        if !config_path.exists() {
            let default = Settings::default();
            let content = serde_json::to_string_pretty(&default)
                .context("Failed to serialize default settings")?;
            std::fs::write(&config_path, content)
                .context("Failed to write default config file")?;
            info!("Created default config at {}", config_path.display());
        }

        info!("Loading configuration from {}", config_path.display());
        let content =
            std::fs::read_to_string(&config_path).context("Failed to read config file")?;
        let mut settings: Settings =
            serde_json::from_str(&content).context("json parse error")?;
        settings.resolve_log_dir(dir);
        info!("Configuration loaded successfully");
        Ok(settings)
    }

    pub(crate) fn save_to_path(&self, dir: &Path) -> Result<(), BoxError> {
        let config_path = dir.join(CONFIG_FILE_NAME);
        let content = serde_json::to_string_pretty(self)
            .context("Failed to serialize settings")?;
        std::fs::write(&config_path, content)
            .context("Failed to write config file")?;
        info!("Configuration saved to {}", config_path.display());
        Ok(())
    }

    fn resolve_log_dir(&mut self, data_dir: &Path) {
        match &self.log.dir {
            None => {
                self.log.dir = Some(data_dir.join("logs").to_string_lossy().to_string());
            }
            Some(dir) => {
                let path = Path::new(dir);
                if !path.is_absolute() {
                    self.log.dir = Some(data_dir.join(path).to_string_lossy().to_string());
                }
            }
        }
        let log_dir = Path::new(self.log.dir.as_ref().unwrap());
        std::fs::create_dir_all(log_dir).expect("Failed to create log directory");
    }
}