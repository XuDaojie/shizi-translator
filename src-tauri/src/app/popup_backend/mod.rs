//! 弹窗后端：ViewModel、PopupBackend trait 与 PopupHost 调度。
//! 不实现 WebviewPopupBackend 接入（后续任务）。

#![allow(dead_code)]

pub mod host;
pub mod trait_api;
pub mod types;
pub mod view_model;

#[allow(unused_imports)]
pub use host::{resolve_popup_backend_kind, PopupHost, POPUP_WINUI_FEATURE};
#[allow(unused_imports)]
pub use trait_api::PopupBackend;
#[allow(unused_imports)]
pub use types::*;
#[allow(unused_imports)]
pub use view_model::apply_translation_event;
