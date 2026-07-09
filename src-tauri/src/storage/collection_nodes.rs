use crate::storage::HeaderPair;
use crate::storage::ApiCollection;
use crate::storage::ApiTreeNode;
use crate::config::db::{CollectionNodeRow, Db};
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

    /// Atomically checks that the target node is NOT the last root collection,
    /// then deletes the subtree. See `DeleteNodeIfNotLast`.
    fn delete_node_if_not_last(&self, node_id: i64) -> Result<(), sqlite::Error>;
}

impl CollectionNodesRepository for Db {
    fn load_all_collections(&self) -> Result<Vec<ApiCollection>, sqlite::Error> {
        let nodes = self.load_all_collection_nodes()?;
        if nodes.is_empty() {
            return Ok(Vec::new());
        }

        // Collect all request_ids (non-NULL) and batch-fetch from requests table
        let request_ids: Vec<i64> = nodes
            .iter()
            .filter_map(|n| {
                if n.node_type == "request" {
                    n.request_id
                } else {
                    None
                }
            })
            .collect();

        let requests_data = self.find_requests_by_ids_inner(&request_ids)?;

        // Build a map: request_id -> RequestData
        let mut requests_map: HashMap<i64, RequestData> = HashMap::new();
        for row in &requests_data {
            requests_map.insert(
                row.id,
                RequestData {
                    method: row.method.clone(),
                    uri: row.uri.clone(),
                    headers: row.headers.clone(),
                    body: row.body.clone(),
                    query: row.query.clone(),
                    cookies: row.cookies.clone(),
                    body_type: row.body_type.clone(),
                    auth_type: row.auth_type.clone(),
                    auth_data: row.auth_data.clone(),
                    name: row.name.clone(),
                },
            );
        }

        // Assemble collections from root nodes (parent_id = 0)
        let collections: Vec<ApiCollection> = nodes
            .iter()
            .filter(|n| n.parent_id == 0 && n.node_type == "collection")
            .map(|n| ApiCollection {
                id: n.id,
                name: n.name.clone(),
                children: assemble_tree(&nodes, n.id, &requests_map),
                created_at: n.created_at,
                updated_at: n.updated_at,
            })
            .collect();

        Ok(collections)
    }

    fn create_collection(&self, name: &str, timestamp: i64) -> Result<i64, sqlite::Error> {
        self.create_collection_inner(name, timestamp)
    }

    fn create_folder(&self, parent_id: i64, name: &str, timestamp: i64) -> Result<i64, sqlite::Error> {
        self.create_folder_inner(parent_id, name, timestamp)
    }

    fn create_request_node(
        &self,
        parent_id: i64,
        name: &str,
        request_id: i64,
        timestamp: i64,
    ) -> Result<i64, sqlite::Error> {
        self.create_request_node_inner(parent_id, name, request_id, timestamp)
    }

    fn rename_node(&self, id: i64, new_name: &str, timestamp: i64) -> Result<(), sqlite::Error> {
        self.rename_node_inner(id, new_name, timestamp)
    }

    fn move_node(&self, id: i64, new_parent_id: i64, timestamp: i64) -> Result<(), sqlite::Error> {
        self.move_node_inner(id, new_parent_id, timestamp)
    }

    fn delete_node_if_not_last(&self, node_id: i64) -> Result<(), sqlite::Error> {
        self.delete_node_if_not_last_inner(node_id)
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

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
    nodes: &[CollectionNodeRow],
    parent_id: i64,
    requests_map: &HashMap<i64, RequestData>,
) -> Vec<ApiTreeNode> {
    // Collect children, sort by sort_order, then by id for determinism
    let mut children: Vec<&CollectionNodeRow> = nodes
        .iter()
        .filter(|n| n.parent_id == parent_id)
        .collect();
    children.sort_by(|a, b| a.sort_order.cmp(&b.sort_order).then_with(|| a.id.cmp(&b.id)));

    children
        .into_iter()
        .map(|n| match n.node_type.as_str() {
            "folder" => ApiTreeNode::Folder {
                id: n.id,
                name: n.name.clone(),
                children: assemble_tree(nodes, n.id, requests_map),
            },
            "request" => {
                let key = n.request_id.unwrap_or(0);
                let req = requests_map.get(&key);
                ApiTreeNode::Request {
                    id: n.id,
                    name: n.name.clone(),
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
                    request_id: n.request_id.unwrap_or(0),
                }
            }
            _ => unreachable!("unknown node_type: {}", n.node_type),
        })
        .collect()
}

/// Parses a JSON array `[{"key":"x","value":"y"}]` into `Vec<HeaderPair>`.
fn parse_kv_array(json_str: &str) -> Vec<HeaderPair> {
    serde_json::from_str::<Vec<HeaderPair>>(json_str).unwrap_or_default()
}
