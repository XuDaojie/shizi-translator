//! 原生弹窗表面（**路径 B：Win32**）。
//!
//! - `WS_POPUP | WS_CLIPCHILDREN`，无系统厚边框
//! - `WS_EX_TOOLWINDOW`：不进任务栏
//! - 初始 `SW_HIDE`，逻辑尺寸约 420×360
//! - DWM 圆角（best-effort）
//! - 不依赖 Microsoft.UI.Xaml / WinAppSDK

use std::sync::Once;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DWM_WINDOW_CORNER_PREFERENCE,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, SetBkMode, SetTextColor,
    TextOutW, HGDIOBJ, PAINTSTRUCT, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, IsWindow, IsWindowVisible, LoadCursorW,
    RegisterClassExW, SetWindowPos, ShowWindow, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, HMENU,
    HWND_TOP, IDC_ARROW, SWP_NOZORDER, SWP_SHOWWINDOW, SW_HIDE, SW_SHOW, USER_DEFAULT_SCREEN_DPI,
    WM_CLOSE, WM_DESTROY, WM_PAINT, WNDCLASSEXW, WS_CLIPCHILDREN, WS_EX_TOOLWINDOW, WS_POPUP,
};

use crate::app::popup_backend::types::PopupPositionMode;
use crate::app::popup_window::{compute_popup_position, LogicalPos, LogicalRect, LogicalSize};
use crate::platform::cursor_logical_context;

/// 弹窗逻辑宽度（与 WebView 弹窗一致）。
pub const POPUP_LOGICAL_WIDTH: f64 = 420.0;
/// 弹窗逻辑高度（与 WebView builder 初始高度一致）。
pub const POPUP_LOGICAL_HEIGHT: f64 = 360.0;

const CLASS_NAME: PCWSTR = w!("Shizi.NativePopup.B");

/// Send-safe HWND 包装。
///
/// `HWND` 本身非 `Send`；句柄值可跨线程携带，但**销毁应尽量在创建线程**。
#[derive(Debug)]
pub struct NativePopupHwnd {
    raw: isize,
}

// SAFETY: 原生 HWND 为内核句柄值，跨线程传递句柄值是常见模式；
// 消息与 DestroyWindow 的线程亲和由调用方约束。
unsafe impl Send for NativePopupHwnd {}

impl NativePopupHwnd {
    fn from_hwnd(hwnd: HWND) -> Self {
        Self {
            raw: hwnd.0 as isize,
        }
    }

    fn hwnd(&self) -> HWND {
        HWND(self.raw as *mut core::ffi::c_void)
    }

    /// 窗口是否仍有效。
    pub fn is_valid(&self) -> bool {
        unsafe { IsWindow(self.hwnd()).as_bool() }
    }

    /// 当前是否可见。
    pub fn is_visible(&self) -> bool {
        unsafe { IsWindowVisible(self.hwnd()).as_bool() }
    }
}

static REGISTER_CLASS: Once = Once::new();
static mut CLASS_ATOM: u16 = 0;

fn register_class_once() -> Result<(), String> {
    let mut result = Ok(());
    REGISTER_CLASS.call_once(|| {
        result = unsafe { register_class_inner() };
    });
    // Once 只跑一次；若首次失败，后续 create 会因 atom=0 再报错
    if result.is_err() {
        return result;
    }
    if unsafe { CLASS_ATOM } == 0 {
        return Err("注册原生弹窗窗口类失败".to_string());
    }
    Ok(())
}

unsafe fn register_class_inner() -> Result<(), String> {
    let hinstance = GetModuleHandleW(None)
        .map_err(|e| format!("GetModuleHandleW 失败: {e}"))?;

    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: windows::Win32::Foundation::HINSTANCE(hinstance.0),
        hIcon: Default::default(),
        hCursor: LoadCursorW(None, IDC_ARROW)
            .map_err(|e| format!("LoadCursorW 失败: {e}"))?,
        // 浅色客户区由 WM_PAINT 自绘
        hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH(std::ptr::null_mut()),
        lpszMenuName: PCWSTR::null(),
        lpszClassName: CLASS_NAME,
        hIconSm: Default::default(),
    };

    let atom = RegisterClassExW(&wc);
    if atom == 0 {
        // 类已存在时 atom 也可能为 0；GetLastError 区分
        let err = windows::Win32::Foundation::GetLastError();
        if err == windows::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS {
            CLASS_ATOM = 1; // 非 0 表示可用
            return Ok(());
        }
        return Err(format!("RegisterClassExW 失败: {err:?}"));
    }
    CLASS_ATOM = atom;
    Ok(())
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            if !hdc.is_invalid() {
                let mut rect = RECT::default();
                let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect);
                // 浅灰背景 BGR 0xF5F5F5
                let brush = CreateSolidBrush(COLORREF(0x00F5F5F5));
                FillRect(hdc, &rect, brush);
                let _ = DeleteObject(HGDIOBJ(brush.0));

                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, COLORREF(0x00333333));
                let text: Vec<u16> = "Shizi".encode_utf16().collect();
                let _ = TextOutW(hdc, 16, 16, &text);
            }
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_CLOSE => {
            // 托盘驻留：关 = hide，不 Destroy
            let _ = ShowWindow(hwnd, SW_HIDE);
            LRESULT(0)
        }
        WM_DESTROY => LRESULT(0),
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn logical_to_physical(logical: f64, scale: f64) -> i32 {
    (logical * scale).round() as i32
}

