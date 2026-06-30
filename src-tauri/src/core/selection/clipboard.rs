use arboard::Clipboard;

use super::SelectionError;

pub fn read_text() -> Result<Option<String>, SelectionError> {
    let mut clipboard = Clipboard::new().map_err(|_| SelectionError::ClipboardUnavailable)?;
    match clipboard.get_text() {
        Ok(text) => Ok(Some(text)),
        Err(arboard::Error::ContentNotAvailable) => Ok(None),
        Err(_) => Err(SelectionError::ClipboardUnavailable),
    }
}

pub fn write_text(text: &str) -> Result<(), SelectionError> {
    let mut clipboard = Clipboard::new().map_err(|_| SelectionError::ClipboardUnavailable)?;
    clipboard
        .set_text(text.to_string())
        .map_err(|_| SelectionError::ClipboardUnavailable)
}

pub fn capture_text_snapshot() -> Option<String> {
    read_text().ok().flatten()
}

pub fn restore_text_snapshot(snapshot: Option<String>) {
    if let Some(text) = snapshot {
        let _ = write_text(&text);
    }
}
