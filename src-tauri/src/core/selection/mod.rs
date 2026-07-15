use std::{
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

mod clipboard;
mod clipboard_image;
mod keyboard;

pub use clipboard_image::read_clipboard_image;

#[derive(Debug, thiserror::Error)]
pub enum SelectionError {
    #[error("无法访问剪贴板")]
    ClipboardUnavailable,
    #[error("无法模拟 Ctrl+C，请检查系统权限或手动复制后再翻译")]
    CopyShortcutFailed,
    #[error("未读取到选中文本，请先选中文本后再按 Alt+T")]
    EmptySelection,
    #[error("剪贴板中没有可翻译的文本")]
    EmptyClipboard,
}

pub fn read_clipboard_text() -> Result<String, SelectionError> {
    normalize_clipboard_text(clipboard::read_text()?)
}

fn normalize_clipboard_text(text: Option<String>) -> Result<String, SelectionError> {
    let text = text.unwrap_or_default().trim().to_string();
    if text.is_empty() {
        Err(SelectionError::EmptyClipboard)
    } else {
        Ok(text)
    }
}

pub fn copy_selected_text(restore_clipboard: bool) -> Result<String, SelectionError> {
    let snapshot = clipboard::capture_text_snapshot();
    let sentinel = selection_sentinel();
    clipboard::write_text(&sentinel)?;
    keyboard::send_copy_shortcut()?;

    let deadline = Instant::now() + Duration::from_millis(600);
    let mut selected_text = None;

    while Instant::now() < deadline {
        if let Some(text) = clipboard::read_text()? {
            let text = text.trim().to_string();
            if !text.is_empty() && text != sentinel {
                selected_text = Some(text);
                break;
            }
        }
        thread::sleep(Duration::from_millis(40));
    }

    if should_restore_clipboard(restore_clipboard, &snapshot) {
        clipboard::restore_text_snapshot(snapshot);
    }
    selected_text.ok_or(SelectionError::EmptySelection)
}

fn should_restore_clipboard(restore_clipboard: bool, snapshot: &Option<String>) -> bool {
    restore_clipboard && snapshot.is_some()
}

fn selection_sentinel() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("__SHIZI_SELECTION_SENTINEL_{millis}__")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_text_trims_non_empty_value() {
        assert_eq!(
            normalize_clipboard_text(Some("  hello  ".to_string())).expect("应读取到文本"),
            "hello"
        );
    }

    #[test]
    fn clipboard_text_rejects_empty_value() {
        let error = normalize_clipboard_text(Some("   ".to_string()))
            .expect_err("空文本应失败");

        assert!(matches!(error, SelectionError::EmptyClipboard));
    }

    #[test]
    fn clipboard_text_rejects_missing_value() {
        let error = normalize_clipboard_text(None).expect_err("无文本应失败");

        assert!(matches!(error, SelectionError::EmptyClipboard));
    }

    #[test]
    fn restore_clipboard_flag_controls_restore() {
        assert!(should_restore_clipboard(true, &Some("old".to_string())));
        assert!(!should_restore_clipboard(false, &Some("old".to_string())));
        assert!(!should_restore_clipboard(true, &None));
    }
}
