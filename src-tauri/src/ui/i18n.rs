use crate::{
    app::{state::AppState, tray::TrayI18nHandles},
    core::i18n::{resolve_locale, resolve_messages, scan_language_packs, LanguageSnapshot},
};
use serde::Serialize;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::{Mutex, MutexGuard},
};
use tauri::{AppHandle, Emitter, Manager, State};

// ponytail: 语言切换是低频全局操作；未来出现并发瓶颈时再迁移为 async/per-app 锁。
static LANGUAGE_APPLY_LOCK: Mutex<()> = Mutex::new(());

pub(crate) fn with_interface_language_lock<T>(
    f: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let _guard = lock_interface_language()?;
    f()
}

pub(crate) fn lock_interface_language() -> Result<MutexGuard<'static, ()>, String> {
    LANGUAGE_APPLY_LOCK
        .lock()
        .map_err(|_| "界面语言应用锁已损坏".to_string())
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LanguageChanged<'a> {
    locale: &'a str,
    revision: u64,
}

fn language_pack_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|dir| dir.join("lang"))
        .map_err(|error| format!("无法获取应用配置目录: {error}"))
}

fn build_language_snapshot(
    app: &AppHandle,
    configured_locale: &str,
    revision: u64,
) -> Result<LanguageSnapshot, String> {
    let dir = language_pack_dir(app)?;

    let available = scan_language_packs(&dir, None);
    let locale = resolve_locale(
        configured_locale,
        sys_locale::get_locale().as_deref(),
        &available,
    );
    let scan = scan_language_packs(&dir, Some(&locale));

    Ok(LanguageSnapshot {
        configured_locale: configured_locale.into(),
        locale,
        revision,
        languages: scan.languages,
        user_messages: scan.user_messages,
        errors: scan.errors,
    })
}

pub fn apply_interface_language(
    app: &AppHandle,
    state: &AppState,
    configured_locale: &str,
    increment_revision: bool,
    emit_change: bool,
) -> Result<LanguageSnapshot, String> {
    with_interface_language_lock(|| {
        apply_interface_language_locked(
            app,
            state,
            configured_locale,
            increment_revision,
            emit_change,
        )
    })
}

pub(crate) fn apply_interface_language_locked(
    app: &AppHandle,
    state: &AppState,
    configured_locale: &str,
    increment_revision: bool,
    emit_change: bool,
) -> Result<LanguageSnapshot, String> {
    fs::create_dir_all(language_pack_dir(app)?)
        .map_err(|error| format!("无法创建语言包目录: {error}"))?;
    let mut snapshot =
        build_language_snapshot(app, configured_locale, state.interface_language_revision())?;
    let scan = crate::core::i18n::LanguagePackScan {
        user_messages: snapshot.user_messages.clone(),
        ..Default::default()
    };
    let messages = resolve_messages(&snapshot.locale, &scan);
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

    snapshot.revision = if increment_revision {
        state.next_interface_language_revision()
    } else {
        state.interface_language_revision()
    };
    if emit_change {
        app.emit(
            "interface-language:changed",
            LanguageChanged {
                locale: &snapshot.locale,
                revision: snapshot.revision,
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
    with_interface_language_lock(|| {
        let configured = state
            .config_store
            .get()
            .map_err(|error| error.to_string())?;
        build_language_snapshot(
            &app,
            &configured.interface_language,
            state.interface_language_revision(),
        )
    })
}

#[tauri::command]
pub fn refresh_interface_languages(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<LanguageSnapshot, String> {
    with_interface_language_lock(|| {
        let configured = state
            .config_store
            .get()
            .map_err(|error| error.to_string())?;
        apply_interface_language_locked(&app, &state, &configured.interface_language, true, true)
    })
}

#[tauri::command]
pub fn open_language_pack_directory(app: AppHandle) -> Result<(), String> {
    let dir = language_pack_dir(&app)?;
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
