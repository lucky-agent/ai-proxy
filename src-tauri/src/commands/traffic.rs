use tauri;
use crate::proxy::state::AppState;
use crate::config::db::StoredEntry;

#[tauri::command]
pub async fn load_traffic_history(state: tauri::State<'_, AppState>) -> Result<Vec<StoredEntry>, String> {
    let db = state.db();
    let db = db.lock().map_err(|e| format!("db lock: {e:?}"))?;
    db.load_all().map_err(|e| format!("db: {e:?}"))
}
