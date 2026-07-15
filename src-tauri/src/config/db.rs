use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc;

use rama::http::HeaderMap;
use serde::Serialize;
use sqlite;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct StoredEntry {
    pub id: String,
    #[serde(rename = "sourceType", default)]
    pub source_type: Option<String>,
    #[serde(rename = "collectionId", default)]
    pub collection_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    pub method: String,
    pub uri: String,
    #[serde(rename = "requestTimestamp")]
    pub request_timestamp: i64,
    #[serde(rename = "requestHeaders")]
    pub request_headers: HashMap<String, String>,
    #[serde(rename = "requestBody")]
    pub request_body: Option<String>,
    #[serde(rename = "bodyType", default)]
    pub body_type: Option<String>,
    #[serde(rename = "authType", default)]
    pub auth_type: Option<String>,
    #[serde(rename = "authData", default)]
    pub auth_data: Option<String>,
    #[serde(rename = "requestQuery")]
    pub request_query: Option<HashMap<String, String>>,
    #[serde(default)]
    pub cookies: Option<String>,
    pub status: Option<u16>,
    #[serde(rename = "responseTimestamp")]
    pub response_timestamp: Option<i64>,
    #[serde(rename = "durationMs")]
    pub duration_ms: Option<u64>,
    #[serde(rename = "responseHeaders")]
    pub response_headers: Option<HashMap<String, String>>,
    #[serde(rename = "responseBody")]
    pub response_body: Option<String>,
    pub error: Option<String>,
    pub edited: Option<bool>,
    #[serde(rename = "responseChunks", default)]
    pub response_chunks: Vec<ChunkRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ChunkRecord {
    pub data: String,
}

// ── Writer thread command ────────────────────────────────────────────────────

type SyncSender = mpsc::SyncSender<DbCmd>;

enum DbCmd {
    // ── Traffic logging (requests + response_chunks) ──
    UpsertRequest {
        method: String,
        uri: String,
        timestamp: i64,
        headers_json: String,
        query_json: String,
        body: Option<String>,
        edited: bool,
        source_type: String,
        collection_id: Option<i64>,
        cookies_json: String,
        body_type: String,
        auth_type: String,
        auth_data: String,
        reply: Option<mpsc::Sender<Result<i64, sqlite::Error>>>,
    },
    UpdateResponse {
        id: i64,
        status: u16,
        timestamp: i64,
        duration_ms: u64,
        headers_json: String,
    },
    UpdateResponseBody {
        id: i64,
        body: String,
    },
    SetError {
        id: i64,
        error: String,
    },
    InsertChunk {
        request_id: i64,
        chunk: String,
        seq: i64,
        created_at: i64,
    },
    LoadAll {
        reply: mpsc::Sender<Result<Vec<StoredEntry>, sqlite::Error>>,
    },
    LoadChunks {
        request_id: i64,
        reply: mpsc::Sender<Result<Vec<ChunkRecord>, sqlite::Error>>,
    },
    Clear {
        reply: mpsc::Sender<Result<(), sqlite::Error>>,
    },

    // ── Collection management (collection_nodes + requests) ──
    LoadAllCollectionNodes {
        reply: mpsc::Sender<Result<Vec<CollectionNodeRow>, sqlite::Error>>,
    },
    InsertCollectionRequest {
        collection_id: i64,
        name: String,
        method: String,
        uri: String,
        timestamp: i64,
        reply: mpsc::Sender<Result<i64, sqlite::Error>>,
    },
    UpdateCollectionRequest {
        id: i64,
        method: String,
        uri: String,
        headers: String,
        query: String,
        body: Option<String>,
        body_type: String,
        cookies: String,
        auth_type: String,
        auth_data: String,
    },
    DuplicateCollectionRequest {
        id: i64,
        timestamp: i64,
        reply: mpsc::Sender<Result<i64, sqlite::Error>>,
    },
    FindRequestsByIds {
        ids: Vec<i64>,
        reply: mpsc::Sender<Result<Vec<RequestRow>, sqlite::Error>>,
    },
    CreateCollection {
        name: String,
        timestamp: i64,
        reply: mpsc::Sender<Result<i64, sqlite::Error>>,
    },
    CreateFolder {
        parent_id: i64,
        name: String,
        timestamp: i64,
        reply: mpsc::Sender<Result<i64, sqlite::Error>>,
    },
    CreateRequestNode {
        parent_id: i64,
        name: String,
        request_id: i64,
        timestamp: i64,
        reply: mpsc::Sender<Result<i64, sqlite::Error>>,
    },
    RenameNode {
        id: i64,
        new_name: String,
        timestamp: i64,
    },
    MoveNode {
        id: i64,
        new_parent_id: i64,
        timestamp: i64,
    },
    DeleteNodeSubtree {
        id: i64,
        reply: mpsc::Sender<Result<(), sqlite::Error>>,
    },
    /// Atomically check and delete a root collection — prevents TOCTOU race
    /// where two concurrent delete_node calls could each pass the "is this the
    /// last collection?" check and both proceed, leaving zero collections.
    DeleteNodeIfNotLast {
        node_id: i64,
        reply: mpsc::Sender<Result<(), sqlite::Error>>,
    },

    /// Graceful shutdown — writer thread exits after processing pending commands.
    Shutdown,
}

// ── Row types (returned by channel commands) ─────────────────────────────────

/// A row from the `collection_nodes` table.
#[derive(Debug, Clone)]
pub(crate) struct CollectionNodeRow {
    pub id: i64,
    pub parent_id: i64,
    pub name: String,
    pub node_type: String,
    pub request_id: Option<i64>,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A row from the `requests` table (subset used by collection queries).
#[derive(Debug, Clone)]
pub(crate) struct RequestRow {
    pub id: i64,
    pub method: String,
    pub uri: String,
    pub headers: String,
    pub body: Option<String>,
    pub query: String,
    pub cookies: String,
    pub body_type: String,
    pub auth_type: String,
    pub auth_data: String,
    pub name: String,
}

// ── Db ───────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub(crate) struct Db {
    tx: Option<Arc<SyncSender>>,
}

impl Db {
    // ── Lifecycle ────────────────────────────────────────────────────────────

    pub(crate) fn open(path: &PathBuf) -> Result<Self, sqlite::Error> {
        let db_path = path.to_string_lossy().to_string();
        let conn = sqlite::open(path)?;
        migrate(&conn)?;

        let (cmd_tx, cmd_rx) = mpsc::sync_channel::<DbCmd>(256);
        let tx = Arc::new(cmd_tx);
        std::thread::Builder::new()
            .name("db-writer".into())
            .spawn(move || writer_loop(conn, cmd_rx, db_path))
            .expect("failed to spawn db-writer thread");

        Ok(Self { tx: Some(tx) })
    }

    pub(crate) fn noop() -> Self {
        Self { tx: None }
    }

    /// Shut down the writer thread gracefully. Pending commands are processed first.
    pub(crate) fn shutdown(&self) {
        if let Some(ref tx) = self.tx {
            tx.send(DbCmd::Shutdown).ok();
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    /// Send a command, failing if the writer thread is gone.
    fn send(&self, cmd: DbCmd) -> Result<(), sqlite::Error> {
        match self.tx {
            Some(ref tx) => tx.send(cmd).map_err(|_| sqlite::Error {
                code: None,
                message: Some("db writer thread disconnected".into()),
            }),
            None => Ok(()),
        }
    }

    /// Convert an HTTP HeaderMap to a JSON array of key-value pairs.
    pub(crate) fn headers_to_json(headers: &HeaderMap) -> String {
        let pairs: Vec<serde_json::Value> = headers
            .iter()
            .map(|(k, v)| {
                serde_json::json!({"key": k.to_string(), "value": v.to_str().unwrap_or("")})
            })
            .collect();
        serde_json::to_string(&pairs).unwrap_or_else(|_| "[]".to_string())
    }

    /// Extract URI query parameters as a JSON array of key-value pairs.
    pub(crate) fn query_to_json(uri: &rama::net::uri::Uri) -> String {
        let q_str = uri.query_or_empty();
        if q_str.is_empty() {
            return "[]".to_string();
        }
        let pairs: Vec<serde_json::Value> = q_str
            .split('&')
            .filter_map(|pair| {
                let mut it = pair.splitn(2, '=');
                let key = it.next()?;
                let val = it.next().unwrap_or("");
                Some(serde_json::json!({"key": key, "value": val}))
            })
            .collect();
        serde_json::to_string(&pairs).unwrap_or_else(|_| "[]".to_string())
    }

    /// Return the DB path, or None when persistence is disabled.
    pub(crate) fn async_path(&self) -> Option<String> {
        // With the writer thread, the path is managed internally.
        // This method is kept for backward compat; returns None.
        None
    }

    // ── Traffic logging ──────────────────────────────────────────────────────

    pub(crate) fn upsert_request(
        &self,
        method: &str,
        uri: &str,
        timestamp: i64,
        headers_json: &str,
        query_json: &str,
        body: Option<&str>,
        edited: bool,
        source_type: &str,
        collection_id: Option<i64>,
        cookies_json: &str,
        body_type: &str,
        auth_type: &str,
        auth_data: &str,
    ) -> Result<i64, sqlite::Error> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(DbCmd::UpsertRequest {
            method: method.to_string(),
            uri: uri.to_string(),
            timestamp,
            headers_json: headers_json.to_string(),
            query_json: query_json.to_string(),
            body: body.map(String::from),
            edited,
            source_type: source_type.to_string(),
            collection_id,
            cookies_json: cookies_json.to_string(),
            body_type: body_type.to_string(),
            auth_type: auth_type.to_string(),
            auth_data: auth_data.to_string(),
            reply: Some(reply_tx),
        })?;
        reply_rx.recv().map_err(|_| sqlite::Error {
            code: None,
            message: Some("db writer thread disconnected".into()),
        })?
    }

    pub(crate) fn update_response(
        &self,
        id: i64,
        status: u16,
        timestamp: i64,
        duration_ms: u64,
        headers_json: &str,
    ) -> Result<(), sqlite::Error> {
        self.send(DbCmd::UpdateResponse {
            id,
            status,
            timestamp,
            duration_ms,
            headers_json: headers_json.to_string(),
        })
    }

    pub(crate) fn update_response_body(&self, id: i64, body: &str) -> Result<(), sqlite::Error> {
        self.send(DbCmd::UpdateResponseBody {
            id,
            body: body.to_string(),
        })
    }

    pub(crate) fn set_error(&self, id: i64, error: &str) -> Result<(), sqlite::Error> {
        self.send(DbCmd::SetError {
            id,
            error: error.to_string(),
        })
    }

    pub(crate) fn insert_chunk(
        &self,
        request_id: i64,
        chunk: &str,
        seq: i64,
        created_at: i64,
    ) -> Result<(), sqlite::Error> {
        self.send(DbCmd::InsertChunk {
            request_id,
            chunk: chunk.to_string(),
            seq,
            created_at,
        })
    }

    pub(crate) fn load_all(&self) -> Result<Vec<StoredEntry>, sqlite::Error> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(DbCmd::LoadAll { reply: reply_tx })?;
        reply_rx.recv().map_err(|_| sqlite::Error {
            code: None,
            message: Some("db writer thread disconnected".into()),
        })?
    }

    pub(crate) fn load_chunks(&self, request_id: i64) -> Result<Vec<ChunkRecord>, sqlite::Error> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(DbCmd::LoadChunks {
            request_id,
            reply: reply_tx,
        })?;
        reply_rx.recv().map_err(|_| sqlite::Error {
            code: None,
            message: Some("db writer thread disconnected".into()),
        })?
    }

    #[allow(dead_code)]
    pub(crate) fn clear(&self) -> Result<(), sqlite::Error> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(DbCmd::Clear { reply: reply_tx })?;
        reply_rx.recv().map_err(|_| sqlite::Error {
            code: None,
            message: Some("db writer thread disconnected".into()),
        })?
    }

    // ── Collection management (used by storage traits) ───────────────────────

    pub(crate) fn load_all_collection_nodes(
        &self,
    ) -> Result<Vec<CollectionNodeRow>, sqlite::Error> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(DbCmd::LoadAllCollectionNodes { reply: reply_tx })?;
        reply_rx.recv().map_err(|_| sqlite::Error {
            code: None,
            message: Some("db writer thread disconnected".into()),
        })?
    }

    pub(crate) fn insert_collection_request_inner(
        &self,
        collection_id: i64,
        name: &str,
        method: &str,
        uri: &str,
        timestamp: i64,
    ) -> Result<i64, sqlite::Error> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(DbCmd::InsertCollectionRequest {
            collection_id,
            name: name.to_string(),
            method: method.to_string(),
            uri: uri.to_string(),
            timestamp,
            reply: reply_tx,
        })?;
        reply_rx.recv().map_err(|_| sqlite::Error {
            code: None,
            message: Some("db writer thread disconnected".into()),
        })?
    }

    pub(crate) fn update_collection_request_inner(
        &self,
        id: i64,
        method: &str,
        uri: &str,
        headers: &str,
        query: &str,
        body: Option<&str>,
        body_type: &str,
        cookies: &str,
        auth_type: &str,
        auth_data: &str,
    ) -> Result<(), sqlite::Error> {
        self.send(DbCmd::UpdateCollectionRequest {
            id,
            method: method.to_string(),
            uri: uri.to_string(),
            headers: headers.to_string(),
            query: query.to_string(),
            body: body.map(String::from),
            body_type: body_type.to_string(),
            cookies: cookies.to_string(),
            auth_type: auth_type.to_string(),
            auth_data: auth_data.to_string(),
        })
    }

    pub(crate) fn duplicate_collection_request_inner(
        &self,
        id: i64,
        timestamp: i64,
    ) -> Result<i64, sqlite::Error> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(DbCmd::DuplicateCollectionRequest {
            id,
            timestamp,
            reply: reply_tx,
        })?;
        reply_rx.recv().map_err(|_| sqlite::Error {
            code: None,
            message: Some("db writer thread disconnected".into()),
        })?
    }

    pub(crate) fn find_requests_by_ids_inner(
        &self,
        ids: &[i64],
    ) -> Result<Vec<RequestRow>, sqlite::Error> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(DbCmd::FindRequestsByIds {
            ids: ids.to_vec(),
            reply: reply_tx,
        })?;
        reply_rx.recv().map_err(|_| sqlite::Error {
            code: None,
            message: Some("db writer thread disconnected".into()),
        })?
    }

    pub(crate) fn create_collection_inner(
        &self,
        name: &str,
        timestamp: i64,
    ) -> Result<i64, sqlite::Error> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(DbCmd::CreateCollection {
            name: name.to_string(),
            timestamp,
            reply: reply_tx,
        })?;
        reply_rx.recv().map_err(|_| sqlite::Error {
            code: None,
            message: Some("db writer thread disconnected".into()),
        })?
    }

    pub(crate) fn create_folder_inner(
        &self,
        parent_id: i64,
        name: &str,
        timestamp: i64,
    ) -> Result<i64, sqlite::Error> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(DbCmd::CreateFolder {
            parent_id,
            name: name.to_string(),
            timestamp,
            reply: reply_tx,
        })?;
        reply_rx.recv().map_err(|_| sqlite::Error {
            code: None,
            message: Some("db writer thread disconnected".into()),
        })?
    }

    pub(crate) fn create_request_node_inner(
        &self,
        parent_id: i64,
        name: &str,
        request_id: i64,
        timestamp: i64,
    ) -> Result<i64, sqlite::Error> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(DbCmd::CreateRequestNode {
            parent_id,
            name: name.to_string(),
            request_id,
            timestamp,
            reply: reply_tx,
        })?;
        reply_rx.recv().map_err(|_| sqlite::Error {
            code: None,
            message: Some("db writer thread disconnected".into()),
        })?
    }

    pub(crate) fn rename_node_inner(
        &self,
        id: i64,
        new_name: &str,
        timestamp: i64,
    ) -> Result<(), sqlite::Error> {
        self.send(DbCmd::RenameNode {
            id,
            new_name: new_name.to_string(),
            timestamp,
        })
    }

    pub(crate) fn move_node_inner(
        &self,
        id: i64,
        new_parent_id: i64,
        timestamp: i64,
    ) -> Result<(), sqlite::Error> {
        self.send(DbCmd::MoveNode {
            id,
            new_parent_id,
            timestamp,
        })
    }

    pub(crate) fn delete_node_subtree_inner(&self, id: i64) -> Result<(), sqlite::Error> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(DbCmd::DeleteNodeSubtree {
            id,
            reply: reply_tx,
        })?;
        reply_rx.recv().map_err(|_| sqlite::Error {
            code: None,
            message: Some("db writer thread disconnected".into()),
        })?
    }

    pub(crate) fn delete_node_if_not_last_inner(&self, node_id: i64) -> Result<(), sqlite::Error> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(DbCmd::DeleteNodeIfNotLast {
            node_id,
            reply: reply_tx,
        })?;
        reply_rx.recv().map_err(|_| sqlite::Error {
            code: None,
            message: Some("db writer thread disconnected".into()),
        })?
    }
}

