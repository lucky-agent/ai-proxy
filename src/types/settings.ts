export interface ProxyConfig {
  listen_host: string;
  listen_port: number;
  /** 上游代理开关；关闭时忽略下面的地址/端口，请求直连 */
  upstream_proxy_enabled: boolean;
  /** 上游代理地址；留空表示不使用上游代理，请求直连 */
  upstream_proxy_host: string;
  upstream_proxy_port: number;
  /** 跟随系统代理；开启时忽略地址/端口，按请求读取并跟随 OS 系统代理 */
  upstream_proxy_follow_system: boolean;
  /** 包含规则：为空 = 全部走上游代理；非空时仅命中的主机走代理 */
  upstream_proxy_include: string[];
  /** 排除规则：命中即直连，优先级高于包含规则 */
  upstream_proxy_exclude: string[];
}

/** 网络节流档位；none = 不限速（用于把域名从 `*` 等更宽的规则中排除） */
export type ThrottleProfile = "none" | "2g" | "3g" | "slow4g" | "fast4g" | "5g" | "custom";

/** 节流参数：带宽单位 kbps（0 = 不限速），延迟单位 ms（0 = 不加延迟） */
export interface ThrottleParams {
  download_kbps: number;
  upload_kbps: number;
  latency_ms: number;
}

/** 一条节流规则：域名通配（`*` = 全部）→ 档位 */
export interface ThrottleRule {
  enabled: boolean;
  domain: string;
  profile: ThrottleProfile;
  /** profile = custom 时使用的参数 */
  custom: ThrottleParams;
}

/** 网络节流配置：总开关 + 规则（自上而下命中第一条，每个请求独立限速） */
export interface ThrottleConfig {
  enabled: boolean;
  rules: ThrottleRule[];
}

/** 上游代理规则判定结果（test_upstream_proxy_rules 返回） */
export type UpstreamRouteDecision =
  | { route: "proxy"; rule: string | null }
  | { route: "excluded"; rule: string }
  | { route: "not_included" };

export interface TlsWhitelistItem {
  domain: string;
  enabled: boolean;
}

export interface TlsConfig {
  enabled: boolean;
  whitelist: TlsWhitelistItem[];
  record_decrypted_only: boolean;
}

export interface ScriptItem {
  name: string;
  /** 脚本文件名（持久化后由后端生成）；新建未保存时为空串 */
  file_name: string;
  domain: string;
  /** HTTP 方法匹配（大写，如 "GET"）；空串 = any，匹配所有方法 */
  method: string;
  enabled: boolean;
}

export interface ScriptConfig {
  enabled: boolean;
  scripts: ScriptItem[];
}

export interface AuthzRule {
  name: string;
  /** 域名匹配规则（支持 * 通配符，如 *.example.com） */
  domain: string;
  /** HTTP 方法匹配（大写，如 "GET"）；空串 = any，匹配所有方法 */
  method: string;
  enabled: boolean;
  /** 授权服务检查端点（HTTP/JSON） */
  url: string;
  /** 授权检查超时（毫秒） */
  timeout_ms: number;
  /** 授权服务不可用时的处置：closed = 拒绝 / open = 放行 */
  on_error: "closed" | "open";
}

export interface AuthzConfig {
  enabled: boolean;
  rules: AuthzRule[];
}

/** 反向代理规则：本地监听一个端口，把请求固定转发到指定的远程上游 */
export interface ReverseProxyRule {
  /** 是否启用该规则 */
  enabled: boolean;
  /** 本地监听地址 */
  listen_host: string;
  /** 本地监听端口 */
  listen_port: number;
  /** 上游协议："http" | "https" */
  upstream_scheme: string;
  /** 上游主机（IP 或域名） */
  upstream_host: string;
  /** 上游端口；0 = 使用 scheme 默认（http=80 / https=443） */
  upstream_port: number;
}

/** 反向代理配置：全局开关 + 规则列表 */
export interface ReverseConfig {
  /** 反向代理总开关；关闭时忽略所有规则，不绑定本地监听端口 */
  enabled: boolean;
  rules: ReverseProxyRule[];
}

/** 规则内来源条目：来源名与其会话合并 header 成对。
 *  合并头按序参与会话分组（优先于全局配置），命中即确认来源。 */
export interface AiRuleSource {
  name: string;
  /** 该来源的会话合并 header；空串 = 仅标注，不参与分组 */
  merge_header: string;
}

/** 单条 AI 检测 URL 规则；provider 为 null 表示自动检测（命中即候选，由响应/body 裁决） */
export interface AiUrlRule {
  url: string;
  provider: string | null;
  enabled: boolean;
  /** (来源, 合并头) 对列表 */
  sources: AiRuleSource[];
}

export interface AiConfig {
  enabled: boolean;
  detection: {
    url_patterns: AiUrlRule[];
  };
}

export interface Settings {
  proxy: ProxyConfig;
  tls: TlsConfig;
  script: ScriptConfig;
  log: {
    level: string;
    dir?: string;
    console: boolean;
    max_file_size: number;
    rotation_strategy: string;
  };
  ui: {
    theme: string;
    language: string;
  };
  reverse: ReverseConfig;
}
