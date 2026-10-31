//! 上游代理的包含 / 排除规则（方案 B：两份列表，排除优先）。
//!
//! 判定：`走上游代理 = (include 为空 || 命中 include) && !命中 exclude`。
//!
//! 单条规则支持：
//! - 域名通配符：`*.example.com`、`api.*.com`（与 [`domain_match`] 语义一致，
//!   注意 `*.example.com` **不**匹配 `example.com` 本身）
//! - IP 字面量：`127.0.0.1`、`::1`（也接受 `[::1]`）
//! - CIDR：`10.0.0.0/8`、`fd00::/8`（仅对 IP 字面量的目标生效，不做 DNS 解析）
//! - `<local>`：不含 `.` 的主机名（如 `localhost`、`intranet`），同 Windows 系统代理写法
//!
//! 规则大小写不敏感，前后空白忽略，空行与 `#` 开头的行视为注释跳过。

use std::net::IpAddr;

use rama::utils::thirdparty::wildcard::Wildcard;

/// 单条已编译规则。
enum Rule {
    /// `<local>`：无点主机名
    Local,
    /// 精确 IP
    Ip(IpAddr),
    /// CIDR 网段
    Cidr { network: IpAddr, prefix: u8 },
    /// 域名通配符（已 lowercase）
    Pattern(Wildcard<'static, u8>),
}

impl Rule {
    fn parse(raw: &str) -> Option<Self> {
        let s = raw.trim();
        if s.is_empty() || s.starts_with('#') {
            return None;
        }
        if s.eq_ignore_ascii_case("<local>") {
            return Some(Self::Local);
        }
        if let Some((net, prefix)) = s.split_once('/') {
            let network = parse_ip(net)?;
            let prefix: u8 = prefix.trim().parse().ok()?;
            let max = if network.is_ipv4() { 32 } else { 128 };
            if prefix > max {
                return None;
            }
            return Some(Self::Cidr { network, prefix });
        }
        if let Some(ip) = parse_ip(s) {
            return Some(Self::Ip(ip));
        }
        let pattern = s.to_ascii_lowercase().into_bytes();
        Wildcard::from_owned(pattern).ok().map(Self::Pattern)
    }

    fn matches(&self, host: &str, ip: Option<IpAddr>) -> bool {
        match self {
            Self::Local => ip.is_none() && !host.contains('.'),
            Self::Ip(rule_ip) => ip.is_some_and(|ip| normalize(ip) == normalize(*rule_ip)),
            Self::Cidr { network, prefix } => {
                ip.is_some_and(|ip| cidr_contains(*network, *prefix, ip))
            }
            Self::Pattern(wc) => wc.is_match(host.as_bytes()),
        }
    }
}

fn parse_ip(s: &str) -> Option<IpAddr> {
    let s = s.trim();
    let s = s
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .unwrap_or(s);
    s.parse().ok()
}

/// IPv4-mapped IPv6（`::ffff:a.b.c.d`）归一为 IPv4，便于与 IPv4 规则比较。
fn normalize(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(IpAddr::V6(v6), IpAddr::V4),
        v4 => v4,
    }
}

fn cidr_contains(network: IpAddr, prefix: u8, ip: IpAddr) -> bool {
    match (normalize(network), normalize(ip)) {
        (IpAddr::V4(net), IpAddr::V4(ip)) => {
            let mask = u32::MAX.checked_shl(32 - u32::from(prefix)).unwrap_or(0);
            (u32::from(net) & mask) == (u32::from(ip) & mask)
        }
        (IpAddr::V6(net), IpAddr::V6(ip)) => {
            let mask = u128::MAX.checked_shl(128 - u32::from(prefix)).unwrap_or(0);
            (u128::from(net) & mask) == (u128::from(ip) & mask)
        }
        _ => false,
    }
}

/// 一份已编译的规则列表，保留原始文本用于展示命中项。
struct RuleList(Vec<(String, Rule)>);

impl RuleList {
    fn compile(raw: &[String]) -> Self {
        Self(
            raw.iter()
                .filter_map(|r| Rule::parse(r).map(|rule| (r.trim().to_string(), rule)))
                .collect(),
        )
    }

    fn first_match(&self, host: &str, ip: Option<IpAddr>) -> Option<&str> {
        self.0
            .iter()
            .find(|(_, rule)| rule.matches(host, ip))
            .map(|(raw, _)| raw.as_str())
    }
}

