export interface ProxyConfig {
  listen_host: string
  listen_port: number
  upstream_proxy: boolean
}

export interface Settings {
  proxy: ProxyConfig
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
