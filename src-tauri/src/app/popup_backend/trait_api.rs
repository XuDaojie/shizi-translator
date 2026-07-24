//! PopupBackend trait：弹窗 UI 后端抽象（WebView / WinUI 共用）。

use super::types::{PopupPositionMode, PopupUiBackendKind, PopupViewModel};

/// 弹窗 UI 后端：创建 / 显示 / 隐藏 / 销毁 / 推送 ViewModel。
///
/// 实现须 `Send`，以便在 app 状态中跨线程持有。
pub trait PopupBackend: Send {
    fn kind(&self) -> PopupUiBackendKind;
    fn ensure_created(&mut self) -> Result<(), String>;
    fn show(&mut self, mode: PopupPositionMode) -> Result<(), String>;
    fn hide(&mut self);
    fn destroy(&mut self);
    fn is_visible(&self) -> bool;
    fn is_alive(&self) -> bool;
    fn publish(&mut self, vm: &PopupViewModel);
}
