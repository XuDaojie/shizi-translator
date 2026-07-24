//! 原生弹窗 Runtime 探测 / Bootstrap。
//!
//! **采用路径 B：Win32 表面**——不调用 Windows App SDK Bootstrap，
//! 也不依赖 Microsoft.UI.Xaml / XAML Runtime。配置枚举值仍为 `winui`。

/// Bootstrap / 运行时探测结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapStatus {
    pub ok: bool,
    pub message: String,
}

/// 尝试初始化原生弹窗运行时。
///
/// 路径 B：直接返回 `ok: true`（**未依赖 XAML Runtime** / WinAppSDK Bootstrap）。
/// 后续若迁路径 A，可在此调用 Windows App SDK Bootstrap API。
pub fn try_bootstrap() -> BootstrapStatus {
    BootstrapStatus {
        ok: true,
        message: "路径 B：Win32 表面（未依赖 XAML Runtime）".to_string(),
    }
}

/// 兼容旧调用：将 [`try_bootstrap`] 映射为 `Result`。
pub fn ensure_winui_runtime() -> Result<(), String> {
    let status = try_bootstrap();
    if status.ok {
        Ok(())
    } else {
        Err(status.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_bootstrap_ok_without_xaml_runtime() {
        let status = try_bootstrap();
        assert!(status.ok);
        assert!(status.message.contains("路径 B"));
        assert!(status.message.contains("未依赖 XAML Runtime"));
    }
}
