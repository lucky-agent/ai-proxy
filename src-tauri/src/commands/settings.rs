use crate::AppState;
use crate::config::{AiConfig, ProxyConfig, ScriptConfig, Settings, SslConfig};
use crate::utils::domain_match::{domain_match, url_candidate};
use std::hash::{DefaultHasher, Hash, Hasher};

#[tauri::command]
pub fn get_settings(state: tauri::State<'_, AppState>) -> Result<Settings, String> {
    Ok(state.settings())
}

#[tauri::command]
pub fn save_settings(state: tauri::State<'_, AppState>, proxy: ProxyConfig) -> Result<(), String> {
    let data_dir = state.store().data_dir();
    let mut settings = Settings::load_from_path(&data_dir).map_err(|e| e.to_string())?;
    settings.proxy = proxy;
    settings
        .save_to_path(&data_dir)
        .map_err(|e| e.to_string())?;

    state.set_settings(settings);

    Ok(())
}

#[tauri::command]
pub fn get_script_config(state: tauri::State<'_, AppState>) -> Result<ScriptConfig, String> {
    let settings = state.settings();
    Ok(settings.script)
}

#[tauri::command]
pub fn save_script_config(
    state: tauri::State<'_, AppState>,
    script: ScriptConfig,
) -> Result<(), String> {
    let data_dir = state.store().data_dir();
    let mut settings = Settings::load_from_path(&data_dir).map_err(|e| e.to_string())?;

    // validate and generate file_name for each enabled script
    for item in &script.scripts {
        if item.enabled && item.name.trim().is_empty() {
            return Err("Script name is required when enabled".into());
        }
        // name 不允许全部数字（避免与 name 内部 script-N 自动 ID 混淆）
        if item.name.trim().chars().all(|c| c.is_ascii_digit()) {
            return Err(format!("Script name must contain at least one letter: '{}'", item.name));
        }
    }

    let mut validated = script;
    for item in &mut validated.scripts {
        // method 归一化：trim + 大写；"ANY" 与空串同义（any，匹配所有方法）
        item.method = item.method.trim().to_uppercase();
        if item.method == "ANY" {
            item.method.clear();
        }
        if item.enabled {
            let raw = format!("{}.js", item.name);
            let mut hasher = DefaultHasher::new();
            raw.hash(&mut hasher);
            item.file_name = format!("{:016x}", hasher.finish());
        } else {
            item.file_name.clear();
        }
    }

    settings.script = validated;
    settings
        .save_to_path(&data_dir)
        .map_err(|e| e.to_string())?;
    state.set_settings(settings);
    Ok(())
}

#[tauri::command]
pub fn get_ssl_config(state: tauri::State<'_, AppState>) -> Result<SslConfig, String> {
    let settings = state.settings();
    Ok(settings.ssl)
}

#[tauri::command]
pub fn save_ssl_config(state: tauri::State<'_, AppState>, ssl: SslConfig) -> Result<(), String> {
    let data_dir = state.store().data_dir();
    let mut settings = Settings::load_from_path(&data_dir).map_err(|e| e.to_string())?;
    settings.ssl = ssl;
    settings
        .save_to_path(&data_dir)
        .map_err(|e| e.to_string())?;

    state.set_settings(settings);

    Ok(())
}

/// 仅切换 SSL 解密总开关；白名单等其余配置原样保留（供底部栏快捷开关使用）。
#[tauri::command]
pub fn set_ssl_enabled(state: tauri::State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let data_dir = state.store().data_dir();
    let mut settings = Settings::load_from_path(&data_dir).map_err(|e| e.to_string())?;
    settings.ssl.enabled = enabled;
    settings
        .save_to_path(&data_dir)
        .map_err(|e| e.to_string())?;

    state.set_settings(settings);

    Ok(())
}

