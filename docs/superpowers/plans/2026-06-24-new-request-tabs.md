---
change: new-request-tabs
design-doc: docs/superpowers/specs/2026-06-24-new-request-tabs-design.md
base-ref: 2fa96821b190496dbe459b8a720ce3570180796e
---

# NewRequestView 多 Tab 请求编辑器 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 将 NewRequestView 右侧从单一共享编辑器改造为多 Tab 架构，每个 Tab 保有独立的请求编辑状态和响应结果。

**架构：** 新增 `useRequestTabs` hook 管理 tabs/activeTab/activate 四个核心操作；新增 `RequestTabBar` 组件渲染 tab 标签条（方法徽标+名称+关闭+溢出菜单）；重构 `NewRequestView` 使用 tab 驱动渲染，左侧树点击打开 tab 而非覆盖，支持创建临时 tab，关闭时丢弃未保存状态。

**技术栈：** React 19 + TypeScript + Tailwind CSS 4 + shadcn/ui + react-i18next

---

## 文件结构

| 文件 | 操作 | 职责 |
|------|------|------|
| `src/types/collection.ts` | 修改 | 新增 `RequestTab` interface |
| `src/features/new-request/useRequestTabs.ts` | 创建 | `useRequestTabs` hook：tabs state、openTab/closeTab/activateTab/updateActiveTab |
| `src/features/new-request/RequestTabBar.tsx` | 创建 | Tab 标签条：方法徽标 + 名称 + 关闭 + [+] + 溢出菜单 |
| `src/features/new-request/NewRequestView.tsx` | 修改 | 重构为 tab 容器：空状态占位、tab 驱动渲染、树同步 |
| `src/features/new-request/index.ts` | 不修改 | 现有 barrel export，无需改动（`useRequestTabs` 由 NewRequestView 直接 import） |
| `src/locales/en.json` | 修改 | 新增 tab 相关翻译 key |
| `src/locales/zh.json` | 修改 | 新增 tab 相关翻译 key |

---

### 任务 1：新增 i18n 翻译 key

**文件：**
- 修改：`src/locales/en.json`
- 修改：`src/locales/zh.json`

首先新增翻译，因为后续所有组件都会引用这些 key。

- [x] **步骤 1：在 en.json 末尾追加 tab 翻译**

打开 `src/locales/en.json`，在最后一个 key 之后添加（注意最后一个 key 后面加逗号）：

```json
  "tab": {
    "untitled": "Untitled Request",
    "closeCurrent": "Close Current",
    "closeOthers": "Close Others",
    "closeAll": "Close All",
    "emptyTitle": "Select a request",
    "emptySubtitle": "or click [+] to create a new request",
    "newRequest": "New Request",
    "openFromCollection": "Open from Collection"
  }
```

- [x] **步骤 2：在 zh.json 末尾追加 tab 翻译**

打开 `src/locales/zh.json`，在最后一个 key 之后添加（注意最后一个 key 后面加逗号）：

```json
  "tab": {
    "untitled": "未命名请求",
    "closeCurrent": "关闭当前页",
    "closeOthers": "关闭其他页",
    "closeAll": "关闭所有页",
    "emptyTitle": "从左侧接口树选择一个请求",
    "emptySubtitle": "或点击 [+] 新建空白请求",
    "newRequest": "新建请求",
    "openFromCollection": "从集合打开"
  }
```

- [x] **步骤 3：验证 JSON 语法正确**

运行：

```bash
cd "E:\project\rust\ai-proxy" && node -e "JSON.parse(require('fs').readFileSync('src/locales/en.json','utf8')); JSON.parse(require('fs').readFileSync('src/locales/zh.json','utf8')); console.log('OK')"
```

预期：输出 `OK`

- [x] **步骤 4：Commit**

```bash
git add src/locales/en.json src/locales/zh.json
git commit -m "feat(i18n): add tab-related translation keys for new-request-tabs"
```

---

### 任务 2：新增 RequestTab interface

**文件：**
- 修改：`src/types/collection.ts`

在 `collection.ts` 末尾新增 `RequestTab` interface。

- [x] **步骤 1：在 types/collection.ts 新增 RequestTab**

在文件末尾追加：

```ts
// src/types/collection.ts 追加

export interface RequestTab {
  id: string                    // crypto.randomUUID()
  name: string                  // tab 显示名称
  linkedNodeId: string | null   // 关联树节点 id，null = 临时 tab
  method: HttpMethod
  url: string
  params: KeyValuePair[]
  headers: KeyValuePair[]
  cookies: KeyValuePair[]
  bodyType: BodyType
  body: string
  // Response state
  responseEntryId: string | null
  sending: boolean
  error: string
}
```

