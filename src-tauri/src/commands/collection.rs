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
    repo.load_all_collections().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_collection(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<String, String> {
    let db = state.db();
    let repo = db.lock().unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    let ts = chrono::Utc::now().timestamp_millis();
    repo.create_collection(&id, &name, ts)
        .map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn create_folder(
    state: tauri::State<'_, AppState>,
    parent_id: String,
    name: String,
) -> Result<String, String> {
    let db = state.db();
    let repo = db.lock().unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    let ts = chrono::Utc::now().timestamp_millis();
    repo.create_folder(&id, &parent_id, &name, ts)
        .map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn create_request(
    state: tauri::State<'_, AppState>,
    parent_id: String,
    collection_id: String,
    name: String,
) -> Result<String, String> {
    let db = state.db();
    let repo = db.lock().unwrap();
    let node_id = uuid::Uuid::new_v4().to_string();
    let request_id = uuid::Uuid::new_v4().to_string();
    let ts = chrono::Utc::now().timestamp_millis();
    repo.create_request_node(&node_id, &parent_id, &name, &request_id, ts)
        .map_err(|e| e.to_string())?;
    repo.insert_collection_request(&request_id, &collection_id, &name, "GET", "", ts)
        .map_err(|e| e.to_string())?;
    Ok(node_id)
}

#[tauri::command]
pub fn delete_node(
    state: tauri::State<'_, AppState>,
    node_id: String,
) -> Result<(), String> {
    let db = state.db();
    let repo = db.lock().unwrap();
    repo.delete_node_subtree(&node_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rename_node(
    state: tauri::State<'_, AppState>,
    node_id: String,
    new_name: String,
) -> Result<(), String> {
    let db = state.db();
    let repo = db.lock().unwrap();
    let ts = chrono::Utc::now().timestamp_millis();
    repo.rename_node(&node_id, &new_name, ts)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn move_node(
    state: tauri::State<'_, AppState>,
    node_id: String,
    new_parent_id: String,
) -> Result<(), String> {
    let db = state.db();
    let repo = db.lock().unwrap();
    let ts = chrono::Utc::now().timestamp_millis();
    repo.move_node(&node_id, &new_parent_id, ts)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_request(
    state: tauri::State<'_, AppState>,
    id: String,
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
        &id,
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
    node_id: String,
) -> Result<String, String> {
    let db = state.db();
    let repo = db.lock().unwrap();

    // Load all collections to find the original node's parent_id, name, and request_id
    let collections = repo.load_all_collections().map_err(|e| e.to_string())?;
    let (parent_id, name, request_id) =
        find_request_node(&collections, &node_id).ok_or_else(|| format!("node not found: {}", node_id))?;

    let new_node_id = uuid::Uuid::new_v4().to_string();
    let new_request_id = uuid::Uuid::new_v4().to_string();
    let ts = chrono::Utc::now().timestamp_millis();

    repo.create_request_node(&new_node_id, &parent_id, &format!("{} (副本)", name), &new_request_id, ts)
        .map_err(|e| e.to_string())?;
    repo.duplicate_collection_request(&request_id, &new_request_id, ts)
        .map_err(|e| e.to_string())?;

    Ok(new_node_id)
}

/// Recursively search collections for a request node matching `target_id`.
/// Returns (parent_id, name, request_id) on success.
fn find_request_node(collections: &[ApiCollection], target_id: &str) -> Option<(String, String, String)> {
    for col in collections {
        if let Some(result) = find_in_nodes(&col.children, target_id, &col.id) {
            return Some(result);
        }
        // Also check collection root itself (collection nodes can be at root level)
        if col.id == target_id {
            return Some((String::from("0"), col.name.clone(), String::new()));
        }
    }
    None
}

fn find_in_nodes(nodes: &[ApiTreeNode], target_id: &str, parent_id: &str) -> Option<(String, String, String)> {
    for node in nodes {
        match node {
            ApiTreeNode::Folder { id, name, children } => {
                if id == target_id {
                    return Some((parent_id.to_string(), name.clone(), String::new()));
                }
                if let Some(result) = find_in_nodes(children, target_id, id) {
                    return Some(result);
                }
            }
            ApiTreeNode::Request { id, name, request_id, .. } => {
                if id == target_id {
                    return Some((parent_id.to_string(), name.clone(), request_id.clone()));
                }
            }
        }
    }
    None
}
