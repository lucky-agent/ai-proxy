use log::info;
use rama::error::{BoxError, ErrorContext};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::path::PathBuf;

const CONFIG_FILE_NAME: &str = "setting.json";

/// SSL 解密白名单项
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SslWhitelistItem {
    pub domain: String,
    #[serde(default)]
    pub enabled: bool,
}

/// SSL 解密配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SslConfig {
    /// 全局 SSL 解密开关
    #[serde(default)]
    pub enabled: bool,
    /// 域名白名单，每项可单独开关
    #[serde(default)]
    pub whitelist: Vec<SslWhitelistItem>,
}

impl Default for SslConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            whitelist: Vec::new(),
        }
    }
}

/// 脚本配置项
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScriptItem {
    #[serde(default)]
    pub name: String,
    /// 域名匹配规则（支持 * 通配符，如 *.example.com）
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub enabled: bool,
}

/// 脚本配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScriptConfig {
    /// 全局脚本开关
    #[serde(default)]
    pub enabled: bool,
    /// 脚本列表，每项可单独开关
    #[serde(default)]
    pub scripts: Vec<ScriptItem>,
}

impl Default for ScriptConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            scripts: Vec::new(),
        }
    }
}

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

impl UiConfig {
    pub fn tray_locale(&self) -> &str {
        if self.language == "zh" { "zh" } else { "en" }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Settings {
    /// 代理配置
    #[serde(default)]
    pub proxy: ProxyConfig,
    /// SSL 解密配置
    #[serde(default)]
    pub ssl: SslConfig,
    /// 脚本配置
    #[serde(default)]
    pub script: ScriptConfig,
    /// 日志配置
    #[serde(default)]
    pub log: LogConfig,
    /// UI 配置
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub persistence: Option<bool>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            proxy: ProxyConfig::default(),
            ssl: SslConfig::default(),
            script: ScriptConfig::default(),
            log: LogConfig::default(),
            ui: UiConfig::default(),
            persistence: Some(false),
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
            std::fs::write(&config_path, content).context("Failed to write default config file")?;
            info!("Created default config at {}", config_path.display());
        }

        info!("Loading configuration from {}", config_path.display());
        let content =
            std::fs::read_to_string(&config_path).context("Failed to read config file")?;
        let mut settings: Settings = serde_json::from_str(&content).context("json parse error")?;
        settings.resolve_log_dir(dir);
        info!("Configuration loaded successfully");
        Ok(settings)
    }

    pub(crate) fn save_to_path(&self, dir: &Path) -> Result<(), BoxError> {
        let config_path = dir.join(CONFIG_FILE_NAME);
        let content = serde_json::to_string_pretty(self).context("Failed to serialize settings")?;
        std::fs::write(&config_path, content).context("Failed to write config file")?;
        info!("Configuration saved to {}", config_path.display());
        Ok(())
    }

    fn resolve_log_dir(&mut self, data_dir: &Path) {
        let resolved: PathBuf = match &self.log.dir {
            None => {
                data_dir.join("logs")
            }
            Some(dir) => {
                let path = Path::new(dir);
                if !path.is_absolute() {
                    data_dir.join(path)
                } else {
                    path.to_path_buf()
                }
            }
        };
        self.log.dir = Some(resolved.to_string_lossy().into_owned());
        std::fs::create_dir_all(&resolved).unwrap_or_else(|e| {
            log::error!("Failed to create log directory {}: {}", resolved.display(), e);
        });
    }
}


