// src/features/new-request/useRequestTabs.ts
import { useState, useCallback } from 'react'
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
    authType: node.authType ?? '',
    authData: node.authData ?? '',
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
    authType: '',
    authData: '',
    responseEntryId: null,
    sending: false,
    error: '',
  }
}

export function useRequestTabs() {
  const [tabs, setTabs] = useState<RequestTab[]>([])
  const [activeTabId, setActiveTabId] = useState<string | null>(null)

  // --- openTab ---
  const openTab = useCallback((linkedNodeId: string | null, nodeData?: ApiRequestNode) => {
    if (linkedNodeId !== null) {
      // use functional updater to avoid stale-closure race on dedup
      let alreadyOpen = false
      setTabs(prev => {
        const existing = prev.find(t => t.linkedNodeId === linkedNodeId)
        if (existing) {
          alreadyOpen = true
        }
        return prev
      })
      if (alreadyOpen) {
        // re-acquire the tab id from current state to activate it
        setTabs(prev => {
          const existing = prev.find(t => t.linkedNodeId === linkedNodeId)
          if (existing) setActiveTabId(existing.id)
          return prev
        })
        return
      }
    }

    const tab: RequestTab = linkedNodeId !== null && nodeData
      ? createTabFromNode(nodeData)
      : createEmptyTab()

    setTabs(prev => [...prev, tab])
    setActiveTabId(tab.id)
  }, [])

  // --- closeTab ---
  const closeTab = useCallback((tabId: string) => {
    let nextActiveTabId: string | null = null

    setTabs(prev => {
      const idx = prev.findIndex(t => t.id === tabId)
      if (idx === -1) return prev

      const next = [...prev.slice(0, idx), ...prev.slice(idx + 1)]

      // 如果关闭的是 active tab，计算需要激活的相邻 tab
      if (tabId === activeTabId) {
        if (next.length === 0) {
          nextActiveTabId = null
        } else if (idx < next.length) {
          nextActiveTabId = next[idx].id     // 优先右侧
        } else {
          nextActiveTabId = next[next.length - 1].id // 左侧
        }
      }

      return next
    })

    // sync activeTabId outside the updater
    if (tabId === activeTabId) {
      setActiveTabId(nextActiveTabId)
    }
  }, [activeTabId])

  // --- activateTab ---
  const activateTab = useCallback((tabId: string) => {
    setActiveTabId(tabId)
  }, [])

  // --- updateActiveTab ---
  const updateActiveTab = useCallback((patch: Partial<RequestTab>, tabId?: string) => {
    const targetId = tabId ?? activeTabId

    setTabs(prev => {
      return prev.map(t => {
        if (t.id !== targetId) return t
        const updated = { ...t, ...patch }
        return updated
      })
    })
  }, [activeTabId])

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

  // --- 将未关联 tab 链接到树节点 ---
  const linkTabToNode = useCallback((tabId: string, node: ApiRequestNode) => {
    setTabs(prev =>
      prev.map(t => {
        if (t.id !== tabId) return t
        return { ...t, linkedNodeId: node.id, name: node.name }
      }),
    )
  }, [])

  // --- 同步树节点重命名到已打开的 tab ---
  const syncNodeRename = useCallback((nodeId: string, newName: string) => {
    setTabs(prev =>
      prev.map(t => {
        if (t.linkedNodeId !== nodeId) return t
        return { ...t, name: newName }
      }),
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
    linkTabToNode,
    syncNodeRename,
  }
}
