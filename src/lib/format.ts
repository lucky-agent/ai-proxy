export function statusCategory(status: number | null): string {
  if (status === null) return 'pending'
  if (status < 300) return 'success'
  if (status < 400) return 'redirect'
  if (status < 500) return 'client-error'
  return 'server-error'
}
export function formatDuration(ms: number | null): string {
  if (ms === null) return '...'
  if (ms < 1000) return `${ms}ms`
  return `${(ms / 1000).toFixed(1)}s`
}
export function formatTime(ts: number | null): string {
  if (ts === null) return ''
  return new Date(ts).toLocaleTimeString()
}
export function shortenUri(uri: string): string {
  try {
    const url = new URL(uri)
    return url.host + url.pathname + url.search
  } catch {
    return uri
  }
}
 export function extractHost(uri: string): string {
   try {
     return new URL(uri).host
   } catch {
     try {
        const host = new URL(`https://${uri}`).host
        // 真域名必然含 .（或为 localhost），否则是路径段伪装，交给 Host header 兜底
        if (!host.includes('.') && host !== 'localhost') return '(unknown)'
        return host
     } catch {
       return '(unknown)'
   }
 }
}

// ---------------------------------------------------------------------------
// 生成 cURL 命令
// ---------------------------------------------------------------------------

/**
 * 将 TrafficEntry 转换为可执行的 cURL 命令字符串。
 * 包含 Method、URL、Headers 和 Body（如有）。
 */
export function formatCurl(entry: {
  method: string
  uri: string
  requestHeaders: Record<string, string>
  requestBody: string | null
}): string {
  // 安全的单引号包裹：将内部的 ' 替换为 '\''
  const sq = (s: string) => `'${s.replace(/'/g, "'\\''")}'`
  const lines: string[] = []
  lines.push(`curl -X ${entry.method} ${sq(entry.uri)}`)
  for (const [k, v] of Object.entries(entry.requestHeaders)) {
    lines.push(`  -H ${sq(`${k}: ${v}`)}`)
  }
  if (entry.requestBody) {
    lines.push(`  -d ${sq(entry.requestBody)}`)
  }
  return lines.join(' \\\n')
}
