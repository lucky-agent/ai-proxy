export interface ProxyConfig {
  listen_host: string
  listen_port: number
  upstream_proxy: boolean
}

export interface SslWhitelistItem {
  domain: string
  enabled: boolean
}

export interface SslConfig {
  enabled: boolean
  whitelist: SslWhitelistItem[]
}

export interface ScriptItem {
  name: string
  domain: string
  /** HTTP 方法匹配（大写，如 "GET"）；空串 = any，匹配所有方法 */
  method: string
  enabled: boolean
}

export interface ScriptConfig {
  enabled: boolean
  scripts: ScriptItem[]
}

/** 规则内来源条目：来源名与其会话合并 header 成对。
 *  合并头按序参与会话分组（优先于全局配置），命中即确认来源。 */
export interface AiRuleSource {
  name: string
  /** 该来源的会话合并 header；空串 = 仅标注，不参与分组 */
  merge_header: string
}

/** 单条 AI 检测 URL 规则；provider 为 null 表示自动检测（命中即候选，由响应/body 裁决） */
export interface AiUrlRule {
  url: string
  provider: string | null
  enabled: boolean
  /** (来源, 合并头) 对列表 */
  sources: AiRuleSource[]
}

export interface AiConfig {
  enabled: boolean
  detection: {
    url_patterns: AiUrlRule[]
  }
}

export interface Settings {
  proxy: ProxyConfig
  ssl: SslConfig
  script: ScriptConfig
  log: {
    level: string
    dir?: string
    console: boolean
    max_file_size: number
    rotation_strategy: string
  }
  ui: {
    theme: string
    language: string
  }
}
