## ADDED Requirements

### Requirement: SQLite-backed collection tree persistence

The system SHALL persist API collection tree data (collections, folders, request nodes) in SQLite via the `collection_nodes` table using adjacency list (`parent_id`) model. Root collections SHALL have `parent_id = '0'`.

#### Scenario: App start loads collections from SQLite
- **WHEN** the application starts
- **THEN** the `get_collections` command SHALL query `collection_nodes` and assemble a nested tree structure
- **AND** request nodes SHALL include their linked `requests` row data (method, url, headers, cookies, body, body_type, auth_type, auth_data)
- **AND** the tree structure returned SHALL match the previous JSON format for backward compatibility

#### Scenario: Create root collection
- **WHEN** the user creates a new collection
- **THEN** a new row SHALL be inserted into `collection_nodes` with `parent_id = '0'` and `node_type = 'collection'`

#### Scenario: Create folder under collection
- **WHEN** the user creates a folder inside a collection
- **THEN** a new row SHALL be inserted into `collection_nodes` with `parent_id` set to the target collection's `id`

#### Scenario: Create request node under folder
- **WHEN** the user creates a request node inside a folder
- **THEN** a new row SHALL be inserted into `collection_nodes` with `parent_id` set to the target folder's `id`, `node_type = 'request'`
- **AND** a new row SHALL be inserted into `requests` with `source_type = 'collection'` and `collection_id` pointing to the root collection
- **AND** `collection_nodes.request_id` SHALL reference the new `requests.id`

#### Scenario: Delete node cascades subtree
- **WHEN** the user deletes a folder that contains child nodes
- **THEN** the system SHALL recursively delete all descendant `collection_nodes`
- **AND** for each deleted request node, the associated `requests` row SHALL be deleted

### Requirement: Unified requests table with source_type

The system SHALL use a single `requests` table to store both proxy traffic data and collection-managed request data, differentiated by the `source_type` field.

#### Scenario: Traffic entry has source_type 'traffic'
- **WHEN** a proxy request is captured
- **THEN** the inserted `requests` row SHALL have `source_type = 'traffic'` (default)
- **AND** collection-specific fields (`collection_id`, `body_type`, `auth_type`, `auth_data`, `cookies`) SHALL be NULL

#### Scenario: Collection request has source_type 'collection'
- **WHEN** a request is created via the collection management UI
- **THEN** the inserted `requests` row SHALL have `source_type = 'collection'`
- **AND** response-specific fields (`status`, `response_headers`, `response_body`, `duration_ms`, `error`) SHALL be NULL
- **AND** collection-specific fields SHALL be populated

### Requirement: KV fields use array format

The system SHALL store key-value pair fields (`request_headers`, `request_query`, `cookies`) as JSON arrays in the format `[{key: string, value: string}]`, replacing the previous object format for `request_headers` and `request_query`.

#### Scenario: Headers stored as array
- **WHEN** a request has headers `Content-Type: application/json` and `Authorization: Bearer xxx`
- **THEN** `request_headers` SHALL be stored as `[{"key":"Content-Type","value":"application/json"},{"key":"Authorization","value":"Bearer xxx"}]`

#### Scenario: Query params stored as array
- **WHEN** a request URL is `https://api.example.com/users?page=1&size=20`
- **THEN** `request_query` SHALL be stored as `[{"key":"page","value":"1"},{"key":"size","value":"20"}]`

#### Scenario: Cookies stored as array
- **WHEN** a request has cookies `session=abc123` and `theme=dark`
- **THEN** `cookies` SHALL be stored as `[{"key":"session","value":"abc123"},{"key":"theme","value":"dark"}]`

### Requirement: Auth information stored inline

The system SHALL store authentication configuration inline in the `requests` table using `auth_type` and `auth_data` fields rather than a separate auth table.

#### Scenario: API Key auth stored
- **WHEN** a collection request is configured with API Key authentication (key name `X-API-Key`, value `sk-xxx`, location `header`)
- **THEN** `auth_type` SHALL be `api_key`
- **AND** `auth_data` SHALL be `{"key":"X-API-Key","value":"sk-xxx","in":"header"}`

#### Scenario: Bearer token auth stored
- **WHEN** a collection request is configured with Bearer Token authentication (token `abc123`)
- **THEN** `auth_type` SHALL be `bearer`
- **AND** `auth_data` SHALL be `{"token":"abc123"}`

#### Scenario: Basic auth stored
- **WHEN** a collection request is configured with Basic authentication (username `admin`, password `pass123`)
- **THEN** `auth_type` SHALL be `basic`
- **AND** `auth_data` SHALL be `{"username":"admin","password":"pass123"}`

### Requirement: Fine-grained CRUD commands

The system SHALL expose individual Tauri commands for each data operation instead of a single save_collections command.

#### Scenario: Rename a node
- **WHEN** the user renames a collection node
- **THEN** the `rename_node` command SHALL update only the `name` and `updated_at` fields of that single `collection_nodes` row

#### Scenario: Save request edits
- **WHEN** the user edits a collection request's method, URL, headers, body, etc.
- **THEN** the `save_request` command SHALL update only the corresponding `requests` row

#### Scenario: Move node to different parent
- **WHEN** the user drags a folder to another collection
- **THEN** the `move_node` command SHALL update only the `parent_id` and `updated_at` of the moved node

### Requirement: Remove JSON file storage

The system SHALL no longer read from or write to `collections.json`. The `save_collections` command SHALL be removed from the Tauri command registry.

#### Scenario: No collections.json dependency
- **WHEN** the application starts and `collections.json` does not exist
- **THEN** no error SHALL be raised
- **AND** `get_collections` SHALL return data from SQLite

### Requirement: Backward-compatible read of old KV format

The system SHALL accept both object format `{key: value}` and array format `[{key, value}]` when deserializing `request_headers` and `request_query` from existing `requests` rows, but SHALL always write new data in array format.

#### Scenario: Read old object-format headers
- **WHEN** a `requests` row has `request_headers = '{"Content-Type":"application/json"}'` (old format)
- **THEN** `load_all` SHALL correctly deserialize it into `HashMap<String, String>` used internally

#### Scenario: Write always uses array format
- **WHEN** a new request is inserted or existing request is updated
- **THEN** `request_headers` SHALL be written as `[{"key":"...","value":"..."}]`
