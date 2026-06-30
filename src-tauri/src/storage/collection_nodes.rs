use crate::storage::requests::RequestsRepository;
use crate::storage::HeaderPair;
use crate::storage::ApiCollection;
use crate::storage::ApiTreeNode;
use crate::config::db::Db;
use sqlite;
use std::collections::HashMap;

/// Repository trait for managing the `collection_nodes` table and assembling full API collection trees.
pub(crate) trait CollectionNodesRepository {
    /// Load all collections with their full tree of folders and requests,
    /// including the associated request data from the `requests` table.
    fn load_all_collections(&self) -> Result<Vec<ApiCollection>, sqlite::Error>;

    /// Create a root-level collection node (parent_id = 0, node_type = 'collection').
    /// Returns the auto-generated id.
    fn create_collection(&self, name: &str, timestamp: i64) -> Result<i64, sqlite::Error>;

    /// Create a folder node under the given parent. Returns the auto-generated id.
    fn create_folder(&self, parent_id: i64, name: &str, timestamp: i64) -> Result<i64, sqlite::Error>;

    /// Create a request node referencing a row in the `requests` table.
    /// Returns the auto-generated node id.
    fn create_request_node(&self, parent_id: i64, name: &str, request_id: i64, timestamp: i64) -> Result<i64, sqlite::Error>;

    /// Rename a node (collection, folder, or request).
    fn rename_node(&self, id: i64, new_name: &str, timestamp: i64) -> Result<(), sqlite::Error>;

    /// Move a node to a new parent.
    fn move_node(&self, id: i64, new_parent_id: i64, timestamp: i64) -> Result<(), sqlite::Error>;

    /// Delete a node and all its descendants. For request nodes, also cleans up the
    /// associated `requests` rows.
    fn delete_node_subtree(&self, id: i64) -> Result<(), sqlite::Error>;
}

impl CollectionNodesRepository for Db {
    fn load_all_collections(&self) -> Result<Vec<ApiCollection>, sqlite::Error> {
        let conn = match self.conn_ref() {
            Some(c) => c,
            None => return Ok(Vec::new()),
        };

        // 1. SELECT all nodes ordered by sort_order, created_at
        let mut stmt = conn.prepare(
            "SELECT id, parent_id, name, node_type, request_id, sort_order, created_at, updated_at FROM collection_nodes ORDER BY sort_order, created_at",
        )?;

        // (id, parent_id, name, node_type, request_id, sort_order, created_at, updated_at)
        let mut nodes: Vec<(i64, i64, String, String, Option<i64>, i64, i64, i64)> = Vec::new();
        while let sqlite::State::Row = stmt.next()? {
            nodes.push((
                stmt.read::<i64, _>(0)?,               // id
                stmt.read::<i64, _>(1)?,               // parent_id
                stmt.read::<String, _>(2)?,            // name
                stmt.read::<String, _>(3)?,            // node_type
                stmt.read::<Option<i64>, _>(4)?,       // request_id
                stmt.read::<i64, _>(5)?,               // sort_order
                stmt.read::<i64, _>(6)?,               // created_at
                stmt.read::<i64, _>(7)?,               // updated_at
            ));
        }

        if nodes.is_empty() {
            return Ok(Vec::new());
        }

        // 2. Collect all request_ids (non-NULL) and batch-fetch from requests table
        let request_ids: Vec<i64> = nodes
            .iter()
            .filter_map(|(_, _, _, node_type, request_id, ..)| {
                if node_type == "request" { *request_id } else { None }
            })
            .collect();

        let requests_data = self.find_requests_by_ids(&request_ids)?;

        // Build a map: request_id -> RequestData
        let mut requests_map: HashMap<i64, RequestData> = HashMap::new();
        for (id, method, uri, headers, body, query, cookies, body_type, auth_type, auth_data, name) in &requests_data {
            requests_map.insert(
                *id,
                RequestData {
                    method: method.clone(),
                    uri: uri.clone(),
                    headers: headers.clone(),
                    body: body.clone(),
                    query: query.clone(),
                    cookies: cookies.clone(),
                    body_type: body_type.clone(),
                    auth_type: auth_type.clone(),
                    auth_data: auth_data.clone(),
                    name: name.clone(),
                },
            );
        }

        // 3. Assemble collections from root nodes (parent_id = 0)
        let collections: Vec<ApiCollection> = nodes
            .iter()
            .filter(|(_, parent_id, _, node_type, ..)| *parent_id == 0 && node_type == "collection")
            .map(|(id, _, name, _, _, _, created_at, updated_at)| ApiCollection {
                id: *id,
                name: name.clone(),
                children: assemble_tree(&nodes, *id, &requests_map),
                created_at: *created_at,
                updated_at: *updated_at,
            })
            .collect();

        Ok(collections)
    }

