//! 路径 R：windows-reactor 宿主（M0+）

#![cfg(all(windows, feature = "popup-winui"))]

mod host;
pub mod langs;
pub mod meta;
pub mod state;

// 路径 R 对外 API：bootstrap 探测 + STA host（任务 6 backend 接线）
#[allow(unused_imports)]
pub use host::{
    ensure_process_bootstrap, is_host_started, is_popup_window_visible, is_process_bootstrapped,
    HostCmd, ReactorHostHandle, POPUP_TITLE, SENTINEL_TITLE,
};

#[allow(unused_imports)]
pub use langs::{lang_codes_for_side, lang_display_name, swap_session_langs, LANG_TABLE};
#[allow(unused_imports)]
pub use meta::{display_model_name, is_machine_translate_protocol, should_show_tokens};
#[allow(unused_imports)]
pub use state::{
    first_copyable_service_id, global_snapshot, global_state, resolve_copy_fields,
    resolve_copy_text, store_global, SharedPopupState,
};

/// M0：是否已链接 windows-reactor（编译期存在性）。
#[cfg(test)]
mod tests {
    #[test]
    fn reactor_crate_is_linked() {
        // 使用任意稳定 re-export；若 API 更名，M0 按编译器错误改这一行即可
        let _ = std::any::type_name::<windows_reactor::Element>();
        assert!(!std::any::type_name::<windows_reactor::Element>().is_empty());
    }
}
