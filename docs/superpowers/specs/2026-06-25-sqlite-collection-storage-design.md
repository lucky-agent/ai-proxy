---
comet_change: sqlite-collection-storage
role: technical-design
canonical_spec: openspec
---

# SQLite 接口管理持久化 — 技术设计

## 1. 概述

将 new-request 左侧接口管理数据从 `collections.json` 文件存储迁移到 SQLite（`traffic.db`），采用 Repository 模式封装数据访问层。

## 2. 模块架构

```
src-tauri/src/
├── collection/                    ← 新增模块
│   ├── mod.rs                     ← 数据类型定义
│   ├── collection_nodes.rs        ← CollectionNodesRepository trait + impl for Db
│   └── requests.rs                ← RequestsRepository trait + impl for Db
├── commands/
│   └── collection.rs             ← Tauri commands（重写，依赖 trait）
├── config/
│   ├── db.rs                      ← Db struct，migrate() 扩展建表
│   └── store.rs                   ← 移除 collections_path()
└── lib.rs                         ← 注册新命令，移除 save_collections
```

## 3. 数据库设计

### 3.1 `collection_nodes` — 树结构

```sql
CREATE TABLE IF NOT EXISTS collection_nodes (
    id          TEXT PRIMARY KEY,
    parent_id   TEXT NOT NULL DEFAULT '0',  -- '0' = 根
    name        TEXT NOT NULL,
    node_type   TEXT NOT NULL,              -- 'collection' | 'folder' | 'request'
    request_id  TEXT,                       -- node_type='request' 时指向 requests.id
    sort_order  INTEGER DEFAULT 0,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
```

### 3.2 `requests` — 扩展已有表

在 `Db::migrate()` 建表语句中直接加入新字段（无 ALTER TABLE）：

```sql
CREATE TABLE IF NOT EXISTS requests (
    id                 TEXT PRIMARY KEY,
    source_type        TEXT NOT NULL DEFAULT 'traffic',   -- 新增
    collection_id      TEXT,                              -- 新增

    -- 接口管理专用
    name               TEXT,                              -- 新增（未使用，预留）
    cookies            TEXT DEFAULT '[]',                 -- 新增，JSON [{key, value}]
    body_type          TEXT,                              -- 新增
    auth_type          TEXT,                              -- 新增
    auth_data          TEXT,                              -- 新增

    -- 请求通用
    method             TEXT NOT NULL,
    uri                TEXT NOT NULL,
    request_timestamp  INTEGER NOT NULL,
    request_headers    TEXT NOT NULL DEFAULT '[]',        -- 修改默认值
    request_body       TEXT,
    request_query      TEXT DEFAULT '[]',                 -- 修改默认值

    -- 代理流量
    status             INTEGER,
    response_timestamp INTEGER,
    duration_ms        INTEGER,
    response_headers   TEXT,
    response_body      TEXT,
    error              TEXT,
    edited             INTEGER NOT NULL DEFAULT 0,

    FOREIGN KEY (collection_id) REFERENCES collection_nodes(id) ON DELETE SET NULL
);
```

建表顺序：`collection_nodes` 先于 `requests`（外键依赖）。`PRAGMA foreign_keys = ON` 在 `migrate()` 开头执行。

## 4. Repository 层设计

### 4.1 `CollectionNodesRepository` trait (`collection/collection_nodes.rs`)

```rust
pub(crate) trait CollectionNodesRepository {
    fn load_all_collections(&self) -> Result<Vec<ApiCollection>, sqlite::Error>;
    fn create_collection(&self, id: &str, name: &str, timestamp: i64) -> Result<(), sqlite::Error>;
    fn create_folder(&self, id: &str, parent_id: &str, name: &str, timestamp: i64) -> Result<(), sqlite::Error>;
    fn create_request_node(&self, id: &str, parent_id: &str, name: &str, request_id: &str, timestamp: i64) -> Result<(), sqlite::Error>;
    fn rename_node(&self, id: &str, new_name: &str, timestamp: i64) -> Result<(), sqlite::Error>;
    fn move_node(&self, id: &str, new_parent_id: &str, timestamp: i64) -> Result<(), sqlite::Error>;
    fn delete_node_subtree(&self, id: &str) -> Result<(), sqlite::Error>;
}
```

**`load_all_collections` 组装流程：**

1. `SELECT * FROM collection_nodes ORDER BY sort_order` → 全量节点
2. 收集所有 `request_id`，批量 `SELECT * FROM requests WHERE id IN (...)` → 关联请求数据
3. 构建 `HashMap<parent_id, Vec<children>>`，从 `parent_id='0'` 递归组装 `Vec<ApiCollection>`
4. 请求节点填充 `requests` 行的 method/url/headers/cookies/body/bodyType/auth 等字段

