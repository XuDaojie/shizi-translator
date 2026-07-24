//! WinUI 窗口与控件（骨架）。
//!
//! 后续任务实现真实 HWND / XAML 表面；当前仅 stub。

/// 显示弹窗占位。
pub fn show_stub() -> Result<(), String> {
    Err("winui::ui::show_stub not implemented".to_string())
}

/// 隐藏弹窗占位（幂等 no-op）。
pub fn hide_stub() -> Result<(), String> {
    Ok(())
}

/// 销毁弹窗占位（幂等 no-op）。
pub fn destroy_stub() -> Result<(), String> {
    Ok(())
}
