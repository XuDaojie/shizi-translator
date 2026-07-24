//! WebView 弹窗 UI 适配：委托现有 `popup_window` 路径，保持零回归。

use tauri::{Manager, PhysicalSize, Size};

use super::PopupUi;
use crate::app::popup_window::{self, PopupPositionMode, POPUP_LABEL};
use crate::app::state::AppState;
use crate::core::config::AppConfig;

/// 默认 webview 后端：行为与改前 `popup_window` 一致。
#[derive(Debug, Default, Clone, Copy)]
pub struct WebviewPopupUi;

impl WebviewPopupUi {
    pub fn new() -> Self {
        Self
    }
}

impl PopupUi for WebviewPopupUi {
    fn ensure(&self, app: &tauri::AppHandle) -> Result<(), String> {
        popup_window::ensure_popup_exists(app).map(|_| ())
    }

    fn show(&self, app: &tauri::AppHandle, mode: PopupPositionMode) -> Result<(), String> {
        // show_popup 的 config 参数在 blocking 路径未使用；仍从 state 取以保持 API 一致。
        let config = app
            .try_state::<AppState>()
            .and_then(|s| s.config_store.get().ok())
            .unwrap_or_else(AppConfig::default);
        // 已存在 → blocking；不存在 → 独立线程建窗（Windows 回调栈安全）。
        popup_window::show_popup(app, &config, mode)
    }

    fn hide(&self, app: &tauri::AppHandle) -> Result<(), String> {
        popup_window::hide_popup(app);
        Ok(())
    }

    fn set_always_on_top(&self, app: &tauri::AppHandle, on: bool) -> Result<(), String> {
        // 注意：POPUP_LABEL 实际为 "main"（翻译弹窗），非设置窗。
        if let Some(window) = app.get_webview_window(POPUP_LABEL) {
            window
                .set_always_on_top(on)
                .map_err(|error| format!("设置弹窗置顶失败: {error}"))?;
        }
        Ok(())
    }

    fn set_size(&self, app: &tauri::AppHandle, width: f64, height: f64) -> Result<(), String> {
        if let Some(window) = app.get_webview_window(POPUP_LABEL) {
            // 优先逻辑尺寸；失败时再试物理像素（scale=1 时等价）。
            let logical = tauri::LogicalSize::new(width, height);
            if window.set_size(Size::Logical(logical)).is_err() {
                window
                    .set_size(Size::Physical(PhysicalSize::new(
                        width.round() as u32,
                        height.round() as u32,
                    )))
                    .map_err(|error| format!("设置弹窗尺寸失败: {error}"))?;
            }
        }
        Ok(())
    }

    fn is_available(&self) -> bool {
        true
    }
}