    fn create_collection(&self, name: &str, timestamp: i64) -> Result<i64, sqlite::Error> {
        let conn = match self.conn_ref() {
            Some(c) => c,
            None => return Ok(0),
        };
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

    fn create_folder(&self, parent_id: i64, name: &str, timestamp: i64) -> Result<i64, sqlite::Error> {
        let conn = match self.conn_ref() {
            Some(c) => c,
            None => return Ok(0),
        };
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

    fn create_request_node(&self, parent_id: i64, name: &str, request_id: i64, timestamp: i64) -> Result<i64, sqlite::Error> {
        let conn = match self.conn_ref() {
            Some(c) => c,
            None => return Ok(0),
        };
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

    fn rename_node(&self, id: i64, new_name: &str, timestamp: i64) -> Result<(), sqlite::Error> {
        let conn = match self.conn_ref() {
            Some(c) => c,
            None => return Ok(()),
        };
        let mut stmt = conn.prepare(
            "UPDATE collection_nodes SET name = ?, updated_at = ? WHERE id = ?",
        )?;
        stmt.bind((1_usize, new_name))?;
        stmt.bind((2_usize, timestamp))?;
        stmt.bind((3_usize, id))?;
        stmt.next()?;
        Ok(())
    }

    fn move_node(&self, id: i64, new_parent_id: i64, timestamp: i64) -> Result<(), sqlite::Error> {
        let conn = match self.conn_ref() {
            Some(c) => c,
            None => return Ok(()),
        };
        let mut stmt = conn.prepare(
            "UPDATE collection_nodes SET parent_id = ?, updated_at = ? WHERE id = ?",
        )?;
        stmt.bind((1_usize, new_parent_id))?;
        stmt.bind((2_usize, timestamp))?;
        stmt.bind((3_usize, id))?;
        stmt.next()?;
        Ok(())
    }

    fn delete_node_subtree(&self, id: i64) -> Result<(), sqlite::Error> {
        let conn = match self.conn_ref() {
            Some(c) => c,
            None => return Ok(()),
        };

        // 1. Recursively collect all descendant node IDs starting from id
        let mut to_delete: Vec<i64> = Vec::new();
        let mut queue: Vec<i64> = vec![id];
        while let Some(current_id) = queue.pop() {
            to_delete.push(current_id);
            let mut stmt = conn.prepare(
                "SELECT id FROM collection_nodes WHERE parent_id = ?",
            )?;
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
            let req_placeholders: String = request_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let req_sql = format!("DELETE FROM requests WHERE id IN ({})", req_placeholders);
            let mut req_stmt = conn.prepare(req_sql)?;
            for (i, req_id) in request_ids.iter().enumerate() {
                req_stmt.bind(((i + 1) as usize, *req_id))?;
            }
            req_stmt.next()?;
        }

        // 4. Delete the nodes themselves
        let del_sql = format!("DELETE FROM collection_nodes WHERE id IN ({})", placeholders);
        let mut del_stmt = conn.prepare(del_sql)?;
        for (i, node_id) in to_delete.iter().enumerate() {
            del_stmt.bind(((i + 1) as usize, *node_id))?;
        }
        del_stmt.next()?;

        Ok(())
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Parses a JSON array `[{"key":"x","value":"y"}]` into `Vec<HeaderPair>`.
fn parse_kv_array(json_str: &str) -> Vec<HeaderPair> {
    serde_json::from_str::<Vec<HeaderPair>>(json_str).unwrap_or_default()
}

/// Intermediate representation of a request row used for tree assembly.
struct RequestData {
    method: String,
    uri: String,
    headers: String,
    body: Option<String>,
    query: String,
    cookies: String,
    body_type: String,
    auth_type: String,
    auth_data: String,
    name: String,
}

/// Recursively assemble child `ApiTreeNode`s for a given parent.
fn assemble_tree(
    nodes: &[(i64, i64, String, String, Option<i64>, i64, i64, i64)],
    parent_id: i64,
    requests_map: &HashMap<i64, RequestData>,
) -> Vec<ApiTreeNode> {
    // Collect children, sort by sort_order (idx 5), then by id for determinism
    let mut children: Vec<&(i64, i64, String, String, Option<i64>, i64, i64, i64)> = nodes
        .iter()
        .filter(|(_, p_id, ..)| *p_id == parent_id)
        .collect();
    children.sort_by(|a, b| a.5.cmp(&b.5).then_with(|| a.0.cmp(&b.0)));

    children
        .into_iter()
        .map(|(id, _, name, node_type, request_id, ..)| match node_type.as_str() {
            "folder" => ApiTreeNode::Folder {
                id: *id,
                name: name.clone(),
                children: assemble_tree(nodes, *id, requests_map),
            },
            "request" => {
                let key = request_id.unwrap_or(0);
                let req = requests_map.get(&key);
                ApiTreeNode::Request {
                    id: *id,
                    name: name.clone(),
                    method: req.map_or(String::new(), |r| r.method.clone()),
                    url: req.map_or(String::new(), |r| r.uri.clone()),
                    headers: req.map_or(Vec::new(), |r| parse_kv_array(&r.headers)),
                    params: req.map_or(Vec::new(), |r| parse_kv_array(&r.query)),
                    cookies: req.map_or(Vec::new(), |r| parse_kv_array(&r.cookies)),
                    body_type: req.map_or(String::new(), |r| r.body_type.clone()),
                    body: req.and_then(|r| r.body.clone()).unwrap_or_default(),
                    auth_type: {
                        let v = req.map_or(String::new(), |r| r.auth_type.clone());
                        if v.is_empty() { None } else { Some(v) }
                    },
                    auth_data: {
                        let v = req.map_or(String::new(), |r| r.auth_data.clone());
                        if v.is_empty() { None } else { Some(v) }
                    },
                    request_id: request_id.unwrap_or(0),
                }
            }
            _ => unreachable!("unknown node_type: {}", node_type),
        })
        .collect()
}