// ── Writer thread ────────────────────────────────────────────────────────────

fn migrate(conn: &sqlite::Connection) -> Result<(), sqlite::Error> {
    conn.execute("PRAGMA journal_mode=WAL")?;
    conn.execute("PRAGMA foreign_keys = ON")?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS collection_nodes (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            parent_id   INTEGER NOT NULL DEFAULT 0,
            name        TEXT NOT NULL,
            node_type   TEXT NOT NULL,
            request_id  INTEGER,
            sort_order  INTEGER DEFAULT 0,
            created_at  INTEGER NOT NULL,
            updated_at  INTEGER NOT NULL
        )",
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS requests (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_type TEXT NOT NULL DEFAULT 'traffic',
            collection_id INTEGER,
            name TEXT,
            method TEXT NOT NULL,
            uri TEXT NOT NULL,
            request_timestamp INTEGER NOT NULL,
            request_headers TEXT NOT NULL DEFAULT '[]',
            request_body TEXT,
            body_type TEXT,
            auth_type TEXT,
            auth_data TEXT,
            request_query TEXT DEFAULT '[]',
            cookies TEXT DEFAULT '[]',
            status INTEGER,
            response_timestamp INTEGER,
            duration_ms INTEGER,
            response_headers TEXT,
            response_body TEXT,
            error TEXT,
            edited INTEGER NOT NULL DEFAULT 0
        )",
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS response_chunks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            request_id INTEGER NOT NULL,
            chunk TEXT NOT NULL,
            seq INTEGER NOT NULL,
            created_at INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (request_id) REFERENCES requests(id)
        )",
    )?;
    Ok(())
}

