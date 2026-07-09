use tauri;
use crate::proxy::state::AppState;
use crate::config::db::StoredEntry;

#[tauri::command]
pub async fn load_traffic_history(state: tauri::State<'_, AppState>) -> Result<Vec<StoredEntry>, String> {
    let db = state.db();
    let mut entries = db.load_all().map_err(|e| format!("db: {e:?}"))?;
    for entry in &mut entries {
        entry.response_chunks = db.load_chunks(entry.id.parse::<i64>().unwrap_or(0)).map_err(|e| format!("db: {e:?}"))?;
    }
    Ok(entries)
}
