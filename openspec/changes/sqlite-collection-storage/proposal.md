## Why

当前 new-request 左侧接口管理数据以单个 `collections.json` 文件存储，每次变更需整体序列化写入，缺乏事务保障且在数据增长后性能下降。项目已有 SQLite 基础设施（`traffic.db`），统一使用关系型数据库可提供更可靠的数据持久化、增量写入和结构化查询能力。

## What Changes

- 新增 `collection_nodes` 表，存储接口管理的树形结构（集合 + 文件夹 + 请求节点），使用邻接表 `parent_id` 模式，`parent_id='0'` 表示根节点
- **BREAKING**: 改造已有 `requests` 表：`request_headers` 和 `request_query` 从 JSON 对象格式 `{key: value}` 改为 JSON 数组格式 `[{key, value}]`，新增 `source_type`、`collection_id`、`cookies`、`body_type`、`auth_type`、`auth_data` 字段
- 移除 JSON 文件存储方式，`Store::collections_path()` 方法删除
- 后端 Tauri commands 重写：`get_collections` 从 SQLite 查询并组装树结构，新增 `create_collection`、`create_folder`、`delete_node`、`rename_node`、`move_node`、`duplicate_request`、`save_request` 等细粒度命令
- 前端 `useCollections` hook 适配新的 invoke 接口
- 前/后端类型定义同步更新（`ApiTreeNode`、`ApiCollection`、`RequestTab` 等）

## Capabilities

### New Capabilities
- `collection-persistence`: 接口管理数据的 SQLite 持久化存储与 CRUD 操作

### Modified Capabilities
<!-- 无已有 spec 需要修改 -->

## Impact

- Rust: `config/db.rs`（DDL + CRUD）、`commands/collection.rs`（重写）、`config/store.rs`（移除 collections_path）、`lib.rs`（注册新命令）
- Frontend: `hooks/useCollections.ts`（适配新接口）、`types/collection.ts`（类型调整）
- 数据：`traffic.db` 新增 `collection_nodes` 表，`requests` 表新增字段
- 已有 `traffic` 行不受影响（`source_type` 默认 `'traffic'`）
