use crate::{
    app::{
        shortcuts::{replace_global_shortcuts, ShortcutBindingError},
        state::AppState,
        window::show_window,
    },
    core::config::AppConfig,
};
use tauri::Emitter;

#[tauri::command]
pub fn open_settings(app: tauri::AppHandle) -> Result<(), String> {
    show_window(&app);
    Ok(())
}

#[tauri::command]
pub async fn get_app_config(state: tauri::State<'_, AppState>) -> Result<AppConfig, String> {
    state.config_store.get().map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn save_app_config(
    config: AppConfig,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<AppConfig, ShortcutBindingError> {
    let old_config = state
        .config_store
        .get()
        .map_err(|error| ShortcutBindingError::global(format!("无法读取旧配置: {error}")))?;
    let config = config.normalized();

    replace_global_shortcuts(&app, &old_config, &config)?;

    let saved_config = state
        .config_store
        .save(config)
        .map_err(|error| ShortcutBindingError::global(format!("无法保存配置: {error}")))?;

    app.emit("app-config:changed", &saved_config)
        .map_err(|error| ShortcutBindingError::global(format!("无法广播配置变更: {error}")))?;

    Ok(saved_config)
}