/// The writer-loop: owns the single write connection, processes commands sequentially.
fn writer_loop(conn: sqlite::Connection, rx: mpsc::Receiver<DbCmd>, db_path: String) {
    while let Ok(cmd) = rx.recv() {
        match cmd {
            DbCmd::Shutdown => break,

            // ── Traffic logging ──────────────────────────────────────────
            DbCmd::UpsertRequest {
                method,
                uri,
                timestamp,
                headers_json,
                query_json,
                body,
                edited,
                source_type,
                collection_id,
                cookies_json,
                body_type,
                auth_type,
                auth_data,
                reply,
            } => {
                let result = do_upsert_request(
                    &conn,
                    &method,
                    &uri,
                    timestamp,
                    &headers_json,
                    &query_json,
                    body.as_deref(),
                    edited,
                    &source_type,
                    collection_id,
                    &cookies_json,
                    &body_type,
                    &auth_type,
                    &auth_data,
                );
                if let Some(reply) = reply {
                    reply.send(result).ok();
                }
            }

            DbCmd::UpdateResponse {
                id,
                status,
                timestamp,
                duration_ms,
                headers_json,
            } => {
                do_update_response(&conn, id, status, timestamp, duration_ms, &headers_json)
                    .unwrap_or_else(|e| log::warn!("update_response: {e}"));
            }

            DbCmd::UpdateResponseBody { id, body } => {
                do_update_response_body(&conn, id, &body)
                    .unwrap_or_else(|e| log::warn!("update_response_body: {e}"));
            }

            DbCmd::SetError { id, error } => {
                do_set_error(&conn, id, &error).unwrap_or_else(|e| log::warn!("set_error: {e}"));
            }

            DbCmd::InsertChunk {
                request_id,
                chunk,
                seq,
                created_at,
            } => {
                do_insert_chunk(&conn, request_id, &chunk, seq, created_at)
                    .unwrap_or_else(|e| log::warn!("insert_chunk: {e}"));
            }

            DbCmd::LoadAll { reply } => {
                reply.send(do_load_all(&conn)).ok();
            }

            DbCmd::LoadChunks { request_id, reply } => {
                reply.send(do_load_chunks(&conn, request_id)).ok();
            }

            DbCmd::Clear { reply } => {
                let result = (|| {
                    conn.execute("DELETE FROM response_chunks")?;
                    conn.execute("DELETE FROM requests")?;
                    Ok(())
                })();
                reply.send(result).ok();
            }

            // ── Collection management ─────────────────────────────────────
            DbCmd::LoadAllCollectionNodes { reply } => {
                reply.send(do_load_all_collection_nodes(&conn)).ok();
            }

            DbCmd::InsertCollectionRequest {
                collection_id,
                name,
                method,
                uri,
                timestamp,
                reply,
            } => {
                reply
                    .send(do_insert_collection_request(
                        &conn,
                        collection_id,
                        &name,
                        &method,
                        &uri,
                        timestamp,
                    ))
                    .ok();
            }

            DbCmd::UpdateCollectionRequest {
                id,
                method,
                uri,
                headers,
                query,
                body,
                body_type,
                cookies,
                auth_type,
                auth_data,
            } => {
                do_update_collection_request(
                    &conn,
                    id,
                    &method,
                    &uri,
                    &headers,
                    &query,
                    body.as_deref(),
                    &body_type,
                    &cookies,
                    &auth_type,
                    &auth_data,
                )
                .unwrap_or_else(|e| log::warn!("update_collection_request: {e}"));
            }

            DbCmd::DuplicateCollectionRequest {
                id,
                timestamp,
                reply,
            } => {
                reply
                    .send(do_duplicate_collection_request(&conn, id, timestamp))
                    .ok();
            }

            DbCmd::FindRequestsByIds { ids, reply } => {
                reply.send(do_find_requests_by_ids(&conn, &ids)).ok();
            }

            DbCmd::CreateCollection {
                name,
                timestamp,
                reply,
            } => {
                reply
                    .send(do_create_collection(&conn, &name, timestamp))
                    .ok();
            }

            DbCmd::CreateFolder {
                parent_id,
                name,
                timestamp,
                reply,
            } => {
                reply
                    .send(do_create_folder(&conn, parent_id, &name, timestamp))
                    .ok();
            }

            DbCmd::CreateRequestNode {
                parent_id,
                name,
                request_id,
                timestamp,
                reply,
            } => {
                reply
                    .send(do_create_request_node(
                        &conn, parent_id, &name, request_id, timestamp,
                    ))
                    .ok();
            }

            DbCmd::RenameNode {
                id,
                new_name,
                timestamp,
            } => {
                do_rename_node(&conn, id, &new_name, timestamp)
                    .unwrap_or_else(|e| log::warn!("rename_node: {e}"));
            }

            DbCmd::MoveNode {
                id,
                new_parent_id,
                timestamp,
            } => {
                do_move_node(&conn, id, new_parent_id, timestamp)
                    .unwrap_or_else(|e| log::warn!("move_node: {e}"));
            }

            DbCmd::DeleteNodeSubtree { id, reply } => {
                reply.send(do_delete_node_subtree(&conn, id)).ok();
            }

            DbCmd::DeleteNodeIfNotLast { node_id, reply } => {
                reply.send(do_delete_node_if_not_last(&conn, node_id)).ok();
            }
        }
    }
    log::info!("db writer thread exiting (path={db_path})");
}

