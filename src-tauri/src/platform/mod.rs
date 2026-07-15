#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(not(target_os = "windows"))]
pub mod unsupported;

#[cfg(target_os = "windows")]
pub use windows::{capture_screen, cursor_logical_context, recognize_region};

// 纯识别 API 由后续 OCR 弹窗 command 消费
#[cfg(target_os = "windows")]
#[allow(unused_imports)]
pub use windows::{recognize_cropped_full, recognize_image_full};

#[cfg(not(target_os = "windows"))]
pub use unsupported::{capture_screen, cursor_logical_context, recognize_region};

#[cfg(not(target_os = "windows"))]
#[allow(unused_imports)]
pub use unsupported::{recognize_cropped_full, recognize_image_full};
