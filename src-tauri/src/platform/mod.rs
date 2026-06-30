#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(not(target_os = "windows"))]
pub mod unsupported;

pub use crate::core::ocr_translation::OcrTranslationError;

#[cfg(target_os = "windows")]
pub use windows::capture_and_recognize;

#[cfg(not(target_os = "windows"))]
pub use unsupported::capture_and_recognize;
