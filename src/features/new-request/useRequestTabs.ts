// src/features/new-request/useRequestTabs.ts
import { useState, useCallback, useRef } from 'react'
import type { RequestTab, ApiRequestNode } from '@/types/collection'

function makeTabId(): string {
  return crypto.randomUUID()
}

/** 从 ApiRequestNode 创建 RequestTab */
function createTabFromNode(node: ApiRequestNode): RequestTab {
  return {
    id: makeTabId(),
    name: node.name,
    linkedNodeId: node.id,
    method: node.method,
    url: node.url,
    params: node.params ?? [],
    headers: node.headers ?? [],
    cookies: node.cookies ?? [],
    bodyType: node.bodyType ?? 'json',
    body: node.body ?? '',
    responseEntryId: null,
    sending: false,
    error: '',
  }
}

/** 创建空白临时 tab */
function createEmptyTab(): RequestTab {
  return {
    id: makeTabId(),
    name: '',
    linkedNodeId: null,
    method: 'GET',
    url: '',
    params: [],
    headers: [],
    cookies: [],
    bodyType: 'json',
    body: '',
    responseEntryId: null,
    sending: false,
    error: '',
  }
}

export function useRequestTabs(
  updateRequest: (nodeId: string, data: Partial<ApiRequestNode>) => void,
) {
  const [tabs, setTabs] = useState<RequestTab[]>([])
  const [activeTabId, setActiveTabId] = useState<string | null>(null)
  const syncTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  // --- openTab ---
  const openTab = useCallback((linkedNodeId: string | null, nodeData?: ApiRequestNode) => {
    if (linkedNodeId !== null) {
      const existing = tabs.find(t => t.linkedNodeId === linkedNodeId)
      if (existing) {
        setActiveTabId(existing.id)
        return
      }
    }

    const tab: RequestTab = linkedNodeId !== null && nodeData
      ? createTabFromNode(nodeData)
      : createEmptyTab()

    setTabs(prev => [...prev, tab])
    setActiveTabId(tab.id)
  }, [tabs])

  // --- closeTab ---
  const closeTab = useCallback((tabId: string) => {
    setTabs(prev => {
      const idx = prev.findIndex(t => t.id === tabId)
      if (idx === -1) return prev

      const next = [...prev.slice(0, idx), ...prev.slice(idx + 1)]

      // 如果关闭的是 active tab，激活相邻 tab
      if (tabId === activeTabId) {
        if (next.length === 0) {
          setActiveTabId(null)
        } else if (idx < next.length) {
          setActiveTabId(next[idx].id)  // 优先右侧
        } else {
          setActiveTabId(next[next.length - 1].id) // 左侧
        }
      }

      return next
    })
  }, [activeTabId])

  // --- activateTab ---
  const activateTab = useCallback((tabId: string) => {
    setActiveTabId(tabId)
  }, [])

  // --- updateActiveTab ---
  const updateActiveTab = useCallback((patch: Partial<RequestTab>) => {
    setTabs(prev => {
      return prev.map(t => {
        if (t.id !== activeTabId) return t
        const updated = { ...t, ...patch }
        return updated
      })
    })

    // debounced 同步到树：300ms
    setTabs(prev => {
      const updated = prev.find(t => t.id === activeTabId)
      if (!updated || updated.linkedNodeId === null) return prev

      if (syncTimer.current) clearTimeout(syncTimer.current)
      syncTimer.current = setTimeout(() => {
        updateRequest(updated.linkedNodeId!, {
          method: updated.method,
          url: updated.url,
          params: updated.params,
          headers: updated.headers,
          cookies: updated.cookies,
          bodyType: updated.bodyType,
          body: updated.body,
        })
      }, 300)

      return prev
    })
  }, [activeTabId, updateRequest])

  // --- closeOthers / closeAll ---
  const closeOthers = useCallback(() => {
    if (!activeTabId) return
    setTabs(prev => prev.filter(t => t.id === activeTabId))
  }, [activeTabId])

  const closeAll = useCallback(() => {
    setTabs([])
    setActiveTabId(null)
  }, [])

  // --- 取消链接（树节点被删除时外部调用） ---
  const unlinkNode = useCallback((nodeId: string) => {
    setTabs(prev =>
      prev.map(t =>
        t.linkedNodeId === nodeId
          ? { ...t, linkedNodeId: null }
          : t,
      ),
    )
  }, [])

  // Derived
  const activeTab = activeTabId
    ? (tabs.find(t => t.id === activeTabId) ?? null)
    : null

  return {
    tabs,
    activeTabId,
    activeTab,
    openTab,
    closeTab,
    activateTab,
    updateActiveTab,
    closeOthers,
    closeAll,
    unlinkNode,
  }
}
