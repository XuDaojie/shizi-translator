use tauri::Manager;
use crate::{app::state::AppState, core::config::AppConfig};

#[tauri::command]
pub fn open_settings(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_app_config(state: tauri::State<'_, AppState>) -> Result<AppConfig, String> {
    state.config_store.get().map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn save_app_config(
    config: AppConfig,
    state: tauri::State<'_, AppState>,
) -> Result<AppConfig, String> {
    state
        .config_store
        .save(config)
        .map_err(|error| error.to_string())
}
