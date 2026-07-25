use std::collections::HashMap;
use std::sync::mpsc;

use crate::storage::ApiCollection;
use crate::storage::ApiTreeNode;
use crate::storage::DbTable;
use crate::storage::HeaderPair;

// ── Row type ───────────────────────────────────────────────────────────────────

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

// ── Table marker ──────────────────────────────────────────────────────────────

pub(crate) struct CollectionNodesTable;

// ── Repository trait ───────────────────────────────────────────────────────────

pub(crate) trait CollectionNodesRepository {
    fn load_all_collections(&self) -> Result<Vec<ApiCollection>, sqlite::Error>;
    fn create_collection(&self, name: &str, timestamp: i64) -> Result<i64, sqlite::Error>;
    fn create_folder(
        &self,
        parent_id: i64,
        name: &str,
        timestamp: i64,
    ) -> Result<i64, sqlite::Error>;
    fn create_request_node(
        &self,
        parent_id: i64,
        name: &str,
        request_id: i64,
        timestamp: i64,
    ) -> Result<i64, sqlite::Error>;
    fn rename_node(&self, id: i64, new_name: &str, timestamp: i64) -> Result<(), sqlite::Error>;
    fn move_node(&self, id: i64, new_parent_id: i64, timestamp: i64) -> Result<(), sqlite::Error>;
    fn delete_node_if_not_last(&self, node_id: i64) -> Result<(), sqlite::Error>;
}

// ── Db API ─────────────────────────────────────────────────────────────────────

use crate::config::db::Db;
use crate::config::db::DbCmd;
use crate::storage::collection_requests::CollectionRequestsRepository;

impl CollectionNodesRepository for Db {
    fn load_all_collections(&self) -> Result<Vec<ApiCollection>, sqlite::Error> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(DbCmd::LoadAllCollectionNodes { reply: reply_tx })?;
        let nodes: Vec<CollectionNodeRow> = reply_rx.recv().map_err(|_| sqlite::Error {
            code: None,
            message: Some("db writer thread disconnected".into()),
        })??;

        if nodes.is_empty() {
            return Ok(Vec::new());
        }

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

        let requests_data = self.find_collection_requests_by_ids(&request_ids)?;

        let mut requests_map: HashMap<i64, RequestData> = HashMap::new();
        for row in &requests_data {
            requests_map.insert(
                row.id,
                RequestData {
                    method: row.method.clone(),
                    uri: row.uri.clone(),
                    request_headers: row.request_headers.clone(),
                    request_body: row.request_body.clone(),
                    request_query: row.request_query.clone(),
                    cookies: row.cookies.clone(),
                    body_type: row.body_type.clone(),
                    auth_type: row.auth_type.clone(),
                    auth_data: row.auth_data.clone(),
                    name: row.name.clone(),
                },
            );
        }

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

    fn create_folder(
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

    fn create_request_node(
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

    fn rename_node(&self, id: i64, new_name: &str, timestamp: i64) -> Result<(), sqlite::Error> {
        self.send(DbCmd::RenameNode {
            id,
            new_name: new_name.to_string(),
            timestamp,
        })
    }

    fn move_node(&self, id: i64, new_parent_id: i64, timestamp: i64) -> Result<(), sqlite::Error> {
        self.send(DbCmd::MoveNode {
            id,
            new_parent_id,
            timestamp,
        })
    }

    fn delete_node_if_not_last(&self, node_id: i64) -> Result<(), sqlite::Error> {
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

// ── Internal helpers ───────────────────────────────────────────────────────────

struct RequestData {
    method: String,
    uri: String,
    request_headers: String,
    request_body: Option<String>,
    request_query: String,
    cookies: String,
    body_type: String,
    auth_type: String,
    auth_data: String,
    name: String,
}

fn assemble_tree(
    nodes: &[CollectionNodeRow],
    parent_id: i64,
    requests_map: &HashMap<i64, RequestData>,
) -> Vec<ApiTreeNode> {
    let mut children: Vec<&CollectionNodeRow> =
        nodes.iter().filter(|n| n.parent_id == parent_id).collect();
    children.sort_by(|a, b| {
        a.sort_order
            .cmp(&b.sort_order)
            .then_with(|| a.id.cmp(&b.id))
    });

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
                    headers: req.map_or(Vec::new(), |r| parse_kv_array(&r.request_headers)),
                    params: req.map_or(Vec::new(), |r| parse_kv_array(&r.request_query)),
                    cookies: req.map_or(Vec::new(), |r| parse_kv_array(&r.cookies)),
                    body_type: req.map_or(String::new(), |r| r.body_type.clone()),
                    body: req.and_then(|r| r.request_body.clone()).unwrap_or_default(),
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

fn parse_kv_array(json_str: &str) -> Vec<HeaderPair> {
    serde_json::from_str::<Vec<HeaderPair>>(json_str).unwrap_or_default()
}

// ── SQL operations (called from writer thread) ─────────────────────────────────

pub(crate) fn do_load_all_collection_nodes(
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

pub(crate) fn do_create_collection(
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

pub(crate) fn do_create_folder(
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

pub(crate) fn do_create_request_node(
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

pub(crate) fn do_rename_node(
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

    // Also update the name in collection_requests for request-type nodes
    let mut stmt = conn.prepare(
        "UPDATE collection_requests SET name = ?, updated_at = ? WHERE id = (SELECT request_id FROM collection_nodes WHERE id = ? AND request_id IS NOT NULL)",
    )?;
    stmt.bind((1_usize, new_name))?;
    stmt.bind((2_usize, timestamp))?;
    stmt.bind((3_usize, id))?;
    stmt.next()?;

    Ok(())
}

pub(crate) fn do_move_node(
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

pub(crate) fn do_delete_node_subtree(
    conn: &sqlite::Connection,
    id: i64,
) -> Result<(), sqlite::Error> {
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

    if !request_ids.is_empty() {
        let req_placeholders: String = request_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let req_sql = format!(
            "DELETE FROM collection_requests WHERE id IN ({})",
            req_placeholders
        );
        let mut req_stmt = conn.prepare(req_sql)?;
        for (i, req_id) in request_ids.iter().enumerate() {
            req_stmt.bind(((i + 1) as usize, *req_id))?;
        }
        req_stmt.next()?;
    }

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

pub(crate) fn do_delete_node_if_not_last(
    conn: &sqlite::Connection,
    node_id: i64,
) -> Result<(), sqlite::Error> {
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

// ── Migration ─────────────────────────────────────────────────────────────────

impl DbTable for CollectionNodesTable {
    fn migrate(conn: &sqlite::Connection) -> Result<(), sqlite::Error> {
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
        Ok(())
    }
}
