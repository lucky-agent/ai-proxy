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
     return new URL(uri).hostname
   } catch {
     // 隧道 URI 格式: host:port，直接提取主机名无需 dot 检查
     const colonAt = uri.lastIndexOf(':')
     if (colonAt > 0 && !uri.includes('/')) {
       return uri.substring(0, colonAt)
     }
     try {
        const host = new URL(`https://${uri}`).hostname
        // 真域名必然含 .（或为 localhost），否则是路径段伪装，交给 Host header 兜底
        if (!host.includes('.') && host !== 'localhost') return '(unknown)'
        return host
     } catch {
       return '(unknown)'
   }
 }
 }

// ---------------------------------------------------------------------------
// 请求类型分类（用于筛选栏）
// ---------------------------------------------------------------------------

export type TypeFilter =
  | 'all'
  | 'http'
  | 'https'
  | 'websocket'
  | 'js'
  | 'css'
  | 'html'
  | 'json'
  | 'img'
  | 'font'
  | 'media'
  | 'other'

export const TYPE_FILTERS: TypeFilter[] = [
  'all', 'http', 'https', 'websocket', 'js', 'css', 'html', 'json', 'img', 'font', 'media', 'other',
]

const IMG_EXTS = /\.(png|jpe?g|gif|svg|webp|ico|bmp|avif|tiff?)($|\?)/i
const FONT_EXTS = /\.(woff2?|ttf|otf|eot)($|\?)/i
const MEDIA_EXTS = /\.(mp4|webm|ogg|mp3|m4a|wav|flac|aac|avi|mov)($|\?)/i

/**
 * 根据 URI、scheme 和响应 Content-Type 对请求进行分类。
 * 优先使用 Content-Type（更精确），回退到 URI 扩展名。
 */
export function classifyEntry(entry: {
  uri: string
  decrypted?: boolean
  responseHeaders: Record<string, string> | null
}): TypeFilter {
  const uri = entry.uri
  const ct = entry.responseHeaders
    ? (entry.responseHeaders['content-type'] ?? entry.responseHeaders['Content-Type'] ?? '').toLowerCase()
    : ''

  // WebSocket 升级
  if (ct.includes('text/event-stream') || uri.startsWith('ws://') || uri.startsWith('wss://')) {
    return 'websocket'
  }

  // 优先用 Content-Type 分类
  if (ct) {
    if (ct.includes('javascript')) return 'js'
    if (ct.includes('text/css')) return 'css'
    if (ct.includes('text/html')) return 'html'
    if (ct.includes('json')) return 'json'
    if (ct.includes('image/')) return 'img'
    if (ct.includes('font/') || ct.includes('application/x-font')) return 'font'
    if (ct.includes('video/') || ct.includes('audio/')) return 'media'
  }

  // 回退到 URI 扩展名
  if (uri.endsWith('.js') || uri.endsWith('.mjs') || uri.endsWith('.cjs')) return 'js'
  if (uri.endsWith('.css')) return 'css'
  if (uri.endsWith('.html') || uri.endsWith('.htm')) return 'html'
  if (uri.endsWith('.json') || uri.endsWith('.jsonl')) return 'json'
  if (IMG_EXTS.test(uri)) return 'img'
  if (FONT_EXTS.test(uri)) return 'font'
  if (MEDIA_EXTS.test(uri)) return 'media'

  // 根据 scheme 区分 HTTP / HTTPS
  if (uri.startsWith('http://')) return 'http'
  if (uri.startsWith('https://') || entry.decrypted === true) return 'https'

  // 兜底：如果无 Content-Type 也无法从 URI 判断
  if (ct) return 'other'
  return 'https' // MITM 代理多数是 HTTPS
}

// ---------------------------------------------------------------------------
// 生成 cURL 命令
// ---------------------------------------------------------------------------

export { formatCurl } from './curl'
export type { FormatCurlOptions } from './curl'