fn window_scale(hwnd: HWND) -> f64 {
    let dpi = unsafe {
        let d = GetDpiForWindow(hwnd);
        if d == 0 {
            GetDpiForSystem().max(USER_DEFAULT_SCREEN_DPI)
        } else {
            d
        }
    };
    dpi as f64 / USER_DEFAULT_SCREEN_DPI as f64
}

fn system_scale() -> f64 {
    let dpi = unsafe { GetDpiForSystem().max(USER_DEFAULT_SCREEN_DPI) };
    dpi as f64 / USER_DEFAULT_SCREEN_DPI as f64
}

fn apply_dwm_round_corners(hwnd: HWND) {
    let pref = DWMWCP_ROUND;
    let _ = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const DWM_WINDOW_CORNER_PREFERENCE as *const core::ffi::c_void,
            std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        )
    };
}

/// 创建隐藏的原生弹窗（约 420×360 逻辑像素）。
pub fn create_hidden_popup() -> Result<NativePopupHwnd, String> {
    register_class_once()?;

    let scale = system_scale();
    let width = logical_to_physical(POPUP_LOGICAL_WIDTH, scale);
    let height = logical_to_physical(POPUP_LOGICAL_HEIGHT, scale);

    let hinstance = unsafe {
        GetModuleHandleW(None).map_err(|e| format!("GetModuleHandleW 失败: {e}"))?
    };

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            CLASS_NAME,
            w!("Shizi"),
            WS_POPUP | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            width,
            height,
            HWND::default(),
            HMENU::default(),
            windows::Win32::Foundation::HINSTANCE(hinstance.0),
            None,
        )
        .map_err(|e| format!("CreateWindowExW 失败: {e}"))?
    };

    if hwnd.is_invalid() {
        return Err("CreateWindowExW 返回无效 HWND".to_string());
    }

    apply_dwm_round_corners(hwnd);

    // 确保初始隐藏
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }

    Ok(NativePopupHwnd::from_hwnd(hwnd))
}

/// 显示弹窗。`NearCursor` 复用 `compute_popup_position`；`Restore` 不改坐标。
pub fn show_popup(window: &NativePopupHwnd, mode: PopupPositionMode) -> Result<(), String> {
    if !window.is_valid() {
        return Err("原生弹窗 HWND 无效".to_string());
    }
    let hwnd = window.hwnd();

    match mode {
        PopupPositionMode::NearCursor => {
            let scale = window_scale(hwnd);
            if let Some((cx, cy, wx, wy, ww, wh)) = cursor_logical_context(scale) {
                let pos = compute_popup_position(
                    LogicalPos { x: cx, y: cy },
                    LogicalSize {
                        width: POPUP_LOGICAL_WIDTH,
                        height: POPUP_LOGICAL_HEIGHT,
                    },
                    LogicalRect {
                        x: wx,
                        y: wy,
                        width: ww,
                        height: wh,
                    },
                );
                let x = logical_to_physical(pos.x, scale);
                let y = logical_to_physical(pos.y, scale);
                let w = logical_to_physical(POPUP_LOGICAL_WIDTH, scale);
                let h = logical_to_physical(POPUP_LOGICAL_HEIGHT, scale);
                unsafe {
                    SetWindowPos(hwnd, HWND_TOP, x, y, w, h, SWP_NOZORDER | SWP_SHOWWINDOW)
                        .map_err(|e| format!("SetWindowPos 失败: {e}"))?;
                }
            } else {
                unsafe {
                    let _ = ShowWindow(hwnd, SW_SHOW);
                }
            }
        }
        PopupPositionMode::Restore => {
            // 不改坐标，直接显示
            unsafe {
                let _ = ShowWindow(hwnd, SW_SHOW);
            }
        }
    }

    Ok(())
}

/// 隐藏弹窗（幂等）。
pub fn hide_popup(window: &NativePopupHwnd) {
    if window.is_valid() {
        unsafe {
            let _ = ShowWindow(window.hwnd(), SW_HIDE);
        }
    }
}

/// 销毁 HWND（幂等）。应尽量在创建线程调用。
pub fn destroy_popup(window: &NativePopupHwnd) {
    if window.is_valid() {
        unsafe {
            let _ = DestroyWindow(window.hwnd());
        }
    }
}

// 保留占位名给旧调用方（若有）
pub fn show_stub() -> Result<(), String> {
    Err("use create_hidden_popup / show_popup".to_string())
}

pub fn hide_stub() -> Result<(), String> {
    Ok(())
}

pub fn destroy_stub() -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_hide_destroy_lifecycle() {
        let win = create_hidden_popup().expect("create");
        assert!(win.is_valid());
        assert!(!win.is_visible());

        show_popup(&win, PopupPositionMode::Restore).expect("show restore");
        // 可见性依赖消息循环；至少不应报错且句柄仍有效
        assert!(win.is_valid());

        hide_popup(&win);
        hide_popup(&win); // 幂等
        assert!(win.is_valid());

        destroy_popup(&win);
        assert!(!win.is_valid());
        destroy_popup(&win); // 幂等
    }

    #[test]
    fn near_cursor_show_does_not_panic() {
        let win = create_hidden_popup().expect("create");
        show_popup(&win, PopupPositionMode::NearCursor).expect("near cursor");
        hide_popup(&win);
        destroy_popup(&win);
    }
}
