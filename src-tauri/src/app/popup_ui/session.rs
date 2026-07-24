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
}
