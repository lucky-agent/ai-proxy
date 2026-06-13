use tauri;
use crate::proxy::state::AppState;
use crate::config::db::StoredEntry;

#[tauri::command]
pub async fn load_traffic_history(state: tauri::State<'_, AppState>) -> Result<Vec<StoredEntry>, String> {
    let db = state.db();
    let db = db.lock().map_err(|e| format!("db lock: {e:?}"))?;
    let mut entries = db.load_all().map_err(|e| format!("db: {e:?}"))?;
    for entry in &mut entries {
        entry.response_chunks = db.load_chunks(&entry.id).map_err(|e| format!("db: {e:?}"))?;
    }
    Ok(entries)
}
