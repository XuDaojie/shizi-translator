//! 路径 R：windows-reactor 宿主（M0+）

#![cfg(all(windows, feature = "popup-winui"))]

mod host;

// M0 对外 API：bootstrap 探测 + STA host（后续任务 3+ 使用 handle / 命令）
#[allow(unused_imports)]
pub use host::{
    ensure_process_bootstrap, is_popup_window_visible, is_process_bootstrapped, HostCmd,
    ReactorHostHandle, POPUP_TITLE, SENTINEL_TITLE,
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
