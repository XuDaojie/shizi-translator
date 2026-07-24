//! WinUI 弹窗 UI 适配：经 popup_bridge::host 子进程 IPC。
//!
//! `#[cfg(windows)]` 模块；非 Windows 不编译本文件。

#![cfg(windows)]

use tauri::AppHandle;

use super::PopupUi;
use crate::app::popup_bridge::host;
use crate::app::popup_window::{
    compute_popup_position, LogicalPos, LogicalRect, LogicalSize, PopupPositionMode,
};
use crate::platform::cursor_logical_context;

/// WinUI 后端：ensure/show/hide 映射到 Shizi.Popup 子进程。
#[derive(Debug, Default, Clone, Copy)]
pub struct WinUiPopupUi;

impl WinUiPopupUi {
    pub fn new() -> Self {
        Self
    }
}

impl PopupUi for WinUiPopupUi {
    fn ensure(&self, app: &AppHandle) -> Result<(), String> {
        if !host::is_available() {
            return Err(
                "Shizi.Popup.exe 不可用（请 build native/windows/popup）".into(),
            );
        }
        host::ensure(app)
    }

    fn show(&self, app: &AppHandle, mode: PopupPositionMode) -> Result<(), String> {
        // 确保进程与隐藏窗
        host::ensure(app)?;

        let (x, y, mode_i) = match mode {
            PopupPositionMode::NearCursor => {
                // scale=1.0：Win32 物理像素 / scale → 逻辑；与 webview 路径在 scale 未知时一致的可接受近似
                let scale = 1.0;
                if let Some((cx, cy, wx, wy, ww, wh)) = cursor_logical_context(scale) {
                    const POPUP_W: f64 = 420.0;
                    const POPUP_H: f64 = 480.0;
                    let pos = compute_popup_position(
                        LogicalPos { x: cx, y: cy },
                        LogicalSize {
                            width: POPUP_W,
                            height: POPUP_H,
                        },
                        LogicalRect {
                            x: wx,
                            y: wy,
                            width: ww,
                            height: wh,
                        },
                    );
                    (pos.x, pos.y, 0)
                } else {
                    (0.0, 0.0, 0)
                }
            }
            PopupPositionMode::Restore => (0.0, 0.0, 1),
        };

        host::show(x, y, mode_i)
    }

    fn hide(&self, _app: &AppHandle) -> Result<(), String> {
        host::hide()
    }

    fn set_always_on_top(&self, _app: &AppHandle, on: bool) -> Result<(), String> {
        if !host::is_running() {
            return Ok(());
        }
        host::set_always_on_top(on)
    }

    fn set_size(&self, _app: &AppHandle, width: f64, height: f64) -> Result<(), String> {
        if !host::is_running() {
            return Ok(());
        }
        host::set_size(width, height)
    }

    fn is_available(&self) -> bool {
        host::is_available()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn winui_popup_ui_reports_availability_without_panic() {
        let ui = WinUiPopupUi::new();
        // 不强制本机已编译 native；仅保证 is_available 可调用
        let _ = ui.is_available();
    }
}
