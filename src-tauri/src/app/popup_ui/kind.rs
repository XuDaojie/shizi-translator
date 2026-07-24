//! 弹窗 UI 后端类型（webview / winui）解析与平台解析。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupUiKind {
    Webview,
    WinUi,
}

impl Default for PopupUiKind {
    fn default() -> Self {
        Self::Webview
    }
}

impl PopupUiKind {
    pub fn parse(raw: &str) -> Self {
        match raw {
            "winui" => Self::WinUi,
            _ => Self::Webview,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Webview => "webview",
            Self::WinUi => "winui",
        }
    }

    pub fn resolve_for_platform(raw: &str, is_windows: bool) -> Self {
        let parsed = Self::parse(raw);
        if !is_windows && parsed == Self::WinUi {
            return Self::Webview;
        }
        parsed
    }

    pub fn resolve_from_config(raw: &str) -> Self {
        Self::resolve_for_platform(raw, cfg!(windows))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_webview_and_winui() {
        assert_eq!(PopupUiKind::parse("webview"), PopupUiKind::Webview);
        assert_eq!(PopupUiKind::parse("winui"), PopupUiKind::WinUi);
        assert_eq!(PopupUiKind::parse("nope"), PopupUiKind::Webview);
    }

    #[test]
    fn resolve_forces_webview_on_non_windows() {
        assert_eq!(
            PopupUiKind::resolve_for_platform("winui", /* is_windows */ false),
            PopupUiKind::Webview
        );
        assert_eq!(
            PopupUiKind::resolve_for_platform("winui", true),
            PopupUiKind::WinUi
        );
    }
}
