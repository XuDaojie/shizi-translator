//! PopupHost：持有 PopupBackend，统一调度 ensure/show/hide/publish。

use crate::core::translation::TranslationEvent;

use super::trait_api::PopupBackend;
use super::types::{PopupPositionMode, PopupUiBackendKind, PopupViewModel};
use super::view_model::apply_translation_event;

/// 编译期：是否启用 WinUI 弹窗后端 feature（且在 Windows 上）。
pub const POPUP_WINUI_FEATURE: bool = cfg!(all(windows, feature = "popup-winui"));

/// 弹窗宿主：包装具体 `PopupBackend`，维护 ViewModel 并转发生命周期操作。
pub struct PopupHost {
    backend: Box<dyn PopupBackend>,
    view_model: PopupViewModel,
    /// 若曾从 WinUI 降级到 WebView，供诊断/设置页提示。
    degraded_from_winui: bool,
}

impl PopupHost {
    pub fn from_backend(backend: Box<dyn PopupBackend>) -> Self {
        Self {
            backend,
            view_model: PopupViewModel::default(),
            degraded_from_winui: false,
        }
    }

    /// 销毁当前 backend 并替换为新后端（典型：WinUI 初始化失败 → WebView）。
    ///
    /// 设置 `degraded_from_winui`；`view_model` 重置为默认（WebView 靠后续 event 再填）。
    pub fn replace_backend(&mut self, backend: Box<dyn PopupBackend>) {
        self.backend.destroy();
        self.backend = backend;
        self.view_model = PopupViewModel::default();
        self.degraded_from_winui = true;
    }

    /// 是否曾从 WinUI 降级到 WebView（本进程内）。
    pub fn is_degraded_from_winui(&self) -> bool {
        self.degraded_from_winui
    }

    pub fn ensure_created(&mut self) -> Result<(), String> {
        self.backend.ensure_created()
    }

    /// 仅转发 `backend.show`；不在热路径同步 `ensure_created`。
    /// 预建仍走独立的 [`Self::ensure_created`]（启动路径）。
    pub fn show(&mut self, mode: PopupPositionMode) -> Result<(), String> {
        self.backend.show(mode)
    }

    pub fn hide(&mut self) {
        self.backend.hide();
    }

    pub fn destroy(&mut self) {
        self.backend.destroy();
    }

    pub fn is_visible(&self) -> bool {
        self.backend.is_visible()
    }

    pub fn is_alive(&self) -> bool {
        self.backend.is_alive()
    }

    pub fn kind(&self) -> PopupUiBackendKind {
        self.backend.kind()
    }

    pub fn view_model(&self) -> &PopupViewModel {
        &self.view_model
    }

    /// 归并 translation 事件并推送到后端。
    pub fn publish_from_event(&mut self, event: &TranslationEvent) {
        apply_translation_event(&mut self.view_model, event);
        self.backend.publish(&self.view_model);
    }

    /// 直接用当前 ViewModel 推送（例如语言切换后刷新）。
    pub fn publish_current(&mut self) {
        self.backend.publish(&self.view_model);
    }
}

