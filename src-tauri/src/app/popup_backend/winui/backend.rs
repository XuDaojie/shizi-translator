//! `WinuiPopupBackend`：路径 R（windows-reactor 真 WinUI 3）。
//!
//! 配置枚举仍为 `winui`；窗口宿主为 `ReactorHostHandle`（专用 STA + 哨兵）。
//! 翻译协议 / 配置持久化 / 历史写入不在本层。
//! GDI `ui.rs` 仍可编译（任务 11 删除），本后端不再引用 `NativePopupHwnd`。

use super::actions;
use super::bootstrap;
use super::reactor::{state as reactor_state, ReactorHostHandle};
use crate::app::popup_backend::trait_api::PopupBackend;
use crate::app::popup_backend::types::{PopupPositionMode, PopupUiBackendKind, PopupViewModel};

/// 基于 windows-reactor 的翻译弹窗后端（feature 名 / 配置值仍为 winui）。
pub struct WinuiPopupBackend {
    app: tauri::AppHandle,
    host: Option<ReactorHostHandle>,
}

impl WinuiPopupBackend {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app, host: None }
    }

    /// 绑定 `AppHandle`；路径 R 的 view 直调 `handle_user_action`，须有 bound app。
    /// `install_action_handler` 仍注册 GDI 回调（GDI 若仍编译），对 R 无害。
    fn bind_app_for_ui(&self) {
        actions::install_action_handler();
        actions::bind_app(self.app.clone());
    }
}

impl PopupBackend for WinuiPopupBackend {
    fn kind(&self) -> PopupUiBackendKind {
        PopupUiBackendKind::Winui
    }

    fn ensure_created(&mut self) -> Result<(), String> {
        if self.host.as_ref().is_some_and(|h| h.is_alive()) {
            self.bind_app_for_ui();
            return Ok(());
        }
        // 失效 / 无 host：清掉再启
        self.host = None;

        let status = bootstrap::try_bootstrap();
        if !status.ok {
            return Err(status.message);
        }
        log::debug!("reactor popup bootstrap: {}", status.message);

        self.bind_app_for_ui();
        let handle = ReactorHostHandle::start()?;
        self.host = Some(handle);
        Ok(())
    }

    fn show(&mut self, mode: PopupPositionMode) -> Result<(), String> {
        // 与 WebView show 可建窗类似：未创建则 ensure（Host 热路径不保证先 ensure）
        self.ensure_created()?;
        self.host
            .as_ref()
            .ok_or_else(|| "Reactor 弹窗未创建".to_string())?
            .show(mode)
    }

    fn hide(&mut self) {
        if let Some(h) = self.host.as_ref() {
            h.hide();
        }
    }

    fn destroy(&mut self) {
        // 产品 destroy：hide 并放弃 handle；STA 由 HOST_STARTED 防双启。
        // 降级 replace_backend 时走此路径。
        if let Some(h) = self.host.take() {
            h.shutdown();
        }
    }

    fn is_visible(&self) -> bool {
        self.host.as_ref().is_some_and(|h| h.is_visible())
    }

    fn is_alive(&self) -> bool {
        self.host.as_ref().is_some_and(|h| h.is_alive())
    }

    fn publish(&mut self, vm: &PopupViewModel) {
        // 非阻塞：host 写全局快照 + post UI；无 host 时仅 pending store。
        // 注意：PopupHost 可能持锁调用本方法，禁止同步等待 UI 线程。
        if let Some(h) = self.host.as_ref() {
            h.publish(vm);
        } else {
            reactor_state::store_global(vm);
        }
    }
}
