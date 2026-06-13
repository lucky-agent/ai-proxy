import { useState, useCallback, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { PlusIcon, Trash2Icon, SendIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { useLocale } from '@/hooks/useLocale'
import { cn } from '@/lib/utils'
import { useCollections } from '@/hooks/useCollections'
import { ApiCollectionPanel } from './ApiCollectionPanel'
import type { ApiRequestNode, HttpMethod } from '@/types/collection'

interface HeaderPair {
  key: string
  value: string
}

interface NewRequestViewProps {
  onSendSuccess: (entryId: string) => void
}

const METHODS: HttpMethod[] = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS']

const METHOD_COLORS: Record<string, string> = {
  GET: 'badge-get',
  POST: 'badge-post',
  PUT: 'badge-put',
  DELETE: 'badge-delete',
  PATCH: 'badge-patch',
  HEAD: 'badge-head',
  OPTIONS: 'badge-options',
}

const MIN_PANEL_RATIO = 0.15
const MAX_PANEL_RATIO = 0.4

export function NewRequestView({ onSendSuccess }: NewRequestViewProps) {
  const { t } = useLocale()
  const {
    collections,
    loading,
    addFolder,
    addRequest,
    removeNode,
    renameNode,
    updateRequest,
    duplicateRequest,
    renameCollection,
  } = useCollections()

  const [method, setMethod] = useState<HttpMethod>('GET')
  const [url, setUrl] = useState('')
  const [headers, setHeaders] = useState<HeaderPair[]>([])
  const [body, setBody] = useState('')
  const [sending, setSending] = useState(false)
  const [error, setError] = useState('')
  const [selectedId, setSelectedId] = useState<string | null>(null)

  // 左侧面板宽度比例
  const [panelRatio, setPanelRatio] = useState(0.22)
  const [isDragging, setIsDragging] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)
  const liveRatio = useRef(panelRatio)

  if (!isDragging) liveRatio.current = panelRatio

  // 拖拽调整面板宽度（与 TrafficLog 模式一致）
  const handleDividerPointerDown = useCallback((e: React.PointerEvent) => {
    e.preventDefault()
    setIsDragging(true)
    const container = containerRef.current
    if (!container) return

    const onMove = (ev: PointerEvent) => {
      const rect = container.getBoundingClientRect()
      const ratio = ev.clientX / rect.width
      liveRatio.current = Math.min(MAX_PANEL_RATIO, Math.max(MIN_PANEL_RATIO, ratio))
      container.style.setProperty('--collection-ratio', String(liveRatio.current))
    }

    const onUp = () => {
      setPanelRatio(liveRatio.current)
      setIsDragging(false)
      document.removeEventListener('pointermove', onMove)
      document.removeEventListener('pointerup', onUp)
    }

    document.addEventListener('pointermove', onMove)
    document.addEventListener('pointerup', onUp)
  }, [])

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
      onSendSuccess(entryId)
    } catch (err) {
      setError(String(err))
    } finally {
      setSending(false)
    }
  }, [sending, url, method, headers, body, onSendSuccess])

  // 点击请求节点时，填入编辑区
  const handleSelectRequest = useCallback((node: ApiRequestNode) => {
    setSelectedId(node.id)
    setMethod(node.method)
    setUrl(node.url)
    setHeaders(node.headers.map(h => ({ key: h.key, value: h.value })))
    setBody(node.body)
  }, [])

  // 将编辑区配置同步到 collection
  const handleSave = useCallback(() => {
    if (!selectedId) return
    updateRequest(selectedId, {
      method,
      url,
      headers: headers.filter(h => h.key.trim()),
      body,
    })
  }, [selectedId, method, url, headers, body, updateRequest])

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center bg-surface-deep text-muted-foreground text-xs">
        {t('settings.loading')}
      </div>
    )
  }

  return (
    <div
      ref={containerRef}
      className={cn('flex h-full flex-col bg-surface-deep', isDragging && 'select-none')}
    >
      {/* Top bar: method + URL + send + save */}
      <div className="flex items-center gap-2 px-4 py-2 border-b border-border bg-surface-base/50">
        <select
          value={method}
          onChange={e => setMethod(e.target.value as HttpMethod)}
          className={cn(
            'shrink-0 rounded-md border border-input bg-background px-2.5 py-1.5 text-xs font-semibold outline-none focus:ring-1 focus:ring-primary',
            METHOD_COLORS[method] && `text-${METHOD_COLORS[method]}`
          )}>
          {METHODS.map(m => (
            <option key={m} value={m}>{m}</option>
          ))}
        </select>
        <input
          type="text"
          value={url}
          onChange={e => setUrl(e.target.value)}
          className="flex-1 rounded-md border border-input bg-background px-3 py-1.5 text-xs text-foreground font-mono outline-none focus:ring-1 focus:ring-primary"
          placeholder="https://api.example.com/v1/endpoint"
        />
        {selectedId && (
          <Button onClick={handleSave} variant="outline" size="sm">
            {t('settings.save')}
          </Button>
        )}
        <Button onClick={handleSend} disabled={sending || !url.trim()} size="sm">
          <SendIcon className="size-3.5" />
          {sending ? '...' : t('sendRequest.send')}
        </Button>
      </div>

      {/* Content area: left panel + divider + right editor */}
      <div className={cn('flex min-h-0 flex-1 overflow-hidden', isDragging && 'cursor-col-resize')}>
        {/* 左侧：接口管理面板 */}
        <div className="shrink-0 overflow-hidden" style={{ width: `${panelRatio * 100}%` }}>
          <ApiCollectionPanel
            collections={collections}
            selectedId={selectedId}
            onSelectRequest={handleSelectRequest}
            addFolder={addFolder}
            addRequest={addRequest}
            removeNode={removeNode}
            renameNode={renameNode}
            duplicateRequest={duplicateRequest}
            renameCollection={renameCollection}
          />
        </div>

        {/* 拖拽分隔线 */}
        <div
          className="group relative shrink-0 w-[1px] bg-border hover:bg-primary/30 cursor-col-resize"
          onPointerDown={handleDividerPointerDown}
        >
          <div className="absolute inset-y-0 -left-1 -right-1 group-hover:bg-primary/10" />
          <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 opacity-0 group-hover:opacity-100">
            <div className="flex flex-col gap-0.5">
              <div className="size-1 rounded-full bg-foreground/50" />
              <div className="size-1 rounded-full bg-foreground/50" />
              <div className="size-1 rounded-full bg-foreground/50" />
            </div>
          </div>
        </div>

        {/* 右侧：请求编辑区 */}
        <div className="flex-1 overflow-y-auto p-4 space-y-4 min-h-0">
          {/* Headers */}
          <div>
            <div className="flex items-center justify-between mb-1.5">
              <span className="text-xs font-medium text-foreground/80">{t('detail.headers')}</span>
              <button
                onClick={handleAddHeader}
                className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors">
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
                    className="shrink-0 rounded p-1 text-muted-foreground hover:text-destructive transition-colors">
                    <Trash2Icon className="size-3" />
                  </button>
                </div>
              ))}
            </div>
          </div>

          {/* Body */}
          <div>
            <span className="text-xs font-medium text-foreground/80 block mb-1.5">{t('detail.body')}</span>
            <textarea
              value={body}
              onChange={e => setBody(e.target.value)}
              className="w-full min-h-[200px] rounded-md border border-input bg-background px-3 py-2 text-xs text-foreground font-mono resize-y outline-none focus:ring-1 focus:ring-primary"
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
      </div>
    </div>
  )
}
