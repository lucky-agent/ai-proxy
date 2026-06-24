---
change: sqlite-collection-storage
design-doc: docs/superpowers/specs/2026-06-25-sqlite-collection-storage-design.md
base-ref: daa0e641d5d7a182c97de967b2693cce03cdd0e7
---

# SQLite 接口管理持久化 实现计划

## 架构概览

新增 `src-tauri/src/collection/` Repository 模块，`Db` struct 实现两个 trait：
- `CollectionNodesRepository` → 树结构 CRUD
- `RequestsRepository` → 请求数据 CRUD

Tauri commands 层重写，前端 hook 从 `save_collections` 批量改为逐操作调用。

## 文件变更清单

| 文件 | 操作 | 职责 |
|------|------|------|
| `src-tauri/src/config/db.rs` | 修改 | 扩展 `requests` 建表 + 新增 `collection_nodes` 建表，headers_to_json/query_to_json 改为数组格式，load_all 读新字段 |
| `src-tauri/src/config/store.rs` | 修改 | 移除 `collections_path()` |
| `src-tauri/src/collection/mod.rs` | **新建** | 模块入口 + 数据类型（ApiCollection, ApiTreeNode, HeaderPair） |
| `src-tauri/src/collection/collection_nodes.rs` | **新建** | `CollectionNodesRepository` trait + impl for Db |
| `src-tauri/src/collection/requests.rs` | **新建** | `RequestsRepository` trait + impl for Db |
| `src-tauri/src/commands/collection.rs` | 重写 | 9 个细粒度 Tauri commands |
| `src-tauri/src/commands/mod.rs` | 修改 | 更新导出 |
| `src-tauri/src/lib.rs` | 修改 | 注册新命令，移除 `save_collections` |
| `src/types/collection.ts` | 修改 | `ApiRequestNode`、`RequestTab` 新增 `authType`/`authData` |
| `src/hooks/useCollections.ts` | 重写 | 逐操作 invoke，树操作立即、save_request debounce 300ms |
| `src/features/new-request/useRequestTabs.ts` | 修改 | sync 回调适配 |

## 任务列表

使用 `- [ ]` 追踪进度。逐任务执行，每完成一个 commit。

### Task 1 — `collection` 模块骨架

**目标**：创建模块文件，定义数据类型

**验证**：`cargo check` 通过

- [x] **1.1** 创建 `src-tauri/src/collection/mod.rs`
  - 从 `commands/collection.rs` 迁入 `ApiCollection`、`ApiTreeNode`（含 `HeaderPair`）
  - 扩展 `ApiTreeNode::Request` 增加 `params: Vec<HeaderPair>`、`cookies: Vec<HeaderPair>`、`bodyType: String`、`authType: Option<String>`、`authData: Option<String>`
  - `ApiCollection` 增加 `created_at: u64`、`updated_at: u64`（camelCase serde）

- [x] **1.2** 在 `lib.rs` 中声明 `mod collection;` 并暂时 `use crate::collection::*;`

### Task 2 — `requests` 表 DDL 迁移 + `requests.rs` Repository

**目标**：修改 `Db::migrate()` 扩展 `requests` 表，创建 `collection_nodes` 表。实现 `RequestsRepository` trait。

**验证**：`cargo check` 通过

- [x] **2.1** 修改 `config/db.rs` — `migrate()` 扩展建表 + PRAGMA foreign_keys
  - 新增列：`source_type TEXT NOT NULL DEFAULT 'traffic'`, `collection_id TEXT`, `cookies TEXT DEFAULT '[]'`, `body_type TEXT`, `auth_type TEXT`, `auth_data TEXT`
  - 修改默认值：`request_headers` 和 `request_query` 从 `'{}'` 改为 `'[]'`
  - 新增 `collection_nodes` 建表语句
  - 开头执行 `PRAGMA foreign_keys = ON`

- [x] **2.2** 修改 `config/db.rs` — `headers_to_json()` 输出数组格式 `[{key,value}]`

- [x] **2.3** 修改 `config/db.rs` — `query_to_json()` 输出数组格式 `[{key,value}]`

- [x] **2.4** 修改 `config/db.rs` — `upsert_request()` 签名增加新字段参数（`source_type`, `collection_id`, `cookies`, `body_type`, `auth_type`, `auth_data`）

- [x] **2.5** 修改 `config/db.rs` — `StoredEntry` 结构体增加新字段

