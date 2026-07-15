use arboard::Clipboard;

use crate::core::capture::{CapturedImage, CapturedImageFormat};

/// 从系统剪贴板读取位图。无图时返回 `Ok(None)`；访问失败返回 Err。
pub fn read_clipboard_image() -> Result<Option<CapturedImage>, String> {
    let mut cb = Clipboard::new().map_err(|e| e.to_string())?;
    match cb.get_image() {
        Ok(img) => Ok(Some(CapturedImage {
            bytes: img.bytes.into_owned(),
            width: img.width as u32,
            height: img.height as u32,
            format: CapturedImageFormat::Rgba8,
        })),
        Err(arboard::Error::ContentNotAvailable) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
