//! 路径 R：windows-reactor 宿主（M0+）

#![cfg(all(windows, feature = "popup-winui"))]

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
