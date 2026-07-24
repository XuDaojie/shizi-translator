//! WinUI 弹窗 WebView2 Bridge：协议信封与 UI→Rust 请求分发。
//!
//! - [`protocol`]：JSON 信封与 payload 类型
//! - [`dispatch`]：`classify` / `handle_bridge_request` 接到现有业务入口

pub mod dispatch;
pub mod protocol;

// 宿主任务接线时直接用 `crate::app::popup_bridge::dispatch::...` 或后续再 re-export。
