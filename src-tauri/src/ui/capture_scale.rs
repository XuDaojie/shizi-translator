//! 截图框选 DPI 缩放解析。
//!
//! overlay 前端用 CSS 逻辑像素回传框选矩形，后端按 `scale_factor` 换算物理像素裁剪。
//! **不可只读翻译弹窗 `main` WebView**：WinUI/原生 backend 下 `main` 常不存在，
//! 若回落到 1.0 会在高 DPI 上只裁到左上角（画面像「左上角被放大」）。

use tauri::{AppHandle, Manager};

/// 校验 scale：有限且 > 0 才可用，否则视为无效。
pub fn sanitize_capture_scale(scale: f64) -> Option<f64> {
    if scale.is_finite() && scale > 0.0 {
        Some(scale)
    } else {
        None
    }
}

/// 解析截图框选用的显示器 DPI 缩放。
///
/// 优先级：
/// 1. 任一已存在 WebView 的 `scale_factor`（main / settings / ocr / overlay）
/// 2. 光标所在显示器
/// 3. 主显示器
/// 4. `1.0`
pub fn resolve_capture_scale_factor(app: &AppHandle) -> f64 {
    const LABELS: &[&str] = &["main", "settings", "ocr", "overlay"];
    for label in LABELS {
        if let Some(window) = app.get_webview_window(label) {
            if let Ok(scale) = window.scale_factor() {
                if let Some(s) = sanitize_capture_scale(scale) {
                    return s;
                }
            }
        }
    }

    if let Ok(pos) = app.cursor_position() {
        if let Ok(Some(monitor)) = app.monitor_from_point(pos.x, pos.y) {
            if let Some(s) = sanitize_capture_scale(monitor.scale_factor()) {
                return s;
            }
        }
    }

    if let Ok(Some(monitor)) = app.primary_monitor() {
        if let Some(s) = sanitize_capture_scale(monitor.scale_factor()) {
            return s;
        }
    }

    1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rejects_non_positive_and_non_finite() {
        assert_eq!(sanitize_capture_scale(1.0), Some(1.0));
        assert_eq!(sanitize_capture_scale(1.5), Some(1.5));
        assert_eq!(sanitize_capture_scale(2.0), Some(2.0));
        assert_eq!(sanitize_capture_scale(0.0), None);
        assert_eq!(sanitize_capture_scale(-1.0), None);
        assert_eq!(sanitize_capture_scale(f64::NAN), None);
        assert_eq!(sanitize_capture_scale(f64::INFINITY), None);
    }
}
