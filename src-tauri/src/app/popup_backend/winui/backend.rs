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
    /// 逻辑可见标志（show/hide 写入；`is_visible` 优先 `IsWindowVisible`）。
    #[allow(dead_code)]
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
        // 优先真实 HWND 的 IsWindowVisible；句柄无效则不可见
        if let Some(hwnd) = self.hwnd.as_ref() {
            if hwnd.is_valid() {
                return hwnd.is_visible();
            }
        }
        false
    }

    fn is_alive(&self) -> bool {
        self.hwnd_alive()
    }

    fn publish(&mut self, vm: &PopupViewModel) {
        // 非阻塞：写共享快照 + PostMessage 触发 UI 线程 InvalidateRect。
        // 注意：PopupHost 可能持锁调用本方法，禁止同步等待 UI 线程。
        if let Some(hwnd) = self.hwnd.as_ref() {
            ui::publish_view_model(hwnd, vm);
        } else {
            // 窗尚未创建时仍落快照，ensure/show 后首次 PAINT 可见
            let _ = ui::store_paint_snapshot(vm);
        }
    }
}
