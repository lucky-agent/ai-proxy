import { useCallback, useRef, useEffect } from 'react'
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
import RequestTabBar from './RequestTabBar'
import { useRequestTabs } from './useRequestTabs'
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { usePanelRef } from 'react-resizable-panels'
import type { ApiRequestNode, KeyValuePair } from '@/types/collection'
import type { TrafficEntry } from '@/types/proxy'

interface NewRequestViewProps {
  onSendSuccess: (entryId: string) => void
  entries: TrafficEntry[]
}

const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'] as const

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

  const {
    tabs,
    activeTab,
    openTab,
    closeTab,
    activateTab,
    updateActiveTab,
    closeOthers,
    closeAll,
    unlinkNode,
  } = useRequestTabs(updateRequest)

  // 左侧树点击 request → 打开 tab
  const handleSelectRequest = useCallback((node: ApiRequestNode) => {
    openTab(node.id, node)
  }, [openTab])

  // 发送请求
  const handleSend = useCallback(async () => {
    if (!activeTab) return
    if (activeTab.sending) return
    if (!activeTab.url.trim()) return

    const sendingTabId = activeTab.id

    updateActiveTab({ sending: true, error: '' }, sendingTabId)

    const headerMap: Record<string, string> = {}
    for (const { key, value } of activeTab.headers) {
      if (key.trim()) headerMap[key.trim()] = value
    }

    const cookieStr = serializeCookies(activeTab.cookies)
    if (cookieStr) {
      headerMap['Cookie'] = cookieStr
    }

    const filledParams = activeTab.params.filter(p => p.key.trim())
    let finalUrl = activeTab.url.trim()
    if (filledParams.length > 0) {
      const sep = finalUrl.includes('?') ? '&' : '?'
      const qs = filledParams
        .map(p => `${encodeURIComponent(p.key.trim())}=${encodeURIComponent(p.value)}`)
        .join('&')
      finalUrl = finalUrl + sep + qs
    }

    try {
      const entryId = await invoke<string>('resend_request', {
        method: activeTab.method,
        url: finalUrl,
        headers: headerMap,
        body: activeTab.body || null,
      })
      updateActiveTab({ responseEntryId: entryId, sending: false }, sendingTabId)
      onSendSuccess(entryId)
    } catch (err) {
      updateActiveTab({ sending: false, error: String(err) }, sendingTabId)
    }
  }, [activeTab, updateActiveTab, onSendSuccess])

  // 树节点删除 → 取消关联 tab
  const handleRemoveNode = useCallback((nodeId: string) => {
    unlinkNode(nodeId)
    removeNode(nodeId)
  }, [unlinkNode, removeNode])

  // 根据 activeTab.responseEntryId 查找 TrafficEntry
  const activeEntry = activeTab?.responseEntryId
    ? entries.find(e => e.id === activeTab.responseEntryId)
    : undefined

  // 控制 response panel collapse/expand
  const responsePanelRef = usePanelRef()
  const prevHadEntryRef = useRef(!!activeEntry)
  useEffect(() => {
    const hadEntry = prevHadEntryRef.current
    const hasEntry = !!activeEntry
    prevHadEntryRef.current = hasEntry

    // expand when entry appears; collapse when it goes away
    if (hasEntry && !hadEntry) {
      responsePanelRef.current?.expand()
    } else if (!hasEntry && hadEntry) {
      responsePanelRef.current?.collapse()
    }
  }, [activeEntry, responsePanelRef])

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
            selectedId={activeTab?.linkedNodeId ?? null}
            onSelectRequest={handleSelectRequest}
            addFolder={addFolder}
            addRequest={addRequest}
            removeNode={handleRemoveNode}
            renameNode={renameNode}
            duplicateRequest={duplicateRequest}
            renameCollection={renameCollection}
          />
        </div>
      </ResizablePanel>

      <ResizableHandle withHandle />

      {/* Right: tab container or empty state */}
      <ResizablePanel id="right" defaultSize="78%" minSize="60%">
        {tabs.length === 0 ? (
          /* --- 空状态占位 --- */
          <div className="flex h-full flex-col items-center justify-center gap-3 text-muted-foreground">
            <SendIcon className="size-10 opacity-20" />
            <p className="text-sm font-medium">{t('tab.emptyTitle')}</p>
            <p className="text-xs">{t('tab.emptySubtitle')}</p>
            <div className="flex gap-2 mt-2">
              <Button variant="outline" size="sm" onClick={() => openTab(null)}>
                + {t('tab.newRequest')}
              </Button>
              <Button variant="ghost" size="sm" onClick={() => {
                // 聚焦左侧面板——通过点击 ResizablePanel 无法程序化触发
                // 因此改为仅提示，用户需手动点击树节点
              }}>
                {t('tab.openFromCollection')}
              </Button>
            </div>
          </div>
        ) : (
          /* --- Tab 区域 --- */
          <div className="flex flex-col h-full min-h-0">
            {/* Tab 标签条 */}
            <RequestTabBar
              tabs={tabs}
              activeTabId={activeTab?.id ?? null}
              onActivate={activateTab}
              onClose={closeTab}
              onNew={() => openTab(null)}
              onCloseOthers={closeOthers}
              onCloseAll={closeAll}
            />

            {/* 活跃 tab 内容（仅挂载 active tab） */}
            {activeTab && (
              <div className="flex flex-col min-h-0 h-full overflow-hidden">
                <div className="flex shrink-0 items-center gap-2 px-4 py-2 border-b border-border bg-surface-base/50">
                  <InputGroup className="flex-1">
                    <InputGroupAddon align="inline-start" className="py-0 pl-0">
                      <Select
                        value={activeTab.method}
                        onValueChange={v => updateActiveTab({ method: v as typeof METHODS[number] })}
                      >
                        <SelectTrigger className={cn(
                          'h-8 py-0 border-0 shadow-none rounded-none rounded-l-lg bg-transparent',
                          'focus-visible:ring-0 focus-visible:ring-offset-0',
                          'min-w-0 w-auto px-2 text-xs font-semibold',
                          'data-[size=sm]:h-8',
                          METHOD_COLORS[activeTab.method] ? `text-${METHOD_COLORS[activeTab.method]}` : '',
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
                      value={activeTab.url}
                      onChange={e => updateActiveTab({ url: e.target.value })}
                      className="text-xs font-mono"
                      placeholder="https://api.example.com/v1/endpoint"
                    />
                  </InputGroup>
                  {activeTab.linkedNodeId && (
                    <Button
                      onClick={() => {
                        if (!activeTab.linkedNodeId) return
                        updateRequest(activeTab.linkedNodeId, {
                          method: activeTab.method,
                          url: activeTab.url,
                          params: activeTab.params.filter(p => p.key.trim()),
                          headers: activeTab.headers.filter(h => h.key.trim()),
                          cookies: activeTab.cookies.filter(c => c.key.trim()),
                          bodyType: activeTab.bodyType,
                          body: activeTab.body,
                        })
                      }}
                      variant="outline"
                      size="sm"
                    >
                      {t('settings.save')}
                    </Button>
                  )}
                  <Button onClick={handleSend} disabled={activeTab.sending || !activeTab.url.trim()} size="sm">
                    <SendIcon className="size-3.5" />
                    {activeTab.sending ? '...' : t('sendRequest.send')}
                  </Button>
                </div>

                {/* Always render the vertical split; collapse response panel when no entry */}
                <ResizablePanelGroup orientation="vertical" id="new-request-vertical" className="flex-1 min-h-0">
                  <ResizablePanel id="editor" defaultSize={activeEntry ? 45 : 100} minSize={15} maxSize={activeEntry ? 75 : 100}>
                    <div className="flex flex-col min-h-0 h-full overflow-hidden">
                      <RequestEditor
                        params={activeTab.params}
                        headers={activeTab.headers}
                        cookies={activeTab.cookies}
                        body={activeTab.body}
                        bodyType={activeTab.bodyType}
                        onParamsChange={v => updateActiveTab({ params: v })}
                        onHeadersChange={v => updateActiveTab({ headers: v })}
                        onCookiesChange={v => updateActiveTab({ cookies: v })}
                        onBodyChange={v => updateActiveTab({ body: v })}
                        onBodyTypeChange={v => updateActiveTab({ bodyType: v })}
                      />
                      {activeTab.error && (
                        <Alert variant="destructive" className="shrink-0 mx-4 mb-2">
                          <AlertDescription>{activeTab.error}</AlertDescription>
                        </Alert>
                      )}
                    </div>
                  </ResizablePanel>
                  <ResizableHandle withHandle />
                  <ResizablePanel
                    id="response"
                    defaultSize={55}
                    minSize={25}
                    collapsible
                    collapsedSize={0}
                    panelRef={responsePanelRef}
                  >
                    <div className="h-full min-h-0">
                      {activeEntry && <DetailPanel entry={activeEntry} showRequest={false} />}
                    </div>
                  </ResizablePanel>
                </ResizablePanelGroup>
              </div>
            )}
          </div>
        )}
      </ResizablePanel>
    </ResizablePanelGroup>
  )
}
