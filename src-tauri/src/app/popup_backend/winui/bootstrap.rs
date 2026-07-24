//! 原生弹窗 Runtime 探测 / Bootstrap。
//!
//! **路径 R 探测（M0）**：调用 `windows_reactor::bootstrap()`（进程级一次）。
//! 成功表示 Windows App Runtime 可用；失败时 `ok: false`，上层可降级 WebView。
//!
//! 注意：当前 `WinuiPopupBackend` 仍为路径 B（GDI）；本探测为路径 R 否决门与后续切换预留。
//! GDI `ensure_created` 仍调用本函数——本机有 Runtime 时 GDI 继续可用；无 Runtime 时
//! 会走 `create_host_with_winui_fallback` 降级（与路径 R 最终行为一致）。

/// Bootstrap / 运行时探测结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapStatus {
    pub ok: bool,
    pub message: String,
}

/// 尝试初始化原生弹窗运行时（路径 R：WinAppSDK bootstrap）。
///
/// - 成功：`ok: true`，message 含路径 R / framework-dependent 说明
/// - 失败：`ok: false`，message 含错误详情（Runtime 缺失等）
pub fn try_bootstrap() -> BootstrapStatus {
    #[cfg(all(windows, feature = "popup-winui"))]
    {
        match super::reactor::ensure_process_bootstrap() {
            Ok(()) => BootstrapStatus {
                ok: true,
                message: "路径 R：windows_reactor::bootstrap 成功（framework-dependent / S1 STA）"
                    .to_string(),
            },
            Err(e) => BootstrapStatus {
                ok: false,
                message: format!("路径 R：bootstrap 失败（可降级 WebView）: {e}"),
            },
        }
    }
    #[cfg(not(all(windows, feature = "popup-winui")))]
    {
        BootstrapStatus {
            ok: false,
            message: "非 Windows 或未启用 popup-winui".to_string(),
        }
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
    fn try_bootstrap_reports_path_r_not_path_b() {
        let status = try_bootstrap();
        // 不再断言「路径 B」恒真；消息应标明路径 R
        assert!(
            status.message.contains("路径 R"),
            "期望路径 R 探测文案，实际: {}",
            status.message
        );
        assert!(!status.message.contains("路径 B"));
        // 本机有 Runtime 时期望 ok；无 Runtime 时 ok=false 也合法（否决门/降级路径）
        if status.ok {
            assert!(status.message.contains("bootstrap 成功"));
        } else {
            assert!(status.message.contains("bootstrap 失败") || status.message.contains("非 Windows"));
        }
    }
}
