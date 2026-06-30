use crate::config::db::Db;
use sqlite;

/// Repository trait for managing collection requests stored in the `requests` table.
pub(crate) trait RequestsRepository {
    /// Insert a new request with `source_type = 'collection'`. Returns the auto-generated id.
    fn insert_collection_request(
        &self,
        collection_id: i64,
        name: &str,
        method: &str,
        uri: &str,
        timestamp: i64,
    ) -> Result<i64, sqlite::Error>;

    /// Update a collection request's method, URI, headers, query, body, cookies, auth.
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
    ) -> Result<(), sqlite::Error>;

    /// Duplicate a request row under a new id. Returns the auto-generated new id.
    fn duplicate_collection_request(
        &self,
        id: i64,
        timestamp: i64,
    ) -> Result<i64, sqlite::Error>;

    /// Find requests by a list of IDs.
    /// Returns: (id, method, uri, headers_json, body_opt, query_json, cookies, body_type, auth_type, auth_data, name)
    #[allow(clippy::type_complexity)]
    fn find_requests_by_ids(
        &self,
        ids: &[i64],
    ) -> Result<
        Vec<(
            i64,
            String,
            String,
            String,
            Option<String>,
            String,
            String,
            String,
            String,
            String,
            String,
        )>,
        sqlite::Error,
    >;
}

impl RequestsRepository for Db {
    fn insert_collection_request(
        &self,
        collection_id: i64,
        name: &str,
        method: &str,
        uri: &str,
        timestamp: i64,
    ) -> Result<i64, sqlite::Error> {
        let conn = match self.conn_ref() {
            Some(c) => c,
            None => return Ok(0),
        };
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
    ) -> Result<(), sqlite::Error> {
        let conn = match self.conn_ref() {
            Some(c) => c,
            None => return Ok(()),
        };
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

    fn duplicate_collection_request(
        &self,
        id: i64,
        timestamp: i64,
    ) -> Result<i64, sqlite::Error> {
        let conn = match self.conn_ref() {
            Some(c) => c,
            None => return Ok(0),
        };
        // Copy all columns except id (use autoincrement) and request_timestamp (use timestamp).
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

    fn find_requests_by_ids(
        &self,
        ids: &[i64],
    ) -> Result<
        Vec<(
            i64,
            String,
            String,
            String,
            Option<String>,
            String,
            String,
            String,
            String,
            String,
            String,
        )>,
        sqlite::Error,
    > {
        let conn = match self.conn_ref() {
            Some(c) => c,
            None => return Ok(Vec::new()),
        };
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        // Build IN clause placeholders: "?,?,?..."
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
            results.push((
                stmt.read::<i64, _>(0)?,            // id
                stmt.read::<String, _>(1)?,         // method
                stmt.read::<String, _>(2)?,         // uri
                stmt.read::<String, _>(3)?,         // request_headers
                stmt.read::<Option<String>, _>(4)?, // request_body
                stmt.read::<String, _>(5)?,         // request_query
                stmt.read::<String, _>(6)?,         // cookies
                stmt.read::<String, _>(7)?,         // body_type
                stmt.read::<String, _>(8)?,         // auth_type
                stmt.read::<String, _>(9)?,         // auth_data
                stmt.read::<String, _>(10)?,        // name
            ));
        }
        Ok(results)
    }
}
