# 接口管理（Api Collection）设计文档

日期：2026-06-13

## 概述

在 NewRequestView 中新增左侧接口管理面板，提供 Postman 风格的 Collection 树形菜单。用户可创建文件夹和请求，分组管理保存的接口配置。首次启动时自动创建一个名为"默认集合"的空 Collection。

## 数据模型

```typescript
interface ApiCollection {
  id: string        // uuid
  name: string      // 如 "默认集合"
  children: ApiTreeNode[]
  createdAt: number
  updatedAt: number
}

interface ApiFolderNode {
  id: string
  type: 'folder'
  name: string
  children: ApiTreeNode[]
}

interface ApiRequestNode {
  id: string
  type: 'request'
  name: string
  method: HttpMethod  // GET | POST | PUT | PATCH | DELETE | HEAD | OPTIONS
  url: string
  headers: { key: string; value: string }[]
  body: string
}

type ApiTreeNode = ApiFolderNode | ApiRequestNode
```

默认数据：首次 `get_collections` 返回空时，前端创建一个 `{ id: uuid(), name: "默认集合", children: [], createdAt: now, updatedAt: now }`。

## UI 布局

NewRequestView 从纯垂直布局改为左右布局：

```
┌─────────────────────────────────────────────────────┐
│  [Method ▼]  [_____________URL_____________]  [Send] │  ← 顶部栏（不变）
├──────────────┬──────────────────────────────────────┤
│              │                                      │
│  接口管理     │  Headers                             │
│  ┌────────── │  [Key] [Value] [×]                   │
│  │ ▼ 默认集合│  [Key] [Value] [×]                   │
│  │   ▼ 用户  │  [+ Add header]                      │
│  │     POST 登│                                      │
│  │     GET 列│  Body                                 │
│  │   ▼ 订单  │  [textarea_______________]           │
│  │     POST 创│                                      │
│  │           │                                      │
│  └────────── │                                      │
│  [+ 新建文件夹]│                                      │
│  [+ 新建请求] │                                      │
│              │                                      │
├──────────────┴──────────────────────────────────────┤
```

左侧面板宽度可拖拽调整（与 TrafficLog 拖拽模式一致，比例范围 0.15–0.4）。

## 组件结构

```
features/new-request/
├── NewRequestView.tsx          # 主组件（改为左右布局）
├── components/
│   ├── ApiCollectionPanel.tsx  # 左侧面板容器（树 + 操作按钮）
│   ├── ApiTreeView.tsx         # 树形菜单渲染
│   ├── ApiTreeItem.tsx         # 单个树节点（文件夹/请求）
│   └── index.ts                # barrel export
├── index.ts
```

遵循 CLAUDE.md 组织原则：仅该区域用的子组件放在 `features/<区域>/components/`。

新增 Hook：

```
hooks/useCollections.ts  # 读取/保存 collections，增删改状态管理
```

## 交互行为

- **点击请求节点** → 提取 method/url/headers/body → 填入右侧编辑区
- **点击文件夹节点** → 展开/折叠子节点
- **右键请求** → 编辑名称 / 复制 / 删除
- **右键文件夹** → 重命名 / 新建子文件夹 / 新建请求 / 删除
- **树顶部** → "+ 新建文件夹" 和 "+ 新建请求" 按钮
- **请求节点显示** → Method badge + 名称（如 `POST 登录接口`）

## 前后端通信

### 新增 Tauri Commands

```rust
// src-tauri/src/commands/collection.rs
fn get_collections() -> Result<Vec<ApiCollection>, String>
fn save_collections(collections: Vec<ApiCollection>) -> Result<(), String>
```

在 `src-tauri/src/commands/mod.rs` 注册新模块，在 `lib.rs` 中注册 command。

### 存储文件

`~/.ai-proxy/collections.json`（与 `setting.json` 同目录，通过 `config/store.rs` 的数据目录管理）。

### 数据流

1. NewRequestView mount → `useCollections` 调用 `get_collections` 加载
2. 首次加载为空 → 自动创建默认集合
3. 用户操作树（增删改/重命名）→ 更新本地状态 → 调用 `save_collections` 持久化
4. 点击请求节点 → 提取配置 → 填入编辑区

## Rust 后端改动

1. 新增 `src-tauri/src/commands/collection.rs` — `get_collections` / `save_collections`
2. `src-tauri/src/commands/mod.rs` — 添加 `mod collection`
3. `src-tauri/src/lib.rs` — 注册 `get_collections` / `save_collections` invoke handler
4. `src-tauri/src/config/store.rs` — 可能需要扩展数据目录路径方法（新增 `collections_path()`）

## 不包含的功能（YAGNI）

- 拖拽排序（后续迭代）
- 导入/导出 Collection（后续迭代）
- 多 Collection 切换（当前只有一个默认集合，但数据结构支持多个）
- 请求历史记录（后续迭代）
- 环境变量/变量替换（后续迭代）
