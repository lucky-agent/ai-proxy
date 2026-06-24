use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

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

pub(crate) struct Db {
    conn: Option<sqlite::Connection>,
    db_path: Option<String>,
    cleanup_done: AtomicBool,
}

impl Db {
    pub(crate) fn open(path: &PathBuf) -> Result<Self, sqlite::Error> {
        let db_path = path.to_string_lossy().to_string();
        let conn = sqlite::open(path)?;
        let db = Self {
            conn: Some(conn),
            db_path: Some(db_path),
            cleanup_done: AtomicBool::new(false),
        };
        Ok(db)
    }

    pub(crate) fn noop() -> Self {
        Self {
            conn: None,
            db_path: None,
            cleanup_done: AtomicBool::new(false),
        }
    }

    /// Return a reference to the inner connection, if any.
    pub(crate) fn conn_ref(&self) -> Option<&sqlite::Connection> {
        self.conn.as_ref()
    }

    /// Create a temporary Db instance for use inside spawned threads.
    fn ephemeral(conn: sqlite::Connection) -> Self {
        Self {
            conn: Some(conn),
            db_path: None,
            cleanup_done: AtomicBool::new(false),
        }
    }

    fn migrate(&self) -> Result<(), sqlite::Error> {
        let conn = match self.conn {
            Some(ref conn) => conn,
            None => return Ok(()),
        };
        conn.execute("PRAGMA foreign_keys = ON")?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS collection_nodes (
                id          TEXT PRIMARY KEY,
                parent_id   TEXT NOT NULL DEFAULT '0',
                name        TEXT NOT NULL,
                node_type   TEXT NOT NULL,
                request_id  TEXT,
                sort_order  INTEGER DEFAULT 0,
                created_at  INTEGER NOT NULL,
                updated_at  INTEGER NOT NULL
            )",
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS requests (
                id TEXT PRIMARY KEY,
                source_type TEXT NOT NULL DEFAULT 'traffic',
                collection_id TEXT,
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
                request_id TEXT NOT NULL,
                chunk TEXT NOT NULL,
                seq INTEGER NOT NULL,
                created_at INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (request_id) REFERENCES requests(id)
            )",
        )?;
        Ok(())
    }

    pub(crate) fn upsert_request(
        &self,
        id: &str,
        method: &str,
        uri: &str,
        timestamp: i64,
        headers_json: &str,
        query_json: &str,
        body: Option<&str>,
        edited: bool,
        source_type: &str,
        collection_id: Option<&str>,
        cookies_json: &str,
        body_type: &str,
        auth_type: &str,
        auth_data: &str,
    ) -> Result<(), sqlite::Error> {
        let conn = match self.conn {
            Some(ref conn) => conn,
            None => return Ok(()),
        };
        let mut stmt = conn.prepare(
            "INSERT INTO requests (id, source_type, collection_id, method, uri, request_timestamp, request_headers, request_query, cookies, request_body, body_type, auth_type, auth_data, edited) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )?;
        stmt.bind((1_usize, id))?;
        stmt.bind((2_usize, source_type))?;
        match collection_id {
            Some(cid) => stmt.bind((3_usize, cid))?,
            None => stmt.bind((3_usize, sqlite::Value::Null))?,
        }
        stmt.bind((4_usize, method))?;
        stmt.bind((5_usize, uri))?;
        stmt.bind((6_usize, timestamp as i64))?;
        stmt.bind((7_usize, headers_json))?;
        stmt.bind((8_usize, query_json))?;
        stmt.bind((9_usize, cookies_json))?;
        match body {
            Some(b) => stmt.bind((10_usize, b))?,
            None => stmt.bind((10_usize, sqlite::Value::Null))?,
        }
        stmt.bind((11_usize, body_type))?;
        stmt.bind((12_usize, auth_type))?;
        stmt.bind((13_usize, auth_data))?;
        stmt.bind((14_usize, if edited { 1 } else { 0 }))?;
        stmt.next()?;
        Ok(())
    }

    pub(crate) fn update_response(
        &self,
        id: &str,
        status: u16,
        timestamp: i64,
        duration_ms: u64,
        headers_json: &str,
    ) -> Result<(), sqlite::Error> {
        let conn = match self.conn {
            Some(ref conn) => conn,
            None => return Ok(()),
        };
        let mut stmt = conn.prepare(
            "UPDATE requests SET status = ?, response_timestamp = ?, duration_ms = ?, response_headers = ? WHERE id = ?",
        )?;
        stmt.bind((1_usize, status as i64))?;
        stmt.bind((2_usize, timestamp as i64))?;
        stmt.bind((3_usize, duration_ms as i64))?;
        stmt.bind((4_usize, headers_json))?;
        stmt.bind((5_usize, id))?;
        stmt.next()?;
        Ok(())
    }

    pub(crate) fn update_response_body(&self, id: &str, body: &str) -> Result<(), sqlite::Error> {
        let conn = match self.conn {
            Some(ref conn) => conn,
            None => return Ok(()),
        };
        let mut stmt = conn.prepare("UPDATE requests SET response_body = ? WHERE id = ?")?;
        stmt.bind((1_usize, body))?;
        stmt.bind((2_usize, id))?;
        stmt.next()?;
        Ok(())
    }

    pub(crate) fn set_error(&self, id: &str, error: &str) -> Result<(), sqlite::Error> {
        let conn = match self.conn {
            Some(ref conn) => conn,
            None => return Ok(()),
        };
        let mut stmt = conn.prepare("UPDATE requests SET error = ? WHERE id = ?")?;
        stmt.bind((1_usize, error))?;
        stmt.bind((2_usize, id))?;
        stmt.next()?;
        Ok(())
    }

    /// Convert an HTTP HeaderMap to a JSON array of key-value pairs.
    /// Duplicate Set-Cookie headers are emitted as separate array elements.
    pub(crate) fn headers_to_json(headers: &HeaderMap) -> String {
        let mut pairs: Vec<serde_json::Value> = Vec::new();
        for (k, v) in headers.iter() {
            let key = k.to_string();
            let val = v.to_str().ok().unwrap_or("");
            if key.to_lowercase() == "set-cookie" {
                // Split cookies by `; `? No — each Set-Cookie line is one cookie.
                // Emit each occurrence as a separate pair (loop already iterates each).
                pairs.push(serde_json::json!({"key": key, "value": val}));
            } else {
                pairs.push(serde_json::json!({"key": key, "value": val}));
            }
        }
        serde_json::to_string(&pairs).unwrap_or_else(|_| "[]".to_string())
    }

    /// Extract URI query parameters as a JSON array of key-value pairs.
    pub(crate) fn query_to_json(uri: &rama::http::Uri) -> String {
        let q_str = uri.query().unwrap_or("");
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

    /// Spawn a background thread to upsert request. Returns immediately.
    pub(crate) fn upsert_request_async(
        &self,
        id: &str,
        method: &str,
        uri: &str,
        timestamp: i64,
        headers_json: &str,
        query_json: &str,
        body: Option<&str>,
        edited: bool,
        retention_days: u32,
    ) {
        let path = match self.db_path {
            Some(ref p) => p.clone(),
            None => return,
        };
        // Trigger async cleanup once per session.
        if retention_days > 0 && !self.cleanup_done.swap(true, Ordering::SeqCst) {
            let cleanup_path = path.clone();
            let retention = retention_days;
            std::thread::spawn(move || {
                if let Ok(conn) = sqlite::open(&cleanup_path) {
                    let cutoff =
                        chrono::Utc::now().timestamp_millis() - (retention as i64) * 86_400_000;
                    // Delete chunks older than cutoff, then requests.
                    if let Ok(mut stmt) =
                        conn.prepare("DELETE FROM response_chunks WHERE created_at < ?")
                    {
                        stmt.bind((1_usize, cutoff)).ok();
                        stmt.next().ok();
                    }
                    if let Ok(mut stmt) =
                        conn.prepare("DELETE FROM requests WHERE request_timestamp < ?")
                    {
                        stmt.bind((1_usize, cutoff)).ok();
                        stmt.next().ok();
                    }
                }
            });
        }
        let rid = id.to_string();
        let m = method.to_string();
        let u = uri.to_string();
        let ts = timestamp;
        let h = headers_json.to_string();
        let q = query_json.to_string();
        let b = body.map(String::from);
        let ed = edited;
        std::thread::spawn(move || {
            if let Ok(conn) = sqlite::open(&path) {
                let db = Self::ephemeral(conn);
                if let Err(e) = db.upsert_request(
                    &rid, &m, &u, ts, &h, &q, b.as_deref(), ed,
                    "traffic", None, "[]", "", "", "",
                ) {
                    log::warn!("upsert_request_async failed: {e}");
                }
            }
        });
    }

    /// Spawn a background thread to update response. Returns immediately.
    pub(crate) fn update_response_async(
        &self,
        id: &str,
        status: u16,
        timestamp: i64,
        duration_ms: u64,
        headers_json: &str,
    ) {
        let path = match self.db_path {
            Some(ref p) => p.clone(),
            None => return,
        };
        let rid = id.to_string();
        let st = status;
        let ts = timestamp;
        let dur = duration_ms;
        let h = headers_json.to_string();
        std::thread::spawn(move || {
            if let Ok(conn) = sqlite::open(&path) {
                let db = Self::ephemeral(conn);
                if let Err(e) = db.update_response(&rid, st, ts, dur, &h) {
                    log::warn!("update_response_async failed: {e}");
                }
            }
        });
    }

    /// Spawn a background thread to set error. Returns immediately.
    pub(crate) fn set_error_async(&self, id: &str, error: &str) {
        let path = match self.db_path {
            Some(ref p) => p.clone(),
            None => return,
        };
        let rid = id.to_string();
        let err = error.to_string();
        std::thread::spawn(move || {
            if let Ok(conn) = sqlite::open(&path) {
                let db = Self::ephemeral(conn);
                if let Err(e) = db.set_error(&rid, &err) {
                    log::warn!("set_error_async failed: {e}");
                }
            }
        });
    }

    /// Synchronous insert of a single response chunk.
    pub(crate) fn insert_chunk(
        &self,
        request_id: &str,
        chunk: &str,
        seq: i64,
        created_at: i64,
    ) -> Result<(), sqlite::Error> {
        let conn = match self.conn {
            Some(ref conn) => conn,
            None => return Ok(()),
        };
        let mut stmt = conn.prepare(
            "INSERT INTO response_chunks (request_id, chunk, seq, created_at) VALUES (?, ?, ?, ?)",
        )?;
        stmt.bind((1_usize, request_id))?;
        stmt.bind((2_usize, chunk))?;
        stmt.bind((3_usize, seq))?;
        stmt.bind((4_usize, created_at))?;
        stmt.next()?;
        Ok(())
    }

    /// Spawn a background thread to insert a response chunk. Returns immediately.
    #[allow(dead_code)]
    pub(crate) fn insert_chunk_async(&self, request_id: &str, chunk: &str, seq: i64) {
        let path = match self.db_path {
            Some(ref p) => p.clone(),
            None => return,
        };
        let rid = request_id.to_string();
        let c = chunk.to_string();
        let s = seq;
        let ts = chrono::Utc::now().timestamp_millis();
        std::thread::spawn(move || {
            if let Ok(conn) = sqlite::open(&path) {
                let db = Self::ephemeral(conn);
                if let Err(e) = db.insert_chunk(&rid, &c, s, ts) {
                    log::warn!("insert_chunk_async failed: {e}");
                }
            }
        });
    }

    /// Spawn a background thread to update the full response body. Returns immediately.
    #[allow(dead_code)]
    pub(crate) fn update_response_body_async(&self, id: &str, body: &str) {
        let path = match self.db_path {
            Some(ref p) => p.clone(),
            None => return,
        };
        let rid = id.to_string();
        let b = body.to_string();
        std::thread::spawn(move || {
            if let Ok(conn) = sqlite::open(&path) {
                let db = Self::ephemeral(conn);
                if let Err(e) = db.update_response_body(&rid, &b) {
                    log::warn!("update_response_body_async failed: {e}");
                }
            }
        });
    }

    /// Return the DB path for async spawning, or None when persistence is disabled.
    pub(crate) fn async_path(&self) -> Option<String> {
        self.db_path.clone()
    }

    /// Parse JSON headers or query from either array format `[{"key":"x","value":"y"}]`
    /// or legacy object format `{"key":"value"}`, returning a HashMap.
    fn parse_kv_json(s: &str) -> HashMap<String, String> {
        // Try array format first: [{"key":"x","value":"y"}]
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
        // Fallback to legacy object format {"key":"value"}
        serde_json::from_str(s).unwrap_or_default()
    }

    pub(crate) fn load_all(&self) -> Result<Vec<StoredEntry>, sqlite::Error> {
        let conn = match self.conn {
            Some(ref conn) => conn,
            None => return Ok(Vec::new()),
        };
        let mut stmt = conn.prepare(
            "SELECT id, source_type, collection_id, name, method, uri, request_timestamp, request_headers, request_body, body_type, auth_type, auth_data, request_query, cookies, status, response_timestamp, duration_ms, response_headers, response_body, error, edited FROM requests ORDER BY request_timestamp DESC",
        )?;
        let mut entries = Vec::new();
        while let sqlite::State::Row = stmt.next()? {
            let headers_str: String = stmt.read::<String, _>(7)?;
            let headers: HashMap<String, String> =
                Self::parse_kv_json(&headers_str);
            let query_str: Option<String> = stmt.read::<Option<String>, _>(12)?;
            let query: Option<HashMap<String, String>> =
                query_str.map(|s| Self::parse_kv_json(&s));
            let resp_headers_str: Option<String> = stmt.read::<Option<String>, _>(18)?;
            let resp_headers: Option<HashMap<String, String>> =
                resp_headers_str.and_then(|s| serde_json::from_str(&s).ok());
            let edited_int: i64 = stmt.read::<i64, _>(21)?;

            entries.push(StoredEntry {
                id: stmt.read::<String, _>(0)?,
                source_type: stmt.read::<Option<String>, _>(1)?,
                collection_id: stmt.read::<Option<String>, _>(2)?,
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

    /// Load response chunks for a given request, ordered by seq.
    pub(crate) fn load_chunks(&self, request_id: &str) -> Result<Vec<ChunkRecord>, sqlite::Error> {
        let conn = match self.conn {
            Some(ref conn) => conn,
            None => return Ok(Vec::new()),
        };
        let mut stmt =
            conn.prepare("SELECT chunk FROM response_chunks WHERE request_id = ? ORDER BY seq")?;
        stmt.bind((1_usize, request_id))?;
        let mut chunks = Vec::new();
        while let sqlite::State::Row = stmt.next()? {
            chunks.push(ChunkRecord {
                data: stmt.read::<String, _>(0)?,
            });
        }
        Ok(chunks)
    }

    #[allow(dead_code)]
    pub(crate) fn clear(&self) -> Result<(), sqlite::Error> {
        let conn = match self.conn {
            Some(ref conn) => conn,
            None => return Ok(()),
        };
        conn.execute("DELETE FROM response_chunks")?;
        conn.execute("DELETE FROM requests")?;
        Ok(())
    }
}

/// Spawn a background thread to insert a response chunk (free-function variant).
/// Usable when a Db instance is not available (e.g. from the body stream map closure).
pub(crate) fn spawn_insert_chunk(
    path: &str,
    request_id: &str,
    chunk: &str,
    seq: i64,
    created_at: i64,
) {
    let p = path.to_string();
    let rid = request_id.to_string();
    let c = chunk.to_string();
    let ts = created_at;
    std::thread::spawn(move || {
        if let Ok(conn) = sqlite::open(&p) {
            let db = Db::ephemeral(conn);
            if let Err(e) = db.insert_chunk(&rid, &c, seq, ts) {
                log::warn!("spawn_insert_chunk failed: {e}");
            }
        }
    });
}

/// Spawn a background thread to update the full response body (free-function variant).
pub(crate) fn spawn_update_response_body(path: &str, id: &str, body: &str) {
    let p = path.to_string();
    let rid = id.to_string();
    let b = body.to_string();
    std::thread::spawn(move || {
        if let Ok(conn) = sqlite::open(&p) {
            let db = Db::ephemeral(conn);
            if let Err(e) = db.update_response_body(&rid, &b) {
                log::warn!("spawn_update_response_body failed: {e}");
            }
        }
    });
}
