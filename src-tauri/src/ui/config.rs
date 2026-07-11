use crate::ui::i18n::{apply_interface_language_locked, with_interface_language_lock};
use crate::{
    app::{
        shortcuts::{replace_global_shortcuts, ShortcutBindingError},
        state::AppState,
        window::show_settings_window,
    },
    core::config::AppConfig,
};
use tauri::Emitter;

#[tauri::command]
pub fn open_settings(app: tauri::AppHandle) -> Result<(), String> {
    show_settings_window(&app)
}

#[tauri::command]
pub async fn get_app_config(state: tauri::State<'_, AppState>) -> Result<AppConfig, String> {
    state.config_store.get().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_app_config(
    config: AppConfig,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<AppConfig, ShortcutBindingError> {
    with_interface_language_lock(|| {
        let old_config = state
            .config_store
            .get()
            .map_err(|error| format!("无法读取旧配置: {error}"))?;
        let config = config.normalized();

        replace_global_shortcuts(&app, &old_config, &config).map_err(|error| error.to_string())?;

        let saved_config = state
            .config_store
            .save(config)
            .map_err(|error| format!("无法保存配置: {error}"))?;

        log::set_max_level(crate::app::logging::parse_level_filter(
            &saved_config.log_level,
        ));
        let _ = state.set_shortcut_conflicts(Vec::new());

        apply_interface_language_locked(&app, &state, &saved_config.interface_language, true, true)
            .map_err(|error| format!("配置已保存且快捷键已更新，但界面语言同步失败: {error}"))?;

        app.emit("app-config:changed", &saved_config)
            .map_err(|error| format!("无法广播配置变更: {error}"))?;

        Ok(saved_config)
    })
    .map_err(ShortcutBindingError::global)
}

#[tauri::command]
pub fn get_shortcut_conflicts(state: tauri::State<'_, AppState>) -> Vec<ShortcutBindingError> {
    state.shortcut_conflicts().unwrap_or_default()
}
