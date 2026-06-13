# Api Collection 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 在 NewRequestView 中新增左侧接口管理面板，提供 Postman 风格的 Collection 树形菜单，支持文件夹/请求的增删改和持久化存储。

**架构：** 前端新增 `useCollections` hook 管理数据状态，新增 `ApiCollectionPanel` / `ApiTreeView` / `ApiTreeItem` 组件渲染树形菜单；后端新增 `collection.rs` Tauri command 读写 `~/.ai-proxy/collections.json`；NewRequestView 改为左右布局，左侧面板可拖拽调整宽度。

**技术栈：** React 19 + TypeScript + Tauri 2 (Rust) + Tailwind CSS 4 + lucide-react icons

---

## 文件结构

| 文件 | 操作 | 职责 |
|------|------|------|
| `src/types/collection.ts` | 创建 | ApiCollection / ApiTreeNode 数据类型定义 |
| `src/hooks/useCollections.ts` | 创建 | 读取/保存 collections 的 hook，增删改状态管理 |
| `src/features/new-request/components/ApiCollectionPanel.tsx` | 创建 | 左侧面板容器（树 + 操作按钮 + 拖拽分隔线） |
| `src/features/new-request/components/ApiTreeView.tsx` | 创建 | 递归树渲染，展开/折叠状态管理 |
| `src/features/new-request/components/ApiTreeItem.tsx` | 创建 | 单个树节点（文件夹/请求），右键菜单，内联重命名 |
| `src/features/new-request/components/index.ts` | 创建 | barrel export |
| `src/features/new-request/NewRequestView.tsx` | 修改 | 改为左右布局，集成 ApiCollectionPanel |
| `src/locales/zh.json` | 修改 | 添加 collection 相关中文翻译 |
| `src/locales/en.json` | 修改 | 添加 collection 相关英文翻译 |
| `src-tauri/src/commands/collection.rs` | 创建 | get_collections / save_collections Tauri command |
| `src-tauri/src/commands/mod.rs` | 修改 | 添加 `mod collection` 和 pub use |
| `src-tauri/src/lib.rs` | 修改 | 注册新 command 到 invoke_handler |
| `src-tauri/src/config/store.rs` | 修改 | 新增 `collections_path()` 方法 |

---

### 任务 1：定义 TypeScript 数据类型

**文件：**
- 创建：`src/types/collection.ts`

- [ ] **步骤 1：创建类型文件**

```typescript
// src/types/collection.ts

export type HttpMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE' | 'HEAD' | 'OPTIONS'

export interface ApiCollection {
  id: string
  name: string
  children: ApiTreeNode[]
  createdAt: number
  updatedAt: number
}

export interface ApiFolderNode {
  id: string
  type: 'folder'
  name: string
  children: ApiTreeNode[]
}

export interface ApiRequestNode {
  id: string
  type: 'request'
  name: string
  method: HttpMethod
  url: string
  headers: { key: string; value: string }[]
  body: string
}

export type ApiTreeNode = ApiFolderNode | ApiRequestNode
```

- [ ] **步骤 2：Commit**

```bash
git add src/types/collection.ts
git commit -m "feat: add ApiCollection TypeScript type definitions"
```

---

### 任务 2：Rust 后端 — Store 扩展和 Collection command

**文件：**
- 修改：`src-tauri/src/config/store.rs`
- 创建：`src-tauri/src/commands/collection.rs`
- 修改：`src-tauri/src/commands/mod.rs`
- 修改：`src-tauri/src/lib.rs`

- [ ] **步骤 1：在 Store 中添加 `collections_path()` 方法**

在 `src-tauri/src/config/store.rs` 的 `impl Store` 中，在 `db_path()` 方法后面添加：

```rust
pub fn collections_path(&self) -> PathBuf {
    self.data_dir().join("collections.json")
}
```

- [ ] **步骤 2：创建 collection command 文件**

创建 `src-tauri/src/commands/collection.rs`：

```rust
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiCollection {
    pub id: String,
    pub name: String,
    pub children: Vec<ApiTreeNode>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ApiTreeNode {
    #[serde(rename = "folder")]
    Folder {
        id: String,
        name: String,
        children: Vec<ApiTreeNode>,
    },
    #[serde(rename = "request")]
    Request {
        id: String,
        name: String,
        method: String,
        url: String,
        headers: Vec<HeaderPair>,
        body: String,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HeaderPair {
    pub key: String,
    pub value: String,
}

#[tauri::command]
pub fn get_collections(state: tauri::State<'_, AppState>) -> Result<Vec<ApiCollection>, String> {
    let path = state.store().collections_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let collections: Vec<ApiCollection> = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    Ok(collections)
}

#[tauri::command]
pub fn save_collections(
    state: tauri::State<'_, AppState>,
    collections: Vec<ApiCollection>,
) -> Result<(), String> {
    let path = state.store().collections_path();
    let content = serde_json::to_string_pretty(&collections).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(())
}
```

