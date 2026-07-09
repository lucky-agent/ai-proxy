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
        self.insert_collection_request_inner(collection_id, name, method, uri, timestamp)
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
        self.update_collection_request_inner(
            id, method, uri, headers, query, body, body_type, cookies, auth_type, auth_data,
        )
    }

    fn duplicate_collection_request(
        &self,
        id: i64,
        timestamp: i64,
    ) -> Result<i64, sqlite::Error> {
        self.duplicate_collection_request_inner(id, timestamp)
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
        let rows = self.find_requests_by_ids_inner(ids)?;
        Ok(rows
            .into_iter()
            .map(|r| {
                (
                    r.id,
                    r.method,
                    r.uri,
                    r.headers,
                    r.body,
                    r.query,
                    r.cookies,
                    r.body_type,
                    r.auth_type,
                    r.auth_data,
                    r.name,
                )
            })
            .collect())
    }
}
