#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(not(target_os = "windows"))]
pub mod unsupported;

#[cfg(target_os = "windows")]
pub use windows::{capture_screen, recognize_region};

#[cfg(not(target_os = "windows"))]
pub use unsupported::{capture_screen, recognize_region};