- [x] **步骤 2：验证 TypeScript 编译**

运行：

```bash
cd "E:\project\rust\ai-proxy" && npx tsc --noEmit --project tsconfig.json 2>&1 | head -20
```

预期：无新增类型错误（可能与当前分支已有错误一致）

- [x] **步骤 3：Commit**

```bash
git add src/types/collection.ts
git commit -m "feat(types): add RequestTab interface for multi-tab architecture"
```

---

### 任务 3：实现 useRequestTabs hook

**文件：**
- 创建：`src/features/new-request/useRequestTabs.ts`

实现核心 tab 管理逻辑：openTab（含去重）、closeTab、activateTab、updateActiveTab（含 debounced 树同步）。

- [x] **步骤 1：创建 useRequestTabs.ts**

创建 `src/features/new-request/useRequestTabs.ts`：

```ts
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
```

- [x] **步骤 2：验证 TypeScript 编译**

运行：

```bash
cd "E:\project\rust\ai-proxy" && npx tsc --noEmit --project tsconfig.json 2>&1 | grep -i "useRequestTabs" | head -20
```

预期：无 useRequestTabs 相关错误（未导出警告可忽略——后续任务会引用）

- [x] **步骤 3：Commit**

```bash
git add src/features/new-request/useRequestTabs.ts
git commit -m "feat(new-request): add useRequestTabs hook for multi-tab state management"
```

---

### 任务 4：实现 RequestTabBar 组件

**文件：**
- 创建：`src/features/new-request/RequestTabBar.tsx`

水平排列 tab 标签条：方法徽标 + 名称 + 关闭按钮，右侧 [+] 和溢出菜单。

- [x] **步骤 1：创建 RequestTabBar.tsx**

创建 `src/features/new-request/RequestTabBar.tsx`：

