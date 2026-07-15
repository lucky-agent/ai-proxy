/// 域名/URL 通配符匹配：pattern 支持 `*`（匹配任意字节序列）和 `?`（匹配任意单个字符）。
/// pattern 为空字符串时匹配所有。
/// 大小写不敏感（匹配前两端 lowercase）。也支持含路径的 pattern，如 `*.example.com/api/*`。
///
use rama::utils::thirdparty::wildcard;
pub fn domain_match(pattern: &str, host: &str) -> bool {
    if pattern.is_empty() {
        return true;
    }
    let p = pattern.to_lowercase();
    let h = host.to_lowercase();
    wildcard::Wildcard::new(p.as_bytes())
        .map(|wc| wc.is_match(h.as_bytes()))
        .unwrap_or(false)
}