// ── SQL operations (called from writer thread) ───────────────────────────────

fn do_upsert_request(
    conn: &sqlite::Connection,
    method: &str,
    uri: &str,
    timestamp: i64,
    headers_json: &str,
    query_json: &str,
    body: Option<&str>,
    edited: bool,
    source_type: &str,
    collection_id: Option<i64>,
    cookies_json: &str,
    body_type: &str,
    auth_type: &str,
    auth_data: &str,
) -> Result<i64, sqlite::Error> {
    let mut stmt = conn.prepare(
        "INSERT INTO requests (source_type, collection_id, method, uri, request_timestamp, request_headers, request_query, cookies, request_body, body_type, auth_type, auth_data, edited) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )?;
    stmt.bind((1_usize, source_type))?;
    match collection_id {
        Some(cid) => stmt.bind((2_usize, cid as i64))?,
        None => stmt.bind((2_usize, sqlite::Value::Null))?,
    }
    stmt.bind((3_usize, method))?;
    stmt.bind((4_usize, uri))?;
    stmt.bind((5_usize, timestamp as i64))?;
    stmt.bind((6_usize, headers_json))?;
    stmt.bind((7_usize, query_json))?;
    stmt.bind((8_usize, cookies_json))?;
    match body {
        Some(b) => stmt.bind((9_usize, b))?,
        None => stmt.bind((9_usize, sqlite::Value::Null))?,
    }
    stmt.bind((10_usize, body_type))?;
    stmt.bind((11_usize, auth_type))?;
    stmt.bind((12_usize, auth_data))?;
    stmt.bind((13_usize, if edited { 1 } else { 0 }))?;
    stmt.next()?;
    let mut id_stmt = conn.prepare("SELECT last_insert_rowid()")?;
    id_stmt.next()?;
    Ok(id_stmt.read::<i64, _>(0)?)
}

fn do_update_response(
    conn: &sqlite::Connection,
    id: i64,
    status: u16,
    timestamp: i64,
    duration_ms: u64,
    headers_json: &str,
) -> Result<(), sqlite::Error> {
    let mut stmt = conn.prepare(
        "UPDATE requests SET status = ?, response_timestamp = ?, duration_ms = ?, response_headers = ? WHERE id = ?",
    )?;
    stmt.bind((1_usize, status as i64))?;
    stmt.bind((2_usize, timestamp as i64))?;
    stmt.bind((3_usize, duration_ms as i64))?;
    stmt.bind((4_usize, headers_json))?;
    stmt.bind((5_usize, id as i64))?;
    stmt.next()?;
    Ok(())
}

fn do_update_response_body(
    conn: &sqlite::Connection,
    id: i64,
    body: &str,
) -> Result<(), sqlite::Error> {
    let mut stmt = conn.prepare("UPDATE requests SET response_body = ? WHERE id = ?")?;
    stmt.bind((1_usize, body))?;
    stmt.bind((2_usize, id as i64))?;
    stmt.next()?;
    Ok(())
}

fn do_set_error(conn: &sqlite::Connection, id: i64, error: &str) -> Result<(), sqlite::Error> {
    let mut stmt = conn.prepare("UPDATE requests SET error = ? WHERE id = ?")?;
    stmt.bind((1_usize, error))?;
    stmt.bind((2_usize, id as i64))?;
    stmt.next()?;
    Ok(())
}

fn do_insert_chunk(
    conn: &sqlite::Connection,
    request_id: i64,
    chunk: &str,
    seq: i64,
    created_at: i64,
) -> Result<(), sqlite::Error> {
    let mut stmt = conn.prepare(
        "INSERT INTO response_chunks (request_id, chunk, seq, created_at) VALUES (?, ?, ?, ?)",
    )?;
    stmt.bind((1_usize, request_id as i64))?;
    stmt.bind((2_usize, chunk))?;
    stmt.bind((3_usize, seq))?;
    stmt.bind((4_usize, created_at))?;
    stmt.next()?;
    Ok(())
}

fn parse_kv_json(s: &str) -> HashMap<String, String> {
    if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(s) {
        let mut map = HashMap::new();
        for item in arr {
            if let (Some(k), Some(v)) = (
                item.get("key").and_then(|v| v.as_str()),
                item.get("value").and_then(|v| v.as_str()),
            ) {
                map.insert(k.to_string(), v.to_string());
            }
        }
        return map;
    }
    serde_json::from_str(s).unwrap_or_default()
}

fn do_load_all(conn: &sqlite::Connection) -> Result<Vec<StoredEntry>, sqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, source_type, collection_id, name, method, uri, request_timestamp, request_headers, request_body, body_type, auth_type, auth_data, request_query, cookies, status, response_timestamp, duration_ms, response_headers, response_body, error, edited FROM requests ORDER BY request_timestamp DESC",
    )?;
    let mut entries = Vec::new();
    while let sqlite::State::Row = stmt.next()? {
        let headers_str: String = stmt.read::<String, _>(7)?;
        let headers: HashMap<String, String> = parse_kv_json(&headers_str);
        let query_str: Option<String> = stmt.read::<Option<String>, _>(12)?;
        let query: Option<HashMap<String, String>> = query_str.map(|s| parse_kv_json(&s));
        let resp_headers_str: Option<String> = stmt.read::<Option<String>, _>(18)?;
        let resp_headers: Option<HashMap<String, String>> =
            resp_headers_str.and_then(|s| serde_json::from_str(&s).ok());
        let edited_int: i64 = stmt.read::<i64, _>(21)?;

        entries.push(StoredEntry {
            id: stmt.read::<i64, _>(0)?.to_string(),
            source_type: stmt.read::<Option<String>, _>(1)?,
            collection_id: stmt.read::<Option<i64>, _>(2)?.map(|v| v.to_string()),
            name: stmt.read::<Option<String>, _>(3)?,
            method: stmt.read::<String, _>(4)?,
            uri: stmt.read::<String, _>(5)?,
            request_timestamp: stmt.read::<i64, _>(6)?,
            request_headers: headers,
            request_body: stmt.read::<Option<String>, _>(8)?,
            body_type: stmt.read::<Option<String>, _>(9)?,
            auth_type: stmt.read::<Option<String>, _>(10)?,
            auth_data: stmt.read::<Option<String>, _>(11)?,
            request_query: query,
            cookies: stmt.read::<Option<String>, _>(13)?,
            status: stmt.read::<Option<i64>, _>(15)?.map(|v| v as u16),
            response_timestamp: stmt.read::<Option<i64>, _>(16)?,
            duration_ms: stmt.read::<Option<i64>, _>(17)?.map(|v| v as u64),
            response_headers: resp_headers,
            response_body: stmt.read::<Option<String>, _>(19)?,
            error: stmt.read::<Option<String>, _>(20)?,
            edited: if edited_int != 0 { Some(true) } else { None },
            response_chunks: Vec::new(),
        });
    }
    Ok(entries)
}

