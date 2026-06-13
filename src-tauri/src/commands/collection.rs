use crate::AppState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiCollection {
    pub id: String,
    pub name: String,
    pub children: Vec<ApiTreeNode>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ApiTreeNode {
    #[serde(rename = "folder")]
    Folder {
        id: String,
        name: String,
        children: Vec<ApiTreeNode>,
    },
    #[serde(rename = "request")]
    Request {
        id: String,
        name: String,
        method: String,
        url: String,
        headers: Vec<HeaderPair>,
        body: String,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HeaderPair {
    pub key: String,
    pub value: String,
}

#[tauri::command]
pub fn get_collections(state: tauri::State<'_, AppState>) -> Result<Vec<ApiCollection>, String> {
    let path = state.store().collections_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let collections: Vec<ApiCollection> =
        serde_json::from_str(&content).map_err(|e| e.to_string())?;
    Ok(collections)
}

#[tauri::command]
pub fn save_collections(
    state: tauri::State<'_, AppState>,
    collections: Vec<ApiCollection>,
) -> Result<(), String> {
    let path = state.store().collections_path();
    let content = serde_json::to_string_pretty(&collections).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(())
}
