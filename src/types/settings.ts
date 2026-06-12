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