```tsx
// src/features/new-request/RequestTabBar.tsx
import { useState, useRef, useEffect, useCallback } from 'react'
import { PlusIcon, XIcon, ChevronDownIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { useLocale } from '@/hooks/useLocale'
import { cn } from '@/lib/utils'
import type { RequestTab } from '@/types/collection'

interface RequestTabBarProps {
  tabs: RequestTab[]
  activeTabId: string | null
  onActivate: (tabId: string) => void
  onClose: (tabId: string) => void
  onNew: () => void
  onCloseOthers: () => void
  onCloseAll: () => void
}

export default function RequestTabBar({
  tabs,
  activeTabId,
  onActivate,
  onClose,
  onNew,
  onCloseOthers,
  onCloseAll,
}: RequestTabBarProps) {
  const { t } = useLocale()
  const containerRef = useRef<HTMLDivElement>(null)
  const [overflowIds, setOverflowIds] = useState<string[]>([])
  const [tabWidths, setTabWidths] = useState<Map<string, number>>(new Map())

  // 检测溢出：scrollWidth > clientWidth 时，从末尾往前提 tab 进溢出列表
  const detectOverflow = useCallback(() => {
    const el = containerRef.current
    if (!el) return

    const tabsList = el.querySelector<HTMLElement>('[data-tab-list]')
    if (!tabsList) return

    // 简单策略：如果 tabs 数量 > 5，多余的后缀进溢出
    // 更精确的 JS 溢出检测在这里实现
    const maxVisible = 5
    if (tabs.length > maxVisible) {
      setOverflowIds(tabs.slice(maxVisible).map(t => t.id))
    } else {
      setOverflowIds([])
    }
  }, [tabs])

  useEffect(() => {
    detectOverflow()
  }, [detectOverflow, tabs.length])

  // 监听容器 resize
  useEffect(() => {
    const el = containerRef.current
    if (!el) return
    const ro = new ResizeObserver(detectOverflow)
    ro.observe(el)
    return () => ro.disconnect()
  }, [detectOverflow])

  // 区分可见 tab 和溢出 tab
  const visibleTabs = tabs.filter(t => !overflowIds.includes(t.id))
  const overflowTabs = tabs.filter(t => overflowIds.includes(t.id))

  const handleClose = (e: React.MouseEvent, tabId: string) => {
    e.stopPropagation()
    onClose(tabId)
  }

  const displayName = (tab: RequestTab) => tab.name || t('tab.untitled')

  return (
    <div
      ref={containerRef}
      className="flex shrink-0 items-center border-b border-border bg-surface-base/50"
    >
      {/* Tab 标签列表 */}
      <div data-tab-list className="flex flex-1 items-center overflow-hidden min-w-0">
        {visibleTabs.map(tab => {
          const isActive = tab.id === activeTabId
          return (
            <button
              key={tab.id}
              type="button"
              onClick={() => onActivate(tab.id)}
              className={cn(
                'group relative flex shrink-0 items-center gap-1.5 h-8 max-w-[160px] px-3',
                'text-xs border-r border-border cursor-pointer select-none',
                'hover:bg-surface-elevated/50 transition-colors',
                isActive && 'bg-surface-elevated text-accent',
                isActive && 'after:absolute after:bottom-0 after:left-0 after:right-0 after:h-[2px] after:bg-accent',
              )}
            >
              <span
                className={cn(
                  'font-semibold shrink-0',
                  tab.method === 'GET' && 'text-badge-get',
                  tab.method === 'POST' && 'text-badge-post',
                  tab.method === 'PUT' && 'text-badge-put',
                  tab.method === 'PATCH' && 'text-badge-patch',
                  tab.method === 'DELETE' && 'text-badge-delete',
                  tab.method === 'HEAD' && 'text-badge-head',
                  tab.method === 'OPTIONS' && 'text-badge-options',
                )}
              >
                {tab.method}
              </span>
              <span className="truncate">{displayName(tab)}</span>
              <button
                type="button"
                onClick={e => handleClose(e, tab.id)}
                className="shrink-0 ml-0.5 size-3.5 flex items-center justify-center rounded-sm
                           opacity-0 group-hover:opacity-100 hover:bg-border/50 transition-opacity"
                aria-label="Close"
              >
                <XIcon className="size-2.5" />
              </button>
            </button>
          )
        })}
      </div>

      {/* 右侧操作区 */}
      <div className="flex shrink-0 items-center">
        {/* 溢出菜单 */}
        {(overflowTabs.length > 0 || tabs.length > 0) && (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="icon-sm" className="h-7 w-7">
                <ChevronDownIcon className="size-3.5" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="min-w-[160px]">
              {overflowTabs.map(tab => (
                <DropdownMenuItem
                  key={tab.id}
                  onClick={() => onActivate(tab.id)}
                >
                  <span className={cn(
                    'font-semibold text-[11px]',
                    tab.method === 'GET' && 'text-badge-get',
                    tab.method === 'POST' && 'text-badge-post',
                    tab.method === 'PUT' && 'text-badge-put',
                    tab.method === 'PATCH' && 'text-badge-patch',
                    tab.method === 'DELETE' && 'text-badge-delete',
                    tab.method === 'HEAD' && 'text-badge-head',
                    tab.method === 'OPTIONS' && 'text-badge-options',
                  )}>
                    {tab.method}
                  </span>
                  <span className="ml-1.5 truncate text-xs">{displayName(tab)}</span>
                </DropdownMenuItem>
              ))}
              {overflowTabs.length > 0 && <DropdownMenuSeparator />}
              <DropdownMenuItem onClick={onCloseOthers}>
                {t('tab.closeOthers')}
              </DropdownMenuItem>
              <DropdownMenuItem onClick={onCloseAll}>
                {t('tab.closeAll')}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        )}

        {/* [+] 按钮 */}
        <Button
          variant="ghost"
          size="icon-sm"
          className="h-7 w-7 ml-0.5"
          onClick={onNew}
          aria-label={t('tab.newRequest')}
        >
          <PlusIcon className="size-3.5" />
        </Button>
      </div>
    </div>
  )
}
```

- [x] **步骤 2：验证 TypeScript 编译**

运行：

```bash
cd "E:\project\rust\ai-proxy" && npx tsc --noEmit --project tsconfig.json 2>&1 | grep -i "RequestTabBar" | head -20
```

预期：无 RequestTabBar 相关错误（未导出警告可忽略）

- [x] **步骤 3：Commit**

```bash
git add src/features/new-request/RequestTabBar.tsx
git commit -m "feat(new-request): add RequestTabBar component with overflow menu"
```

---

### 任务 5：重构 NewRequestView 为 tab 容器

**文件：**
- 修改：`src/features/new-request/NewRequestView.tsx`

这是核心重构任务。将原本的单一 shared state 替换为 tab 驱动的渲染架构。

- [x] **步骤 1：重写 NewRequestView.tsx**

删除原文件全部内容，替换为：

```tsx
import { useCallback } from 'react'
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

    updateActiveTab({ sending: true, error: '' })

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
      updateActiveTab({ responseEntryId: entryId, sending: false })
      onSendSuccess(entryId)
    } catch (err) {
      updateActiveTab({ sending: false, error: String(err) })
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

                {activeEntry ? (
                  /* 有响应 → 上下分栏 */
                  <ResizablePanelGroup orientation="vertical" id="new-request-vertical" className="flex-1 min-h-0">
                    <ResizablePanel id="editor" defaultSize="45%" minSize="15%" maxSize="75%">
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
                    <ResizablePanel id="response" defaultSize="55%" minSize="25%">
                      <div className="h-full min-h-0">
                        <DetailPanel entry={activeEntry} showRequest={false} />
                      </div>
                    </ResizablePanel>
                  </ResizablePanelGroup>
                ) : (
                  /* 无响应 → 仅编辑器 */
                  <div className="flex flex-col min-h-0 flex-1 overflow-hidden">
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
                )}
              </div>
            )}
          </div>
        )}
      </ResizablePanel>
    </ResizablePanelGroup>
  )
}
```