fn do_load_chunks(
    conn: &sqlite::Connection,
    request_id: i64,
) -> Result<Vec<ChunkRecord>, sqlite::Error> {
    let mut stmt =
        conn.prepare("SELECT chunk FROM response_chunks WHERE request_id = ? ORDER BY seq")?;
    stmt.bind((1_usize, request_id as i64))?;
    let mut chunks = Vec::new();
    while let sqlite::State::Row = stmt.next()? {
        chunks.push(ChunkRecord {
            data: stmt.read::<String, _>(0)?,
        });
    }
    Ok(chunks)
}

// ── Collection management SQL ────────────────────────────────────────────────

fn do_load_all_collection_nodes(
    conn: &sqlite::Connection,
) -> Result<Vec<CollectionNodeRow>, sqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, parent_id, name, node_type, request_id, sort_order, created_at, updated_at FROM collection_nodes ORDER BY sort_order, created_at",
    )?;
    let mut nodes = Vec::new();
    while let sqlite::State::Row = stmt.next()? {
        nodes.push(CollectionNodeRow {
            id: stmt.read::<i64, _>(0)?,
            parent_id: stmt.read::<i64, _>(1)?,
            name: stmt.read::<String, _>(2)?,
            node_type: stmt.read::<String, _>(3)?,
            request_id: stmt.read::<Option<i64>, _>(4)?,
            sort_order: stmt.read::<i64, _>(5)?,
            created_at: stmt.read::<i64, _>(6)?,
            updated_at: stmt.read::<i64, _>(7)?,
        });
    }
    Ok(nodes)
}

