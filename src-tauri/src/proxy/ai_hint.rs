//! URL 信号 → AiHint 计算。纯函数，便于单测。
//! 候选串 = uri.host_str() + uri.path()（rama Uri 访问器自动剥 scheme/query/默认端口）。
//! MITM 解密后的请求 URI 为 origin-form（如 /v1/chat/completions），此时 uri.host()
//! 为空，转而从 Host 请求头取值。

use rama::net::uri::Uri;

use crate::config::Settings;
use crate::utils::domain_match::domain_match;

use super::events::AiHint;

/// 遍历 settings.ai.detection.url_patterns，首条 domain_match 命中即止。
pub(crate) fn compute_ai_hint(uri: &Uri, host_hint: Option<&str>, settings: &Settings) -> AiHint {
    let host = uri.host_str();
    let host = host.as_deref().or(host_hint).unwrap_or("");
    let path = uri.path_or_root();
    let candidate = format!("{host}{path}");
    for rule in &settings.ai.detection.url_patterns {
        if rule.url.is_empty() || !rule.enabled {
            continue;
        }
        if domain_match(&rule.url, &candidate) {
            return match rule.provider.as_deref() {
                Some("openai") | Some("anthropic") => {
                    AiHint::Provider(rule.provider.clone().unwrap())
                }
                _ => AiHint::Candidate,
            };
        }
    }
    AiHint::None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rama::net::uri::Uri;

    #[test]
    fn disabled_rule_is_skipped() {
        let uri: Uri = "https://api.openai.com/v1/chat/completions"
            .parse()
            .unwrap();
        let mut settings = Settings::default();
        // 默认规则启用时命中 openai
        assert!(matches!(
            compute_ai_hint(&uri, None, &settings),
            AiHint::Provider(ref p) if p == "openai"
        ));
        // 逐条停用后不再命中
        for r in &mut settings.ai.detection.url_patterns {
            r.enabled = false;
        }
        assert!(matches!(compute_ai_hint(&uri, None, &settings), AiHint::None));
    }
}
