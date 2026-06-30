use std::{thread, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};

mod clipboard;
mod keyboard;

#[derive(Debug, thiserror::Error)]
pub enum SelectionError {
    #[error("无法访问剪贴板")]
    ClipboardUnavailable,
    #[error("无法模拟 Ctrl+C，请检查系统权限或手动复制后再翻译")]
    CopyShortcutFailed,
    #[error("未读取到选中文本，请先选中文本后再按 Alt+T")]
    EmptySelection,
}

pub fn copy_selected_text() -> Result<String, SelectionError> {
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

    clipboard::restore_text_snapshot(snapshot);
    selected_text.ok_or(SelectionError::EmptySelection)
}

fn selection_sentinel() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("__SHIZI_SELECTION_SENTINEL_{millis}__")
}