fn do_insert_collection_request(
    conn: &sqlite::Connection,
    collection_id: i64,
    name: &str,
    method: &str,
    uri: &str,
    timestamp: i64,
) -> Result<i64, sqlite::Error> {
    let mut stmt = conn.prepare(
        "INSERT INTO requests (source_type, collection_id, name, method, uri, request_timestamp, request_headers, request_query, cookies, body_type, auth_type, auth_data, edited) VALUES ('collection', ?, ?, ?, ?, ?, '[]', '[]', '[]', '', '', '', 0)",
    )?;
    stmt.bind((1_usize, collection_id as i64))?;
    stmt.bind((2_usize, name))?;
    stmt.bind((3_usize, method))?;
    stmt.bind((4_usize, uri))?;
    stmt.bind((5_usize, timestamp as i64))?;
    stmt.next()?;
    let mut id_stmt = conn.prepare("SELECT last_insert_rowid()")?;
    id_stmt.next()?;
    Ok(id_stmt.read::<i64, _>(0)?)
}

fn do_update_collection_request(
    conn: &sqlite::Connection,
    id: i64,
    method: &str,
    uri: &str,
    headers: &str,
    query: &str,
    body: Option<&str>,
    body_type: &str,
    cookies: &str,
    auth_type: &str,
    auth_data: &str,
) -> Result<(), sqlite::Error> {
    let mut stmt = conn.prepare(
        "UPDATE requests SET method = ?, uri = ?, request_headers = ?, request_query = ?, request_body = ?, body_type = ?, cookies = ?, auth_type = ?, auth_data = ? WHERE id = ?",
    )?;
    stmt.bind((1_usize, method))?;
    stmt.bind((2_usize, uri))?;
    stmt.bind((3_usize, headers))?;
    stmt.bind((4_usize, query))?;
    match body {
        Some(b) => stmt.bind((5_usize, b))?,
        None => stmt.bind((5_usize, sqlite::Value::Null))?,
    }
    stmt.bind((6_usize, body_type))?;
    stmt.bind((7_usize, cookies))?;
    stmt.bind((8_usize, auth_type))?;
    stmt.bind((9_usize, auth_data))?;
    stmt.bind((10_usize, id as i64))?;
    stmt.next()?;
    Ok(())
}

