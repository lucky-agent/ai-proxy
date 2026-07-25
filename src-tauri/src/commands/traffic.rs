use crate::proxy::state::AppState;
use crate::storage::traffic::TrafficLogEntry;
use tauri;

#[tauri::command]
pub async fn load_traffic_history(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<TrafficLogEntry>, String> {
    let db = state.db();
    let mut entries = db.load_all_traffic().map_err(|e| format!("db: {e:?}"))?;
    for entry in &mut entries {
        entry.response_chunks = db
            .load_chunks(entry.id as i64)
            .map_err(|e| format!("db: {e:?}"))?;
    }
    Ok(entries)
}

#[tauri::command]
pub async fn get_traffic_detail(
    state: tauri::State<'_, AppState>,
    id: u64,
) -> Result<TrafficLogEntry, String> {
    let db = state.db();
    let mut entry = db
        .load_traffic_detail(id as i64)
        .map_err(|e| format!("db: {e:?}"))?;
    entry.response_chunks = db
        .load_chunks(id as i64)
        .map_err(|e| format!("db: {e:?}"))?;
    Ok(entry)
}
