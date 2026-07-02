use crate::AppState;
use crate::storage::ApiCollection;
use crate::storage::ApiTreeNode;
use crate::storage::HeaderPair;
use crate::storage::collection_nodes::CollectionNodesRepository;
use crate::storage::requests::RequestsRepository;

#[tauri::command]
pub fn get_collections(state: tauri::State<'_, AppState>) -> Result<Vec<ApiCollection>, String> {
    let db = state.db();
    let repo = db.lock().unwrap();
    let mut collections = repo.load_all_collections().map_err(|e| e.to_string())?;

    // Auto-create a default collection on first launch (SQLite migration from collections.json)
    if collections.is_empty() {
        let ts = chrono::Utc::now().timestamp_millis();
        repo.create_collection("默认模块", ts)
            .map_err(|e| e.to_string())?;
        // Re-read to get the full tree
        collections = repo.load_all_collections().map_err(|e| e.to_string())?;
    }

    Ok(collections)
}

#[tauri::command]
pub fn create_collection(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<String, String> {
    let db = state.db();
    let repo = db.lock().unwrap();
    let ts = chrono::Utc::now().timestamp_millis();
    let id = repo.create_collection(&name, ts)
        .map_err(|e| e.to_string())?;
    Ok(id.to_string())
}

#[tauri::command]
pub fn create_folder(
    state: tauri::State<'_, AppState>,
    parent_id: i64,
    name: String,
) -> Result<String, String> {
    let db = state.db();
    let repo = db.lock().unwrap();
    let ts = chrono::Utc::now().timestamp_millis();
    let id = repo.create_folder(parent_id, &name, ts)
        .map_err(|e| e.to_string())?;
    Ok(id.to_string())
}

#[tauri::command]
pub fn create_request(
    state: tauri::State<'_, AppState>,
    parent_id: i64,
    collection_id: i64,
    name: String,
) -> Result<String, String> {
    let db = state.db();
    let repo = db.lock().unwrap();
    let ts = chrono::Utc::now().timestamp_millis();
    // Insert into requests table first to get the request_id
    let request_id = repo.insert_collection_request(collection_id, &name, "GET", "", ts)
        .map_err(|e| e.to_string())?;
    // Then create the node referencing it
    let node_id = repo.create_request_node(parent_id, &name, request_id, ts)
        .map_err(|e| e.to_string())?;
    // Return both IDs as JSON so frontend can populate requestId for later saves
    Ok(serde_json::json!({ "nodeId": node_id, "requestId": request_id }).to_string())
}

#[tauri::command]
pub fn delete_node(
    state: tauri::State<'_, AppState>,
    node_id: i64,
) -> Result<(), String> {
    let db = state.db();
    let repo = db.lock().unwrap();

    // Prevent deleting the last remaining collection (default module is always present)
    let collections = repo.load_all_collections().map_err(|e| e.to_string())?;
    let is_root_collection = collections.iter().any(|c| c.id == node_id);
    if is_root_collection && collections.len() <= 1 {
        return Err("cannot delete the last collection".to_string());
    }

    repo.delete_node_subtree(node_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rename_node(
    state: tauri::State<'_, AppState>,
    node_id: i64,
    new_name: String,
) -> Result<(), String> {
    let db = state.db();
    let repo = db.lock().unwrap();
    let ts = chrono::Utc::now().timestamp_millis();
    repo.rename_node(node_id, &new_name, ts)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn move_node(
    state: tauri::State<'_, AppState>,
    node_id: i64,
    new_parent_id: i64,
) -> Result<(), String> {
    let db = state.db();
    let repo = db.lock().unwrap();
    let ts = chrono::Utc::now().timestamp_millis();
    repo.move_node(node_id, new_parent_id, ts)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_request(
    state: tauri::State<'_, AppState>,
    id: i64,
    method: String,
    url: String,
    headers: Option<Vec<HeaderPair>>,
    params: Option<Vec<HeaderPair>>,
    body: Option<String>,
    body_type: Option<String>,
    cookies: Option<Vec<HeaderPair>>,
    auth_type: Option<String>,
    auth_data: Option<String>,
) -> Result<(), String> {
    let db = state.db();
    let repo = db.lock().unwrap();
    let headers_json =
        serde_json::to_string(&headers.unwrap_or_default()).unwrap_or_default();
    let params_json =
        serde_json::to_string(&params.unwrap_or_default()).unwrap_or_default();
    let cookies_json =
        serde_json::to_string(&cookies.unwrap_or_default()).unwrap_or_default();
    repo.update_collection_request(
        id,
        &method,
        &url,
        &headers_json,
        &params_json,
        body.as_deref(),
        body_type.as_deref().unwrap_or(""),
        &cookies_json,
        auth_type.as_deref().unwrap_or(""),
        auth_data.as_deref().unwrap_or(""),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn duplicate_request(
    state: tauri::State<'_, AppState>,
    node_id: i64,
) -> Result<String, String> {
    let db = state.db();
    let repo = db.lock().unwrap();

    // Load all collections to find the original node's parent_id, name, and request_id
    let collections = repo.load_all_collections().map_err(|e| e.to_string())?;
    let (parent_id, name, request_id) =
        find_request_node(&collections, node_id).ok_or_else(|| format!("node not found: {}", node_id))?;

    let ts = chrono::Utc::now().timestamp_millis();

    // Duplicate the request row to get a new request_id
    let new_request_id = repo.duplicate_collection_request(request_id, ts)
        .map_err(|e| e.to_string())?;
    // Create a new collection node referencing the duplicated request
    let new_node_id = repo.create_request_node(parent_id, &format!("{} (副本)", name), new_request_id, ts)
        .map_err(|e| e.to_string())?;

    Ok(new_node_id.to_string())
}

/// Recursively search collections for a request node matching `target_id`.
/// Returns (parent_id, name, request_id) on success.
fn find_request_node(collections: &[ApiCollection], target_id: i64) -> Option<(i64, String, i64)> {
    for col in collections {
        if let Some(result) = find_in_nodes(&col.children, target_id, col.id) {
            return Some(result);
        }
        // Also check collection root itself
        if col.id == target_id {
            return Some((0, col.name.clone(), 0));
        }
    }
    None
}

fn find_in_nodes(nodes: &[ApiTreeNode], target_id: i64, parent_id: i64) -> Option<(i64, String, i64)> {
    for node in nodes {
        match node {
            ApiTreeNode::Folder { id, name, children } => {
                if *id == target_id {
                    return Some((parent_id, name.clone(), 0));
                }
                if let Some(result) = find_in_nodes(children, target_id, *id) {
                    return Some(result);
                }
            }
            ApiTreeNode::Request { id, name, request_id, .. } => {
                if *id == target_id {
                    return Some((parent_id, name.clone(), *request_id));
                }
            }
        }
    }
    None
}
