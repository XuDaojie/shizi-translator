//! 用户动作分发：view 子模块 → 静态 handler（由 actions 注册）。

#![cfg(all(windows, feature = "popup-winui"))]

use std::sync::Mutex;

use crate::app::popup_backend::types::PopupUserAction;

type UserActionHandler = fn(PopupUserAction);

static USER_ACTION_HANDLER: Mutex<Option<UserActionHandler>> = Mutex::new(None);

/// 由 `actions::install_action_handler` 注册（典型为 `handle_user_action`）。
pub fn set_user_action_handler(handler: UserActionHandler) {
    if let Ok(mut g) = USER_ACTION_HANDLER.lock() {
        *g = Some(handler);
    }
}

pub fn dispatch_user_action(action: PopupUserAction) {
    let handler = USER_ACTION_HANDLER.lock().ok().and_then(|g| *g);
    if let Some(h) = handler {
        h(action);
    } else {
        log::warn!("Reactor 弹窗未注册动作处理器，忽略: {action:?}");
    }
}
