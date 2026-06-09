use std::collections::HashMap;
use std::path::Path;

use rama::http::HeaderMap;
use serde::Serialize;
use sqlite;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct StoredEntry {
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
    pub edited: Option<bool>,
}

pub(crate) struct Db {
    conn: Option<sqlite::Connection>,
    db_path: Option<String>,
}

impl Db {
    pub(crate) fn open(path: &Path) -> Result<Self, sqlite::Error> {
        let conn = sqlite::open(path)?;
        let db_path = Some(path.to_string_lossy().to_string());
        let db = Self {
            conn: Some(conn),
            db_path,
        };
        db.migrate()?;
        Ok(db)
    }

    pub(crate) fn noop() -> Self {
        Self {
            conn: None,
            db_path: None,
        }
    }

    fn migrate(&self) -> Result<(), sqlite::Error> {
        let conn = match self.conn {
            Some(ref conn) => conn,
            None => return Ok(()),
        };
        conn.execute(
            "CREATE TABLE IF NOT EXISTS requests (
                id TEXT PRIMARY KEY,
                method TEXT NOT NULL,
                uri TEXT NOT NULL,
                request_timestamp INTEGER NOT NULL,
                request_headers TEXT NOT NULL DEFAULT '{}',
                request_body TEXT,
                request_query TEXT DEFAULT '{}',
                status INTEGER,
                response_timestamp INTEGER,
                duration_ms INTEGER,
                response_headers TEXT,
                response_body TEXT,
                error TEXT,
                edited INTEGER NOT NULL DEFAULT 0
            )",
        )?;
        let conn = match self.conn {
            Some(ref conn) => conn,
            None => return Ok(()),
        };
        conn.execute(
            "CREATE TABLE IF NOT EXISTS response_chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                request_id TEXT NOT NULL,
                chunk TEXT NOT NULL,
                seq INTEGER NOT NULL,
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
    ) -> Result<(), sqlite::Error> {
        let conn = match self.conn {
            Some(ref conn) => conn,
            None => return Ok(()),
        };
        let mut stmt = conn.prepare(
            "INSERT INTO requests (id, method, uri, request_timestamp, request_headers, request_query, request_body, edited) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )?;
        stmt.bind((1_usize, id))?;
        stmt.bind((2_usize, method))?;
        stmt.bind((3_usize, uri))?;
        stmt.bind((4_usize, timestamp as i64))?;
        stmt.bind((5_usize, headers_json))?;
        stmt.bind((6_usize, query_json))?;
        match body {
            Some(b) => stmt.bind((7_usize, b))?,
            None => stmt.bind((7_usize, sqlite::Value::Null))?,
        }
        stmt.bind((8_usize, if edited { 1 } else { 0 }))?;
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

    /// Convert an HTTP HeaderMap to a JSON string.
    pub(crate) fn headers_to_json(headers: &HeaderMap) -> String {
        let h: std::collections::HashMap<String, String> = headers
            .iter()
            .filter_map(|(k, v)| Some((k.to_string(), v.to_str().ok()?.to_string())))
            .collect();
        serde_json::to_string(&h).unwrap_or_default()
    }

    /// Extract URI query parameters as a JSON string.
    pub(crate) fn query_to_json(uri: &rama::http::Uri) -> String {
        let q_str = uri.query().unwrap_or("");
        if q_str.is_empty() {
            return "{}".to_string();
        }
        let q: std::collections::HashMap<String, String> = q_str
            .split('&')
            .filter_map(|pair| {
                let mut it = pair.splitn(2, '=');
                let key = it.next()?;
                let val = it.next().unwrap_or("");
                Some((key.to_string(), val.to_string()))
            })
            .collect();
        serde_json::to_string(&q).unwrap_or_default()
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
    ) {
        let path = match self.db_path {
            Some(ref p) => p.clone(),
            None => return,
        };
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
                let db = Self {
                    conn: Some(conn),
                    db_path: None,
                };
                if let Err(e) = db.upsert_request(&rid, &m, &u, ts, &h, &q, b.as_deref(), ed) {
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
                let db = Self {
                    conn: Some(conn),
                    db_path: None,
                };
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
                let db = Self {
                    conn: Some(conn),
                    db_path: None,
                };
                if let Err(e) = db.set_error(&rid, &err) {
                    log::warn!("set_error_async failed: {e}");
                }
            }
        });
    }

    pub(crate) fn load_all(&self) -> Result<Vec<StoredEntry>, sqlite::Error> {
        let conn = match self.conn {
            Some(ref conn) => conn,
            None => return Ok(Vec::new()),
        };
        let mut stmt = conn.prepare(
            "SELECT id, method, uri, request_timestamp, request_headers, request_body, request_query, status, response_timestamp, duration_ms, response_headers, response_body, error, edited FROM requests ORDER BY request_timestamp DESC",
        )?;
        let mut entries = Vec::new();
        while let sqlite::State::Row = stmt.next()? {
            let headers_str: String = stmt.read::<String, _>(4)?;
            let headers: HashMap<String, String> =
                serde_json::from_str(&headers_str).unwrap_or_default();
            let query_str: Option<String> = stmt.read::<Option<String>, _>(6)?;
            let query: Option<HashMap<String, String>> =
                query_str.and_then(|s| serde_json::from_str(&s).ok());
            let resp_headers_str: Option<String> = stmt.read::<Option<String>, _>(10)?;
            let resp_headers: Option<HashMap<String, String>> =
                resp_headers_str.and_then(|s| serde_json::from_str(&s).ok());
            let edited_int: i64 = stmt.read::<i64, _>(13)?;

            entries.push(StoredEntry {
                id: stmt.read::<String, _>(0)?,
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
                edited: if edited_int != 0 { Some(true) } else { None },
            });
        }
        Ok(entries)
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
