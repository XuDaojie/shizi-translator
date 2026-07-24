//! WinUI 弹窗 WebView2 Bridge：协议信封、UI→Rust 请求分发、Rust→宿主推送。
//!
//! - [`protocol`]：JSON 信封与 payload 类型
//! - [`dispatch`]：`classify` / `handle_bridge_request` 接到现有业务入口
//! - [`push`]：`BridgePushHub` 双投翻译/配置等事件到宿主 sink

pub mod dispatch;
pub mod protocol;
pub mod push;

// 宿主任务接线时直接用 `crate::app::popup_bridge::{dispatch,push}::...` 或后续再 re-export。
