//! 原生弹窗后端（Windows + `popup-winui` feature）。
//!
//! **仅路径 R：windows-reactor 真 WinUI 3**（专用 STA + 哨兵；`WinuiPopupBackend` →
//! `ReactorHostHandle`）。配置枚举值仍为 `winui`。GDI 路径 B（原 `ui.rs`）已移除。

#![cfg(all(windows, feature = "popup-winui"))]

mod actions;
mod backend;
mod bootstrap;
mod reactor;

#[allow(unused_imports)]
pub use actions::handle_user_action;
pub use backend::WinuiPopupBackend;
#[allow(unused_imports)]
pub use bootstrap::{try_bootstrap, BootstrapStatus};