**`delete_node_subtree` 级联删除：**

1. 递归收集子树所有节点 ID
2. 删除其中 `node_type='request'` 节点关联的 `requests` 行
3. 删除子树所有 `collection_nodes` 行

### 4.2 `RequestsRepository` trait (`collection/requests.rs`)

```rust
pub(crate) trait RequestsRepository {
    fn insert_collection_request(&self, id: &str, collection_id: &str, name: &str, method: &str, uri: &str, timestamp: i64) -> Result<(), sqlite::Error>;
    fn update_collection_request(&self, id: &str, method: &str, uri: &str, headers: &str, query: &str, body: Option<&str>, body_type: &str, cookies: &str, auth_type: &str, auth_data: &str) -> Result<(), sqlite::Error>;
    fn duplicate_collection_request(&self, id: &str, new_id: &str, timestamp: i64) -> Result<(), sqlite::Error>;
    fn find_requests_by_ids(&self, ids: &[String]) -> Result<Vec<(id, method, uri, headers, body, query, cookies, body_type, auth_type, auth_data)>, sqlite::Error>;
}
```

### 4.3 依赖注入方式

复用现有 `AppState::db() -> Arc<Mutex<Db>>`。Commands 层获取锁后直接调用 trait 方法：

```rust
#[tauri::command]
fn create_folder(state: State<'_, AppState>, parent_id: String, name: String) -> Result<String, String> {
    let db = state.db();
    let repo = db.lock().unwrap();
    let id = Uuid::new_v4().to_string();
    let ts = chrono::Utc::now().timestamp_millis();
    repo.create_folder(&id, &parent_id, &name, ts).map_err(|e| e.to_string())?;
    Ok(id)
}
```

## 5. KV 字段格式变更

| 字段 | 旧格式 | 新格式 |
|------|--------|--------|
| `request_headers` | `{"key": "value"}` | `[{"key":"...","value":"..."}]` |
| `request_query` | `{"key": "value"}` | `[{"key":"...","value":"..."}]` |
| `cookies` | 不存在 | `[{"key":"...","value":"..."}]` |

影响范围：
- `Db::headers_to_json` / `Db::query_to_json`：输出改为数组格式
- `Db::upsert_request` / `Db::load_all`：新的序列化/反序列化
- `StoredEntry` 的 `request_headers`、`request_query` 字段类型保持不变（反序列化为 `HashMap` 后使用），内部序列化用数组

## 6. 前端适配

### 6.1 `useCollections` hook 变更

| 操作 | 旧 invoke | 新 invoke | 延迟 |
|------|-----------|-----------|------|
| 加载 | `get_collections` | `get_collections` | 无 |
| 创建集合 | — | `create_collection` | 立即 |
| 创建文件夹 | `save_collections` | `create_folder` | 立即 |
| 创建请求 | `save_collections` | `create_request` | 立即 |
| 删除节点 | `save_collections` | `delete_node` | 立即 |
| 重命名 | `save_collections` | `rename_node` | 立即 |
| 更新请求内容 | `save_collections` | `save_request` | 300ms debounce |
| 复制请求 | `save_collections` | `duplicate_request` | 立即 |
| 移动节点 | — | `move_node` | 立即 |

### 6.2 类型扩展

`ApiRequestNode` 新增：`authType?: string`、`authData?: string`
`RequestTab` 新增：`authType: string`、`authData: string`

## 7. 数据流示例

### 创建请求

```
前端 addRequest(parentId)
  ↓ invoke('create_request', { parentId })
Tauri command create_request
  ↓ db.lock().unwrap()
  ↓ repo.create_request_node(id, parentId, name, requestId, ts)   ← collection_nodes 行
  ↓ repo.insert_collection_request(requestId, collectionId, ...)   ← requests 行
  ↓ Ok(node_id)
前端 setCollections(prev => ...) ← 乐观更新
```

### 编辑并保存

```
前端 updateActiveTab({ method: 'POST', body: '...' })
  ↓ (debounce 300ms)
前端 updateRequest(linkedNodeId, { method, body, ... })
  ↓ invoke('save_request', { id, method, body, ... })
Tauri command save_request
  ↓ repo.update_collection_request(...)
  ↓ Ok(())
```

## 8. 向后兼容

- 无历史数据，不做旧格式兼容
- `source_type` 默认值 `'traffic'` 保证已有代理流量写入不受影响
- `requests` 表新增字段均有 DEFAULT 或允许 NULL，旧读取逻辑不受影响
