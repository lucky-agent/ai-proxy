---
comet_change: new-request-tabs
role: technical-design
canonical_spec: openspec
archived-with: 2026-06-24-new-request-tabs
status: final
---

# NewRequestView 多 Tab 请求编辑器

## 1. 概述

将 NewRequestView 右侧从单一共享编辑器改造为多 Tab 架构。每个 Tab 保有独立的请求编辑状态（method、url、params、headers、cookies、body、bodyType）和响应结果。左侧接口树点击 request 时打开 Tab 而非覆盖，支持创建临时 Tab，Tab 关闭时丢弃未保存状态。

## 2. 架构

```
NewRequestView
├── ResizablePanelGroup (horizontal)
│   ├── 左侧: ApiCollectionPanel (22%) — 不变
│   │
│   └── 右侧: Tab 容器 (78%)
│       ├── RequestTabBar                          [NEW]
│       │   ├── Tab 标签列表 (自适应宽度 + overflow 截断)
│       │   │   └── [方法徽标] [名称] [✕]
│       │   ├── [+] 按钮
│       │   └── [...] 溢出菜单
│       │       ├── 溢出的 tab 列表
│       │       ├── ──────────────
│       │       ├── 关闭当前页
│       │       ├── 关闭其他页
│       │       └── 关闭所有页
│       │
│       ├── URL 栏 + 方法选择器 + Send 按钮 (activeTab 驱动)
│       ├── RequestEditor (受控，接收 activeTab 数据)
│       └── DetailPanel / 响应区 (activeTab.responseEntryId 驱动)
│
│   或: 空状态占位视图 (无 tab 时)
```

## 3. 组件设计

### 3.1 RequestTabBar

位置：右侧面板顶部，URL 栏上方。

布局：水平排列 tab 标签，右侧固定 [+] 按钮和 [...] 溢出菜单。

- Tab 标签：`[方法徽标] [名称] [✕]`，方法徽标使用 METHOD_COLORS 常量
- 活跃 tab：底部 2px accent 色下划线、背景色区分
- 宽度策略：内容自适应 (`fit-content`)，flex-shrink: 0，`max-width: 160px`，超出用 `text-overflow: ellipsis` 截断
- 溢出：JS 检测父容器溢出（`scrollWidth > clientWidth`），溢出 tab 移入 "..." 下拉菜单
- "..." 下拉：展示溢出 tab 列表 + 分隔线 + 关闭当前/关闭其他/关闭所有 三个操作
- [+] 按钮：调用 `openTab(null)` 创建临时 tab

### 3.2 空状态占位视图

条件：`tabs.length === 0`

```tsx
<div centered>
  <icon />
  <p>从左侧接口树选择一个请求</p>
  <p>或点击 [+] 新建空白请求</p>
  <button>+ 新建请求</button>
  <button>从集合打开</button>
</div>
```

两个按钮分别调用 `openTab(null)` 和聚焦左侧面板。

## 4. 数据模型

### 4.1 RequestTab

```ts
// src/types/collection.ts 新增
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

### 4.2 useRequestTabs Hook

```ts
// src/features/new-request/useRequestTabs.ts
export function useRequestTabs(
  updateRequest: (nodeId: string, data: Partial<ApiRequestNode>) => void
) {
  // State
  const [tabs, setTabs] = useState<RequestTab[]>([])
  const [activeTabId, setActiveTabId] = useState<string | null>(null)

  // Derived
  const activeTab = tabs.find(t => t.id === activeTabId)

  // 打开 tab：linkedNodeId 非 null 时先检查去重
  const openTab = (linkedNodeId: string | null, nodeData?: ApiRequestNode) => { ... }

  // 关闭 tab：移除；若为 active 则激活相邻；若为 last 则空状态
  const closeTab = (tabId: string) => { ... }

  // 激活 tab
  const activateTab = (tabId: string) => { ... }

  // 更新 active tab 内容；linkedNodeId 非 null 时 debounced 同步到树
  const updateActiveTab = (patch: Partial<RequestTab>) => { ... }

  return { tabs, activeTabId, activeTab, openTab, closeTab, activateTab, updateActiveTab }
}
```

**openTab 逻辑**：
1. 若 `linkedNodeId != null`，检查是否已有 `tabs.find(t => t.linkedNodeId === linkedNodeId)`
2. 有 → `activateTab(existingTab.id)`
3. 无 → 创建新 RequestTab（从 nodeData 或默认值填充），`setTabs` + `setActiveTabId`

**closeTab 逻辑**：
1. `setTabs` filter 掉该 tab
2. 若关闭的是 active：找相邻（优先右侧，其次左侧），否则 `setActiveTabId(null)`

**updateActiveTab 逻辑**：
1. `setTabs` 更新 active tab 数据
2. 若 `activeTab.linkedNodeId != null`：debounced (300ms) 调用 `updateRequest(linkedNodeId, patch)`

## 5. 渲染策略

**仅挂载活跃 tab**：`{activeTab && <Editor ... />}`。切换时卸载/重新挂载。优势：
- 避免 5+ 个编辑器实例同时存在导致的 DOM 膨胀
- RequestEditor 子 tab 状态回退到默认（params→body），可接受

## 6. 树同步与删除处理

### 链接 tab 同步

`updateActiveTab` → debounced `updateRequest(linkedNodeId, {...})` → `useCollections.updateRequest` → `debouncedSave` → Rust backend

### 树节点删除

当 `useCollections.removeNode` 被调用时，检查所有 tab 的 `linkedNodeId`：
- 匹配的 tab → `linkedNodeId = null`（变成临时 tab，数据不丢）
- 不关闭 tab，用户可继续编辑或手动关闭

此逻辑在 NewRequestView 中实现：调用 `removeNode` 后同步更新 `tabs`。

## 7. i18n

新增翻译 key 到 `en.json` 和 `zh.json`：

| Key | EN | ZH |
|-----|----|----|
| `tab.untitled` | Untitled Request | 未命名请求 |
| `tab.closeCurrent` | Close Current | 关闭当前页 |
| `tab.closeOthers` | Close Others | 关闭其他页 |
| `tab.closeAll` | Close All | 关闭所有页 |
| `tab.emptyTitle` | Select a request | 从左侧接口树选择一个请求 |
| `tab.emptySubtitle` | or click [+] to create a new request | 或点击 [+] 新建空白请求 |
| `tab.newRequest` | New Request | 新建请求 |
| `tab.openFromCollection` | Open from Collection | 从集合打开 |

## 8. 任务分解

| # | Task | 文件 |
|---|------|------|
| 1.1 | 新增 `RequestTab` interface | `src/types/collection.ts` |
| 1.2 | 实现 `useRequestTabs` hook | `src/features/new-request/useRequestTabs.ts` |
| 2.1 | 实现 `RequestTabBar` 组件 | `src/features/new-request/RequestTabBar.tsx` |
| 3.1 | 重构 `NewRequestView` 为 tab 容器 | `src/features/new-request/NewRequestView.tsx` |
| 3.2 | 空状态占位视图 | 同上 |
| 3.3 | 树节点删除 → tab 转临时 | 同上 |
| 4.1 | 新增 i18n key | `src/locales/en.json`, `src/locales/zh.json` |