- [x] **2.6** 修改 `config/db.rs` — `load_all()` 读取新列

- [x] **2.7** 创建 `collection/requests.rs`
  - 定义 `RequestsRepository` trait
  - `impl RequestsRepository for Db`：`insert_collection_request()`、`update_collection_request()`、`duplicate_collection_request()`、`find_requests_by_ids()`

### Task 3 — `collection_nodes.rs` Repository

**目标**：实现 `CollectionNodesRepository` trait

**验证**：`cargo check` 通过

- [x] **3.1** 创建 `collection/collection_nodes.rs`
  - 定义 `CollectionNodesRepository` trait
  - `impl CollectionNodesRepository for Db` 方法：
    - `load_all_collections()` — 全量查询 + HashMap 递归组装
    - `create_collection()` — 插入 root 节点
    - `create_folder()` — 插入文件夹
    - `create_request_node()` — 插入请求节点
    - `rename_node()` — 更新 name
    - `move_node()` — 更新 parent_id
    - `delete_node_subtree()` — 递归收集 + 级联删除

### Task 4 — Tauri commands 重写

**目标**：重写 `commands/collection.rs`，9 个细粒度命令

**验证**：`cargo check` 通过

- [ ] **4.1** 实现 `get_collections` — 调用 `CollectionNodesRepository::load_all_collections()`

- [ ] **4.2** 实现 `create_collection` — `parent_id='0'`, 生成 UUID + 时间戳

- [ ] **4.3** 实现 `create_folder` — 生成 UUID，插入到指定 parent

- [ ] **4.4** 实现 `create_request` — 生成两个 UUID（node + request），关联插入

- [ ] **4.5** 实现 `delete_node` — 级联删除

- [ ] **4.6** 实现 `rename_node` — 更新 name + updated_at

- [ ] **4.7** 实现 `move_node` — 更新 parent_id + updated_at

- [ ] **4.8** 实现 `save_request` — 调用 `RequestsRepository::update_collection_request()`

- [ ] **4.9** 实现 `duplicate_request` — 复制节点 + 请求行

### Task 5 — 注册与清理

**目标**：注册新命令，移除旧代码

**验证**：`cargo check` 通过

- [ ] **5.1** 更新 `commands/mod.rs` 导出全部新命令，移除 `save_collections` 导出

- [ ] **5.2** 更新 `lib.rs` invoke_handler 注册全部新命令，移除 `save_collections`

- [ ] **5.3** 移除 `Store::collections_path()` 方法

- [ ] **5.4** 清理 `collections.json` 相关引用

### Task 6 — 前端类型更新

**目标**：TypeScript 类型与后端对齐

**验证**：`bun run build:vite` 通过

- [ ] **6.1** `ApiRequestNode` 新增 `authType?: string`、`authData?: string`

- [ ] **6.2** `RequestTab` 新增 `authType: string`、`authData: string`

### Task 7 — 前端 hook 适配

**目标**：`useCollections` + `useRequestTabs` 适配新 invoke 接口

**验证**：`bun run build` 通过

- [ ] **7.1** 改写 `useCollections`：加载仍用 `get_collections`，各操作方法改为细粒度 invoke

- [ ] **7.2** `addFolder` → `invoke('create_folder', { parentId, name })`

- [ ] **7.3** `addRequest` → `invoke('create_request', { parentId, name })`

- [ ] **7.4** `removeNode` → `invoke('delete_node', { nodeId })`

- [ ] **7.5** `renameNode` → `invoke('rename_node', { nodeId, newName })`

- [ ] **7.6** `updateRequest` → `invoke('save_request', { ... })`（debounce 300ms 保留）

- [ ] **7.7** `duplicateRequest` → `invoke('duplicate_request', { nodeId })`

- [ ] **7.8** `renameCollection` → `invoke('rename_node', { nodeId, newName })`

- [ ] **7.9** 移除 debounced `save_collections` 逻辑

- [ ] **7.10** 更新 `useRequestTabs` sync 逻辑适配

### Task 8 — 构建验证

**目标**：确认完整构建通过

**验证**：`bun run build` 无错误退出

- [ ] **8.1** 运行 `cargo check` 确认 Rust 编译

- [ ] **8.2** 运行 `bun run build:vite` 确认前端编译

- [ ] **8.3** 运行 `bun run build` 完整 Tauri 构建