/// 仅切换脚本总开关；脚本列表原样保留（供底部栏快捷开关使用）。
#[tauri::command]
pub fn set_script_enabled(state: tauri::State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let data_dir = state.store().data_dir();
    let mut settings = Settings::load_from_path(&data_dir).map_err(|e| e.to_string())?;
    settings.script.enabled = enabled;
    settings
        .save_to_path(&data_dir)
        .map_err(|e| e.to_string())?;

    state.set_settings(settings);

    Ok(())
}

/// 读取指定脚本文件的内容。file_name 是 save_script_config 时生成的 hash 文件名。
#[tauri::command]
pub fn get_script_content(
    state: tauri::State<'_, AppState>,
    file_name: String,
) -> Result<String, String> {
    if file_name.is_empty() {
        return Err("file_name is empty".into());
    }
    let script_path = state.store().scripts_dir().join(&file_name);
    std::fs::read_to_string(&script_path)
        .map_err(|e| format!("Failed to read script {}: {}", file_name, e))
}

/// 保存脚本文件内容。file_name 是 save_script_config 时生成的 hash 文件名。
#[tauri::command]
pub fn save_script_content(
    state: tauri::State<'_, AppState>,
    file_name: String,
    content: String,
) -> Result<(), String> {
    if file_name.is_empty() {
        return Err("file_name is empty".into());
    }
    let script_path = state.store().scripts_dir().join(&file_name);
    std::fs::write(&script_path, &content)
        .map_err(|e| format!("Failed to write script {}: {}", file_name, e))
}

/// 测试 url 对一组 pattern 的命中情况（配置弹窗「匹配测试」行使用）。
/// 与运行时同走 domain_match：match_path=false 仅取 host（SSL/脚本语义），
/// true 取 host+path（AI 检测语义）。url 无法解析时全部返回 false。
#[tauri::command]
pub fn test_rule_match(patterns: Vec<String>, url: String, match_path: bool) -> Vec<bool> {
    let Some(candidate) = url_candidate(&url, match_path) else {
        return vec![false; patterns.len()];
    };
    patterns
        .iter()
        .map(|p| domain_match(p, &candidate))
        .collect()
}

#[tauri::command]
pub fn get_ai_config(state: tauri::State<'_, AppState>) -> Result<AiConfig, String> {
    let settings = state.settings();
    Ok(settings.ai)
}

#[tauri::command]
pub fn save_ai_config(state: tauri::State<'_, AppState>, ai: AiConfig) -> Result<(), String> {
    let data_dir = state.store().data_dir();
    let mut settings = Settings::load_from_path(&data_dir).map_err(|e| e.to_string())?;

    // 防御性剔除空 url 规则；其余（含启用状态）以前端为准整体覆盖
    let mut ai = ai;
    ai.detection
        .url_patterns
        .retain(|r| !r.url.trim().is_empty());

    // AI 检测依赖 MITM 解密：启用时联动打开 SSL 总开关并补齐白名单域名
    crate::config::sync_ssl_for_ai(&mut settings.ssl, &ai);

    settings.ai = ai;
    settings
        .save_to_path(&data_dir)
        .map_err(|e| e.to_string())?;

    state.set_settings(settings);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_match_host_vs_path_semantics() {
        let patterns = vec![
            "*.openai.com".to_string(),
            "api.openai.com/v1/*".to_string(),
            String::new(),
        ];
        let url = "https://api.openai.com/v1/chat/completions".to_string();
        // host 语义（SSL/脚本）：带路径的 pattern 永不命中；空 pattern 匹配一切（脚本「不限域名」）
        assert_eq!(
            test_rule_match(patterns.clone(), url.clone(), false),
            vec![true, false, true]
        );
        // host+path 语义（AI 检测）：纯域名 pattern 匹配不到带路径的候选串
        assert_eq!(test_rule_match(patterns, url, true), vec![false, true, true]);
    }

    #[test]
    fn rule_match_unparsable_url_all_false() {
        assert_eq!(
            test_rule_match(vec!["*".into()], "://".into(), false),
            vec![false]
        );
    }
}
