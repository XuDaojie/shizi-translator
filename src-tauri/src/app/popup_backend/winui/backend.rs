//! `WinuiPopupBackend`：原生 WinUI 弹窗后端骨架。
//!
//! 生命周期与 UI 绑定在后续任务实现；当前 ensure/show 返回未实现错误。

use super::bootstrap;
use super::ui;
use crate::app::popup_backend::trait_api::PopupBackend;
use crate::app::popup_backend::types::{PopupPositionMode, PopupUiBackendKind, PopupViewModel};

/// 基于 WinUI 的翻译弹窗后端（骨架）。
pub struct WinuiPopupBackend {
    #[allow(dead_code)]
    app: tauri::AppHandle,
    alive: bool,
    visible: bool,
}

impl WinuiPopupBackend {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self {
            app,
            alive: false,
            visible: false,
        }
    }
}

impl PopupBackend for WinuiPopupBackend {
    fn kind(&self) -> PopupUiBackendKind {
        PopupUiBackendKind::Winui
    }

    fn ensure_created(&mut self) -> Result<(), String> {
        // 后续任务：bootstrap + 建窗
        let _ = bootstrap::ensure_winui_runtime();
        Err("WinuiPopupBackend::ensure_created not implemented".to_string())
    }

    fn show(&mut self, _mode: PopupPositionMode) -> Result<(), String> {
        let _ = ui::show_stub();
        Err("WinuiPopupBackend::show not implemented".to_string())
    }

    fn hide(&mut self) {
        self.visible = false;
        let _ = ui::hide_stub();
    }

    fn destroy(&mut self) {
        self.alive = false;
        self.visible = false;
        let _ = ui::destroy_stub();
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn is_alive(&self) -> bool {
        self.alive
    }

    fn publish(&mut self, _vm: &PopupViewModel) {
        // 后续任务：绑定 ViewModel 到原生控件
    }
}