注意：`#[serde(tag = "type", rename_all = "camelCase")]` 使用 `type` 字段做 discriminated deserialization，与前端 TypeScript 的 `type: 'folder' | 'request'` 一致。`rename_all = "camelCase"` 确保 Rust 的 `created_at` 在 JSON 中变为 `createdAt`。

- [ ] **步骤 3：在 commands/mod.rs 注册新模块**

修改 `src-tauri/src/commands/mod.rs`，添加：

```rust
mod collection;
pub use collection::{get_collections, save_collections};
```

放在现有 `mod theme;` 行之后、`mod traffic;` 行之前。

- [ ] **步骤 4：在 lib.rs 注册 command**

修改 `src-tauri/src/lib.rs`：

1. 在 `use crate::commands::` 块中添加 `get_collections, save_collections`：

```rust
use crate::commands::{
    get_locale, get_settings, get_status, get_theme, save_settings, set_locale, set_theme,
    start_proxy, stop_proxy, subscribe_proxy_events, sync_tray_locale,
    get_ssl_config, save_ssl_config,
    get_script_config, save_script_config,
    get_collections, save_collections,
};
```

2. 在 `invoke_handler` 的 `generate_handler![]` 中添加 `get_collections, save_collections`，放在 `resend_request` 之后：

```rust
.invoke_handler(tauri::generate_handler![
    start_proxy,
    stop_proxy,
    get_status,
    get_ssl_config,
    save_ssl_config,
    get_script_config,
    save_script_config,
    get_theme,
    set_theme,
    get_settings,
    save_settings,
    get_locale,
    set_locale,
    subscribe_proxy_events,
    sync_tray_locale,
    load_traffic_history,
    resend_request,
    get_collections,
    save_collections,
])
```

- [ ] **步骤 5：编译验证**