fn do_duplicate_collection_request(
    conn: &sqlite::Connection,
    id: i64,
    timestamp: i64,
) -> Result<i64, sqlite::Error> {
    let mut stmt = conn.prepare(
        "INSERT INTO requests (source_type, collection_id, name, method, uri, request_timestamp, request_headers, request_body, body_type, auth_type, auth_data, request_query, cookies, status, response_timestamp, duration_ms, response_headers, response_body, error, edited)
         SELECT source_type, collection_id, name, method, uri, ?, request_headers, request_body, body_type, auth_type, auth_data, request_query, cookies, NULL, NULL, NULL, NULL, NULL, NULL, 0
         FROM requests WHERE id = ?",
    )?;
    stmt.bind((1_usize, timestamp as i64))?;
    stmt.bind((2_usize, id as i64))?;
    stmt.next()?;
    let mut id_stmt = conn.prepare("SELECT last_insert_rowid()")?;
    id_stmt.next()?;
    Ok(id_stmt.read::<i64, _>(0)?)
}

fn do_find_requests_by_ids(
    conn: &sqlite::Connection,
    ids: &[i64],
) -> Result<Vec<RequestRow>, sqlite::Error> {
    let placeholders: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT id, method, uri, request_headers, request_body, request_query, cookies, body_type, auth_type, auth_data, name FROM requests WHERE id IN ({})",
        placeholders
    );
    let mut stmt = conn.prepare(sql)?;
    for (i, id) in ids.iter().enumerate() {
        stmt.bind(((i + 1) as usize, *id as i64))?;
    }
    let mut results = Vec::new();
    while let sqlite::State::Row = stmt.next()? {
        results.push(RequestRow {
            id: stmt.read::<i64, _>(0)?,
            method: stmt.read::<String, _>(1)?,
            uri: stmt.read::<String, _>(2)?,
            headers: stmt.read::<String, _>(3)?,
            body: stmt.read::<Option<String>, _>(4)?,
            query: stmt.read::<String, _>(5)?,
            cookies: stmt.read::<String, _>(6)?,
            body_type: stmt.read::<String, _>(7)?,
            auth_type: stmt.read::<String, _>(8)?,
            auth_data: stmt.read::<String, _>(9)?,
            name: stmt.read::<String, _>(10)?,
        });
    }
    Ok(results)
}

fn do_create_collection(
    conn: &sqlite::Connection,
    name: &str,
    timestamp: i64,
) -> Result<i64, sqlite::Error> {
    let mut stmt = conn.prepare(
        "INSERT INTO collection_nodes (parent_id, name, node_type, sort_order, created_at, updated_at) VALUES (0, ?, 'collection', 0, ?, ?)",
    )?;
    stmt.bind((1_usize, name))?;
    stmt.bind((2_usize, timestamp))?;
    stmt.bind((3_usize, timestamp))?;
    stmt.next()?;
    let mut id_stmt = conn.prepare("SELECT last_insert_rowid()")?;
    id_stmt.next()?;
    Ok(id_stmt.read::<i64, _>(0)?)
}

fn do_create_folder(
    conn: &sqlite::Connection,
    parent_id: i64,
    name: &str,
    timestamp: i64,
) -> Result<i64, sqlite::Error> {
    let mut stmt = conn.prepare(
        "INSERT INTO collection_nodes (parent_id, name, node_type, sort_order, created_at, updated_at) VALUES (?, ?, 'folder', 0, ?, ?)",
    )?;
    stmt.bind((1_usize, parent_id))?;
    stmt.bind((2_usize, name))?;
    stmt.bind((3_usize, timestamp))?;
    stmt.bind((4_usize, timestamp))?;
    stmt.next()?;
    let mut id_stmt = conn.prepare("SELECT last_insert_rowid()")?;
    id_stmt.next()?;
    Ok(id_stmt.read::<i64, _>(0)?)
}

