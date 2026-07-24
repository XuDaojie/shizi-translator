//! 弹窗 UI facade：按 config/session 选择 backend，统一 ensure/show/hide。
//!
//! 本任务 WinUI 宿主未接入：desired=winui 时会话级回退到 webview。

use std::sync::{Mutex, OnceLock};

use tauri::{AppHandle, Manager};

use super::kind::PopupUiKind;
use super::session::PopupUiSession;
use super::{PopupUi, WebviewPopupUi};
use crate::app::popup_window::PopupPositionMode;
use crate::app::state::AppState;
use crate::core::config::AppConfig;

fn webview_ui() -> WebviewPopupUi {
    WebviewPopupUi::new()
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

fn set_desired_from_config(app: &AppHandle, config: &AppConfig) -> Result<(), String> {
    let kind = PopupUiKind::resolve_from_config(&config.popup_ui);
    with_session(app, |session| session.set_desired(kind))
}

fn active_kind(app: &AppHandle) -> PopupUiKind {
    with_session(app, |session| session.active()).unwrap_or(PopupUiKind::Webview)
}

/// 若 active 为 WinUi（宿主未就绪），会话回退到 webview 并返回最终 active。
fn resolve_active_with_winui_stub(app: &AppHandle) -> Result<PopupUiKind, String> {
    let active = active_kind(app);
    if active == PopupUiKind::WinUi {
        log::warn!("WinUI 弹窗宿主尚未就绪，本会话回退到 webview");
        with_session(app, |session| session.fallback_to_webview_for_session())?;
        return Ok(PopupUiKind::Webview);
    }
    Ok(active)
}

/// 按 config 更新 desired，并在 `window_precreate` 允许时 ensure 当前 active 后端。
pub fn ensure_for_config(app: &AppHandle, config: &AppConfig) -> Result<(), String> {
    set_desired_from_config(app, config)?;

    let pair = config
        .window_precreate
        .for_launch(crate::app::autostart::is_autostart_process());
    if !pair.popup {
        return Ok(());
    }

    let active = resolve_active_with_winui_stub(app)?;
    match active {
        PopupUiKind::Webview => webview_ui().ensure(app),
        PopupUiKind::WinUi => {
            // resolve 已回退；防御性分支
            webview_ui().ensure(app)
        }
    }
}

/// 按 config 更新 session；show 前 hide 非 active 后端；active 为 winui 时 stub 回退。
pub fn show_for_config(
    app: &AppHandle,
    config: &AppConfig,
    mode: PopupPositionMode,
) -> Result<(), String> {
    set_desired_from_config(app, config)?;

    let active = resolve_active_with_winui_stub(app)?;
    // 互斥：hide 非 active。WinUI 尚未实现，仅保证 webview 在非 webview 时被 hide。
    match active {
        PopupUiKind::Webview => {
            // 未来：hide winui
        }
        PopupUiKind::WinUi => {
            let _ = webview_ui().hide(app);
        }
    }

    match active {
        PopupUiKind::Webview | PopupUiKind::WinUi => webview_ui().show(app, mode),
    }
}

/// 隐藏当前（及未来其它）弹窗后端；可同时 hide 两边保证互斥。
pub fn hide_active(app: &AppHandle) -> Result<(), String> {
    webview_ui().hide(app)?;
    // 未来：hide winui
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