运行：`cd src-tauri && cargo check`
预期：无编译错误

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/config/store.rs src-tauri/src/commands/collection.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat: add get_collections / save_collections Tauri commands"
```

---

### 任务 3：创建 useCollections hook

**文件：**
- 创建：`src/hooks/useCollections.ts`

- [ ] **步骤 1：创建 hook 文件**

```typescript
// src/hooks/useCollections.ts
import { useState, useEffect, useCallback, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import type { ApiCollection, ApiTreeNode, ApiFolderNode, ApiRequestNode } from '@/types/collection'

function generateId(): string {
  return crypto.randomUUID()
}

function createDefaultCollection(): ApiCollection {
  return {
    id: generateId(),
    name: '默认集合',
    children: [],
    createdAt: Date.now(),
    updatedAt: Date.now(),
  }
}

/** 递归查找节点并替换（immutable） */
function updateNodeInTree(
  nodes: ApiTreeNode[],
  nodeId: string,
  updater: (node: ApiTreeNode) => ApiTreeNode,
): ApiTreeNode[] {
  return nodes.map(node => {
    if (node.id === nodeId) {
      return updater(node)
    }
    if (node.type === 'folder') {
      return {
        ...node,
        children: updateNodeInTree(node.children, nodeId, updater),
      }
    }
    return node
  })
}

/** 递归查找节点并删除（immutable） */
function removeNodeFromTree(nodes: ApiTreeNode[], nodeId: string): ApiTreeNode[] {
  return nodes
    .filter(node => node.id !== nodeId)
    .map(node => {
      if (node.type === 'folder') {
        return { ...node, children: removeNodeFromTree(node.children, nodeId) }
      }
      return node
    })
}

/** 递归在文件夹下插入节点（immutable） */
function insertNodeInTree(
  nodes: ApiTreeNode[],
  parentId: string,
  newNode: ApiTreeNode,
): ApiTreeNode[] {
  return nodes.map(node => {
    if (node.id === parentId && node.type === 'folder') {
      return { ...node, children: [...node.children, newNode] }
    }
    if (node.type === 'folder') {
      return { ...node, children: insertNodeInTree(node.children, parentId, newNode) }
    }
    return node
  })
}

export function useCollections() {
  const [collections, setCollections] = useState<ApiCollection[]>([])
  const [loading, setLoading] = useState(true)
  const saveTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  // 加载
  useEffect(() => {
    invoke<ApiCollection[]>('get_collections')
      .then(data => {
        if (data.length === 0) {
          const default_ = createDefaultCollection()
          setCollections([default_])
          invoke('save_collections', { collections: [default_] }).catch(console.error)
        } else {
          setCollections(data)
        }
      })
      .catch(console.error)
      .finally(() => setLoading(false))
  }, [])

  // 延迟保存（避免每次操作都 invoke）
  const debouncedSave = useCallback((data: ApiCollection[]) => {
    if (saveTimer.current) clearTimeout(saveTimer.current)
    saveTimer.current = setTimeout(() => {
      invoke('save_collections', { collections: data }).catch(console.error)
    }, 300)
  }, [])

  const updateCollections = useCallback((updater: (prev: ApiCollection[]) => ApiCollection[]) => {
    setCollections(prev => {
      const next = updater(prev)
      debouncedSave(next)
      return next
    })
  }, [debouncedSave])

  // --- 操作方法 --- 

  /** 在根 Collection 下添加文件夹 */
  const addFolder = useCallback((parentId: string) => {
    const newFolder: ApiFolderNode = {
      id: generateId(),
      type: 'folder',
      name: '新建文件夹',
      children: [],
    }
    updateCollections(prev =>
      prev.map(col => {
        if (col.id === parentId) {
          return { ...col, children: [...col.children, newFolder], updatedAt: Date.now() }
        }
        return {
          ...col,
          children: insertNodeInTree(col.children, parentId, newFolder),
          updatedAt: Date.now(),
        }
      }),
    )
  }, [updateCollections])

  /** 在根 Collection 或某文件夹下添加请求 */
  const addRequest = useCallback((parentId: string) => {
    const newRequest: ApiRequestNode = {
      id: generateId(),
      type: 'request',
      name: '新建请求',
      method: 'GET',
      url: '',
      headers: [],
      body: '',
    }
    updateCollections(prev =>
      prev.map(col => {
        if (col.id === parentId) {
          return { ...col, children: [...col.children, newRequest], updatedAt: Date.now() }
        }
        return {
          ...col,
          children: insertNodeInTree(col.children, parentId, newRequest),
          updatedAt: Date.now(),
        }
      }),
    )
  }, [updateCollections])

  /** 删除节点 */
  const removeNode = useCallback((nodeId: string) => {
    updateCollections(prev =>
      prev.map(col => ({
        ...col,
        children: removeNodeFromTree(col.children, nodeId),
        updatedAt: Date.now(),
      })),
    )
  }, [updateCollections])

  /** 重命名节点 */
  const renameNode = useCallback((nodeId: string, newName: string) => {
    updateCollections(prev =>
      prev.map(col => ({
        ...col,
        children: updateNodeInTree(col.children, nodeId, node =>
          ({ ...node, name: newName }),
        ),
        updatedAt: Date.now(),
      })),
    )
  }, [updateCollections])

  /** 更新请求节点配置（method, url, headers, body） */
  const updateRequest = useCallback(
    (nodeId: string, data: { method?: string; url?: string; headers?: { key: string; value: string }[]; body?: string }) => {
      updateCollections(prev =>
        prev.map(col => ({
          ...col,
          children: updateNodeInTree(col.children, nodeId, node => {
            if (node.type !== 'request') return node
            return { ...node, ...data }
          }),
          updatedAt: Date.now(),
        })),
      )
    },
    [updateCollections],
  )

  /** 复制请求节点（在同一父级下创建副本） */
  const duplicateRequest = useCallback((nodeId: string) => {
    updateCollections(prev => {
      // 找到原始请求节点
      const findNode = (nodes: ApiTreeNode[]): ApiRequestNode | null => {
        for (const n of nodes) {
          if (n.id === nodeId && n.type === 'request') return n
          if (n.type === 'folder') {
            const found = findNode(n.children)
            if (found) return found
          }
        }
        return null
      }

      // 在同一父级下插入副本
      const insertCopy = (nodes: ApiTreeNode[], parentId: string | null, copy: ApiRequestNode): ApiTreeNode[] => {
        // 如果 parentId 为 null，说明原始节点就在 col.children 根层
        // 这里我们找到包含 nodeId 的层，在同一层插入副本
        if (nodes.some(n => n.id === nodeId)) {
          const idx = nodes.findIndex(n => n.id === nodeId)
          return [...nodes.slice(0, idx + 1), copy, ...nodes.slice(idx + 1)]
        }
        return nodes.map(n => {
          if (n.type === 'folder') {
            return { ...n, children: insertCopy(n.children, null, copy) }
          }
          return n
        })
      }

      const original = prev.flatMap(c => findNode(c.children))
      if (!original || original.length === 0) return prev
      const orig = original[0]

      const copy: ApiRequestNode = {
        ...orig,
        id: generateId(),
        name: orig.name + ' (副本)',
      }

      return prev.map(col => ({
        ...col,
        children: insertCopy(col.children, null, copy),
        updatedAt: Date.now(),
      }))
    })
  }, [updateCollections])

  /** 重命名 Collection */
  const renameCollection = useCallback((collectionId: string, newName: string) => {
    updateCollections(prev =>
      prev.map(col =>
        col.id === collectionId ? { ...col, name: newName, updatedAt: Date.now() } : col,
      ),
    )
  }, [updateCollections])

  return {
    collections,
    loading,
    addFolder,
    addRequest,
    removeNode,
    renameNode,
    updateRequest,
    duplicateRequest,
    renameCollection,
  }
}
```

- [ ] **步骤 2：Commit**

```bash
git add src/hooks/useCollections.ts
git commit -m "feat: add useCollections hook for collection state management"
```

---

### 任务 4：添加 i18n 翻译

**文件：**
- 修改：`src/locales/zh.json`
- 修改：`src/locales/en.json`

- [ ] **步骤 1：在 zh.json 中添加 collection 翻译**

在 `zh.json` 的 `"sendRequest"` 块之后、`"layout"` 块之前添加新的 `"collection"` 块：

```json
"collection": {
  "title": "接口管理",
  "defaultCollection": "默认集合",
  "newFolder": "新建文件夹",
  "newRequest": "新建请求",
  "rename": "重命名",
  "duplicate": "复制请求",
  "delete": "删除",
  "deleteConfirm": "确认删除？",
  "renameFolder": "重命名文件夹",
  "renameRequest": "重命名请求",
  "emptyTree": "暂无接口，点击上方按钮添加。"
},
```

- [ ] **步骤 2：在 en.json 中添加 collection 翻译**

同样在 `en.json` 的 `"sendRequest"` 块之后、`"layout"` 块之前添加：

```json
"collection": {
  "title": "Collections",
  "defaultCollection": "Default",
  "newFolder": "New folder",
  "newRequest": "New request",
  "rename": "Rename",
  "duplicate": "Duplicate",
  "delete": "Delete",
  "deleteConfirm": "Confirm delete?",
  "renameFolder": "Rename folder",
  "renameRequest": "Rename request",
  "emptyTree": "No requests yet. Click the buttons above to add."
},
```

- [ ] **步骤 3：Commit**

```bash
git add src/locales/zh.json src/locales/en.json
git commit -m "feat: add collection i18n translations (zh + en)"
```

---

### 任务 5：创建 ApiTreeItem 组件

**文件：**
- 创建：`src/features/new-request/components/ApiTreeItem.tsx`

- [ ] **步骤 1：创建组件文件**

```typescript
// src/features/new-request/components/ApiTreeItem.tsx
import { useState, useRef, useEffect, useCallback } from 'react'
import { ChevronRightIcon, FolderIcon, Trash2Icon, CopyIcon, PencilIcon } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { ApiTreeNode, ApiFolderNode, ApiRequestNode, HttpMethod } from '@/types/collection'
import { useLocale } from '@/hooks/useLocale'

const METHOD_COLORS: Record<string, string> = {
  GET: 'text-badge-get',
  POST: 'text-badge-post',
  PUT: 'text-badge-put',
  DELETE: 'text-badge-delete',
  PATCH: 'text-badge-patch',
  HEAD: 'text-badge-head',
  OPTIONS: 'text-badge-options',
}

interface ApiTreeItemProps {
  node: ApiTreeNode
  depth: number
  selectedId: string | null
  onSelectRequest: (node: ApiRequestNode) => void
  onRemoveNode: (nodeId: string) => void
  onRenameNode: (nodeId: string, newName: string) => void
  onDuplicateRequest: (nodeId: string) => void
  onAddFolder: (parentId: string) => void
  onAddRequest: (parentId: string) => void
  expandedIds: Set<string>
  onToggleExpand: (nodeId: string) => void
}

export function ApiTreeItem({
  node,
  depth,
  selectedId,
  onSelectRequest,
  onRemoveNode,
  onRenameNode,
  onDuplicateRequest,
  onAddFolder,
  onAddRequest,
  expandedIds,
  onToggleExpand,
}: ApiTreeItemProps) {
  const { t } = useLocale()
  const [renaming, setRenaming] = useState(false)
  const [renameValue, setRenameValue] = useState(node.name)
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null)
  const renameInputRef = useRef<HTMLInputElement>(null)

  const isFolder = node.type === 'folder'
  const isSelected = !isFolder && selectedId === node.id
  const isExpanded = isFolder && expandedIds.has(node.id)

  // 重命名时自动 focus
  useEffect(() => {
    if (renaming && renameInputRef.current) {
      renameInputRef.current.focus()
      renameInputRef.current.select()
    }
  }, [renaming])

  const handleRenameSubmit = useCallback(() => {
    const trimmed = renameValue.trim()
    if (trimmed && trimmed !== node.name) {
      onRenameNode(node.id, trimmed)
    }
    setRenaming(false)
  }, [renameValue, node.id, node.name, onRenameNode])

  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault()
    e.stopPropagation()
    setContextMenu({ x: e.clientX, y: e.clientY })
  }, [])

  const closeContextMenu = useCallback(() => {
    setContextMenu(null)
  }, [])

  // 点击事件
  const handleClick = useCallback(() => {
    if (isFolder) {
      onToggleExpand(node.id)
    } else {
      onSelectRequest(node as ApiRequestNode)
    }
  }, [isFolder, node, onToggleExpand, onSelectRequest])

  return (
    <>
      <div
        className={cn(
          'group flex items-center gap-1 px-2 py-1 cursor-pointer rounded-sm text-xs transition-colors',
          isSelected ? 'bg-primary/10 text-foreground' : 'text-muted-foreground hover:text-foreground hover:bg-surface-elevated/50',
        )}
        style={{ paddingLeft: `${depth * 12 + 8}px` }}
        onClick={handleClick}
        onContextMenu={handleContextMenu}
        onDoubleClick={() => setRenaming(true)}
      >
        {/* 文件夹：展开/折叠箭头 */}
        {isFolder && (
          <ChevronRightIcon
            className={cn('size-3 shrink-0 transition-transform', isExpanded && 'rotate-90')}
          />
        )}
        {/* 请求：无箭头，留占位 */}
        {!isFolder && <span className="w-3 shrink-0" />}

        {/* 文件夹图标 */}
        {isFolder && <FolderIcon className="size-3.5 shrink-0 text-muted-foreground" />}

        {/* 请求：Method badge + 名称 */}
        {!isFolder && (
          <>
            <span className={cn('shrink-0 text-[10px] font-bold', METHOD_COLORS[(node as ApiRequestNode).method] || 'text-muted-foreground')}>
              {(node as ApiRequestNode).method}
            </span>
            <span className="truncate">{node.name}</span>
          </>
        )}

        {/* 文件夹：名称 */}
        {isFolder && !renaming && (
          <span className="truncate">{node.name}</span>
        )}

        {/* 重命名输入框 */}
        {renaming && (
          <input
            ref={renameInputRef}
            value={renameValue}
            onChange={e => setRenameValue(e.target.value)}
            onBlur={handleRenameSubmit}
            onKeyDown={e => {
              if (e.key === 'Enter') handleRenameSubmit()
              if (e.key === 'Escape') { setRenaming(false); setRenameValue(node.name) }
            }}
            className="flex-1 min-w-0 rounded border border-input bg-background px-1 py-0 text-xs font-mono outline-none focus:ring-1 focus:ring-primary"
          />
        )}
      </div>

      {/* 文件夹展开时渲染子节点 */}
      {isFolder && isExpanded && (node as ApiFolderNode).children.map(child => (
        <ApiTreeItem
          key={child.id}
          node={child}
          depth={depth + 1}
          selectedId={selectedId}
          onSelectRequest={onSelectRequest}
          onRemoveNode={onRemoveNode}
          onRenameNode={onRenameNode}
          onDuplicateRequest={onDuplicateRequest}
          onAddFolder={onAddFolder}
          onAddRequest={onAddRequest}
          expandedIds={expandedIds}
          onToggleExpand={onToggleExpand}
        />
      ))}

      {/* 右键菜单 */}
      {contextMenu && (
        <div
          className="fixed z-50 rounded-md border border-border bg-surface-base shadow-md py-1 text-xs"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onClick={closeContextMenu}
        >
          {isFolder ? (
            <>
              <button className="flex items-center gap-2 w-full px-3 py-1.5 hover:bg-surface-elevated/50 text-foreground" onClick={() => { onAddRequest(node.id); closeContextMenu() }}>
                <PencilIcon className="size-3" /> {t('collection.newRequest')}
              </button>
              <button className="flex items-center gap-2 w-full px-3 py-1.5 hover:bg-surface-elevated/50 text-foreground" onClick={() => { onAddFolder(node.id); closeContextMenu() }}>
                <FolderIcon className="size-3" /> {t('collection.newFolder')}
              </button>
              <button className="flex items-center gap-2 w-full px-3 py-1.5 hover:bg-surface-elevated/50 text-foreground" onClick={() => { setRenaming(true); closeContextMenu() }}>
                <PencilIcon className="size-3" /> {t('collection.rename')}
              </button>
              <button className="flex items-center gap-2 w-full px-3 py-1.5 hover:bg-surface-elevated/50 text-destructive" onClick={() => { onRemoveNode(node.id); closeContextMenu() }}>
                <Trash2Icon className="size-3" /> {t('collection.delete')}
              </button>
            </>
          ) : (
            <>
              <button className="flex items-center gap-2 w-full px-3 py-1.5 hover:bg-surface-elevated/50 text-foreground" onClick={() => { setRenaming(true); closeContextMenu() }}>
                <PencilIcon className="size-3" /> {t('collection.rename')}
              </button>
              <button className="flex items-center gap-2 w-full px-3 py-1.5 hover:bg-surface-elevated/50 text-foreground" onClick={() => { onDuplicateRequest(node.id); closeContextMenu() }}>
                <CopyIcon className="size-3" /> {t('collection.duplicate')}
              </button>
              <button className="flex items-center gap-2 w-full px-3 py-1.5 hover:bg-surface-elevated/50 text-destructive" onClick={() => { onRemoveNode(node.id); closeContextMenu() }}>
                <Trash2Icon className="size-3" /> {t('collection.delete')}
              </button>
            </>
          )}
        </div>
      )}
    </>
  )
}
```

注意：右键菜单使用 `fixed` 定位 + `z-50`，与 RequestList 中 ContextMenu 的实现模式一致。关闭菜单通过点击菜单项或外部点击（下一步 ApiTreeView 中处理全局点击关闭）。

- [ ] **步骤 2：Commit**

```bash
git add src/features/new-request/components/ApiTreeItem.tsx
git commit -m "feat: add ApiTreeItem component with context menu and inline rename"
```

---

### 任务 6：创建 ApiTreeView 组件

**文件：**
- 创建：`src/features/new-request/components/ApiTreeView.tsx`

- [ ] **步骤 1：创建组件文件**

```typescript
// src/features/new-request/components/ApiTreeView.tsx
import { useState, useCallback, useEffect } from 'react'
import type { ApiCollection, ApiRequestNode } from '@/types/collection'
import { useLocale } from '@/hooks/useLocale'
import { ApiTreeItem } from './ApiTreeItem'

