// 类型与纯函数在任务 4 的窗口管理函数中使用前暂时允许 dead_code。
#![allow(dead_code)]

pub const POPUP_LABEL: &str = "main";

use tauri::{LogicalPosition, Manager};

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

    // 右溢出 → 左移，使右边界贴工作区右边。
    if x + popup_size.width > work_area.x + work_area.width {
        x = work_area.x + work_area.width - popup_size.width;
    }
    // 下溢出 → 上移，使底边贴工作区底边。
    if y + popup_size.height > work_area.y + work_area.height {
        y = work_area.y + work_area.height - popup_size.height;
    }
    // 不低于工作区左上。
    if x < work_area.x {
        x = work_area.x;
    }
    if y < work_area.y {
        y = work_area.y;
    }

    LogicalPos { x, y }
}

/// main 窗口由 Tauri 配置创建，这里保留幂等入口供启动流程复用。
pub fn ensure_popup_window(_app: &tauri::AppHandle, _config: &AppConfig) -> Result<(), String> {
    Ok(())
}

/// 隐藏翻译弹窗。截图前调用，避免把弹窗打进 DXGI 帧；幂等。
pub fn hide_popup(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(POPUP_LABEL) {
        let _ = window.hide();
    }
}

/// 翻译弹窗唤起时的定位策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PopupPositionMode {
    /// 跟随光标并钳制到工作区（划词 / 截图译 / 快捷键触发）。
    #[default]
    NearCursor,
    /// 不改坐标：保留上一次位置；首次创建依赖 `tauri.conf` 的 `center`。
    /// 托盘手动打开空弹窗等「非上下文触发」入口使用。
    Restore,
}

/// 唤起弹窗：复用 main 翻译窗口。
/// - [`PopupPositionMode::NearCursor`]：按光标定位；光标上下文不可用时不改位置。
/// - [`PopupPositionMode::Restore`]：不重新定位（上次位置或默认居中）。
pub fn show_popup(
    app: &tauri::AppHandle,
    _config: &AppConfig,
    mode: PopupPositionMode,
) -> Result<(), String> {
    let window = app
        .get_webview_window(POPUP_LABEL)
        .ok_or_else(|| "翻译弹窗未创建".to_string())?;

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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn work_area_1920x1080() -> LogicalRect {
        LogicalRect { x: 0.0, y: 0.0, width: 1920.0, height: 1080.0 }
    }

    fn popup_400x300() -> LogicalSize {
        LogicalSize { width: 400.0, height: 300.0 }
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
