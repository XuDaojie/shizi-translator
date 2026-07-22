use tauri::{Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::app::icon::app_icon_image;
use crate::app::shortcuts::attach_app_shortcut_focus_listener;
use crate::app::tray::TrayI18nHandles;

pub const SETTINGS_LABEL: &str = "settings";
pub const SETTINGS_URL: &str = "settings.html";
pub const SETTINGS_INITIAL_VISIBLE: bool = false;

/// 仅用于高频/托盘驻留窗（`main`）：关窗改为 hide，不销毁 WebView。
/// 设置页与文字识别等低频窗不要挂此钩子，关闭即销毁。
pub(crate) fn attach_close_to_hide(window: &WebviewWindow) {
    let window_to_hide = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = window_to_hide.hide();
        }
    });
}

fn present_window(window: &WebviewWindow) -> Result<(), String> {
    window.show().map_err(|error| error.to_string())?;
    window.unminimize().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;
    Ok(())
}

/// 托盘双击等：已有 main 则 show；否则走 `show_popup`（独立线程建窗）。
pub fn show_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = present_window(&window);
        return;
    }
    let config = app
        .try_state::<crate::app::state::AppState>()
        .and_then(|s| s.config_store.get().ok())
        .unwrap_or_else(crate::core::config::AppConfig::default);
    if let Err(error) =
        crate::app::popup_window::show_popup(app, &config, crate::app::popup_window::PopupPositionMode::Restore)
    {
        log::warn!("打开翻译弹窗失败: {error}");
    }
}

/// 创建设置窗口（若尚不存在）。
///
/// 低频窗：用户关闭即销毁（不挂 `close_to_hide`）；下次打开再重建。
///
/// **Windows 注意**：`WebviewWindowBuilder::build` 在同步 command / 托盘与菜单事件回调里
/// 调用会死锁（WebView2 / wry#583）。首次创建须在 `async` command 或独立线程中执行；
/// 见 [`request_show_settings_window`]。
pub fn ensure_settings_window(app: &tauri::AppHandle) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(SETTINGS_LABEL) {
        return Ok(window);
    }

    // Windows/WebView2：Tauri 默认启用原生文件拖放处理器，会劫持 DOM 的
    // HTML5 drag&drop，导致设置页服务列表重排无效。关闭后前端 draggable 才可用。
    // 见 WebviewWindowBuilder::disable_drag_drop_handler 文档。
    let mut builder =
        WebviewWindowBuilder::new(app, SETTINGS_LABEL, WebviewUrl::App(SETTINGS_URL.into()))
            .title(
                app.state::<TrayI18nHandles>()
                    .settings_title
                    .read()
                    .map(|title| title.clone())
                    .unwrap_or_else(|_| "Shizi 设置".into()),
            )
            .inner_size(820.0, 600.0)
            .resizable(false)
            .minimizable(false)
            .maximizable(false)
            .center()
            .visible(SETTINGS_INITIAL_VISIBLE)
            .disable_drag_drop_handler();
    // 按主屏 DPI 设标题栏小图标，避免系统用大图缩小发糊（与托盘同源位图）。
    if let Ok(icon) = app_icon_image(app) {
        builder = builder.icon(icon).map_err(|error| format!("设置窗口图标失败: {error}"))?;
    }
    let window = builder
        .build()
        .map_err(|error| format!("创建设置窗口失败: {error}"))?;
    attach_app_shortcut_focus_listener(&window, app);
    Ok(window)
}

/// 打开设置窗。已存在则仅 show；已销毁或不存在则 `ensure` 重建。
/// 供 async command / 已脱离主事件回调栈的上下文使用。
pub fn show_settings_window(app: &tauri::AppHandle) -> Result<(), String> {
    let window = ensure_settings_window(app)?;
    present_window(&window)
}

/// 托盘 / 同步快捷键等事件回调用：在独立线程打开，避免 Windows 上首次建窗死锁。
/// 不阻塞调用方，也不在主线程上 `recv` 等待创建完成。
pub fn request_show_settings_window(app: &tauri::AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        if let Err(error) = show_settings_window(&app) {
            log::warn!("打开设置失败: {error}");
        }
    });
}

pub const OCR_LABEL: &str = "ocr";
pub const OCR_URL: &str = "ocr.html";

/// 创建文字识别窗口（若尚不存在）。
///
/// 低频窗：用户关闭即销毁（不挂 `close_to_hide`）；下次打开再重建。
/// 截图前临时 [`hide_ocr_window`] 仍只 hide，避免抓到本窗内容。
/// Windows 首次创建约束同 [`ensure_settings_window`]。
pub fn ensure_ocr_window(app: &tauri::AppHandle) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(OCR_LABEL) {
        return Ok(window);
    }

    let mut builder = WebviewWindowBuilder::new(app, OCR_LABEL, WebviewUrl::App(OCR_URL.into()))
        .title("Shizi 文字识别")
        .inner_size(960.0, 640.0)
        .min_inner_size(720.0, 480.0)
        .resizable(true)
        .center()
        .visible(false);
    if let Ok(icon) = app_icon_image(app) {
        builder = builder.icon(icon).map_err(|error| format!("设置窗口图标失败: {error}"))?;
    }
    let window = builder
        .build()
        .map_err(|error| format!("创建文字识别窗口失败: {error}"))?;
    attach_app_shortcut_focus_listener(&window, app);
    Ok(window)
}

/// 打开文字识别窗。已存在则仅 show；已销毁或不存在则 `ensure` 重建。
pub fn show_ocr_window(app: &tauri::AppHandle) -> Result<(), String> {
    let window = ensure_ocr_window(app)?;
    present_window(&window)
}

/// 托盘 / 同步事件回调用：独立线程打开，避免 Windows 首次建窗死锁。
pub fn request_show_ocr_window(app: &tauri::AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        if let Err(error) = show_ocr_window(&app) {
            log::warn!("打开文字识别窗口失败: {error}");
        }
    });
}

/// 隐藏文字识别窗口。纯识别截图前调用，避免窗口内容进帧；幂等。
pub fn hide_ocr_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(OCR_LABEL) {
        let _ = window.hide();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_settings_window_starts_hidden() {
        assert!(!SETTINGS_INITIAL_VISIBLE);
    }

    #[test]
    fn ocr_window_label_is_ocr() {
        assert_eq!(OCR_LABEL, "ocr");
    }
}
