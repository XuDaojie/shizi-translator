//! 弹窗 UI facade：按 config/session 选择 backend，统一 ensure/show/hide。
//!
//! WinUI：ensure/show 失败则会话级回退 webview（不写回 config）。

use std::sync::{Mutex, OnceLock};

use tauri::{AppHandle, Manager};

use super::kind::PopupUiKind;
use super::session::PopupUiSession;
use super::{PopupUi, WebviewPopupUi};
use crate::app::popup_window::PopupPositionMode;
use crate::app::state::AppState;
use crate::core::config::AppConfig;

#[cfg(windows)]
use super::WinUiPopupUi;

fn webview_ui() -> WebviewPopupUi {
    WebviewPopupUi::new()
}

#[cfg(windows)]
fn winui_ui() -> WinUiPopupUi {
    WinUiPopupUi::new()
}

/// 进程级后备 session（AppState 尚未 manage 时）。优先用 AppState 单一真相。
fn process_session() -> &'static Mutex<PopupUiSession> {
    static SESSION: OnceLock<Mutex<PopupUiSession>> = OnceLock::new();
    SESSION.get_or_init(|| Mutex::new(PopupUiSession::new()))
}

fn with_session<R>(app: &AppHandle, f: impl FnOnce(&mut PopupUiSession) -> R) -> Result<R, String> {
    if let Some(state) = app.try_state::<AppState>() {
        let mut guard = state
            .popup_ui_session
            .lock()
            .map_err(|_| "popup_ui session 锁已损坏".to_string())?;
        return Ok(f(&mut guard));
    }
    let mut guard = process_session()
        .lock()
        .map_err(|_| "popup_ui session 锁已损坏".to_string())?;
    Ok(f(&mut guard))
}

/// ensure/show 路径：同步 desired，**不**清除会话回退。
fn sync_desired_from_config(app: &AppHandle, config: &AppConfig) -> Result<(), String> {
    let kind = PopupUiKind::resolve_from_config(&config.popup_ui);
    with_session(app, |session| session.sync_desired(kind))
}

fn active_kind(app: &AppHandle) -> PopupUiKind {
    with_session(app, |session| session.active()).unwrap_or(PopupUiKind::Webview)
}

/// 按 config 同步 desired，并在 `window_precreate` 允许时 ensure 当前 active 后端。
pub fn ensure_for_config(app: &AppHandle, config: &AppConfig) -> Result<(), String> {
    sync_desired_from_config(app, config)?;

    let pair = config
        .window_precreate
        .for_launch(crate::app::autostart::is_autostart_process());
    if !pair.popup {
        return Ok(());
    }

    ensure_active(app)
}

fn ensure_active(app: &AppHandle) -> Result<(), String> {
    let active = active_kind(app);
    match active {
        PopupUiKind::Webview => webview_ui().ensure(app),
        PopupUiKind::WinUi => {
            #[cfg(windows)]
            {
                match winui_ui().ensure(app) {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        log::warn!("WinUI ensure 失败，本会话回退 webview: {e}");
                        with_session(app, |session| session.fallback_to_webview_for_session())?;
                        webview_ui().ensure(app)
                    }
                }
            }
            #[cfg(not(windows))]
            {
                webview_ui().ensure(app)
            }
        }
    }
}

/// 成功 show 后推送 `show_context`（无 sink 时 no-op）。
/// pending 仅有 take 无 peek：`sourceText` 置空，宿主 cold-start 可调 `take_pending_source_text`。
fn push_show_context(mode: PopupPositionMode) {
    let position_mode = match mode {
        PopupPositionMode::NearCursor => "nearCursor",
        PopupPositionMode::Restore => "restore",
    };
    crate::app::popup_bridge::push::global().push_json(
        "show_context",
        serde_json::json!({
            "sourceText": "",
            "sourceBadge": "selectedText",
            "positionMode": position_mode,
        }),
    );
}

fn hide_inactive(app: &AppHandle, active: PopupUiKind) {
    match active {
        PopupUiKind::Webview => {
            #[cfg(windows)]
            {
                let _ = winui_ui().hide(app);
            }
        }
        PopupUiKind::WinUi => {
            let _ = webview_ui().hide(app);
        }
    }
}

/// 按 config 同步 session；show 前 hide 非 active 后端。
pub fn show_for_config(
    app: &AppHandle,
    config: &AppConfig,
    mode: PopupPositionMode,
) -> Result<(), String> {
    sync_desired_from_config(app, config)?;

    // 先 ensure（含 winui 失败回退）
    ensure_active(app)?;
    let active = active_kind(app);
    hide_inactive(app, active);

    let result = match active {
        PopupUiKind::Webview => webview_ui().show(app, mode),
        PopupUiKind::WinUi => {
            #[cfg(windows)]
            {
                match winui_ui().show(app, mode) {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        log::warn!("WinUI show 失败，本会话回退 webview: {e}");
                        with_session(app, |session| session.fallback_to_webview_for_session())?;
                        let _ = winui_ui().hide(app);
                        webview_ui().show(app, mode)
                    }
                }
            }
            #[cfg(not(windows))]
            {
                webview_ui().show(app, mode)
            }
        }
    };

    if result.is_ok() {
        push_show_context(mode);
    }
    result
}

/// 与 `show_for_config` 相同的 session/backend 路由，但 webview 分支用阻塞 show。
pub fn show_blocking_for_config(
    app: &AppHandle,
    config: &AppConfig,
    mode: PopupPositionMode,
) -> Result<(), String> {
    sync_desired_from_config(app, config)?;
    ensure_active(app)?;
    let active = active_kind(app);
    hide_inactive(app, active);

    let result = match active {
        PopupUiKind::Webview => {
            crate::app::popup_window::show_popup_blocking(app, config, mode)
        }
        PopupUiKind::WinUi => {
            #[cfg(windows)]
            {
                match winui_ui().show(app, mode) {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        log::warn!("WinUI show(blocking) 失败，本会话回退 webview: {e}");
                        with_session(app, |session| session.fallback_to_webview_for_session())?;
                        let _ = winui_ui().hide(app);
                        crate::app::popup_window::show_popup_blocking(app, config, mode)
                    }
                }
            }
            #[cfg(not(windows))]
            {
                crate::app::popup_window::show_popup_blocking(app, config, mode)
            }
        }
    };
    if result.is_ok() {
        push_show_context(mode);
    }
    result
}

/// 隐藏当前弹窗后端；可同时 hide 两边保证互斥。
pub fn hide_active(app: &AppHandle) -> Result<(), String> {
    let _ = webview_ui().hide(app);
    #[cfg(windows)]
    {
        let _ = winui_ui().hide(app);
    }
    Ok(())
}

/// 配置中 `popup_ui` 变更后：更新 desired、hide 两边；**不** ensure。
pub fn on_popup_ui_config_changed(app: &AppHandle, new_config: &AppConfig) {
    let kind = PopupUiKind::resolve_from_config(&new_config.popup_ui);
    if let Err(error) = with_session(app, |session| session.set_desired(kind)) {
        log::warn!("更新 popup_ui session 失败: {error}");
    }
    if let Err(error) = hide_active(app) {
        log::warn!("popup_ui 配置变更后 hide 失败: {error}");
    }
}

/// 进程退出时 best-effort 关闭 WinUI 子进程。
#[cfg(windows)]
pub fn shutdown_winui_host() {
    crate::app::popup_bridge::host::shutdown();
}

#[cfg(not(windows))]
pub fn shutdown_winui_host() {}