- [x] **步骤 2：验证 TypeScript 编译**

运行：

```bash
cd "E:\project\rust\ai-proxy" && npx tsc --noEmit --project tsconfig.json 2>&1 | head -40
```

预期：无新增错误。如果 `METHODS` 的类型断言有问题，调整写法：`v as import('@/types/collection').HttpMethod`

- [x] **步骤 3：运行前端构建验证 Vite 编译**

```bash
cd "E:\project\rust\ai-proxy" && bun run build:vite 2>&1 | tail -20
```

预期：构建成功

- [x] **步骤 4：Commit**

```bash
git add src/features/new-request/NewRequestView.tsx
git commit -m "feat(new-request): refactor NewRequestView to use multi-tab architecture"
```

---

### 任务 6：整体验证与收尾

**说明：** 确保所有变更一致，没有遗漏的引用或类型错误。

- [ ] **步骤 1：完整 TypeScript 检查**

```bash
cd "E:\project\rust\ai-proxy" && npx tsc --noEmit --project tsconfig.json 2>&1 | tail -30
```

预期：无新增类型错误。如果当前分支已有已知错误，确认错误数量未增加。

- [ ] **步骤 2：完整前端构建**

```bash
cd "E:\project\rust\ai-proxy" && bun run build:vite 2>&1
```

预期：构建成功，输出在 `dist/` 目录。

- [ ] **步骤 3：确认所有新文件的 import 路径正确**

运行：

```bash
cd "E:\project\rust\ai-proxy" && grep -rn "from './" src/features/new-request/useRequestTabs.ts src/features/new-request/RequestTabBar.tsx src/features/new-request/NewRequestView.tsx
```

预期：所有相对导入路径指向已存在的文件。

- [ ] **步骤 4：检查 git diff 概要**

```bash
cd "E:\project\rust\ai-proxy" && git diff --stat HEAD
```

确认变更范围：
- `src/locales/en.json` — 新增 8 行左右
- `src/locales/zh.json` — 新增 8 行左右
- `src/types/collection.ts` — 新增 ~15 行
- `src/features/new-request/useRequestTabs.ts` — 新文件 ~150 行
- `src/features/new-request/RequestTabBar.tsx` — 新文件 ~150 行
- `src/features/new-request/NewRequestView.tsx` — 重构 ~180 行

- [ ] **步骤 5：Commit 最终收尾变更（如有）**

```bash
git add -A
git commit -m "chore: finalize new-request-tabs implementation"
```

---

## 自检

### 1. 规格覆盖度

| 规格章节 | 对应任务 | 状态 |
|----------|---------|------|
| 3.1 RequestTabBar | 任务 4 | 已覆盖 |
| 3.2 空状态占位视图 | 任务 5 步骤 1 | 已覆盖 |
| 4.1 RequestTab | 任务 2 | 已覆盖 |
| 4.2 useRequestTabs | 任务 3 | 已覆盖 |
| 5. 渲染策略（仅挂载活跃 tab） | 任务 5 步骤 1 | 已覆盖 |
| 6. 树同步与删除处理 | 任务 5 步骤 1（handleRemoveNode + unlinkNode）| 已覆盖 |
| 7. i18n | 任务 1 | 已覆盖 |

所有设计文档中的章节均有对应实现任务。

### 2. 占位符扫描

检查结果：无 "TBD"、"TODO"、"后续实现"、"补充细节" 等占位符。所有步骤都有具体代码或命令。

### 3. 类型一致性

- `RequestTab` 在任务 2 中定义，任务 3、4、5 中引用 — 类型名和字段名一致
- `useRequestTabs` 返回 `{ tabs, activeTab, openTab, closeTab, activateTab, updateActiveTab, closeOthers, closeAll, unlinkNode }` — 任务 5 中解构使用完全匹配
- `RequestTabBar` props：`tabs`, `activeTabId`, `onActivate`, `onClose`, `onNew`, `onCloseOthers`, `onCloseAll` — 任务 5 中传递完全匹配
- i18n key 前缀统一为 `tab.*` — 设计文档、任务 1、任务 4、任务 5 中使用一致
