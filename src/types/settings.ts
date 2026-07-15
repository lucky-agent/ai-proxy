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
  enabled: boolean
}

export interface ScriptConfig {
  enabled: boolean
  scripts: ScriptItem[]
}

/** 单条 AI 检测 URL 规则；provider 为 null 表示自动检测（命中即候选，由响应/body 裁决） */
export interface AiUrlRule {
  url: string
  provider: string | null
  enabled: boolean
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
