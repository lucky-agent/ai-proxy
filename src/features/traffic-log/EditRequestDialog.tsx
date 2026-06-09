import { useState, useEffect, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { PlusIcon, Trash2Icon } from 'lucide-react'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import type { TrafficEntry } from '@/types/proxy'

interface HeaderPair {
  key: string
  value: string
}

interface Props {
  open: boolean
  onOpenChange: (open: boolean) => void
  entry: TrafficEntry | null
  onResend: (method: string, url: string, headers: Record<string, string>, body: string | null) => void
}

const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS']

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

export default function EditRequestDialog({ open, onOpenChange, entry, onResend }: Props) {
  const { t } = useTranslation()

  const [method, setMethod] = useState('GET')
  const [url, setUrl] = useState('')
  const [headers, setHeaders] = useState<HeaderPair[]>([])
  const [body, setBody] = useState('')
  const [sending, setSending] = useState(false)

  useEffect(() => {
    if (!entry || !open) return
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
    setSending(false)
  }, [entry, open])

  const handleAddHeader = useCallback(() => setHeaders(h => [...h, { key: '', value: '' }]), [])
  const handleRemoveHeader = useCallback((i: number) => setHeaders(h => h.filter((_, idx) => idx !== i)), [])
  const handleHeaderChange = useCallback((i: number, field: 'key' | 'value', val: string) => {
    setHeaders(h => h.map((pair, idx) => idx === i ? { ...pair, [field]: val } : pair))
  }, [])

  const handleSend = useCallback(() => {
    if (!entry || sending) return
    setSending(true)
    const headerMap: Record<string, string> = {}
    for (const { key, value } of headers) {
      if (key.trim()) headerMap[key.trim()] = value
    }
    onResend(method, url, headerMap, body || null)
    onOpenChange(false)
    setSending(false)
  }, [entry, sending, headers, method, url, body, onResend, onOpenChange])

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[80vh] flex flex-col sm:max-w-lg">
        <DialogHeader>
          <DialogTitle className="text-sm">{t('requestList.edit')}</DialogTitle>
        </DialogHeader>

        <div className="flex-1 overflow-y-auto space-y-3 min-h-0">
          {/* Method + URL */}
          <div className="flex gap-2">
            <select
              value={method}
              onChange={e => setMethod(e.target.value)}
              className="shrink-0 rounded-md border border-input bg-background px-2 py-1.5 text-xs text-foreground outline-none focus:ring-1 focus:ring-primary"
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
              placeholder="URL"
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
                <span>{t('requestList.edit')}</span>
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
              placeholder={t('detail.noRequestBody')}
            />
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t('settings.cancel')}
          </Button>
          <Button onClick={handleSend} disabled={sending}>
            {sending ? '...' : t('requestList.repeat')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
