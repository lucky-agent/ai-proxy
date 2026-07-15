use crate::utils::domain_match;
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

impl SslConfig {
    /// host 是否命中已启用的 MITM 解密白名单。
    /// 总开关关闭时恒为 false，不再迭代白名单。
    pub fn should_mitm(&self, host: &str) -> bool {
        self.enabled
            && self
                .whitelist
                .iter()
                .any(|item| item.enabled && domain_match::domain_match(&item.domain, host))
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
    /// 脚本文件名（name + ".js" 的 SHA256 前 16 位，后端在保存时自动生成）
    #[serde(default)]
    pub file_name: String,
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
    /// 脚本文件目录，不序列化（后端运行时注入）
    #[serde(skip)]
    pub scripts_dir: Option<std::path::PathBuf>,
}

impl Default for ScriptConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            scripts: Vec::new(),
            scripts_dir: None,
        }
    }
}

/// AI 流量检测配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AiDetectionConfig {
    /// URL glob 规则列表，首条命中即止。
    /// 缺省（配置里无此段）时用内置默认规则播种；显式空数组则尊重用户清空。
    #[serde(default = "default_ai_url_rules")]
    pub url_patterns: Vec<AiUrlRule>,
}

impl Default for AiDetectionConfig {
    fn default() -> Self {
        Self {
            url_patterns: default_ai_url_rules(),
        }
    }
}

/// 单条 AI URL 规则
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AiUrlRule {
    /// URL glob，候选串为 host + path（已剥 scheme/query/默认端口）
    #[serde(default)]
    pub url: String,
    /// "openai" | "anthropic" | null；null/非法 → Candidate
    #[serde(default)]
    pub provider: Option<String>,
    /// 该条规则是否启用，缺省视为启用（兼容旧配置）
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// AI 配置根
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AiConfig {
    /// AI 检测总开关。关闭时后端完全不做 AI 检测/归一化/推送事件。
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub detection: AiDetectionConfig,
    #[serde(default)]
    pub session: AiSessionConfig,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            detection: AiDetectionConfig::default(),
            session: AiSessionConfig::default(),
        }
    }
}

/// AI 会话分组配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AiSessionConfig {
    /// 会话标识请求头名单，按顺序取第一个命中的 header 值作为会话标识。
    /// 不写死在分组逻辑中，用户可增删。
    #[serde(default = "default_session_headers")]
    pub session_headers: Vec<String>,
    /// 无 session header 时是否启用消息前缀匹配兜底。
    #[serde(default = "default_true")]
    pub prefix_match_fallback: bool,
    /// 内存会话表上限，超过后按 LRU 淘汰。
    #[serde(default = "default_session_max")]
    pub max_sessions: usize,
}

fn default_session_headers() -> Vec<String> {
    vec![
        "x-claude-code-session-id".to_string(),
        "x-session-id".to_string(),
    ]
}

fn default_session_max() -> usize {
    500
}

impl Default for AiSessionConfig {
    fn default() -> Self {
        Self {
            session_headers: default_session_headers(),
            prefix_match_fallback: true,
            max_sessions: default_session_max(),
        }
    }
}

/// 内置默认 URL 规则（OpenAI/Anthropic 官方 + DeepSeek/Azure/OpenRouter 常见），
/// 作为初始种子写入配置；之后用户可自由增删/启用停用，全部以配置文件为准。
pub(crate) fn default_ai_url_rules() -> Vec<AiUrlRule> {
    vec![
        AiUrlRule {
            url: "api.openai.com/v1/chat/completions".into(),
            provider: Some("openai".into()),
            enabled: true,
        },
        AiUrlRule {
            url: "api.anthropic.com/v1/messages".into(),
            provider: Some("anthropic".into()),
            enabled: true,
        },
        AiUrlRule {
            url: "api.deepseek.com/v1/chat/completions".into(),
            provider: Some("openai".into()),
            enabled: true,
        },
        AiUrlRule {
            url: "*.openai.azure.com/openai/deployments/*/chat/completions".into(),
            provider: Some("openai".into()),
            enabled: true,
        },
        AiUrlRule {
            url: "openrouter.ai/api/v1/chat/completions".into(),
            provider: None,
            enabled: true,
        },
    ]
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
    /// AI 流量检测配置
    #[serde(default)]
    pub ai: AiConfig,
    #[serde(default)]
    pub persistence: Option<bool>,
    /// 数据保留天数（0 = 永久保留），默认 30 天
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
}

fn default_retention_days() -> u32 {
    30
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            proxy: ProxyConfig::default(),
            ssl: SslConfig::default(),
            script: ScriptConfig::default(),
            log: LogConfig::default(),
            ui: UiConfig::default(),
            ai: AiConfig::default(),
            persistence: Some(false),
            retention_days: default_retention_days(),
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
            None => data_dir.join("logs"),
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
            log::error!(
                "Failed to create log directory {}: {}",
                resolved.display(),
                e
            );
        });
    }
}
