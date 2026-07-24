//! 原生弹窗后端（Windows + `popup-winui` feature）。
//!
//! **路径 R：windows-reactor 真 WinUI 3**（专用 STA + 哨兵；`WinuiPopupBackend` →
//! `ReactorHostHandle`）。配置枚举值仍为 `winui`。GDI `ui.rs` 仍可编译（遗留，
//! 任务 11 删除），backend 不再引用。

#![cfg(all(windows, feature = "popup-winui"))]

mod actions;
mod backend;
mod bootstrap;
mod reactor;
mod ui;

#[allow(unused_imports)]
pub use actions::handle_user_action;
pub use backend::WinuiPopupBackend;
#[allow(unused_imports)]
pub use bootstrap::{try_bootstrap, BootstrapStatus};
