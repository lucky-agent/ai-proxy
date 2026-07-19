use std::sync::mpsc;

use sqlite;

use crate::storage::DbTable;

// ── Row type ───────────────────────────────────────────────────────────────────

/// A row from the `collection_requests` table.
#[derive(Debug, Clone)]
pub(crate) struct CollectionRequestRow {
    pub id: i64,
    pub name: String,
    pub method: String,
    pub uri: String,
    pub request_headers: String,
    pub request_body: Option<String>,
    pub request_query: String,
    pub cookies: String,
    pub body_type: String,
    pub auth_type: String,
    pub auth_data: String,
    pub created_at: i64,
    pub updated_at: i64,
}

// ── Table marker ──────────────────────────────────────────────────────────────

pub(crate) struct CollectionRequestsTable;

// ── Repository trait ───────────────────────────────────────────────────────────

pub(crate) trait CollectionRequestsRepository {
    fn insert_collection_request(
        &self,
        name: &str,
        method: &str,
        uri: &str,
        timestamp: i64,
    ) -> Result<i64, sqlite::Error>;

    fn update_collection_request(
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
        timestamp: i64,
    ) -> Result<(), sqlite::Error>;

    fn duplicate_collection_request(&self, id: i64, timestamp: i64) -> Result<i64, sqlite::Error>;

    fn find_collection_requests_by_ids(
        &self,
        ids: &[i64],
    ) -> Result<Vec<CollectionRequestRow>, sqlite::Error>;
}

// ── Db API ─────────────────────────────────────────────────────────────────────

use crate::config::db::Db;
use crate::config::db::DbCmd;

impl CollectionRequestsRepository for Db {
    fn insert_collection_request(
        &self,
        name: &str,
        method: &str,
        uri: &str,
        timestamp: i64,
    ) -> Result<i64, sqlite::Error> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(DbCmd::InsertCollectionRequest {
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

    fn update_collection_request(
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
        timestamp: i64,
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
            timestamp,
        })
    }

    fn duplicate_collection_request(&self, id: i64, timestamp: i64) -> Result<i64, sqlite::Error> {
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

    fn find_collection_requests_by_ids(
        &self,
        ids: &[i64],
    ) -> Result<Vec<CollectionRequestRow>, sqlite::Error> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(DbCmd::FindCollectionRequestsByIds {
            ids: ids.to_vec(),
            reply: reply_tx,
        })?;
        reply_rx.recv().map_err(|_| sqlite::Error {
            code: None,
            message: Some("db writer thread disconnected".into()),
        })?
    }
}

// ── SQL operations (called from writer thread) ─────────────────────────────────

pub(crate) fn do_insert_collection_request(
    conn: &sqlite::Connection,
    name: &str,
    method: &str,
    uri: &str,
    timestamp: i64,
) -> Result<i64, sqlite::Error> {
    let mut stmt = conn.prepare(
        "INSERT INTO collection_requests (name, method, uri, request_headers, request_query, cookies, body_type, auth_type, auth_data, created_at, updated_at) VALUES (?, ?, ?, '[]', '[]', '[]', '', '', '', ?, ?)",
    )?;
    stmt.bind((1_usize, name))?;
    stmt.bind((2_usize, method))?;
    stmt.bind((3_usize, uri))?;
    stmt.bind((4_usize, timestamp as i64))?;
    stmt.bind((5_usize, timestamp as i64))?;
    stmt.next()?;
    let mut id_stmt = conn.prepare("SELECT last_insert_rowid()")?;
    id_stmt.next()?;
    Ok(id_stmt.read::<i64, _>(0)?)
}

