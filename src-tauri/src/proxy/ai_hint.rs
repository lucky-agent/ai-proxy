//! URL 信号 → AiHint 计算。纯函数，便于单测。
//! 候选串 = uri.host_str() + uri.path()（rama Uri 访问器自动剥 scheme/query/默认端口）。
//! MITM 解密后的请求 URI 为 origin-form（如 /v1/chat/completions），此时 uri.host()
//! 为空，转而从 Host 请求头取值。

use rama::net::uri::Uri;

use crate::config::{AiRuleSource, Settings};
use crate::utils::domain_match::domain_match;

use super::events::AiHint;

/// 遍历 settings.ai.detection.url_patterns，首条 domain_match 命中即止。
/// 返回 (AiHint, 命中规则的来源对列表)；未命中为 (None, 空)。
pub(crate) fn compute_ai_hint(
    uri: &Uri,
    host_hint: Option<&str>,
    settings: &Settings,
) -> (AiHint, Vec<AiRuleSource>) {
    let host = uri.host_str();
    let host = host.as_deref().or(host_hint).unwrap_or("");
    let path = uri.path_or_root();
    let candidate = format!("{host}{path}");
    for rule in &settings.ai.detection.url_patterns {
        if rule.url.is_empty() || !rule.enabled {
            continue;
        }
        if domain_match(&rule.url, &candidate) {
            let hint = match rule.provider.as_deref() {
                Some("openai") | Some("anthropic") => {
                    AiHint::Provider(rule.provider.clone().unwrap())
                }
                _ => AiHint::Candidate,
            };
            return (hint, rule.sources.clone());
        }
    }
    (AiHint::None, Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AiRuleSource, AiUrlRule};
    use rama::net::uri::Uri;

    #[test]
    fn disabled_rule_is_skipped() {
        let uri: Uri = "https://api.openai.com/v1/chat/completions"
            .parse()
            .unwrap();
        let mut settings = Settings::default();
        // 默认规则启用时命中 openai
        let (hint, _) = compute_ai_hint(&uri, None, &settings);
        assert!(matches!(hint, AiHint::Provider(ref p) if p == "openai"));
        // 逐条停用后不再命中
        for r in &mut settings.ai.detection.url_patterns {
            r.enabled = false;
        }
        let (hint, sources) = compute_ai_hint(&uri, None, &settings);
        assert!(matches!(hint, AiHint::None));
        assert!(sources.is_empty());
    }

    #[test]
    fn returns_sources_of_matched_rule() {
        let uri: Uri = "https://my-gw.local/v1/chat/completions".parse().unwrap();
        let mut settings = Settings::default();
        settings.ai.detection.url_patterns = vec![AiUrlRule {
            url: "my-gw.local/v1/chat/completions".into(),
            provider: Some("openai".into()),
            enabled: true,
            sources: vec![AiRuleSource {
                name: "Cursor".into(),
                merge_header: "x-cursor-session".into(),
            }],
        }];
        let (hint, sources) = compute_ai_hint(&uri, None, &settings);
        assert!(matches!(hint, AiHint::Provider(ref p) if p == "openai"));
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].name, "Cursor");
        assert_eq!(sources[0].merge_header, "x-cursor-session");
    }

    #[test]
    fn sources_empty_for_default_rules() {
        let uri: Uri = "https://api.openai.com/v1/chat/completions"
            .parse()
            .unwrap();
        let settings = Settings::default();
        let (hint, sources) = compute_ai_hint(&uri, None, &settings);
        assert!(matches!(hint, AiHint::Provider(_)));
        assert!(sources.is_empty());
    }
}
