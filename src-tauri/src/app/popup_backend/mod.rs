//! 弹窗后端：ViewModel、PopupBackend trait、Webview 实现与 PopupHost 调度。
//! WinUI 后端与主路径接入见后续任务。

#![allow(dead_code)]

pub mod host;
pub mod trait_api;
pub mod types;
pub mod view_model;
pub mod webview;

#[allow(unused_imports)]
pub use host::{resolve_popup_backend_kind, PopupHost, POPUP_WINUI_FEATURE};
#[allow(unused_imports)]
pub use trait_api::PopupBackend;
#[allow(unused_imports)]
pub use types::*;
#[allow(unused_imports)]
pub use view_model::apply_translation_event;
#[allow(unused_imports)]
pub use webview::WebviewPopupBackend;
