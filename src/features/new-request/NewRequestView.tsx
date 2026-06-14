import { useState, useCallback, useMemo } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { SendIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { InputGroup, InputGroupAddon, InputGroupInput } from '@/components/ui/input-group'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { useLocale } from '@/hooks/useLocale'
import { cn } from '@/lib/utils'
import { METHOD_COLORS } from '@/lib/http-constants'
import { useCollections } from '@/hooks/useCollections'
import { ApiCollectionPanel } from './ApiCollectionPanel'
import { DetailPanel } from '@/features/detail-panel'
import RequestEditor from './RequestEditor'
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable'
import { Alert, AlertDescription } from '@/components/ui/alert'
import type { ApiRequestNode, HttpMethod, KeyValuePair, BodyType } from '@/types/collection'
import type { TrafficEntry } from '@/types/proxy'

interface NewRequestViewProps {
  onSendSuccess: (entryId: string) => void
  entries: TrafficEntry[]
}

const METHODS: HttpMethod[] = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS']

/** Serialize cookies KV array into a Cookie header value */
function serializeCookies(cookies: KeyValuePair[]): string | null {
  const filled = cookies.filter(c => c.key.trim())
  if (filled.length === 0) return null
  return filled.map(c => `${c.key.trim()}=${c.value}`).join('; ')
}

export function NewRequestView({ onSendSuccess, entries }: NewRequestViewProps) {
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
  const [params, setParams] = useState<KeyValuePair[]>([])
  const [headers, setHeaders] = useState<KeyValuePair[]>([])
  const [cookies, setCookies] = useState<KeyValuePair[]>([])
  const [body, setBody] = useState('')
  const [bodyType, setBodyType] = useState<BodyType>('json')
  const [sending, setSending] = useState(false)
  const [error, setError] = useState('')
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [activeEntryId, setActiveEntryId] = useState<string | null>(null)

  const handleSend = useCallback(async () => {
    if (sending) return
    if (!url.trim()) return

    setSending(true)
    setError('')

    const headerMap: Record<string, string> = {}
    for (const { key, value } of headers) {
      if (key.trim()) headerMap[key.trim()] = value
    }

    const cookieStr = serializeCookies(cookies)
    if (cookieStr) {
      headerMap['Cookie'] = cookieStr
    }

    const filledParams = params.filter(p => p.key.trim())
    let finalUrl = url.trim()
    if (filledParams.length > 0) {
      const sep = finalUrl.includes('?') ? '&' : '?'
      const qs = filledParams
        .map(p => `${encodeURIComponent(p.key.trim())}=${encodeURIComponent(p.value)}`)
        .join('&')
      finalUrl = finalUrl + sep + qs
    }

    try {
      const entryId = await invoke<string>('resend_request', {
        method,
        url: finalUrl,
        headers: headerMap,
        body: body || null,
      })
      setActiveEntryId(entryId)
      onSendSuccess(entryId)
    } catch (err) {
      setError(String(err))
    } finally {
      setSending(false)
    }
  }, [sending, url, method, params, headers, cookies, body, onSendSuccess])

  const handleSelectRequest = useCallback((node: ApiRequestNode) => {
    setSelectedId(node.id)
    setMethod(node.method)
    setUrl(node.url)
    setParams(node.params ?? [])
    setHeaders(node.headers ?? [])
    setCookies(node.cookies ?? [])
    setBodyType(node.bodyType ?? 'json')
    setBody(node.body ?? '')
  }, [])

  const handleSave = useCallback(() => {
    if (!selectedId) return
    updateRequest(selectedId, {
      method,
      url,
      params: params.filter(p => p.key.trim()),
      headers: headers.filter(h => h.key.trim()),
      cookies: cookies.filter(c => c.key.trim()),
      bodyType,
      body,
    })
  }, [selectedId, method, url, params, headers, cookies, bodyType, body, updateRequest])

  const activeEntry = useMemo(() => {
    if (!activeEntryId) return undefined
    return entries.find(e => e.id === activeEntryId)
  }, [activeEntryId, entries])

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center bg-surface-deep text-muted-foreground text-xs">
        {t('settings.loading')}
      </div>
    )
  }

  return (
    <ResizablePanelGroup orientation="horizontal" id="new-request" className="h-full bg-surface-deep">
      {/* Left: API collection panel */}
      <ResizablePanel id="collection" defaultSize="22%" minSize="15%" maxSize="40%" collapsible collapsedSize={0}>
        <div className="h-full overflow-hidden">
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
      </ResizablePanel>

      <ResizableHandle withHandle />

      {/* Right: editor + response */}
      <ResizablePanel id="right" defaultSize="78%" minSize="60%">
        {activeEntry ? (
          <ResizablePanelGroup orientation="vertical" id="new-request-vertical" className="h-full">
            <ResizablePanel id="editor" defaultSize="45%" minSize="15%" maxSize="75%">
              <div className="flex flex-col min-h-0 h-full overflow-hidden">
                <div className="flex shrink-0 items-center gap-2 px-4 py-2 border-b border-border bg-surface-base/50">
                  <InputGroup className="flex-1">
                    <InputGroupAddon align="inline-start" className="py-0 pl-0">
                      <Select value={method} onValueChange={(v) => setMethod(v as HttpMethod)}>
                        <SelectTrigger className={cn(
                          'h-8 py-0 border-0 shadow-none rounded-none rounded-l-lg bg-transparent',
                          'focus-visible:ring-0 focus-visible:ring-offset-0',
                          'min-w-0 w-auto px-2 text-xs font-semibold',
                          'data-[size=sm]:h-8',
                          METHOD_COLORS[method] ? `text-${METHOD_COLORS[method]}` : '',
                        )}>
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent align="start" alignItemWithTrigger={false} className="min-w-[120px] max-h-36 overflow-y-auto [&_[data-slot=select-item]]:py-1 [&_[data-slot=select-item]]:text-xs">
                          {METHODS.map(m => (
                            <SelectItem key={m} value={m}>{m}</SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </InputGroupAddon>
                    <InputGroupInput
                      value={url}
                      onChange={e => setUrl(e.target.value)}
                      className="text-xs font-mono"
                      placeholder="https://api.example.com/v1/endpoint"
                    />
                  </InputGroup>
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
                <RequestEditor
                  params={params}
                  headers={headers}
                  cookies={cookies}
                  body={body}
                  bodyType={bodyType}
                  onParamsChange={setParams}
                  onHeadersChange={setHeaders}
                  onCookiesChange={setCookies}
                  onBodyChange={setBody}
                  onBodyTypeChange={setBodyType}
                />
                {error && (
                  <Alert variant="destructive" className="shrink-0 mx-4 mb-2">
                    <AlertDescription>{error}</AlertDescription>
                  </Alert>
                )}
              </div>
            </ResizablePanel>
            <ResizableHandle withHandle />
            <ResizablePanel id="response" defaultSize="55%" minSize="25%">
              <div className="h-full min-h-0">
                <DetailPanel entry={activeEntry} showRequest={false} />
              </div>
            </ResizablePanel>
          </ResizablePanelGroup>
        ) : (
          <div className="flex flex-col min-h-0 h-full">
            <div className="flex shrink-0 items-center gap-2 px-4 py-2 border-b border-border bg-surface-base/50">
              <InputGroup className="flex-1">
                <InputGroupAddon align="inline-start" className="py-0 pl-0">
                  <Select value={method} onValueChange={(v) => setMethod(v as HttpMethod)}>
                    <SelectTrigger className={cn(
                      'h-full py-0 border-0 shadow-none rounded-none rounded-l-lg bg-transparent',
                      'focus-visible:ring-0 focus-visible:ring-offset-0',
                      'min-w-0 w-auto px-2.5 text-xs font-semibold',
                      METHOD_COLORS[method] ? `text-${METHOD_COLORS[method]}` : '',
                    )}>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent align="start" alignItemWithTrigger={false} className="max-h-36 overflow-y-auto [&_[data-slot=select-item]]:py-1 [&_[data-slot=select-item]]:text-xs">
                      {METHODS.map(m => (
                        <SelectItem key={m} value={m}>{m}</SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </InputGroupAddon>
                <InputGroupInput
                  value={url}
                  onChange={e => setUrl(e.target.value)}
                  className="text-xs font-mono"
                  placeholder="https://api.example.com/v1/endpoint"
                />
              </InputGroup>
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
            <RequestEditor
              params={params}
              headers={headers}
              cookies={cookies}
              body={body}
              bodyType={bodyType}
              onParamsChange={setParams}
              onHeadersChange={setHeaders}
              onCookiesChange={setCookies}
              onBodyChange={setBody}
              onBodyTypeChange={setBodyType}
            />
            {error && (
              <Alert variant="destructive" className="shrink-0 mx-4 mb-2">
                <AlertDescription>{error}</AlertDescription>
              </Alert>
            )}
          </div>
        )}
      </ResizablePanel>
    </ResizablePanelGroup>
  )
}
