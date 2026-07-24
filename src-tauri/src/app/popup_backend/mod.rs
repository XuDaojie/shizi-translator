//! 弹窗后端：ViewModel、PopupBackend trait、Webview / WinUI 实现与 PopupHost 调度。
//!
//! `popup-winui` feature + Windows 时 `Winui` kind 使用 [`winui::WinuiPopupBackend`]（骨架）；
//! 否则回退 WebView。

// 部分 API 供后续 WinUI / 设置页使用，当前主路径尚未全量消费。
#![allow(dead_code)]

pub mod host;
pub mod trait_api;
pub mod types;
pub mod view_model;
pub mod webview;

#[cfg(all(windows, feature = "popup-winui"))]
pub mod winui;

pub use host::{resolve_popup_backend_kind, PopupHost, POPUP_WINUI_FEATURE};
pub use trait_api::PopupBackend;
pub use types::*;
pub use webview::WebviewPopupBackend;

use std::sync::Mutex;
use tauri::{AppHandle, Manager};

/// 按解析后的 kind 创建具体 backend。
///
/// Windows + `popup-winui`：`Winui` → [`winui::WinuiPopupBackend`]；
/// 否则 `Winui` 回退 WebView 并 `log::warn`。
pub fn create_backend(app: &AppHandle, kind: PopupUiBackendKind) -> Box<dyn PopupBackend> {
    match kind {
        PopupUiBackendKind::Webview => Box::new(WebviewPopupBackend::new(app.clone())),
        #[cfg(all(windows, feature = "popup-winui"))]
        PopupUiBackendKind::Winui => Box::new(winui::WinuiPopupBackend::new(app.clone())),
        #[cfg(not(all(windows, feature = "popup-winui")))]
        PopupUiBackendKind::Winui => {
            log::warn!("popupUiBackend=winui 但 WinUI backend 不可用，使用 webview");
            Box::new(WebviewPopupBackend::new(app.clone()))
        }
    }
}

/// 在已 manage 的 [`PopupHost`] 上执行闭包（统一加锁入口，避免业务层直接拿锁）。
pub fn with_host<R>(app: &AppHandle, f: impl FnOnce(&mut PopupHost) -> R) -> Result<R, String> {
    let state = app.state::<Mutex<PopupHost>>();
    let mut guard = state
        .lock()
        .map_err(|_| "PopupHost lock poisoned".to_string())?;
    Ok(f(&mut guard))
}
