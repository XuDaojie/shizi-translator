//! WinUI 弹窗后端（Windows + `popup-winui` feature）。
//!
//! 本模块为骨架：生命周期 API 占位，真实窗体在后续任务实现。

#![cfg(all(windows, feature = "popup-winui"))]

mod backend;
mod bootstrap;
mod ui;

pub use backend::WinuiPopupBackend;
