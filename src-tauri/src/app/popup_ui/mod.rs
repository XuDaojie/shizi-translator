//! 弹窗 UI backend 类型与会话级选择（纯 Rust，无 Tauri 宿主依赖）。

pub mod kind;
pub mod session;

pub use kind::PopupUiKind;
pub use session::PopupUiSession;
