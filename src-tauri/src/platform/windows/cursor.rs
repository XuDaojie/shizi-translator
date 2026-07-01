use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITOR_DEFAULTTONEAREST, MONITORINFO,
};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

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
