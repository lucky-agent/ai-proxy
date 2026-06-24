/// 简易域名通配符匹配：pattern 支持 `*`（匹配所有）和 `*.` 前缀通配符（如 *.example.com）。
/// pattern 为空字符串时匹配所有（未配置域名规则 = 所有请求都执行）。
pub fn domain_match(pattern: &str, host: &str) -> bool {
    let p = pattern.to_lowercase();
    let h = host.to_lowercase();
    if p.is_empty() || p == "*" {
        return true;
    }
    if let Some(suffix) = p.strip_prefix("*.") {
        return h.ends_with(suffix) && h != suffix;
    }
    p == h
}
