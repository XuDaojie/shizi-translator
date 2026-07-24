//! WebView 弹窗后端：包装现网 `popup_window`（Tauri WebView）。

use tauri::Manager;

use super::trait_api::PopupBackend;
use super::types::{PopupPositionMode, PopupUiBackendKind, PopupViewModel};
use crate::app::popup_window::{self, POPUP_LABEL};
use crate::core::config::AppConfig;

/// 基于现有 Tauri WebView 翻译弹窗的 `PopupBackend` 实现。
///
/// 生命周期与定位委托给 [`popup_window`]；`publish` 为 no-op（前端仍走
/// `translation:event`）。
pub struct WebviewPopupBackend {
    app: tauri::AppHandle,
}

impl WebviewPopupBackend {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

impl PopupBackend for WebviewPopupBackend {
    fn kind(&self) -> PopupUiBackendKind {
        PopupUiBackendKind::Webview
    }

    fn ensure_created(&mut self) -> Result<(), String> {
        popup_window::ensure_popup_exists(&self.app).map(|_| ())
    }

    /// 已存在则当前线程 show；不存在则独立线程建窗（与 `show_popup` 一致，避死锁）。
    fn show(&mut self, mode: PopupPositionMode) -> Result<(), String> {
        // 走 show_popup：存在 → blocking；不存在 → spawn 建窗，与现网一致。
        popup_window::show_popup(&self.app, &AppConfig::default(), mode)
    }

    fn hide(&mut self) {
        popup_window::hide_popup(&self.app);
    }

    fn destroy(&mut self) {
        if let Some(w) = self.app.get_webview_window(POPUP_LABEL) {
            // 弹窗挂了 close→hide，`close()` 只会隐藏；须用 destroy 真销毁，
            // 之后 is_alive 为 false，ensure 可再重建。
            if let Err(e) = w.destroy() {
                log::warn!("销毁翻译弹窗失败: {e}");
            }
        }
    }

    fn is_visible(&self) -> bool {
        self.app
            .get_webview_window(POPUP_LABEL)
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false)
    }

    fn is_alive(&self) -> bool {
        self.app.get_webview_window(POPUP_LABEL).is_some()
    }

    fn publish(&mut self, _vm: &PopupViewModel) {
        // WebView 路径继续靠 translation:event；此处 no-op。
    }
}
