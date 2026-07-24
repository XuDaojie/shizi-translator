//! 弹窗 UI backend 抽象：类型 / 会话 / WebView / WinUI 适配 / facade。
//!
//! 默认 webview 路径与现网 `popup_window` 行为一致；Windows 上 WinUI 经子进程 IPC。

pub mod facade;
pub mod kind;
pub mod session;
pub mod webview;

#[cfg(windows)]
pub mod winui;

pub use kind::PopupUiKind;
pub use session::PopupUiSession;
pub use webview::WebviewPopupUi;

#[cfg(windows)]
pub use winui::WinUiPopupUi;

/// 从 `popup_window` 再导出，避免调用方为 mode 类型反向耦合 facade。
pub use crate::app::popup_window::PopupPositionMode;

/// 翻译弹窗 UI 后端能力（webview / winui）。
#[allow(dead_code)]
pub trait PopupUi: Send + Sync {
    fn ensure(&self, app: &tauri::AppHandle) -> Result<(), String>;
    fn show(&self, app: &tauri::AppHandle, mode: PopupPositionMode) -> Result<(), String>;
    fn hide(&self, app: &tauri::AppHandle) -> Result<(), String>;
    fn set_always_on_top(&self, app: &tauri::AppHandle, on: bool) -> Result<(), String>;
    fn set_size(&self, app: &tauri::AppHandle, width: f64, height: f64) -> Result<(), String>;
    fn is_available(&self) -> bool;
}
