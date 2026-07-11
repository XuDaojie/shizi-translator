use crate::{
    app::{state::AppState, tray::TrayI18nHandles},
    core::i18n::{resolve_locale, resolve_messages, scan_language_packs, LanguageSnapshot},
};
use serde::Serialize;
use std::{fs, process::Command};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LanguageChanged<'a> {
    locale: &'a str,
    revision: u64,
}

pub fn apply_interface_language(
    app: &AppHandle,
    state: &AppState,
    configured_locale: &str,
    increment_revision: bool,
    emit_change: bool,
) -> Result<LanguageSnapshot, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("无法获取应用配置目录: {error}"))?
        .join("lang");
    fs::create_dir_all(&dir).map_err(|error| format!("无法创建语言包目录: {error}"))?;

    let available = scan_language_packs(&dir, None);
    let locale = resolve_locale(
        configured_locale,
        sys_locale::get_locale().as_deref(),
        &available,
    );
    let scan = scan_language_packs(&dir, Some(&locale));
    let messages = resolve_messages(&locale, &scan);
    let handles = app.state::<TrayI18nHandles>();

    handles
        .translate
        .set_text(&messages["tray.translate"])
        .map_err(|error| format!("无法更新托盘翻译菜单: {error}"))?;
    handles
        .settings
        .set_text(&messages["tray.settings"])
        .map_err(|error| format!("无法更新托盘设置菜单: {error}"))?;
    handles
        .quit
        .set_text(&messages["tray.quit"])
        .map_err(|error| format!("无法更新托盘退出菜单: {error}"))?;
    handles
        .tray
        .set_tooltip(Some(&messages["tray.tooltip"]))
        .map_err(|error| format!("无法更新托盘提示: {error}"))?;

    *handles
        .popup_title
        .write()
        .map_err(|_| "翻译窗口标题状态锁已损坏".to_string())? =
        messages["window.popupTitle"].clone();
    *handles
        .settings_title
        .write()
        .map_err(|_| "设置窗口标题状态锁已损坏".to_string())? =
        messages["window.settingsTitle"].clone();

    if let Some(window) = app.get_webview_window("main") {
        window
            .set_title(&messages["window.popupTitle"])
            .map_err(|error| format!("无法更新翻译窗口标题: {error}"))?;
    }
    if let Some(window) = app.get_webview_window("settings") {
        window
            .set_title(&messages["window.settingsTitle"])
            .map_err(|error| format!("无法更新设置窗口标题: {error}"))?;
    }

    let revision = if increment_revision {
        state.next_interface_language_revision()
    } else {
        state.interface_language_revision()
    };

    let snapshot = LanguageSnapshot {
        configured_locale: configured_locale.into(),
        locale,
        revision,
        languages: scan.languages,
        user_messages: scan.user_messages,
        errors: scan.errors,
    };
    if emit_change {
        app.emit(
            "interface-language:changed",
            LanguageChanged {
                locale: &snapshot.locale,
                revision,
            },
        )
        .map_err(|error| format!("界面语言已更新，但无法广播语言变更: {error}"))?;
    }
    Ok(snapshot)
}

#[tauri::command]
pub fn get_interface_language_snapshot(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<LanguageSnapshot, String> {
    let configured = state
        .config_store
        .get()
        .map_err(|error| error.to_string())?;
    apply_interface_language(&app, &state, &configured.interface_language, false, false)
}

#[tauri::command]
pub fn refresh_interface_languages(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<LanguageSnapshot, String> {
    let configured = state
        .config_store
        .get()
        .map_err(|error| error.to_string())?;
    apply_interface_language(&app, &state, &configured.interface_language, true, true)
}

#[tauri::command]
pub fn open_language_pack_directory(app: AppHandle) -> Result<(), String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("无法获取应用配置目录: {error}"))?
        .join("lang");
    fs::create_dir_all(&dir).map_err(|error| format!("无法创建语言包目录: {error}"))?;
    #[cfg(windows)]
    {
        Command::new("explorer.exe")
            .arg(dir)
            .spawn()
            .map_err(|error| format!("无法打开语言包目录: {error}"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = dir;
        Err("当前平台暂不支持打开语言包目录".into())
    }
}
