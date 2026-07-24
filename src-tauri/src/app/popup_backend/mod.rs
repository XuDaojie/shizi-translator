//! 弹窗后端：ViewModel 类型与 translation 事件归并（纯函数）。
//! 本模块不实现 trait/host/webview 接入。
//!
//! 后续任务会由 host/WinUI 引用这些类型与函数；在此之前允许 dead_code。

#![allow(dead_code)]

pub mod types;
pub mod view_model;

#[allow(unused_imports)]
pub use types::*;
#[allow(unused_imports)]
pub use view_model::apply_translation_event;