/// 根据配置值、feature 与平台解析弹窗后端种类。
///
/// - 仅当 `config_value` 为 `"winui"`（忽略大小写与首尾空白）、feature 开启且为 Windows 时返回 `Winui`；
/// - 其余一律 `Webview`。
pub fn resolve_popup_backend_kind(
    config_value: &str,
    feature_enabled: bool,
    is_windows: bool,
) -> PopupUiBackendKind {
    let normalized = config_value.trim().to_ascii_lowercase();
    if normalized == "winui" && feature_enabled && is_windows {
        PopupUiBackendKind::Winui
    } else {
        PopupUiBackendKind::Webview
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct MockBackend {
        log: Arc<Mutex<Vec<&'static str>>>,
        visible: bool,
        alive: bool,
    }

    impl PopupBackend for MockBackend {
        fn kind(&self) -> PopupUiBackendKind {
            PopupUiBackendKind::Webview
        }
        fn ensure_created(&mut self) -> Result<(), String> {
            self.alive = true;
            self.log.lock().unwrap().push("ensure");
            Ok(())
        }
        fn show(&mut self, _mode: PopupPositionMode) -> Result<(), String> {
            self.visible = true;
            self.log.lock().unwrap().push("show");
            Ok(())
        }
        fn hide(&mut self) {
            self.visible = false;
            self.log.lock().unwrap().push("hide");
        }
        fn destroy(&mut self) {
            self.alive = false;
            self.visible = false;
            self.log.lock().unwrap().push("destroy");
        }
        fn is_visible(&self) -> bool {
            self.visible
        }
        fn is_alive(&self) -> bool {
            self.alive
        }
        fn publish(&mut self, _vm: &PopupViewModel) {
            self.log.lock().unwrap().push("publish");
        }
    }

    #[test]
    fn host_hide_is_idempotent() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut host = PopupHost::from_backend(Box::new(MockBackend {
            log: log.clone(),
            visible: false,
            alive: false,
        }));
        host.ensure_created().unwrap();
        host.show(PopupPositionMode::NearCursor).unwrap();
        host.hide();
        host.hide();
        assert!(!host.is_visible());
        let ops = log.lock().unwrap().clone();
        assert_eq!(ops.iter().filter(|x| **x == "hide").count(), 2);
    }

    #[test]
    fn degraded_flag_set_on_replace() {
        // 直接 replace_backend 后 is_degraded_from_winui == true，且旧 backend 被 destroy。
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut host = PopupHost::from_backend(Box::new(MockBackend {
            log: log.clone(),
            visible: false,
            alive: true,
        }));
        assert!(!host.is_degraded_from_winui());

        let log2 = Arc::new(Mutex::new(Vec::new()));
        host.replace_backend(Box::new(MockBackend {
            log: log2,
            visible: false,
            alive: false,
        }));

        assert!(host.is_degraded_from_winui());
        assert_eq!(host.kind(), PopupUiBackendKind::Webview);
        let ops = log.lock().unwrap().clone();
        assert!(
            ops.iter().any(|x| *x == "destroy"),
            "replace 须 destroy 旧 backend，ops={ops:?}"
        );
    }

    #[test]
    fn host_show_does_not_ensure_created() {
        // WebView 热路径禁止 Host 同步 ensure 建窗；预建用 ensure_created。
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut host = PopupHost::from_backend(Box::new(MockBackend {
            log: log.clone(),
            visible: false,
            alive: false,
        }));
        host.show(PopupPositionMode::NearCursor).unwrap();
        assert!(host.is_visible());
        // show 本身不 ensure，Mock 的 alive 保持 false
        assert!(!host.is_alive());
        let ops = log.lock().unwrap().clone();
        assert_eq!(ops, vec!["show"]);
    }

    #[test]
    fn resolve_kind_winui_without_feature_falls_back_webview() {
        // 非 windows 或无 feature 时回退 Webview
        assert_eq!(
            resolve_popup_backend_kind("winui", /* feature_enabled */ false, /* is_windows */ true),
            PopupUiBackendKind::Webview
        );
        assert_eq!(
            resolve_popup_backend_kind("winui", true, false),
            PopupUiBackendKind::Webview
        );
        assert_eq!(
            resolve_popup_backend_kind("winui", true, true),
            PopupUiBackendKind::Winui
        );
        assert_eq!(
            resolve_popup_backend_kind("webview", true, true),
            PopupUiBackendKind::Webview
        );
    }

    #[test]
    fn resolve_kind_winui_with_feature_is_winui() {
        // 启动真切换：配置 winui + feature + Windows → kind 必须为 Winui（非假 Webview）
        assert_eq!(
            resolve_popup_backend_kind("winui", true, true),
            PopupUiBackendKind::Winui
        );
        // 与编译期常量联动：Windows + default `popup-winui` 时可用 POPUP_WINUI_FEATURE 解析
        #[cfg(all(windows, feature = "popup-winui"))]
        {
            assert!(POPUP_WINUI_FEATURE);
            assert_eq!(
                resolve_popup_backend_kind("winui", POPUP_WINUI_FEATURE, true),
                PopupUiBackendKind::Winui
            );
        }
        #[cfg(not(all(windows, feature = "popup-winui")))]
        {
            assert!(!POPUP_WINUI_FEATURE);
            assert_eq!(
                resolve_popup_backend_kind("winui", POPUP_WINUI_FEATURE, true),
                PopupUiBackendKind::Webview
            );
        }
    }

    #[test]
    fn resolve_kind_normalizes_trim_and_case() {
        assert_eq!(
            resolve_popup_backend_kind("  WinUI  ", true, true),
            PopupUiBackendKind::Winui
        );
        assert_eq!(
            resolve_popup_backend_kind("WEBVIEW", true, true),
            PopupUiBackendKind::Webview
        );
    }
}