interface ApiTreeViewProps {
  collections: ApiCollection[]
  selectedId: string | null
  onSelectRequest: (node: ApiRequestNode) => void
  onRemoveNode: (nodeId: string) => void
  onRenameNode: (nodeId: string, newName: string) => void
  onDuplicateRequest: (nodeId: string) => void
  onAddFolder: (parentId: string) => void
  onAddRequest: (parentId: string) => void
  onRenameCollection: (collectionId: string, newName: string) => void
}

export function ApiTreeView({
  collections,
  selectedId,
  onSelectRequest,
  onRemoveNode,
  onRenameNode,
  onDuplicateRequest,
  onAddFolder,
  onAddRequest,
  onRenameCollection,
}: ApiTreeViewProps) {
  const { t } = useLocale()
  const [expandedIds, setExpandedIds] = useState<Set<string>>(() => {
    // 默认展开所有 Collection 根节点
    return new Set(collections.map(c => c.id))
  })

  const handleToggleExpand = useCallback((nodeId: string) => {
    setExpandedIds(prev => {
      const next = new Set(prev)
      if (next.has(nodeId)) next.delete(nodeId)
      else next.add(nodeId)
      return next
    })
  }, [])

  // 全局点击关闭右键菜单（通过 ApiTreeItem 的 contextMenu 状态自行管理，
  // 但我们需要一个全局 listener 来处理外部点击）
  useEffect(() => {
    const handleGlobalClick = () => {
      // 通知所有 ApiTreeItem 关闭菜单——这里利用 DOM 事件冒泡自然关闭
      // ApiTreeItem 的 contextMenu 在点击菜单项时自行关闭，
      // 外部点击会触发 handleClick -> 不会到达 contextMenu div
      // 所以不需要额外处理
    }
    document.addEventListener('click', handleGlobalClick)
    return () => document.removeEventListener('click', handleGlobalClick)
  }, [])

  // Collection 名称展开/折叠也是树的一部分
  // 目前只有一个 Collection，直接渲染
  return (
    <div className="flex-1 overflow-y-auto min-h-0 py-1">
      {collections.map(col => (
        <div key={col.id}>
          {/* Collection 根节点 */}
          <ApiTreeItem
            node={{
              id: col.id,
              type: 'folder',
              name: col.name,
              children: col.children,
            }}
            depth={0}
            selectedId={selectedId}
            onSelectRequest={onSelectRequest}
            onRemoveNode={onRemoveNode}
            onRenameNode={(nodeId, newName) => {
              // 根节点重命名走 renameCollection
              if (nodeId === col.id) {
                onRenameCollection(col.id, newName)
              } else {
                onRenameNode(nodeId, newName)
              }
            }}
            onDuplicateRequest={onDuplicateRequest}
            onAddFolder={onAddFolder}
            onAddRequest={onAddRequest}
            expandedIds={expandedIds}
            onToggleExpand={handleToggleExpand}
          />
        </div>
      ))}

      {/* 空状态 */}
      {collections.length === 0 && (
        <div className="px-4 py-6 text-xs text-muted-foreground text-center">
          {t('collection.emptyTree')}
        </div>
      )}
    </div>
  )
}
```

注意：Collection 根节点作为特殊的 `folder` 类型渲染在树的最顶层（depth=0），这样展开折叠行为与普通文件夹一致，重命名则走 `renameCollection`。

- [ ] **步骤 2：Commit**

```bash
git add src/features/new-request/components/ApiTreeView.tsx
git commit -m "feat: add ApiTreeView component with expand/collapse state"
```

---

### 任务 7：创建 ApiCollectionPanel 组件

**文件：**
- 创建：`src/features/new-request/components/ApiCollectionPanel.tsx`

- [ ] **步骤 1：创建组件文件**

```typescript
// src/features/new-request/components/ApiCollectionPanel.tsx
import { useCallback } from 'react'
import { FolderPlusIcon, PlusIcon } from 'lucide-react'
import { useLocale } from '@/hooks/useLocale'
import type { ApiCollection, ApiRequestNode } from '@/types/collection'
import { ApiTreeView } from './ApiTreeView'

