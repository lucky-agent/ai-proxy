## 1. Repository layer — collection_nodes table

- [x] 1.1 Extend `Db::migrate()` to create `collection_nodes` table DDL and enable `PRAGMA foreign_keys = ON`
- [x] 1.2 Create `src-tauri/src/collection/` module: `mod.rs` + `collection_nodes.rs` + `requests.rs`
- [x] 1.3 Define `CollectionNodesRepository` trait in `collection/collection_nodes.rs`:
  - [x] 1.3.1 `load_all_collections()` — query all nodes, assemble nested tree with linked request data
  - [x] 1.3.2 `create_collection(id, name, timestamp)` — insert root collection node
  - [x] 1.3.3 `create_folder(id, parent_id, name, timestamp)` — insert folder node
  - [x] 1.3.4 `create_request_node(id, parent_id, name, request_id, timestamp)` — insert request node
  - [x] 1.3.5 `rename_node(id, new_name, timestamp)` — update name
  - [x] 1.3.6 `move_node(id, new_parent_id, timestamp)` — update parent_id
  - [x] 1.3.7 `delete_node_subtree(id)` — recursive delete subtree + cleanup associated `requests` rows
- [x] 1.4 Implement `CollectionNodesRepository for Db` in `collection/collection_nodes.rs`
- [x] 1.5 Define `RequestsRepository` trait in `collection/requests.rs`:
  - [x] 1.5.1 `insert_collection_request(...)` — insert into `requests` with `source_type='collection'`
  - [x] 1.5.2 `update_collection_request(...)` — update collection request data
  - [x] 1.5.3 `duplicate_collection_request(...)` — copy request row
- [x] 1.6 Implement `RequestsRepository for Db` in `collection/requests.rs`

## 2. Database layer — requests table migration

- [x] 2.1 Add new columns to `requests` table: `source_type`, `collection_id`, `cookies`, `body_type`, `auth_type`, `auth_data`
- [x] 2.2 Update `requests` table `request_headers` and `request_query` default values from `'{}'` to `'[]'`
- [x] 2.3 Update `Db::upsert_request` to handle new fields (`source_type`, `collection_id`, `cookies`, `body_type`, `auth_type`, `auth_data`)
- [x] 2.4 Update `Db::headers_to_json` to output array format `[{key, value}]`
- [x] 2.5 Update `Db::query_to_json` to output array format `[{key, value}]`
- [x] 2.6 Update `StoredEntry` struct to include new fields and reflect array-format headers/query
- [x] 2.7 Update `Db::load_all` to read new fields and handle backward-compatible deserialization of old object-format data

## 3. Rust type updates

- [x] 3.1 Move collection data types to `collection/mod.rs`: `ApiCollection`, `ApiTreeNode`, `HeaderPair` (from `commands/collection.rs`)
- [x] 3.2 Extend `ApiTreeNode::Request` with `params`, `cookies`, `bodyType`, `authType`, `authData` fields
- [x] 3.3 Update `ApiCollection` struct to include `created_at`, `updated_at` with proper serde camelCase

## 4. Tauri commands — CRUD via trait

- [x] 4.1 Rewrite `get_collections` command: delegate to `CollectionNodesRepository::load_all_collections()`
- [x] 4.2 Implement `create_collection` command: delegate to `CollectionNodesRepository::create_collection()`
- [x] 4.3 Implement `create_folder` command: delegate to `CollectionNodesRepository::create_folder()`
- [x] 4.4 Implement `create_request` command: delegate to `CollectionNodesRepository::create_request_node()` + `RequestsRepository::insert_collection_request()`
- [x] 4.5 Implement `delete_node` command: delegate to `CollectionNodesRepository::delete_node_subtree()`
- [x] 4.6 Implement `rename_node` command: delegate to `CollectionNodesRepository::rename_node()`
- [x] 4.7 Implement `move_node` command: delegate to `CollectionNodesRepository::move_node()`
- [x] 4.8 Implement `save_request` command: delegate to `RequestsRepository::update_collection_request()`
- [x] 4.9 Implement `duplicate_request` command: delegate to `CollectionNodesRepository` + `RequestsRepository::duplicate_collection_request()`

## 5. Tauri command registration & cleanup

- [x] 5.1 Register `collection` module in `lib.rs` (declare `mod collection;`)
- [x] 5.2 Register new commands in `commands/mod.rs` exports
- [x] 5.3 Register new commands in `lib.rs` invoke_handler
- [x] 5.4 Remove `save_collections` command and export
- [x] 5.5 Remove `Store::collections_path()` method
- [x] 5.6 Clean up any `collections.json` references in codebase

## 6. Frontend — types update

- [x] 6.1 Update `ApiRequestNode` type to include `authType`, `authData` fields
- [x] 6.2 Update `RequestTab` type to include `authType`, `authData` fields

## 7. Frontend — hook adaptation

- [x] 7.1 Update `useCollections` hook: replace `invoke('save_collections')` with fine-grained commands
- [x] 7.2 Update `useCollections.addFolder` to call `invoke('create_folder')`
- [x] 7.3 Update `useCollections.addRequest` to call `invoke('create_request')`
- [x] 7.4 Update `useCollections.removeNode` to call `invoke('delete_node')`
- [x] 7.5 Update `useCollections.renameNode` to call `invoke('rename_node')`
- [x] 7.6 Update `useCollections.updateRequest` to call `invoke('save_request')`
- [x] 7.7 Update `useCollections.duplicateRequest` to call `invoke('duplicate_request')`
- [x] 7.8 Update `useCollections.renameCollection` to call `invoke('rename_node')`
- [x] 7.9 Remove debounced `save_collections` logic from `useCollections`
- [x] 7.10 Update `useRequestTabs` sync logic: `updateRequest` callback now calls `save_request` via debounced sync

## 8. Frontend cleanup

- [x] 8.1 Remove `save_collections` invoke calls from codebase
- [x] 8.2 Verify auth tab UI is wired to `authType` / `authData` fields in tab state
- [x] 8.3 Verify request editor passes `cookies` through correctly

## 9. Verification

- [x] 9.1 Run `cargo build --release` — passed (1m33s, 0 errors). `bun run build` fails at Tauri bundler timeout, not code error.
- [ ] 9.2 Manual smoke test: create collection → create folder → create request → edit → send → data persists after restart (requires running the app)
