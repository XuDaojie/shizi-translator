// 类型与纯函数在任务 4 的窗口管理函数中使用前暂时允许 dead_code。
#![allow(dead_code)]

pub const POPUP_LABEL: &str = "translation-popup";

use tauri::{LogicalPosition, Manager, WebviewUrl, WebviewWindowBuilder};

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

/// 预创建模式下启动时调用：创建并隐藏翻译弹窗。运行时模式无操作。
/// 已存在则跳过（幂等）。
pub fn ensure_popup_window(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    if !config.popup_precreate {
        return Ok(());
    }
    if app.get_webview_window(POPUP_LABEL).is_some() {
        return Ok(());
    }
    build_popup(app)?;
    Ok(())
}

/// 唤起弹窗：预创建模式 show + 定位；运行时模式复用或创建 + 定位。
/// 光标上下文不可用时退化为不重新定位（保留上一次位置或默认）。
pub fn show_popup(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    let window = if config.popup_precreate {
        app.get_webview_window(POPUP_LABEL)
            .ok_or_else(|| "翻译弹窗未预创建".to_string())?
    } else {
        // 运行时模式：复用已有窗口，避免 close+rebuild 触发的标签冲突
        // （close 会触发 CloseRequested → prevent_close → hide，窗口未销毁）
        match app.get_webview_window(POPUP_LABEL) {
            Some(existing) => existing,
            None => build_popup(app)?,
        }
    };

    let scale = app
        .get_webview_window("main")
        .and_then(|w| w.scale_factor().ok())
        .unwrap_or(1.0);

    if let Some((cx, cy, wx, wy, ww, wh)) = cursor_logical_context(scale) {
        const POPUP_W: f64 = 400.0;
        const POPUP_H: f64 = 300.0;
        let pos = compute_popup_position(
            LogicalPos { x: cx, y: cy },
            LogicalSize { width: POPUP_W, height: POPUP_H },
            LogicalRect { x: wx, y: wy, width: ww, height: wh },
        );
        let _ = window.set_position(LogicalPosition::new(pos.x, pos.y));
    }

    let _ = window.show();
    let _ = window.set_focus();
    Ok(())
}

fn build_popup(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    let window = WebviewWindowBuilder::new(app, POPUP_LABEL, WebviewUrl::App("translate.html".into()))
        .title("Shizi 翻译")
        .inner_size(400.0, 300.0)
        .decorations(true)
        .resizable(true)
        .visible(false)
        .build()
        .map_err(|e| format!("创建翻译弹窗失败: {e}"))?;

    // 关闭事件：隐藏而非销毁（托盘驻留模型）
    let win_clone = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = win_clone.hide();
        }
    });

    Ok(window)
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
