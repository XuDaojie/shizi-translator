//! `WinuiPopupBackend`：原生弹窗后端（**路径 B：Win32 表面**）。
//!
//! 配置枚举仍为 `winui`；实际 UI 为 Win32 `WS_POPUP` 壳，不依赖 XAML Runtime。
//! 翻译协议 / 配置持久化 / 历史写入不在本层。

use super::bootstrap;
use super::ui::{self, NativePopupHwnd};
use crate::app::popup_backend::trait_api::PopupBackend;
use crate::app::popup_backend::types::{PopupPositionMode, PopupUiBackendKind, PopupViewModel};

/// 基于 Win32 原生表面的翻译弹窗后端（feature 名 / 配置值仍为 winui）。
pub struct WinuiPopupBackend {
    #[allow(dead_code)]
    app: tauri::AppHandle,
    hwnd: Option<NativePopupHwnd>,
    /// 逻辑可见标志；与 HWND 可见性双轨，hide 后仍 alive。
    visible: bool,
}

impl WinuiPopupBackend {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self {
            app,
            hwnd: None,
            visible: false,
        }
    }

    fn hwnd_alive(&self) -> bool {
        self.hwnd.as_ref().is_some_and(|h| h.is_valid())
    }
}

impl PopupBackend for WinuiPopupBackend {
    fn kind(&self) -> PopupUiBackendKind {
        PopupUiBackendKind::Winui
    }

    fn ensure_created(&mut self) -> Result<(), String> {
        if self.hwnd_alive() {
            return Ok(());
        }
        // 失效句柄先清掉
        self.hwnd = None;

        let status = bootstrap::try_bootstrap();
        if !status.ok {
            return Err(status.message);
        }
        log::debug!("native popup bootstrap: {}", status.message);

        let hwnd = ui::create_hidden_popup()?;
        self.hwnd = Some(hwnd);
        self.visible = false;
        Ok(())
    }

    fn show(&mut self, mode: PopupPositionMode) -> Result<(), String> {
        // 与 WebView show 可建窗类似：未创建则 ensure（Host 热路径不保证先 ensure）
        self.ensure_created()?;
        let hwnd = self
            .hwnd
            .as_ref()
            .ok_or_else(|| "原生弹窗未创建".to_string())?;
        ui::show_popup(hwnd, mode)?;
        self.visible = true;
        Ok(())
    }

    fn hide(&mut self) {
        if let Some(hwnd) = self.hwnd.as_ref() {
            ui::hide_popup(hwnd);
        }
        self.visible = false;
    }

    fn destroy(&mut self) {
        if let Some(hwnd) = self.hwnd.take() {
            ui::destroy_popup(&hwnd);
        }
        self.visible = false;
    }

    fn is_visible(&self) -> bool {
        if let Some(hwnd) = self.hwnd.as_ref() {
            if hwnd.is_valid() {
                // 优先真实 HWND 状态；不可见时回落逻辑标志
                return hwnd.is_visible() || self.visible;
            }
        }
        false
    }

    fn is_alive(&self) -> bool {
        self.hwnd_alive()
    }

    fn publish(&mut self, _vm: &PopupViewModel) {
        // 任务 8：绑定 ViewModel 到原生控件；本任务仅占位
    }
}
