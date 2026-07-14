use crate::ui::i18n::{apply_interface_language_locked, lock_interface_language};
use crate::{
    app::{
        shortcuts::{replace_global_shortcuts, ShortcutBindingError},
        state::AppState,
        window::show_settings_window,
    },
    core::config::AppConfig,
};
use std::process::Command;
use tauri::Emitter;

#[tauri::command]
pub fn open_settings(app: tauri::AppHandle) -> Result<(), String> {
    show_settings_window(&app)
}

/// 用系统默认浏览器打开 https URL（设置页项目主页等）。
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("仅支持 https URL".into());
    }
    #[cfg(windows)]
    {
        // start 的空参数防止 URL 被当成窗口标题
        Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn()
            .map_err(|error| format!("无法打开链接: {error}"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = url;
        Err("当前平台暂不支持打开外部链接".into())
    }
}

#[tauri::command]
pub fn get_app_config(state: tauri::State<'_, AppState>) -> Result<AppConfig, String> {
    let _guard = lock_interface_language()?;
    state.config_store.get().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_app_config(
    config: AppConfig,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<AppConfig, ShortcutBindingError> {
    let _guard = lock_interface_language().map_err(ShortcutBindingError::global)?;
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

    log::set_max_level(crate::app::logging::parse_level_filter(
        &saved_config.log_level,
    ));
    let _ = state.set_shortcut_conflicts(Vec::new());

    apply_interface_language_locked(&app, &state, &saved_config.interface_language, true, true)
        .map_err(|error| {
            ShortcutBindingError::global(format!(
                "配置已保存且快捷键已更新，但界面语言同步失败: {error}"
            ))
        })?;

    app.emit("app-config:changed", &saved_config)
        .map_err(|error| ShortcutBindingError::global(format!("无法广播配置变更: {error}")))?;

    Ok(saved_config)
}

#[tauri::command]
pub fn get_shortcut_conflicts(state: tauri::State<'_, AppState>) -> Vec<ShortcutBindingError> {
    state.shortcut_conflicts().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_url_rejects_non_https() {
        let err = open_url("http://example.com".into()).unwrap_err();
        assert!(err.contains("https"), "{err}");
        let err = open_url("javascript:alert(1)".into()).unwrap_err();
        assert!(err.contains("https"), "{err}");
    }
}