/// 路由判定结果，附带命中的规则原文（供设置页「测试」展示）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "route", rename_all = "snake_case")]
pub enum RouteDecision {
    /// 走上游代理；`rule` 为命中的 include 规则，include 为空时为 `None`
    Proxy { rule: Option<String> },
    /// 命中 exclude → 直连
    Excluded { rule: String },
    /// include 非空但未命中 → 直连
    NotIncluded,
}

impl RouteDecision {
    pub const fn use_proxy(&self) -> bool {
        matches!(self, Self::Proxy { .. })
    }
}

/// 编译后的包含 / 排除规则。
pub struct UpstreamRules {
    include: RuleList,
    exclude: RuleList,
}

impl UpstreamRules {
    pub fn compile(include: &[String], exclude: &[String]) -> Self {
        Self {
            include: RuleList::compile(include),
            exclude: RuleList::compile(exclude),
        }
    }

    /// 两份列表编译后都为空 → 无需判定，所有请求都走上游代理。
    pub fn is_empty(&self) -> bool {
        self.include.0.is_empty() && self.exclude.0.is_empty()
    }

    /// 判定目标主机（不含端口）是否走上游代理。
    pub fn decide(&self, host: &str) -> RouteDecision {
        let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
        let ip = parse_ip(&host);
        // 通配符匹配用去掉方括号的形式，与用户书写习惯一致
        let host = ip.map_or(host, |ip| ip.to_string());

        if let Some(rule) = self.exclude.first_match(&host, ip) {
            return RouteDecision::Excluded {
                rule: rule.to_string(),
            };
        }
        if self.include.0.is_empty() {
            return RouteDecision::Proxy { rule: None };
        }
        match self.include.first_match(&host, ip) {
            Some(rule) => RouteDecision::Proxy {
                rule: Some(rule.to_string()),
            },
            None => RouteDecision::NotIncluded,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(include: &[&str], exclude: &[&str]) -> UpstreamRules {
        let v = |l: &[&str]| l.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        UpstreamRules::compile(&v(include), &v(exclude))
    }

    #[test]
    fn empty_rules_proxy_everything() {
        let r = rules(&[], &[]);
        assert!(r.is_empty());
        assert!(r.decide("api.openai.com").use_proxy());
    }

    #[test]
    fn exclude_wins_over_include() {
        let r = rules(&["*.openai.com"], &["internal.openai.com"]);
        assert!(r.decide("api.openai.com").use_proxy());
        assert_eq!(
            r.decide("internal.openai.com"),
            RouteDecision::Excluded {
                rule: "internal.openai.com".into()
            }
        );
        assert_eq!(r.decide("example.com"), RouteDecision::NotIncluded);
    }

    #[test]
    fn wildcard_is_case_insensitive_and_strict_on_apex() {
        let r = rules(&[], &["*.Example.com"]);
        assert!(!r.decide("API.example.COM").use_proxy());
        // 与 domain_match 一致：*.example.com 不含 apex
        assert!(r.decide("example.com").use_proxy());
    }

    #[test]
    fn ip_and_cidr() {
        let r = rules(&[], &["127.0.0.1", "::1", "10.0.0.0/8", "fd00::/8"]);
        assert!(!r.decide("127.0.0.1").use_proxy());
        assert!(!r.decide("[::1]").use_proxy());
        assert!(!r.decide("10.20.30.40").use_proxy());
        assert!(!r.decide("fd12::1").use_proxy());
        assert!(r.decide("11.0.0.1").use_proxy());
        // IPv4-mapped IPv6 归一后命中 IPv4 网段
        assert!(!r.decide("::ffff:10.1.1.1").use_proxy());
        // CIDR 不对域名做解析
        assert!(r.decide("ten.example.com").use_proxy());
    }

    #[test]
    fn local_matches_dotless_names_only() {
        let r = rules(&[], &["<local>"]);
        assert!(!r.decide("localhost").use_proxy());
        assert!(!r.decide("intranet").use_proxy());
        assert!(r.decide("api.openai.com").use_proxy());
        assert!(r.decide("127.0.0.1").use_proxy());
    }

    #[test]
    fn invalid_and_comment_lines_are_skipped() {
        let r = rules(&["", "  ", "# comment", "10.0.0.0/99"], &[]);
        assert!(r.is_empty());
        assert!(r.decide("anything").use_proxy());
    }

    #[test]
    fn edge_prefixes() {
        let r = rules(&[], &["0.0.0.0/0"]);
        assert!(!r.decide("8.8.8.8").use_proxy());
        let r = rules(&[], &["8.8.8.8/32"]);
        assert!(!r.decide("8.8.8.8").use_proxy());
        assert!(r.decide("8.8.8.9").use_proxy());
    }
}
