//! 弹窗后端：ViewModel、PopupBackend trait、Webview 实现与 PopupHost 调度。
//! WinUI 后端 UI 见后续任务；M1 下 `Winui` kind 仍回退 WebView。

// 部分 API 供后续 WinUI / 设置页使用，当前主路径尚未全量消费。
#![allow(dead_code)]

pub mod host;
pub mod trait_api;
pub mod types;
pub mod view_model;
pub mod webview;

pub use host::{resolve_popup_backend_kind, PopupHost, POPUP_WINUI_FEATURE};
pub use trait_api::PopupBackend;
pub use types::*;
pub use webview::WebviewPopupBackend;

use std::sync::Mutex;
use tauri::{AppHandle, Manager};

/// 按解析后的 kind 创建具体 backend。
///
/// M1：`Winui` 尚无实现，回退 WebView 并 `log::warn`。
pub fn create_backend(app: &AppHandle, kind: PopupUiBackendKind) -> Box<dyn PopupBackend> {
    match kind {
        PopupUiBackendKind::Webview => Box::new(WebviewPopupBackend::new(app.clone())),
        PopupUiBackendKind::Winui => {
            log::warn!("popupUiBackend=winui 但 WinUI backend 尚未就绪，使用 webview");
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
