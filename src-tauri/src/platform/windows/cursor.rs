use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITOR_DEFAULTTONEAREST, MONITORINFO,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

/// 光标所在显示器的有效 DPI scale（96 DPI → 1.0）。
/// 供截图 overlay 把物理帧换算为 CSS 逻辑尺寸；**不要**改用 WebView `main` 的 scale——
/// main 可能不存在或位于其它 DPI 屏幕，落到 1.0 会在高 DPI 上只裁到左上角。
pub fn cursor_monitor_scale_factor() -> f64 {
    unsafe {
        let mut cursor = POINT::default();
        if GetCursorPos(&mut cursor).is_err() {
            return 1.0;
        }
        let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
        let mut dpi_x = 0u32;
        let mut dpi_y = 0u32;
        if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).is_err() {
            return 1.0;
        }
        let dpi = dpi_x.max(1) as f64;
        (dpi / 96.0).max(0.5)
    }
}

/// 光标所在显示器物理边界：`(x, y, width, height)`，失败返回 `None`。
/// 供 overlay 按抓帧同源的显示器铺满，避免 `.fullscreen(true)` 在多屏下落到主屏。
pub fn cursor_monitor_physical_bounds() -> Option<(i32, i32, u32, u32)> {
    unsafe {
        let mut cursor = POINT::default();
        if GetCursorPos(&mut cursor).is_err() {
            return None;
        }
        let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO::default();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return None;
        }
        let r = info.rcMonitor;
        let w = (r.right - r.left).max(0) as u32;
        let h = (r.bottom - r.top).max(0) as u32;
        if w == 0 || h == 0 {
            return None;
        }
        Some((r.left, r.top, w, h))
    }
}

/// 返回光标所在显示器工作区（逻辑像素）：
/// `(cursor_x, cursor_y, work_x, work_y, work_w, work_h)`，全为逻辑像素。
/// `scale` 用于物理→逻辑换算（MVP 取主窗口 scale，多屏精确缩放留后续）。
/// 任一 Win32 调用失败返回 `None`，由调用方退化为不定位。
pub fn cursor_logical_context(scale: f64) -> Option<(f64, f64, f64, f64, f64, f64)> {
    unsafe {
        let mut cursor = POINT::default();
        if GetCursorPos(&mut cursor).is_err() {
            return None;
        }
        let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO::default();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return None;
        }
        let work = info.rcWork;
        let s = scale.max(0.0001);
        Some((
            cursor.x as f64 / s,
            cursor.y as f64 / s,
            work.left as f64 / s,
            work.top as f64 / s,
            (work.right - work.left) as f64 / s,
            (work.bottom - work.top) as f64 / s,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_monitor_scale_factor_is_sane() {
        let s = cursor_monitor_scale_factor();
        assert!(s >= 0.5 && s <= 8.0, "scale={s}");
    }
}
