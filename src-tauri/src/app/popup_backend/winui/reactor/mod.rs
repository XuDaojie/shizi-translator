//! 路径 R：windows-reactor 宿主 + 分区 UI（打磨后五区组件）。

#![cfg(all(windows, feature = "popup-winui"))]

mod dispatch;
mod host;
pub mod langs;
mod language_bar;
pub mod meta;
mod result_cards;
mod source_card;
pub mod state;
mod status_bar;
mod title_bar;
pub mod tokens;
pub mod view;

// 路径 R 对外 API：bootstrap 探测 + STA host + UI
#[allow(unused_imports)]
pub use host::{
    apply_popup_topmost, ensure_process_bootstrap, is_host_started, is_popup_window_visible,
    is_process_bootstrapped, HostCmd, ReactorHostHandle, POPUP_TITLE, SENTINEL_TITLE,
};

#[allow(unused_imports)]
pub use langs::{lang_codes_for_side, lang_display_name, swap_session_langs, LANG_TABLE};
#[allow(unused_imports)]
pub use meta::{display_model_name, is_machine_translate_protocol, should_show_tokens};
#[allow(unused_imports)]
pub use state::{
    first_copyable_service_id, global_snapshot, global_state, is_pinned, resolve_copy_fields,
    resolve_copy_text, set_pinned, store_global, toggle_pinned, SharedPopupState,
};
#[allow(unused_imports)]
pub use tokens::POPUP_VIEW_WIDTH;
#[allow(unused_imports)]
pub use view::{render_popup, set_user_action_handler};

/// M0：是否已链接 windows-reactor（编译期存在性）。
#[cfg(test)]
mod tests {
    #[test]
    fn reactor_crate_is_linked() {
        let _ = std::any::type_name::<windows_reactor::Element>();
        assert!(!std::any::type_name::<windows_reactor::Element>().is_empty());
    }
}
