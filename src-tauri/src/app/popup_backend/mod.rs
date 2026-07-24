//! 弹窗后端：ViewModel、PopupBackend trait、Webview / WinUI 实现与 PopupHost 调度。
//!
//! `popup-winui` feature + Windows 时 `Winui` kind 使用 [`winui::WinuiPopupBackend`]
//!（**路径 B：Win32 表面**，非 XAML）；否则回退 WebView。
//! WinUI `ensure_created` 失败时同进程降级为 WebView，并（仅 Windows）一次性提示 Runtime。

// 部分 API 供后续 WinUI / 设置页使用，当前主路径尚未全量消费。
#![allow(dead_code)]

pub mod host;
pub mod trait_api;
pub mod types;
pub mod view_model;
pub mod webview;

#[cfg(all(windows, feature = "popup-winui"))]
pub mod winui;

pub use host::{resolve_popup_backend_kind, PopupHost, POPUP_WINUI_FEATURE};
pub use trait_api::PopupBackend;
pub use types::*;
pub use webview::WebviewPopupBackend;

use std::sync::Mutex;
use tauri::{AppHandle, Manager};

/// Windows App SDK / Runtime 官方下载页（降级 dialog「打开下载页」）。
pub const WINUI_RUNTIME_DOWNLOAD_URL: &str =
    "https://learn.microsoft.com/windows/apps/windows-app-sdk/downloads";

/// 按解析后的 kind 创建具体 backend。
///
/// Windows + `popup-winui`：`Winui` → [`winui::WinuiPopupBackend`]；
/// 否则 `Winui` 回退 WebView 并 `log::warn`。
pub fn create_backend(app: &AppHandle, kind: PopupUiBackendKind) -> Box<dyn PopupBackend> {
    match kind {
        PopupUiBackendKind::Webview => Box::new(WebviewPopupBackend::new(app.clone())),
        #[cfg(all(windows, feature = "popup-winui"))]
        PopupUiBackendKind::Winui => Box::new(winui::WinuiPopupBackend::new(app.clone())),
        #[cfg(not(all(windows, feature = "popup-winui")))]
        PopupUiBackendKind::Winui => {
            log::warn!("popupUiBackend=winui 但 WinUI backend 不可用，使用 webview");
            Box::new(WebviewPopupBackend::new(app.clone()))
        }
    }
}

/// 创建 [`PopupHost`]：若 kind 为 WinUI 且 `ensure_created` 失败，则降级 WebView 并提示。
///
/// 路径 B 成功时不弹 dialog；非 Windows 不弹 dialog（`cfg`）。
pub fn create_host_with_winui_fallback(app: &AppHandle, kind: PopupUiBackendKind) -> PopupHost {
    let mut host = PopupHost::from_backend(create_backend(app, kind));
    if kind == PopupUiBackendKind::Winui {
        if let Err(err) = host.ensure_created() {
            log::error!("WinUI 弹窗初始化失败，降级 webview: {err}");
            host.replace_backend(Box::new(WebviewPopupBackend::new(app.clone())));
            #[cfg(windows)]
            spawn_winui_degrade_dialog(app.clone());
            // WebView 预建也可由后续 ensure_popup_window 按 config 走；此处 best-effort 一次。
            if let Err(e) = host.ensure_created() {
                log::warn!("降级 WebView 后 ensure_created 失败: {e}");
            }
        }
    }
    host
}

/// 一次性系统 dialog：说明已降级，可选打开 Runtime 下载页。
///
/// 仅 Windows 编译；`AtomicBool` 保证本进程只弹一次。
#[cfg(windows)]
fn spawn_winui_degrade_dialog(app: AppHandle) {
    use std::sync::atomic::{AtomicBool, Ordering};

    static SHOWN: AtomicBool = AtomicBool::new(false);
    if SHOWN.swap(true, Ordering::SeqCst) {
        return;
    }

    tauri::async_runtime::spawn(async move {
        let app_for_dialog = app.clone();
        let go = tauri::async_runtime::spawn_blocking(move || {
            use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
            app_for_dialog
                .dialog()
                .message(
                    "原生弹窗初始化失败，已自动切换为 WebView 弹窗。\n\
                     若需使用原生弹窗，请安装 Windows App Runtime 后重启应用。",
                )
                .title("弹窗后端已降级")
                .kind(MessageDialogKind::Info)
                .buttons(MessageDialogButtons::OkCancelCustom(
                    "打开下载页".to_string(),
                    "稍后".to_string(),
                ))
                .blocking_show()
        })
        .await
        .unwrap_or(false);

        if go {
            if let Err(e) = crate::ui::config::open_url(WINUI_RUNTIME_DOWNLOAD_URL.to_string()) {
                log::warn!("打开 Windows App Runtime 下载页失败: {e}");
            }
        }
    });
}

/// 在已 manage 的 [`PopupHost`] 上执行闭包（统一加锁入口，避免业务层直接拿锁）。
pub fn with_host<R>(app: &AppHandle, f: impl FnOnce(&mut PopupHost) -> R) -> Result<R, String> {
    let state = app.state::<Mutex<PopupHost>>();
    let mut guard = state
        .lock()
        .map_err(|_| "PopupHost lock poisoned".to_string())?;
    Ok(f(&mut guard))
}
