pub const POPUP_LABEL: &str = "main";
pub const POPUP_URL: &str = "translate.html";

use tauri::{LogicalPosition, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::app::icon::app_icon_image;
use crate::app::shortcuts::attach_app_shortcut_focus_listener;
use crate::app::window::attach_close_to_hide;
use crate::core::config::AppConfig;
use crate::platform::cursor_logical_context;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogicalPos {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogicalSize {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogicalRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// 计算弹窗左上角逻辑坐标：默认放在光标处，若弹窗右/下溢出工作区则左/上移，
/// 最后钳制不低于工作区左上角。纯函数，便于单测。
pub fn compute_popup_position(
    cursor: LogicalPos,
    popup_size: LogicalSize,
    work_area: LogicalRect,
) -> LogicalPos {
    let mut x = cursor.x;
    let mut y = cursor.y;

    if x + popup_size.width > work_area.x + work_area.width {
        x = work_area.x + work_area.width - popup_size.width;
    }
    if y + popup_size.height > work_area.y + work_area.height {
        y = work_area.y + work_area.height - popup_size.height;
    }
    if x < work_area.x {
        x = work_area.x;
    }
    if y < work_area.y {
        y = work_area.y;
    }

    LogicalPos { x, y }
}

fn build_popup(app: &tauri::AppHandle) -> Result<WebviewWindow, String> {
    let mut builder = WebviewWindowBuilder::new(app, POPUP_LABEL, WebviewUrl::App(POPUP_URL.into()))
        .title("Shizi 翻译")
        .inner_size(420.0, 360.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .skip_taskbar(true)
        .center()
        .visible(false);
    // 无系统标题栏时仍写入窗口图标，避免个别场景回退到模糊默认图。
    if let Ok(icon) = app_icon_image(app) {
        builder = builder.icon(icon).map_err(|error| format!("设置窗口图标失败: {error}"))?;
    }
    let window = builder
        .build()
        .map_err(|error| format!("创建翻译弹窗失败: {error}"))?;
    attach_close_to_hide(&window);
    attach_app_shortcut_focus_listener(&window, app);
    Ok(window)
}

/// 确保翻译弹窗存在；不存在则创建（隐藏）。
///
/// **Windows 注意**：勿在同步 tray/快捷键回调栈内首次 build（WebView2 死锁）；
/// 调用方须在 async / 独立线程路径上首次创建。
pub fn ensure_popup_exists(app: &tauri::AppHandle) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(POPUP_LABEL) {
        return Ok(window);
    }
    build_popup(app)
}

/// 启动时按当前启动路径的 `windowPrecreate.*.popup` 决定是否预建。
/// 经 [`crate::app::popup_backend::PopupHost`] 调度（WebView 实现内仍 `ensure_popup_exists`）。
pub fn ensure_popup_window(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    let pair = config
        .window_precreate
        .for_launch(crate::app::autostart::is_autostart_process());
    if !pair.popup {
        return Ok(());
    }
    crate::app::popup_backend::with_host(app, |host| host.ensure_created())
        .and_then(std::convert::identity)
}

/// 隐藏翻译弹窗。截图前调用，避免把弹窗打进 DXGI 帧；幂等。
/// WebView 实现内部使用；业务路径应经 PopupHost / `with_host`。
pub fn hide_popup(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(POPUP_LABEL) {
        let _ = window.hide();
    }
}

/// 翻译弹窗唤起时的定位策略（定义在 `popup_backend::types`，此处 re-export 保持兼容）。
pub use crate::app::popup_backend::types::PopupPositionMode;

fn present_popup(window: &WebviewWindow, mode: PopupPositionMode) {
    if mode == PopupPositionMode::NearCursor {
        let scale = window.scale_factor().unwrap_or(1.0);
        if let Some((cx, cy, wx, wy, ww, wh)) = cursor_logical_context(scale) {
            const POPUP_W: f64 = 420.0;
            const POPUP_H: f64 = 480.0;
            let pos = compute_popup_position(
                LogicalPos { x: cx, y: cy },
                LogicalSize {
                    width: POPUP_W,
                    height: POPUP_H,
                },
                LogicalRect {
                    x: wx,
                    y: wy,
                    width: ww,
                    height: wh,
                },
            );
            let _ = window.set_position(LogicalPosition::new(pos.x, pos.y));
        }
    }
    let _ = window.show();
    let _ = window.set_focus();
}

/// 当前线程 ensure + 定位 show。Windows 上勿在 tray/快捷键同步回调栈内首次调用。
pub fn show_popup_blocking(
    app: &tauri::AppHandle,
    _config: &AppConfig,
    mode: PopupPositionMode,
) -> Result<(), String> {
    let window = ensure_popup_exists(app)?;
    present_popup(&window, mode);
    Ok(())
}

/// 唤起弹窗：已存在则定位 show；不存在则**独立线程**创建后 show（避 Windows 回调栈建窗死锁）。
pub fn show_popup(
    app: &tauri::AppHandle,
    config: &AppConfig,
    mode: PopupPositionMode,
) -> Result<(), String> {
    if app.get_webview_window(POPUP_LABEL).is_some() {
        return show_popup_blocking(app, config, mode);
    }

    let app = app.clone();
    let config = config.clone();
    std::thread::spawn(move || {
        if let Err(error) = show_popup_blocking(&app, &config, mode) {
            log::warn!("创建翻译弹窗失败: {error}");
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn work_area_1920x1080() -> LogicalRect {
        LogicalRect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        }
    }

    fn popup_400x300() -> LogicalSize {
        LogicalSize {
            width: 400.0,
            height: 300.0,
        }
    }

    #[test]
    fn cursor_in_middle_keeps_position() {
        let pos = compute_popup_position(
            LogicalPos { x: 800.0, y: 500.0 },
            popup_400x300(),
            work_area_1920x1080(),
        );
        assert_eq!(pos, LogicalPos { x: 800.0, y: 500.0 });
    }

    #[test]
    fn cursor_near_right_shifts_left() {
        let pos = compute_popup_position(
            LogicalPos { x: 1800.0, y: 500.0 },
            popup_400x300(),
            work_area_1920x1080(),
        );
        assert_eq!(pos, LogicalPos { x: 1520.0, y: 500.0 });
    }

    #[test]
    fn cursor_near_bottom_shifts_up() {
        let pos = compute_popup_position(
            LogicalPos { x: 800.0, y: 950.0 },
            popup_400x300(),
            work_area_1920x1080(),
        );
        assert_eq!(pos, LogicalPos { x: 800.0, y: 780.0 });
    }

    #[test]
    fn cursor_at_corner_clamps_to_work_area_origin() {
        let pos = compute_popup_position(
            LogicalPos { x: -100.0, y: -100.0 },
            popup_400x300(),
            work_area_1920x1080(),
        );
        assert_eq!(pos, LogicalPos { x: 0.0, y: 0.0 });
    }
}
