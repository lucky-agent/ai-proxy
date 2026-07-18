use std::collections::HashMap;
use std::sync::mpsc;

use serde::Serialize;
use sqlite;

use crate::storage::DbTable;

// ── Table marker ──────────────────────────────────────────────────────────────

pub(crate) struct TrafficTable;

/// A row / serialisable entry for the `traffic_logs` table.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct TrafficLogEntry {
    pub id: String,
    pub method: String,
    pub uri: String,
    #[serde(rename = "requestTimestamp")]
    pub request_timestamp: i64,
    #[serde(rename = "requestHeaders")]
    pub request_headers: HashMap<String, String>,
    #[serde(rename = "requestBody")]
    pub request_body: Option<String>,
    #[serde(rename = "requestQuery")]
    pub request_query: Option<HashMap<String, String>>,
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
    #[serde(rename = "responseChunks", default)]
    pub response_chunks: Vec<ChunkRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ChunkRecord {
    pub data: String,
}

// ── Db API (这些方法只是发消息到 writer thread) ────────────────────────────────

use crate::config::db::Db;
use crate::config::db::DbCmd;

impl Db {
    pub(crate) fn upsert_traffic_log(
        &self,
        method: &str,
        uri: &str,
        timestamp: i64,
        headers_json: &str,
        query_json: &str,
        body: Option<&str>,
    ) -> Result<i64, sqlite::Error> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(DbCmd::UpsertTrafficLog {
            method: method.to_string(),
            uri: uri.to_string(),
            timestamp,
            headers_json: headers_json.to_string(),
            query_json: query_json.to_string(),
            body: body.map(String::from),
            reply: Some(reply_tx),
        })?;
        reply_rx.recv().map_err(|_| sqlite::Error {
            code: None,
            message: Some("db writer thread disconnected".into()),
        })?
    }

    pub(crate) fn update_traffic_response(
        &self,
        id: i64,
        status: u16,
        timestamp: i64,
        duration_ms: u64,
        headers_json: &str,
    ) -> Result<(), sqlite::Error> {
        self.send(DbCmd::UpdateTrafficResponse {
            id,
            status,
            timestamp,
            duration_ms,
            headers_json: headers_json.to_string(),
        })
    }

    pub(crate) fn update_traffic_response_body(
        &self,
        id: i64,
        body: &str,
    ) -> Result<(), sqlite::Error> {
        self.send(DbCmd::UpdateTrafficResponseBody {
            id,
            body: body.to_string(),
        })
    }

    pub(crate) fn set_traffic_error(&self, id: i64, error: &str) -> Result<(), sqlite::Error> {
        self.send(DbCmd::SetTrafficError {
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

    pub(crate) fn load_all_traffic(&self) -> Result<Vec<TrafficLogEntry>, sqlite::Error> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(DbCmd::LoadAllTraffic { reply: reply_tx })?;
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
    pub(crate) fn clear_traffic(&self) -> Result<(), sqlite::Error> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(DbCmd::ClearTraffic { reply: reply_tx })?;
        reply_rx.recv().map_err(|_| sqlite::Error {
            code: None,
            message: Some("db writer thread disconnected".into()),
        })?
    }
}

// ── SQL operations (called from writer thread) ─────────────────────────────────

pub(crate) fn do_upsert_traffic_log(
    conn: &sqlite::Connection,
    method: &str,
    uri: &str,
    timestamp: i64,
    headers_json: &str,
    query_json: &str,
    body: Option<&str>,
) -> Result<i64, sqlite::Error> {
    let mut stmt = conn.prepare(
        "INSERT INTO traffic_logs (method, uri, request_timestamp, request_headers, request_query, request_body) VALUES (?, ?, ?, ?, ?, ?)",
    )?;
    stmt.bind((1_usize, method))?;
    stmt.bind((2_usize, uri))?;
    stmt.bind((3_usize, timestamp as i64))?;
    stmt.bind((4_usize, headers_json))?;
    stmt.bind((5_usize, query_json))?;
    match body {
        Some(b) => stmt.bind((6_usize, b))?,
        None => stmt.bind((6_usize, sqlite::Value::Null))?,
    }
    stmt.next()?;
    let mut id_stmt = conn.prepare("SELECT last_insert_rowid()")?;
    id_stmt.next()?;
    Ok(id_stmt.read::<i64, _>(0)?)
}

pub(crate) fn do_update_traffic_response(
    conn: &sqlite::Connection,
    id: i64,
    status: u16,
    timestamp: i64,
    duration_ms: u64,
    headers_json: &str,
) -> Result<(), sqlite::Error> {
    let mut stmt = conn.prepare(
        "UPDATE traffic_logs SET status = ?, response_timestamp = ?, duration_ms = ?, response_headers = ? WHERE id = ?",
    )?;
    stmt.bind((1_usize, status as i64))?;
    stmt.bind((2_usize, timestamp as i64))?;
    stmt.bind((3_usize, duration_ms as i64))?;
    stmt.bind((4_usize, headers_json))?;
    stmt.bind((5_usize, id as i64))?;
    stmt.next()?;
    Ok(())
}

pub(crate) fn do_update_traffic_response_body(
    conn: &sqlite::Connection,
    id: i64,
    body: &str,
) -> Result<(), sqlite::Error> {
    let mut stmt = conn.prepare("UPDATE traffic_logs SET response_body = ? WHERE id = ?")?;
    stmt.bind((1_usize, body))?;
    stmt.bind((2_usize, id as i64))?;
    stmt.next()?;
    Ok(())
}

pub(crate) fn do_set_traffic_error(
    conn: &sqlite::Connection,
    id: i64,
    error: &str,
) -> Result<(), sqlite::Error> {
    let mut stmt = conn.prepare("UPDATE traffic_logs SET error = ? WHERE id = ?")?;
    stmt.bind((1_usize, error))?;
    stmt.bind((2_usize, id as i64))?;
    stmt.next()?;
    Ok(())
}

pub(crate) fn do_insert_chunk(
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

pub(crate) fn parse_kv_json(s: &str) -> HashMap<String, String> {
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

pub(crate) fn do_load_all_traffic(
    conn: &sqlite::Connection,
) -> Result<Vec<TrafficLogEntry>, sqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, method, uri, request_timestamp, request_headers, request_body, request_query, status, response_timestamp, duration_ms, response_headers, response_body, error FROM traffic_logs ORDER BY request_timestamp DESC",
    )?;
    let mut entries = Vec::new();
    while let sqlite::State::Row = stmt.next()? {
        let headers_str: String = stmt.read::<String, _>(4)?;
        let headers: HashMap<String, String> = parse_kv_json(&headers_str);
        let query_str: Option<String> = stmt.read::<Option<String>, _>(6)?;
        let query: Option<HashMap<String, String>> = query_str.map(|s| parse_kv_json(&s));
        let resp_headers_str: Option<String> = stmt.read::<Option<String>, _>(10)?;
        let resp_headers: Option<HashMap<String, String>> =
            resp_headers_str.and_then(|s| serde_json::from_str(&s).ok());

        entries.push(TrafficLogEntry {
            id: stmt.read::<i64, _>(0)?.to_string(),
            method: stmt.read::<String, _>(1)?,
            uri: stmt.read::<String, _>(2)?,
            request_timestamp: stmt.read::<i64, _>(3)?,
            request_headers: headers,
            request_body: stmt.read::<Option<String>, _>(5)?,
            request_query: query,
            status: stmt.read::<Option<i64>, _>(7)?.map(|v| v as u16),
            response_timestamp: stmt.read::<Option<i64>, _>(8)?,
            duration_ms: stmt.read::<Option<i64>, _>(9)?.map(|v| v as u64),
            response_headers: resp_headers,
            response_body: stmt.read::<Option<String>, _>(11)?,
            error: stmt.read::<Option<String>, _>(12)?,
            response_chunks: Vec::new(),
        });
    }
    Ok(entries)
}

pub(crate) fn do_load_chunks(
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

// ── Migration ─────────────────────────────────────────────────────────────────

impl DbTable for TrafficTable {
    fn migrate(conn: &sqlite::Connection) -> Result<(), sqlite::Error> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS traffic_logs (
                id                INTEGER PRIMARY KEY AUTOINCREMENT,
                method            TEXT NOT NULL,
                uri               TEXT NOT NULL,
                request_timestamp INTEGER NOT NULL,
                request_headers   TEXT NOT NULL DEFAULT '[]',
                request_body      TEXT,
                request_query     TEXT DEFAULT '[]',
                status            INTEGER,
                response_timestamp INTEGER,
                duration_ms       INTEGER,
                response_headers  TEXT,
                response_body     TEXT,
                error             TEXT
            )",
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS response_chunks (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                request_id  INTEGER NOT NULL,
                chunk       TEXT NOT NULL,
                seq         INTEGER NOT NULL,
                created_at  INTEGER NOT NULL DEFAULT 0
            )",
        )?;

        Ok(())
    }
}