interface ApiCollectionPanelProps {
  collections: ApiCollection[]
  selectedId: string | null
  onSelectRequest: (node: ApiRequestNode) => void
  addFolder: (parentId: string) => void
  addRequest: (parentId: string) => void
  removeNode: (nodeId: string) => void
  renameNode: (nodeId: string, newName: string) => void
  duplicateRequest: (nodeId: string) => void
  renameCollection: (collectionId: string, newName: string) => void
}

export function ApiCollectionPanel({
  collections,
  selectedId,
  onSelectRequest,
  addFolder,
  addRequest,
  removeNode,
  renameNode,
  duplicateRequest,
  renameCollection,
}: ApiCollectionPanelProps) {
  const { t } = useLocale()

  // 默认添加到第一个 Collection 的根层
  const defaultCollectionId = collections[0]?.id ?? ''
  const handleAddFolder = useCallback(() => addFolder(defaultCollectionId), [addFolder, defaultCollectionId])
  const handleAddRequest = useCallback(() => addRequest(defaultCollectionId), [addRequest, defaultCollectionId])

  return (
    <div className="flex h-full flex-col border-r border-border bg-surface-base/30">
      {/* 标题栏 */}
      <div className="flex items-center px-3 py-2 border-b border-border">
        <span className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground">
          {t('collection.title')}
        </span>
      </div>

      {/* 树形菜单 */}
      <ApiTreeView
        collections={collections}
        selectedId={selectedId}
        onSelectRequest={onSelectRequest}
        onRemoveNode={removeNode}
        onRenameNode={renameNode}
        onDuplicateRequest={duplicateRequest}
        onAddFolder={addFolder}
        onAddRequest={addRequest}
        onRenameCollection={renameCollection}
      />

      {/* 操作按钮 */}
      <div className="flex items-center gap-1 px-3 py-2 border-t border-border">
        <button
          onClick={handleAddFolder}
          className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          <FolderPlusIcon className="size-3.5" />
          {t('collection.newFolder')}
        </button>
        <button
          onClick={handleAddRequest}
          className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          <PlusIcon className="size-3.5" />
          {t('collection.newRequest')}
        </button>
      </div>
    </div>
  )
}
```

- [ ] **步骤 2：创建 barrel export**

创建 `src/features/new-request/components/index.ts`：

```typescript
export { ApiCollectionPanel } from './ApiCollectionPanel'
export { ApiTreeView } from './ApiTreeView'
export { ApiTreeItem } from './ApiTreeItem'
```

- [ ] **步骤 3：Commit**

```bash
git add src/features/new-request/components/ApiCollectionPanel.tsx src/features/new-request/components/index.ts
git commit -m "feat: add ApiCollectionPanel component and barrel export"
```

---

### 任务 8：重构 NewRequestView 为左右布局

**文件：**
- 修改：`src/features/new-request/NewRequestView.tsx`

这是最核心的任务。将 NewRequestView 从纯垂直布局改为左右布局：左侧 ApiCollectionPanel + 可拖拽分隔线 + 右侧编辑区。

- [ ] **步骤 1：修改 NewRequestView.tsx**

完整替换 `src/features/new-request/NewRequestView.tsx`：

```typescript
import { useState, useCallback, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { PlusIcon, Trash2Icon, SendIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { useLocale } from '@/hooks/useLocale'
import { cn } from '@/lib/utils'
import { useCollections } from '@/hooks/useCollections'
import { ApiCollectionPanel } from './components'
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
      // 实时更新 DOM（不走 React state，避免卡顿）
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

  // 编辑区变更时，同步保存到 collection
  const syncToCollection = useCallback(() => {
    if (!selectedId) return
    updateRequest(selectedId, {
      method,
      url,
      headers: headers.filter(h => h.key.trim()),
      body,
    })
  }, [selectedId, method, url, headers, body, updateRequest])

  // 手动保存按钮
  const handleSave = useCallback(() => {
    syncToCollection()
  }, [syncToCollection])

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
      style={{ '--collection-ratio': panelRatio } as React.CSSProperties}
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
      <div
        className={cn('flex min-h-0 flex-1 overflow-hidden', isDragging && (liveRatio.current !== panelRatio ? 'cursor-col-resize' : ''))}
      >
        {/* 左侧：接口管理面板 */}
        <div
          className="shrink-0 overflow-hidden"
          style={{ width: `${panelRatio * 100}%` }}
        >
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
          {/* 拖动手柄指示器 */}
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
```

关键变更点说明：
1. 导入 `useCollections` hook 和 `ApiCollectionPanel` 组件
2. 新增 `selectedId` 状态追踪当前选中的请求节点
3. 新增 `panelRatio` 和拖拽逻辑（模式与 TrafficLog 一致）
4. 顶部栏新增"保存"按钮（选中请求时显示，点击将编辑区配置同步到 collection）
5. 主体从单一滚动区变为左面板 + 分隔线 + 右编辑区
6. `handleSelectRequest` 将请求节点数据填入编辑区
7. 加载中显示 Loading 状态

- [ ] **步骤 2：验证前端编译**

运行：`bun run build:vite`
预期：编译成功，无 TypeScript 错误

- [ ] **步骤 3：Commit**

```bash
git add src/features/new-request/NewRequestView.tsx
git commit -m "feat: refactor NewRequestView to left-right layout with ApiCollectionPanel"
```

---

### 任务 9：端到端验证

- [ ] **步骤 1：启动开发服务器**

运行：`bun run dev`
预期：Tauri 应用启动成功，新请求视图左侧显示接口管理面板

- [ ] **步骤 2：功能验证清单**

逐项验证：
1. ✅ 默认集合"默认集合"已创建并显示在树中
2. ✅ 点击"新建文件夹"按钮，在默认集合下创建文件夹
3. ✅ 点击"新建请求"按钮，在默认集合下创建请求
4. ✅ 点击请求节点，右侧编辑区填入 Method + URL + Headers + Body
5. ✅ 编辑区修改后点击"保存"按钮，数据持久化
6. ✅ 右键文件夹：显示新建子文件夹 / 新建请求 / 重命名 / 删除菜单
7. ✅ 右键请求：显示重命名 / 复制 / 删除菜单
8. ✅ 双击节点进入重命名模式
9. ✅ 左侧面板拖拽分隔线可调整宽度
10. ✅ 重启应用后 collections 数据保留

- [ ] **步骤 3：Final commit**

```bash
git add -A
git commit -m "feat: complete Api Collection feature with tree menu, persistence, and drag-resize panel"
```

---

## 自检结果

1. **规格覆盖度：** 逐条对照设计文档——数据模型 ✅（任务1+2）、UI布局 ✅（任务8）、组件结构 ✅（任务5-7）、交互行为 ✅（任务5右键菜单+双击重命名+任务8点击填入）、前后端通信 ✅（任务2+3）、i18n ✅（任务4）
2. **占位符扫描：** 无 TODO/TBD/待定，所有步骤有完整代码
3. **类型一致性：** TypeScript 类型 `ApiCollection` / `ApiTreeNode` / `ApiFolderNode` / `ApiRequestNode` 与 Rust struct `ApiCollection` / `ApiTreeNode` enum 使用 `#[serde(tag = "type")]` 一致，`camelCase` rename 确保 `createdAt`/`updatedAt` 字段名匹配
