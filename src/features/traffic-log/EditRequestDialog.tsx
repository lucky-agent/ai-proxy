import { useState, useEffect, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { PlusIcon, Trash2Icon, SendIcon } from 'lucide-react'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  DialogDescription,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { useLocale } from '@/hooks/useLocale'
import { cn } from '@/lib/utils'
import type { TrafficEntry } from '@/types/proxy'

interface HeaderPair {
  key: string
  value: string
}

interface Props {
  open: boolean
  onOpenChange: (open: boolean) => void
  entry: TrafficEntry | null
  /** 编辑模式下发送后自动选中并打开详情 */
  onSendSuccess?: (entryId: string) => void
}

const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS']

const METHOD_COLORS: Record<string, string> = {
  GET: 'badge-get',
  POST: 'badge-post',
  PUT: 'badge-put',
  DELETE: 'badge-delete',
  PATCH: 'badge-patch',
  HEAD: 'badge-head',
  OPTIONS: 'badge-options',
}

function buildFullUrl(entry: TrafficEntry): string {
  if (entry.uri.startsWith('http://') || entry.uri.startsWith('https://')) {
    return entry.uri
  }
  const host = entry.requestHeaders?.['host'] ?? entry.requestHeaders?.['Host'] ?? ''
  if (host) {
    return 'https://' + host + entry.uri
  }
  return entry.uri
}

export default function RequestEditorDialog({ open, onOpenChange, entry, onSendSuccess }: Props) {
  const { t } = useLocale()
  const isNew = entry === null

  const [method, setMethod] = useState('GET')
  const [url, setUrl] = useState('')
  const [headers, setHeaders] = useState<HeaderPair[]>([])
  const [body, setBody] = useState('')
  const [sending, setSending] = useState(false)
  const [error, setError] = useState('')

  // 打开时：有 entry 则填充，无 entry 则重置为空白
  useEffect(() => {
    if (!open) return
    setError('')
    setSending(false)

    if (entry) {
      setMethod(entry.method)
      setUrl(buildFullUrl(entry))
      setHeaders(
        Object.entries(entry.requestHeaders)
          .filter(([k]) => {
            const lk = k.toLowerCase()
            return lk !== 'host' && lk !== 'content-length' && lk !== 'transfer-encoding'
          })
          .map(([key, value]) => ({ key, value }))
      )
      setBody(entry.requestBody ?? '')
    } else {
      setMethod('GET')
      setUrl('')
      setHeaders([])
      setBody('')
    }
  }, [open, entry])

  const handleAddHeader = useCallback(() => setHeaders(h => [...h, { key: '', value: '' }]), [])
  const handleRemoveHeader = useCallback((i: number) => setHeaders(h => h.filter((_, idx) => idx !== i)), [])
  const handleHeaderChange = useCallback((i: number, field: 'key' | 'value', val: string) => {
    setHeaders(h => h.map((pair, idx) => idx === i ? { ...pair, [field]: val } : pair))
  }, [])

  const handleSend = useCallback(async () => {
    if (sending) return
    if (!url.trim()) return

    setSending(true)
    setError('')

    const headerMap: Record<string, string> = {}
    for (const { key, value } of headers) {
      if (key.trim()) headerMap[key.trim()] = value
    }

    try {
      const entryId = await invoke<string>('resend_request', {
        method,
        url: url.trim(),
        headers: headerMap,
        body: body || null,
      })
      onSendSuccess?.(entryId)
      onOpenChange(false)
    } catch (err) {
      setError(String(err))
    } finally {
      setSending(false)
    }
  }, [sending, url, method, headers, body, onOpenChange, onSendSuccess])

  const handleOpenChange = useCallback((nextOpen: boolean) => {
    onOpenChange(nextOpen)
  }, [onOpenChange])

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="max-w-2xl max-h-[80vh] flex flex-col">
        <DialogHeader>
          <DialogTitle className="text-sm">
            {isNew ? t('sendRequest.title') : t('requestList.edit')}
          </DialogTitle>
          {isNew && (
            <DialogDescription className="text-xs">{t('sendRequest.description')}</DialogDescription>
          )}
        </DialogHeader>

        <div className="flex-1 overflow-y-auto space-y-3 min-h-0">
          {/* Method + URL */}
          <div className="flex gap-2">
            <select
              value={method}
              onChange={e => setMethod(e.target.value)}
              className={cn(
                'shrink-0 rounded-md border border-input bg-background px-2 py-1.5 text-xs font-semibold outline-none focus:ring-1 focus:ring-primary',
                METHOD_COLORS[method] && `text-${METHOD_COLORS[method]}`
              )}
            >
              {METHODS.map(m => (
                <option key={m} value={m}>{m}</option>
              ))}
            </select>
            <input
              type="text"
              value={url}
              onChange={e => setUrl(e.target.value)}
              className="flex-1 rounded-md border border-input bg-background px-2 py-1.5 text-xs text-foreground outline-none focus:ring-1 focus:ring-primary font-mono"
              placeholder="https://api.example.com/v1/endpoint"
            />
          </div>

          {/* Headers */}
          <div>
            <div className="flex items-center justify-between mb-1">
              <span className="text-xs font-medium text-foreground/80">{t('detail.headers')}</span>
              <button
                onClick={handleAddHeader}
                className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
              >
                <PlusIcon className="size-3" />
                {t('sendRequest.addHeader')}
              </button>
            </div>
            <div className="space-y-1">
              {headers.map((pair, i) => (
                <div key={i} className="flex gap-1 items-center">
                  <input
                    type="text"
                    value={pair.key}
                    onChange={e => handleHeaderChange(i, 'key', e.target.value)}
                    className="flex-1 rounded border border-input bg-background px-2 py-1 text-[11px] text-foreground font-mono outline-none focus:ring-1 focus:ring-primary"
                    placeholder="Key"
                  />
                  <input
                    type="text"
                    value={pair.value}
                    onChange={e => handleHeaderChange(i, 'value', e.target.value)}
                    className="flex-[2] rounded border border-input bg-background px-2 py-1 text-[11px] text-foreground font-mono outline-none focus:ring-1 focus:ring-primary"
                    placeholder="Value"
                  />
                  <button
                    onClick={() => handleRemoveHeader(i)}
                    className="shrink-0 rounded p-1 text-muted-foreground hover:text-destructive transition-colors"
                  >
                    <Trash2Icon className="size-3" />
                  </button>
                </div>
              ))}
            </div>
          </div>

          {/* Body */}
          <div>
            <span className="text-xs font-medium text-foreground/80 block mb-1">{t('detail.body')}</span>
            <textarea
              value={body}
              onChange={e => setBody(e.target.value)}
              className="w-full min-h-[120px] rounded-md border border-input bg-background px-2 py-1.5 text-xs text-foreground font-mono resize-y outline-none focus:ring-1 focus:ring-primary"
              placeholder="{ &quot;key&quot;: &quot;value&quot; }"
            />
          </div>

          {/* Error */}
          {error && (
            <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {error}
            </div>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => handleOpenChange(false)}>
            {t('settings.cancel')}
          </Button>
          <Button onClick={handleSend} disabled={sending || !url.trim()}>
            <SendIcon className="size-3.5" />
            {sending ? '...' : t('sendRequest.send')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
