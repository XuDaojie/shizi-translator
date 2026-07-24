//! WinUI 弹窗 Bridge：协议信封、UI→Rust 请求分发、Rust→宿主推送、子进程宿主。
//!
//! - [`protocol`]：JSON 信封与 payload 类型
//! - [`dispatch`]：`classify` / `handle_bridge_request` 接到现有业务入口
//! - [`push`]：`BridgePushHub` 双投翻译/配置等事件到宿主 sink
//! - [`host`]（Windows）：subprocess TCP 宿主（ensure/show/hide/push）

pub mod dispatch;
pub mod protocol;
pub mod push;

#[cfg(windows)]
pub mod host;
