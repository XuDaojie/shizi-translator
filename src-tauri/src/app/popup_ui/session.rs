//! 会话级弹窗 UI backend 选择：desired 与会话回退（不写回 config）。

use super::kind::PopupUiKind;

#[derive(Debug, Default)]
pub struct PopupUiSession {
    desired: PopupUiKind,
    session_force_webview: bool,
}

impl PopupUiSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_desired(&mut self, kind: PopupUiKind) {
        self.desired = kind;
        if kind == PopupUiKind::WinUi {
            self.session_force_webview = false;
        }
    }

    /// 从配置同步 desired，不清除会话回退标志（ensure/show 路径用）。
    pub fn sync_desired(&mut self, kind: PopupUiKind) {
        self.desired = kind;
    }

    pub fn desired(&self) -> PopupUiKind {
        self.desired
    }

    pub fn active(&self) -> PopupUiKind {
        if self.session_force_webview {
            PopupUiKind::Webview
        } else {
            self.desired
        }
    }

    pub fn fallback_to_webview_for_session(&mut self) {
        self.session_force_webview = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desired_follows_config_until_session_fallback() {
        let mut s = PopupUiSession::new();
        s.set_desired(PopupUiKind::WinUi);
        assert_eq!(s.active(), PopupUiKind::WinUi);
        s.fallback_to_webview_for_session();
        assert_eq!(s.active(), PopupUiKind::Webview);
        // 用户再次选择 winui（set_desired）允许重试
        s.set_desired(PopupUiKind::WinUi);
        assert_eq!(s.active(), PopupUiKind::WinUi);
    }

    #[test]
    fn fallback_does_not_change_desired_config_value() {
        let mut s = PopupUiSession::new();
        s.set_desired(PopupUiKind::WinUi);
        s.fallback_to_webview_for_session();
        assert_eq!(s.desired(), PopupUiKind::WinUi);
        assert_eq!(s.active(), PopupUiKind::Webview);
    }

    #[test]
    fn sync_desired_preserves_session_fallback() {
        let mut s = PopupUiSession::new();
        s.set_desired(PopupUiKind::WinUi);
        s.fallback_to_webview_for_session();
        assert_eq!(s.active(), PopupUiKind::Webview);
        // ensure/show 路径：仅同步 desired，不清除回退
        s.sync_desired(PopupUiKind::WinUi);
        assert_eq!(s.desired(), PopupUiKind::WinUi);
        assert_eq!(s.active(), PopupUiKind::Webview);
        // 用户再次选择 winui（set_desired）才允许重试
        s.set_desired(PopupUiKind::WinUi);
        assert_eq!(s.active(), PopupUiKind::WinUi);
    }
}