fn do_create_request_node(
    conn: &sqlite::Connection,
    parent_id: i64,
    name: &str,
    request_id: i64,
    timestamp: i64,
) -> Result<i64, sqlite::Error> {
    let mut stmt = conn.prepare(
        "INSERT INTO collection_nodes (parent_id, name, node_type, request_id, sort_order, created_at, updated_at) VALUES (?, ?, 'request', ?, 0, ?, ?)",
    )?;
    stmt.bind((1_usize, parent_id))?;
    stmt.bind((2_usize, name))?;
    stmt.bind((3_usize, request_id))?;
    stmt.bind((4_usize, timestamp))?;
    stmt.bind((5_usize, timestamp))?;
    stmt.next()?;
    let mut id_stmt = conn.prepare("SELECT last_insert_rowid()")?;
    id_stmt.next()?;
    Ok(id_stmt.read::<i64, _>(0)?)
}

fn do_rename_node(
    conn: &sqlite::Connection,
    id: i64,
    new_name: &str,
    timestamp: i64,
) -> Result<(), sqlite::Error> {
    let mut stmt =
        conn.prepare("UPDATE collection_nodes SET name = ?, updated_at = ? WHERE id = ?")?;
    stmt.bind((1_usize, new_name))?;
    stmt.bind((2_usize, timestamp))?;
    stmt.bind((3_usize, id))?;
    stmt.next()?;

    // Also update the name in the requests table when renaming a request-type node
    let mut stmt = conn.prepare(
        "UPDATE requests SET name = ? WHERE id = (SELECT request_id FROM collection_nodes WHERE id = ? AND request_id IS NOT NULL)",
    )?;
    stmt.bind((1_usize, new_name))?;
    stmt.bind((2_usize, id))?;
    stmt.next()?;

    Ok(())
}

fn do_move_node(
    conn: &sqlite::Connection,
    id: i64,
    new_parent_id: i64,
    timestamp: i64,
) -> Result<(), sqlite::Error> {
    let mut stmt =
        conn.prepare("UPDATE collection_nodes SET parent_id = ?, updated_at = ? WHERE id = ?")?;
    stmt.bind((1_usize, new_parent_id))?;
    stmt.bind((2_usize, timestamp))?;
    stmt.bind((3_usize, id))?;
    stmt.next()?;
    Ok(())
}

fn do_delete_node_subtree(conn: &sqlite::Connection, id: i64) -> Result<(), sqlite::Error> {
    // 1. Collect all descendant node IDs recursively
    let mut to_delete: Vec<i64> = Vec::new();
    let mut queue: Vec<i64> = vec![id];
    while let Some(current_id) = queue.pop() {
        to_delete.push(current_id);
        let mut stmt = conn.prepare("SELECT id FROM collection_nodes WHERE parent_id = ?")?;
        stmt.bind((1_usize, current_id))?;
        while let sqlite::State::Row = stmt.next()? {
            let child_id: i64 = stmt.read::<i64, _>(0)?;
            queue.push(child_id);
        }
    }

    if to_delete.is_empty() {
        return Ok(());
    }

    // 2. Collect request_ids from request-type nodes in the subtree
    let placeholders: String = to_delete.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT request_id FROM collection_nodes WHERE id IN ({}) AND node_type = 'request' AND request_id IS NOT NULL",
        placeholders
    );
    let mut stmt = conn.prepare(sql)?;
    for (i, node_id) in to_delete.iter().enumerate() {
        stmt.bind(((i + 1) as usize, *node_id))?;
    }
    let mut request_ids: Vec<i64> = Vec::new();
    while let sqlite::State::Row = stmt.next()? {
        request_ids.push(stmt.read::<i64, _>(0)?);
    }

    // 3. Delete associated request rows
    if !request_ids.is_empty() {
        let req_placeholders: String = request_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let req_sql = format!("DELETE FROM requests WHERE id IN ({})", req_placeholders);
        let mut req_stmt = conn.prepare(req_sql)?;
        for (i, req_id) in request_ids.iter().enumerate() {
            req_stmt.bind(((i + 1) as usize, *req_id))?;
        }
        req_stmt.next()?;
    }

    // 4. Delete the nodes themselves
    let del_sql = format!(
        "DELETE FROM collection_nodes WHERE id IN ({})",
        placeholders
    );
    let mut del_stmt = conn.prepare(del_sql)?;
    for (i, node_id) in to_delete.iter().enumerate() {
        del_stmt.bind(((i + 1) as usize, *node_id))?;
    }
    del_stmt.next()?;

    Ok(())
}

/// Atomically checks that `node_id` is NOT the last root collection, then
/// deletes the subtree. Returns `Err` if it IS the last collection, or on any
/// SQL error.
fn do_delete_node_if_not_last(
    conn: &sqlite::Connection,
    node_id: i64,
) -> Result<(), sqlite::Error> {
    // 1. Count root collections and check if node_id is a root collection
    let mut stmt = conn.prepare(
        "SELECT COUNT(*) FROM collection_nodes WHERE parent_id = 0 AND node_type = 'collection'",
    )?;
    stmt.next()?;
    let total: i64 = stmt.read::<i64, _>(0)?;

    let mut stmt = conn.prepare(
        "SELECT id FROM collection_nodes WHERE id = ? AND parent_id = 0 AND node_type = 'collection'",
    )?;
    stmt.bind((1_usize, node_id))?;
    let is_root = matches!(stmt.next(), Ok(sqlite::State::Row));

    if is_root && total <= 1 {
        return Err(sqlite::Error {
            code: None,
            message: Some("cannot delete the last collection".into()),
        });
    }

    do_delete_node_subtree(conn, node_id)
}
