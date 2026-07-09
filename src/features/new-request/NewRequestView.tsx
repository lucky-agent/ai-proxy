// src/features/new-request/NewRequestView.tsx
import { useCallback, useRef, useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { SendIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '@/components/ui/dialog'
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
import type { EnvItem } from './RequestTabBar'
import { useRequestTabs } from './useRequestTabs'
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable'
import { usePanelRef } from 'react-resizable-panels'
import { SaveToCollectionDialog } from './SaveToCollectionDialog'
import { CurlImportDialog } from './CurlImportDialog'
import type { CurlParsedResultOk } from '@/lib/curl'
import type { ApiRequestNode, KeyValuePair } from '@/types/collection'
import type { TrafficEntry } from '@/types/proxy'
import type { DetailPosition } from '@/features/bottom-bar'

interface NewRequestViewProps {
  onSendSuccess: (entryId: string) => void
  entries: TrafficEntry[]
  showSidebar: boolean
  detailPosition: DetailPosition
}

const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'] as const

function serializeCookies(cookies: KeyValuePair[]): string | null {
  const filled = cookies.filter(c => c.key.trim())
  if (filled.length === 0) return null
  return filled.map(c => `${c.key.trim()}=${c.value}`).join('; ')
}

export function NewRequestView({ onSendSuccess, entries, showSidebar, detailPosition }: NewRequestViewProps) {
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
    loadCollections,
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
    linkTabToNode,
    syncNodeRename,
    markTabClean,
  } = useRequestTabs()

  // 左侧树点击 request → 打开 tab
  const handleSelectRequest = useCallback((node: ApiRequestNode) => {
    openTab(node.id, node)
  }, [openTab])

  // 新建节点（从树右键） → 立即创建并进入重命名
  const [renamingId, setRenamingId] = useState<number | null>(null)
  const [envs, setEnvs] = useState<EnvItem[]>([
    { id: 'production', name: '', urlPrefix: '' },
    { id: 'test', name: '', urlPrefix: '' },
  ])
  const [env, setEnv] = useState<string>('production')
  const handleAddRequest = useCallback((parentId: number) => {
    addRequest(parentId).then(newNodeId => {
      if (newNodeId != null) {
        setRenamingId(newNodeId)
        // Auto-open the new request tab
        const newNode: ApiRequestNode = {
          id: newNodeId,
          type: 'request',
          name: 'New Request',
          method: 'GET',
          url: '',
          params: [],
          headers: [],
          cookies: [],
          bodyType: 'json',
          body: '',
        }
        openTab(newNodeId, newNode)
      }
    })
  }, [addRequest, openTab])
  const handleAddFolder = useCallback((parentId: number) => {
    addFolder(parentId).then(newNodeId => {
      if (newNodeId != null) {
        setRenamingId(newNodeId)
      }
    })
  }, [addFolder])

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

    // Check if cancelled before invoke
    const controller = new AbortController()
    cancelRef.current = controller

    try {
      const entryId = await invoke<string>('resend_request', {
        method: activeTab.method,
        url: finalUrl,
        headers: headerMap,
        body: activeTab.body || null,
      })
      // Ignore result if cancelled
      if (cancelRef.current?.signal.aborted) return
      updateActiveTab({ responseEntryId: entryId, sending: false, error: '' }, sendingTabId)
      onSendSuccess(entryId)
    } catch (err) {
      if (cancelRef.current?.signal.aborted) return
      updateActiveTab({ sending: false, error: String(err) }, sendingTabId)
    } finally {
      if (cancelRef.current === controller) {
        cancelRef.current = null
      }
    }
  }, [activeTab, updateActiveTab, onSendSuccess])

  // Save-to-collection dialog state (for unlinked tabs)
  const [saveDialogOpen, setSaveDialogOpen] = useState(false)
  // cURL import dialog state
  const [curlDialogOpen, setCurlDialogOpen] = useState(false)
  // Brief "saved" feedback for the save button
  const [saveFeedback, setSaveFeedback] = useState(false)
  const saveFeedbackTimer = useRef<ReturnType<typeof setTimeout>>(null)

  // Close confirmation dialog for dirty tabs
  const [closeConfirmOpen, setCloseConfirmOpen] = useState(false)
  const pendingCloseRef = useRef<{ tabId: string }>({ tabId: '' })

  // Intercept tab close — check dirty before closing (skip if tab is empty)
  const handleRequestClose = useCallback((tabId: string) => {
    const tab = tabs.find(t => t.id === tabId)
    if (tab?.dirty) {
      // Check if tab has any meaningful content
      const hasUrl = tab.url.trim().length > 0
      const hasBody = tab.body.trim().length > 0
      const hasParams = tab.params.some(p => p.key.trim() || p.value.trim())
      const hasHeaders = tab.headers.some(h => h.key.trim() || h.value.trim())
      const hasCookies = tab.cookies.some(c => c.key.trim() || c.value.trim())
      if (hasUrl || hasBody || hasParams || hasHeaders || hasCookies) {
        pendingCloseRef.current = { tabId }
        setCloseConfirmOpen(true)
        return
      }
    }
    closeTab(tabId)
  }, [tabs, closeTab])

  // 保存到集合（已关联 → 直接保存；未关联 → 弹窗选父节点）
  const handleSave = useCallback(() => {
    if (!activeTab) return
    if (activeTab.linkedNodeId != null) {
      updateRequest(activeTab.linkedNodeId, {
        method: activeTab.method,
        url: activeTab.url,
        params: activeTab.params.filter(p => p.key.trim()),
        headers: activeTab.headers.filter(h => h.key.trim()),
        cookies: activeTab.cookies.filter(c => c.key.trim()),
        bodyType: activeTab.bodyType,
        body: activeTab.body,
      })
      // Brief visual feedback
      setSaveFeedback(true)
      if (saveFeedbackTimer.current) clearTimeout(saveFeedbackTimer.current)
      saveFeedbackTimer.current = setTimeout(() => setSaveFeedback(false), 1200)
      // Clear dirty flag
      markTabClean(activeTab.id)
      // Reload collections after debounced save to ensure DB durability
      setTimeout(() => loadCollections(), 500)
    } else {
      setSaveDialogOpen(true)
    }
  }, [activeTab, updateRequest, markTabClean, loadCollections])

  // Save-and-close from confirmation dialog
  const handleSaveAndClose = useCallback(() => {
    handleSave()
    closeTab(pendingCloseRef.current.tabId)
    setCloseConfirmOpen(false)
  }, [handleSave, closeTab])

  // Discard-and-close from confirmation dialog
  const handleDiscardAndClose = useCallback(() => {
    closeTab(pendingCloseRef.current.tabId)
    setCloseConfirmOpen(false)
  }, [closeTab])

  // Async wrapper for addFolder — returns nodeId (not optimistic local id)
  const addFolderAsync = useCallback(
    (parentId: number, name: string): Promise<number | null> =>
      invoke<string>('create_folder', { parentId, name }).then(id => {
        loadCollections()
        return Number(id)
      }).catch(err => { console.error(err); return null }),
    [loadCollections],
  )

  // cURL 导入弹窗确认：创建 dirty tab（不自动保存）
  const handleImportCurl = useCallback((result: CurlParsedResultOk) => {
    const headerPairs: KeyValuePair[] = Object.entries(result.headers).map(([k, v]) => ({ key: k, value: v }))

    // 从 URL 中提取 query params
    const params: KeyValuePair[] = []
    let cleanUrl = result.url
    const qIdx = result.url.indexOf('?')
    if (qIdx > 0) {
      cleanUrl = result.url.substring(0, qIdx)
      const qs = result.url.substring(qIdx + 1)
      for (const pair of qs.split('&')) {
        const eqIdx = pair.indexOf('=')
        if (eqIdx > 0) {
          params.push({ key: decodeURIComponent(pair.substring(0, eqIdx)), value: decodeURIComponent(pair.substring(eqIdx + 1)) })
        }
      }
    }

    // 创建 dirty tab（linkedNodeId=null，用户手动 Save 保存到集合）
    openTab(null, {
      id: Date.now(),
      type: 'request',
      name: cleanUrl || result.url,
      method: (result.method as typeof METHODS[number]) || 'GET',
      url: cleanUrl || result.url,
      params,
      headers: headerPairs,
      cookies: [],
      bodyType: 'json',
      body: result.body ?? '',
    })
  }, [openTab])

  // 弹窗确认：创建树节点 + 保存数据 + link tab
  const handleSaveToCollection = useCallback(async (parentId: number, collectionId: number, requestName: string) => {
    if (!activeTab) return
    const tabId = activeTab.id

    try {
      const resultJson = await invoke<string>('create_request', {
        parentId,
        collectionId,
        name: requestName || activeTab.name || '未命名请求',
      })
      const { nodeId, requestId: newRequestId } = JSON.parse(resultJson) as { nodeId: number; requestId: number }

      // Build the new ApiRequestNode matching what the tree expects
      const newNode: ApiRequestNode = {
        id: nodeId,
        type: 'request',
        name: requestName || activeTab.name || '未命名请求',
        method: activeTab.method,
        url: activeTab.url,
        params: activeTab.params,
        headers: activeTab.headers,
        cookies: activeTab.cookies,
        bodyType: activeTab.bodyType,
        body: activeTab.body,
        authType: activeTab.authType,
        authData: activeTab.authData,
        requestId: newRequestId,
      }

      linkTabToNode(tabId, newNode)
      const headers = activeTab.headers.filter(h => h.key.trim())
      const params = activeTab.params.filter(p => p.key.trim())
      const cookies = activeTab.cookies.filter(c => c.key.trim())
      // Direct DB write using requestId (not through updateRequest which needs existing tree node)
      invoke('save_request', {
        id: newRequestId,
        method: activeTab.method,
        url: activeTab.url,
        headers,
        params,
        cookies,
        body: activeTab.body,
        bodyType: activeTab.bodyType,
        authType: activeTab.authType,
        authData: activeTab.authData,
      }).then(() => loadCollections()).catch(console.error)
      markTabClean(tabId)
    } catch (err) {
      console.error(err)
    }
  }, [activeTab, linkTabToNode, markTabClean, loadCollections])

  // 树节点重命名 → 同步到已打开的 tab 名称
  const handleRenameNode = useCallback((nodeId: number, newName: string) => {
    renameNode(nodeId, newName)
    syncNodeRename(nodeId, newName)
  }, [renameNode, syncNodeRename])

  // 树节点删除 → 关闭关联 tab 并从树中移除
  const handleRemoveNode = useCallback((nodeId: number) => {
    // Close any open tab linked to this node
    const linkedTab = tabs.find(t => t.linkedNodeId === nodeId)
    if (linkedTab) {
      closeTab(linkedTab.id)
    }
    removeNode(nodeId)
  }, [tabs, removeNode, closeTab])

  // 根据 activeTab.responseEntryId 查找 TrafficEntry
  const activeEntry = activeTab?.responseEntryId
    ? entries.find(e => e.id === activeTab.responseEntryId)
    : undefined

  // 控制左侧集合面板的折叠/展开
  const collectionPanelRef = usePanelRef()
  useEffect(() => {
    const panel = collectionPanelRef.current
    if (!panel) return
    if (showSidebar) {
      panel.resize("22%")
    } else {
      panel.collapse()
    }
  }, [showSidebar])

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

  // Abort controller for cancelling in-flight request
  const cancelRef = useRef<AbortController | null>(null)
  const handleCancel = useCallback(() => {
    cancelRef.current?.abort()
    cancelRef.current = null
    updateActiveTab({ sending: false })
  }, [updateActiveTab])

  // Clean up save feedback timer
  useEffect(() => {
    return () => {
      if (saveFeedbackTimer.current) clearTimeout(saveFeedbackTimer.current)
    }
  }, [])

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center bg-surface-deep text-muted-foreground text-xs">
        {t('settings.loading')}
      </div>
    )
  }

  return (
    <>
      <ResizablePanelGroup orientation="horizontal" id="new-request" className="h-full bg-surface-deep">
      {/* Left: API collection panel */}
      <ResizablePanel id="collection" defaultSize="22%" minSize="15%" maxSize="40%" collapsible collapsedSize={0} panelRef={collectionPanelRef}>
        <div className="h-full overflow-hidden">
          <ApiCollectionPanel
            collections={collections}
            selectedId={activeTab?.linkedNodeId ?? null}
            renamingId={renamingId}
            onClearRenamingId={() => setRenamingId(null)}
            onSelectRequest={handleSelectRequest}
            addFolder={handleAddFolder}
            addRequest={handleAddRequest}
            removeNode={handleRemoveNode}
            renameNode={handleRenameNode}
            duplicateRequest={duplicateRequest}
            onImportCurl={(_parentId) => setCurlDialogOpen(true)}
            renameCollection={renameCollection}
            onRefresh={loadCollections}
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
              <Button variant="outline" size="sm" onClick={() => openTab()}>
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
              env={env}
              envs={envs}
              onEnvChange={setEnv}
              onEnvsChange={setEnvs}
              onActivate={activateTab}
              onClose={handleRequestClose}
              onNew={() => openTab()}
              onCloseOthers={closeOthers}
              onCloseAll={closeAll}
            />

            {/* 活跃 tab 内容（仅挂载 active tab） */}
            {activeTab && (
              <div className="flex flex-col min-h-0 h-full overflow-hidden">
                <div className="relative flex shrink-0 items-center gap-2 px-4 py-2 border-b border-border bg-surface-base/50">
                  <InputGroup className="flex-1">
                    <InputGroupAddon align="inline-start" className="py-0 pl-0">
                      <Select
                        value={activeTab.method}
                        onValueChange={v => updateActiveTab({ method: v as typeof METHODS[number] })}
                        disabled={activeTab.sending}
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
                      disabled={activeTab.sending}
                    />
                  </InputGroup>
                  <Button
                    onClick={handleSave}
                    variant="outline"
                    size="sm"
                    disabled={activeTab.sending}
                  >
                    {t('settings.save')}
                    {saveFeedback && (
                      <svg className="size-3 animate-spin ml-1" viewBox="0 0 24 24" fill="none">
                        <circle cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="3" strokeDasharray="52" strokeDashoffset="16" strokeLinecap="round" />
                      </svg>
                    )}
                  </Button>
                  <Button onClick={handleSend} disabled={activeTab.sending || !activeTab.url.trim()} size="sm">
                    <SendIcon className="size-3.5" />
                    {activeTab.sending ? '...' : t('sendRequest.send')}
                  </Button>
                </div>

                {/* Render the editor/response split based on detailPosition */}
                <div className="relative flex flex-col min-h-0 h-full overflow-hidden">
                  {detailPosition === 'hidden' ? (
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
                    </div>
                  ) : detailPosition === 'bottom' ? (
                    <ResizablePanelGroup orientation="vertical" id="new-request-vertical" className="flex-1 min-h-0">
                      <ResizablePanel id="editor" defaultSize={activeEntry ? "60%" : "100%"} minSize="15%" maxSize={activeEntry ? "80%" : "100%"}>
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
                        </div>
                      </ResizablePanel>
                      <ResizableHandle withHandle />
                      <ResizablePanel
                        id="response"
                        defaultSize="40%"
                        minSize="10%"
                        collapsible
                        collapsedSize="0%"
                        panelRef={responsePanelRef}
                      >
                        <div className="h-full min-h-0">
                          {activeEntry && <DetailPanel entry={activeEntry} showRequest={false} />}
                        </div>
                      </ResizablePanel>
                    </ResizablePanelGroup>
                  ) : (
                    <ResizablePanelGroup orientation="horizontal" id="new-request-horizontal" className="flex-1 min-h-0">
                      <ResizablePanel id="editor" defaultSize={activeEntry ? "60%" : "100%"} minSize="15%" maxSize={activeEntry ? "80%" : "100%"}>
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
                        </div>
                      </ResizablePanel>
                      <ResizableHandle withHandle />
                      <ResizablePanel
                        id="response"
                        defaultSize="40%"
                        minSize="10%"
                        collapsible
                        collapsedSize="0%"
                        panelRef={responsePanelRef}
                      >
                        <div className="h-full min-h-0 min-w-0">
                          {activeEntry && <DetailPanel entry={activeEntry} showRequest={false} />}
                        </div>
                      </ResizablePanel>
                    </ResizablePanelGroup>
                  )}

                  {/* 发送中遮罩层 */}
                  {activeTab.sending && (
                    <div className="absolute inset-0 z-50">
                      <div className="absolute inset-0 bg-background/60 backdrop-blur-[1px]" />
                      <div className="absolute inset-0 flex flex-col items-center justify-center gap-3">
                        <svg className="size-7 animate-spin text-foreground/40" viewBox="0 0 24 24" fill="none">
                          <circle cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="3" strokeDasharray="52" strokeDashoffset="16" strokeLinecap="round" />
                        </svg>
                        <Button variant="ghost" size="sm" onClick={handleCancel} className="text-xs text-destructive">
                          {t('sendRequest.cancel')}
                        </Button>
                      </div>
                    </div>
                  )}
                </div>
              </div>
            )}
          </div>
        )}
      </ResizablePanel>
    </ResizablePanelGroup>

    {/* Save-to-collection dialog for unlinked tabs */}
    <SaveToCollectionDialog
      open={saveDialogOpen}
      onOpenChange={setSaveDialogOpen}
      collections={collections}
      initialRequestName={activeTab?.name || activeTab?.url || ''}
      addFolder={addFolderAsync}
      onConfirm={handleSaveToCollection}
    />

    {/* cURL import dialog */}
    <CurlImportDialog
      open={curlDialogOpen}
      onOpenChange={setCurlDialogOpen}
      onConfirm={handleImportCurl}
    />

    {/* Close confirmation dialog for dirty tabs */}
    <Dialog open={closeConfirmOpen} onOpenChange={setCloseConfirmOpen}>
      <DialogContent className="sm:max-w-[340px]">
        <DialogHeader>
          <DialogTitle>{t('tab.unsavedTitle')}</DialogTitle>
          <DialogDescription>{t('tab.unsavedDesc')}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={() => setCloseConfirmOpen(false)}>
            {t('settings.cancel')}
          </Button>
          <Button variant="ghost" onClick={handleDiscardAndClose}>
            {t('tab.discardClose')}
          </Button>
          <Button onClick={handleSaveAndClose}>
            {t('tab.saveClose')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
    </>
  )
}
