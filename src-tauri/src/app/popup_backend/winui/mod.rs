//! 原生弹窗后端（Windows + `popup-winui` feature）。
//!
//! **采用路径 B：Win32 表面**（`WS_POPUP` + `WS_EX_TOOLWINDOW` + DWM 圆角），
//! 不依赖 Windows App SDK / Microsoft.UI.Xaml。配置枚举值仍为 `winui`。

#![cfg(all(windows, feature = "popup-winui"))]

mod actions;
mod backend;
mod bootstrap;
mod ui;

#[allow(unused_imports)]
pub use actions::handle_user_action;
pub use backend::WinuiPopupBackend;
#[allow(unused_imports)]
pub use bootstrap::{try_bootstrap, BootstrapStatus};
