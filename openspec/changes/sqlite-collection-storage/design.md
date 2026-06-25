## Context

当前 `collections.json` 以完整 JSON 文件读写，每次变更需反序列化→修改→序列化→写回。项目已有 `traffic.db`（SQLite）基础设施用于流量日志持久化，现需将接口管理数据也纳入同一数据库。

涉及两张表的设计：`collection_nodes`（树结构）和 `requests`（请求数据/流量日志合并）。

## Goals / Non-Goals

**Goals:**
- SQLite 持久化接口管理数据，替换 JSON 文件
- `collection_nodes` 表存储集合/文件夹/请求节点的树形层级关系
- 扩展已有 `requests` 表，增加 `source_type` 区分接口管理数据和代理流量数据
- 统一 KV 格式：`request_headers`、`request_query`、`cookies` 全部使用 `[{key, value}]` 数组
- 细粒度 Tauri commands：增删改查各自独立接口

**Non-Goals:**
- 不做旧数据迁移（`collections.json` 无历史数据）
- 不做 multipart 二进制文件存储
- 不做 Auth 信息独立表存储（`auth_type` + `auth_data` 两个字段覆盖）
- 不改变代理流量 `requests` 行的查询/展示逻辑

## Decisions

### 1. 表结构设计

**`collection_nodes` — 树结构（邻接表）**

节点类型 `node_type`：`collection`（根集合，`parent_id='0'`）、`folder`（文件夹）、`request`（请求节点，通过 `request_id` 指向 `requests.id`）

选择邻接表而非物化路径或嵌套集的原因：
- 节点数量少（通常 < 500），递归查询性能足够
- 移动节点只需改一行 `parent_id`，无需批量更新路径
- 实现简单，与前端递归渲染天然匹配

**`requests` — 请求数据与流量日志合并**

一张表承载两种数据源，`source_type` 区分：
- `traffic`：代理抓包流量，填充 response 相关字段
- `collection`：接口管理请求，填充 body_type/auth 相关字段，response 字段为 NULL

合并而非分表的原因：
- 使用场景有交集：接口管理请求发送后插入 traffic 行，展示响应
- 两种数据共享 method/uri/headers/body 等核心字段
- 避免维护两套相似的表结构和 CRUD 代码

### 2. KV 字段统一数组格式

`request_headers`、`request_query`、`cookies` 统一为 `[{key: "...", value: "..."}]` 格式。

选择数组而非对象的原因：
- 保留 key 顺序（query string 签名场景有意义）
- 支持重复 key
- 与前端 `KeyValuePair[]` 类型零转换对接

### 3. Auth 双字段方案

不建独立 `auth_configs` 表，在 `requests` 表用 `auth_type` + `auth_data` 两个字段覆盖。

原因：
- Auth 配置仅被一个请求使用，不存在多对多复用场景
- `auth_data` 存 JSON，不同 auth_type 存不同结构：API Key → `{key, value, in}`, Bearer → `{token}`, Basic → `{username, password}`
- 减少表连接，降低复杂度

### 4. Tauri Commands 设计

后端提供以下命令，操作细粒度：

| 命令 | 操作 | 说明 |
|------|------|------|
| `get_collections` | 读 | 查询全部 tree + 关联的 request 数据，组装嵌套结构给前端 |
| `create_collection` | 增 | 新增根集合（`parent_id='0'`） |
| `create_folder` | 增 | 在指定 parent 下创建文件夹 |
| `create_request` | 增 | 在指定 parent 下创建请求节点 + 插入 requests 行 |
| `save_request` | 改 | 更新请求节点的 requests 行数据 |
| `rename_node` | 改 | 重命名任意节点 |
| `move_node` | 改 | 移动节点到新 parent |
| `delete_node` | 删 | 级联删除子树（collection_nodes + 关联 requests 行） |
| `duplicate_request` | 增 | 复制请求节点及其 requests 数据 |

移除 `save_collections` 命令（全量写回不再需要）。

### 5. Repository 分层架构

采用 Repository 模式，目录为 `collection/`，目录下按表名拆分文件，每个文件自包含 trait 定义 + `impl for Db`：

```
src-tauri/src/
├── collection/
│   ├── mod.rs                  ← 模块入口 + 数据类型（ApiCollection, ApiTreeNode 等）
│   ├── collection_nodes.rs     ← CollectionNodesRepository trait + impl for Db（树结构 CRUD）
│   └── requests.rs             ← RequestsRepository trait + impl for Db（请求数据 CRUD）
```

**职责划分：**

| 文件 | 对应表 | trait | 职责 |
|------|--------|-------|------|
| `mod.rs` | — | — | 数据类型定义、serde 序列化 |
| `collection_nodes.rs` | `collection_nodes` | `CollectionNodesRepository` | 树节点增删改查、子树删除、移动/重命名 |
| `requests.rs` | `requests` | `RequestsRepository` | collection 请求数据插入/更新/复制/查询 |

**trait 定义（collection_nodes.rs）：**

```rust
pub(crate) trait CollectionNodesRepository {
    fn load_all_collections(&self) -> Result<Vec<TreeWithRequests>, sqlite::Error>;
    fn create_collection(&self, id: &str, name: &str, timestamp: i64) -> Result<(), sqlite::Error>;
    fn create_folder(&self, id: &str, parent_id: &str, name: &str, timestamp: i64) -> Result<(), sqlite::Error>;
    fn create_request_node(&self, id: &str, parent_id: &str, name: &str, request_id: &str, timestamp: i64) -> Result<(), sqlite::Error>;
    fn rename_node(&self, id: &str, new_name: &str, timestamp: i64) -> Result<(), sqlite::Error>;
    fn move_node(&self, id: &str, new_parent_id: &str, timestamp: i64) -> Result<(), sqlite::Error>;
    fn delete_node_subtree(&self, id: &str) -> Result<(), sqlite::Error>;
}
```

**trait 定义（requests.rs）：**

```rust
pub(crate) trait RequestsRepository {
    fn insert_collection_request(&self, ...) -> Result<(), sqlite::Error>;
    fn update_collection_request(&self, ...) -> Result<(), sqlite::Error>;
    fn duplicate_collection_request(&self, ...) -> Result<(), sqlite::Error>;
    fn find_by_id(&self, id: &str) -> Result<Option<...>, sqlite::Error>;
}
```

**原因：**
- 按表拆分，一个文件对应一张表的操作，职责清晰
- trait 与 impl 同文件，减少文件跳转
- commands 层依赖 `Arc<dyn CollectionNodesRepository + RequestsRepository>` 或分开注入

### 6. 前端适配

`useCollections` hook 的每个操作方法逐一适配：
- `invoke('save_collections', ...)` 调用改为对应的细粒度命令
- 树结构操作（增删改名移）仅发送对应命令
- 请求内容的保存通过 `save_request` 单独调用
- `get_collections` 返回值结构保持不变，但后端从 SQLite 组装

## Risks / Trade-offs

- **已有 `requests` 表 `request_headers`/`request_query` 格式变更**：存量流量数据读取时需要兼容旧格式。处理方式：`load_all` 读取时尝试两种格式反序列化，写操作统一用新格式
- **`DELETE CASCADE` 不支持**：sqlite crate 的 FOREIGN KEY 需要手动开启 `PRAGMA foreign_keys = ON`。在 `Db::migrate()` 中执行
- **并发写入**：前端 debounce 300ms 已避免高频写入，collection CRUD 本身低频，单连接 SQLite 足够处理
