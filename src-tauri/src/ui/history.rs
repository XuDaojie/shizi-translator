use crate::{app::state::AppState, core::history::HistorySessionDto};

#[tauri::command]
pub async fn list_translation_history(
    limit: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<HistorySessionDto>, String> {
    let config = state.config_store.get().map_err(|error| error.to_string())?;
    let limit = limit.unwrap_or(config.history_limit).max(1);
    state
        .history_store
        .list_recent(limit)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn clear_translation_history(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state
        .history_store
        .clear()
        .map_err(|error| error.to_string())
}
