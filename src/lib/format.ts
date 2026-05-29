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
    return url.pathname + url.search
  } catch {
    return uri
  }
}

export function extractHost(uri: string): string {
  try {
    return new URL(uri).host
  } catch {
    return uri
  }
}