pub(crate) fn do_update_collection_request(
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
    timestamp: i64,
) -> Result<(), sqlite::Error> {
    let mut stmt = conn.prepare(
        "UPDATE collection_requests SET method = ?, uri = ?, request_headers = ?, request_query = ?, request_body = ?, body_type = ?, cookies = ?, auth_type = ?, auth_data = ?, updated_at = ? WHERE id = ?",
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
    stmt.bind((10_usize, timestamp as i64))?;
    stmt.bind((11_usize, id as i64))?;
    stmt.next()?;
    Ok(())
}

pub(crate) fn do_duplicate_collection_request(
    conn: &sqlite::Connection,
    id: i64,
    timestamp: i64,
) -> Result<i64, sqlite::Error> {
    let mut stmt = conn.prepare(
        "INSERT INTO collection_requests (name, method, uri, request_headers, request_body, request_query, cookies, body_type, auth_type, auth_data, created_at, updated_at)
         SELECT name, method, uri, request_headers, request_body, request_query, cookies, body_type, auth_type, auth_data, ?, ?
         FROM collection_requests WHERE id = ?",
    )?;
    stmt.bind((1_usize, timestamp as i64))?;
    stmt.bind((2_usize, timestamp as i64))?;
    stmt.bind((3_usize, id as i64))?;
    stmt.next()?;
    let mut id_stmt = conn.prepare("SELECT last_insert_rowid()")?;
    id_stmt.next()?;
    Ok(id_stmt.read::<i64, _>(0)?)
}

pub(crate) fn do_find_collection_requests_by_ids(
    conn: &sqlite::Connection,
    ids: &[i64],
) -> Result<Vec<CollectionRequestRow>, sqlite::Error> {
    let placeholders: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT id, name, method, uri, request_headers, request_body, request_query, cookies, body_type, auth_type, auth_data, created_at, updated_at FROM collection_requests WHERE id IN ({})",
        placeholders
    );
    let mut stmt = conn.prepare(sql)?;
    for (i, id) in ids.iter().enumerate() {
        stmt.bind(((i + 1) as usize, *id as i64))?;
    }
    let mut results = Vec::new();
    while let sqlite::State::Row = stmt.next()? {
        results.push(CollectionRequestRow {
            id: stmt.read::<i64, _>(0)?,
            name: stmt.read::<String, _>(1)?,
            method: stmt.read::<String, _>(2)?,
            uri: stmt.read::<String, _>(3)?,
            request_headers: stmt.read::<String, _>(4)?,
            request_body: stmt.read::<Option<String>, _>(5)?,
            request_query: stmt.read::<String, _>(6)?,
            cookies: stmt.read::<String, _>(7)?,
            body_type: stmt.read::<String, _>(8)?,
            auth_type: stmt.read::<String, _>(9)?,
            auth_data: stmt.read::<String, _>(10)?,
            created_at: stmt.read::<i64, _>(11)?,
            updated_at: stmt.read::<i64, _>(12)?,
        });
    }
    Ok(results)
}

// ── Migration ─────────────────────────────────────────────────────────────────

impl DbTable for CollectionRequestsTable {
    fn migrate(conn: &sqlite::Connection) -> Result<(), sqlite::Error> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS collection_requests (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                name            TEXT NOT NULL DEFAULT '',
                method          TEXT NOT NULL,
                uri             TEXT NOT NULL,
                request_headers TEXT NOT NULL DEFAULT '[]',
                request_body    TEXT,
                request_query   TEXT DEFAULT '[]',
                cookies         TEXT DEFAULT '[]',
                body_type       TEXT NOT NULL DEFAULT '',
                auth_type       TEXT NOT NULL DEFAULT '',
                auth_data       TEXT NOT NULL DEFAULT '',
                created_at      INTEGER NOT NULL,
                updated_at      INTEGER NOT NULL
            )",
        )?;
        // 旧库迁移：collection_id 为冗余列（归属关系由 collection_nodes.parent_id 表达，
        // 且跨集合移动节点后不会同步更新），存在时直接删除
        let has_legacy_collection_id = {
            let mut stmt = conn.prepare(
                "SELECT 1 FROM pragma_table_info('collection_requests') WHERE name = 'collection_id'",
            )?;
            matches!(stmt.next()?, sqlite::State::Row)
        };
        if has_legacy_collection_id {
            conn.execute("ALTER TABLE collection_requests DROP COLUMN collection_id")?;
        }
        Ok(())
    }
}
