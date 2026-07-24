fn main() {
    tauri_build::build();
    #[cfg(all(windows, feature = "popup-winui"))]
    {
        // framework-dependent：与 v1 发布模型一致
        windows_reactor_setup::as_framework_dependent();
    }
}
