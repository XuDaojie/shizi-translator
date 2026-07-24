//! 原生弹窗表面（**路径 B：Win32**）。
//!
//! - `WS_POPUP | WS_CLIPCHILDREN`，无系统厚边框
//! - `WS_EX_TOOLWINDOW`：不进任务栏
//! - 初始 `SW_HIDE`，逻辑尺寸约 420×360
//! - DWM 圆角（best-effort）
//! - GDI 自绘：源文 + 结果卡（`publish` 更新快照 + `PostMessage` 触发重绘）
//! - 不依赖 Microsoft.UI.Xaml / WinAppSDK

use std::sync::{Mutex, Once};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{BOOL, COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DWM_WINDOW_CORNER_PREFERENCE,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect, InvalidateRect,
    SetBkMode, SetTextColor, DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_WORDBREAK,
    HGDIOBJ, PAINTSTRUCT, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, IsWindow, IsWindowVisible, LoadCursorW,
    PostMessageW, RegisterClassExW, SetWindowPos, ShowWindow, CS_HREDRAW, CS_VREDRAW,
    CW_USEDEFAULT, HMENU, HWND_TOP, IDC_ARROW, SWP_SHOWWINDOW, SW_HIDE, SW_SHOW,
    USER_DEFAULT_SCREEN_DPI, WM_CLOSE, WM_DESTROY, WM_PAINT, WM_USER, WNDCLASSEXW,
    WS_CLIPCHILDREN, WS_EX_TOOLWINDOW, WS_POPUP,
};

use crate::app::popup_backend::types::{PopupCardStatus, PopupPositionMode, PopupViewModel};
use crate::app::popup_window::{compute_popup_position, LogicalPos, LogicalRect, LogicalSize};
use crate::platform::cursor_logical_context;

/// 弹窗逻辑宽度（与 WebView 弹窗一致）。
pub const POPUP_LOGICAL_WIDTH: f64 = 420.0;
/// 弹窗逻辑高度（与 WebView builder 初始高度一致）。
pub const POPUP_LOGICAL_HEIGHT: f64 = 360.0;

const CLASS_NAME: PCWSTR = w!("Shizi.NativePopup.B");

/// `WM_USER + N`：任意线程 `publish` 后投递，UI 线程 `InvalidateRect`。
const WM_POPUP_REFRESH: u32 = WM_USER + 0x51;

/// 单卡渲染快照（与 ViewModel 解耦，便于锁内拷贝）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaintCardSnapshot {
    pub service_name: String,
    pub status: PopupCardStatus,
    pub text: String,
    pub error_message: String,
}

/// 整窗 GDI 绘制快照。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PaintSnapshot {
    pub source_text: String,
    pub cards: Vec<PaintCardSnapshot>,
}

impl PaintSnapshot {
    pub fn from_view_model(vm: &PopupViewModel) -> Self {
        Self {
            source_text: vm.source_text.clone(),
            cards: vm
                .cards
                .iter()
                .map(|c| PaintCardSnapshot {
                    service_name: c.service_name.clone(),
                    status: c.status.clone(),
                    text: c.text.clone(),
                    error_message: c.error_message.clone(),
                })
                .collect(),
        }
    }
}

/// 最近一次 `publish` 的绘制快照（任意线程写，UI 线程读）。
static PAINT_SNAPSHOT: Mutex<PaintSnapshot> = Mutex::new(PaintSnapshot {
    source_text: String::new(),
    cards: Vec::new(),
});

/// 将 ViewModel 写入共享快照（不碰 HWND；任意线程、短路径）。
///
/// 返回写入后的快照副本，便于单测「入队/落盘」而不创建真实窗口。
pub fn store_paint_snapshot(vm: &PopupViewModel) -> PaintSnapshot {
    let snap = PaintSnapshot::from_view_model(vm);
    if let Ok(mut guard) = PAINT_SNAPSHOT.lock() {
        *guard = snap.clone();
    }
    snap
}

/// 读取当前绘制快照（测试 / WM_PAINT）。
pub fn load_paint_snapshot() -> PaintSnapshot {
    PAINT_SNAPSHOT
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default()
}

/// 状态 → 状态文案。
pub fn status_label(status: &PopupCardStatus) -> &'static str {
    match status {
        PopupCardStatus::Pending => "等待",
        PopupCardStatus::Translating => "翻译中",
        PopupCardStatus::Finished => "完成",
        PopupCardStatus::Failed => "失败",
        PopupCardStatus::Cancelled => "已取消",
    }
}

/// 状态 → 文本色（COLORREF，0x00BBGGRR）。
pub fn status_color_bgr(status: &PopupCardStatus) -> u32 {
    match status {
        // 蓝/灰
        PopupCardStatus::Pending => 0x00_88_88_88,
        PopupCardStatus::Translating => 0x00_C0_70_20,
        // 绿
        PopupCardStatus::Finished => 0x00_2E_8B_2E,
        // 红
        PopupCardStatus::Failed => 0x00_22_22_CC,
        PopupCardStatus::Cancelled => 0x00_88_88_88,
    }
}

/// 请求 UI 线程重绘：`PostMessage(WM_POPUP_REFRESH)`（非阻塞）。
///
/// - `hwnd_raw == 0`：仅记为未投递（可测），不调用 Win32。
/// - 有效句柄：`PostMessageW`，失败时 best-effort `InvalidateRect`。
///
/// 返回是否尝试了投递（`hwnd_raw != 0`）。
pub fn enqueue_repaint(hwnd_raw: isize) -> bool {
    if hwnd_raw == 0 {
        return false;
    }
    let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
    unsafe {
        if PostMessageW(hwnd, WM_POPUP_REFRESH, WPARAM(0), LPARAM(0)).is_err() {
            // PostMessage 失败时仍尝试 Invalidate（亦可跨线程）
            let _ = InvalidateRect(hwnd, None, BOOL(1));
        }
    }
    true
}

/// `publish` 热路径：更新快照 + 投递重绘（均非阻塞）。
pub fn publish_view_model(window: &NativePopupHwnd, vm: &PopupViewModel) {
    let _ = store_paint_snapshot(vm);
    if window.is_valid() {
        let _ = enqueue_repaint(window.raw);
    }
}

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

    /// 当前是否可见（`IsWindowVisible`）。
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
    let hinstance = GetModuleHandleW(None).map_err(|e| format!("GetModuleHandleW 失败: {e}"))?;

    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: windows::Win32::Foundation::HINSTANCE(hinstance.0),
        hIcon: Default::default(),
        hCursor: LoadCursorW(None, IDC_ARROW).map_err(|e| format!("LoadCursorW 失败: {e}"))?,
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

fn to_utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// 在指定矩形内绘制文本；返回占用高度（至少 line_hint）。
unsafe fn draw_text_in_rect(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    text: &str,
    rect: &mut RECT,
    color: u32,
    flags: windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT,
) -> i32 {
    if text.is_empty() {
        return 0;
    }
    SetTextColor(hdc, COLORREF(color));
    let mut buf = to_utf16(text);
    // DrawTextW 需要可变缓冲；不写回修改（无 DT_MODIFYSTRING）
    let h = DrawTextW(hdc, &mut buf, rect, flags);
    if h > 0 {
        h
    } else {
        // 空结果时给最小行高，避免布局塌陷
        16
    }
}

unsafe fn paint_popup(hwnd: HWND, hdc: windows::Win32::Graphics::Gdi::HDC) {
    let mut client = RECT::default();
    let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut client);

    // 浅灰背景 BGR 0xF5F5F5
    let brush = CreateSolidBrush(COLORREF(0x00F5F5F5));
    FillRect(hdc, &client, brush);
    let _ = DeleteObject(HGDIOBJ(brush.0));

    SetBkMode(hdc, TRANSPARENT);

    let snap = load_paint_snapshot();
    let pad = 12i32;
    let gap = 8i32;
    let mut y = pad;
    let right = client.right - pad;
    let bottom = client.bottom - pad;

    // —— 源文标题 ——
    {
        let mut r = RECT {
            left: pad,
            top: y,
            right,
            bottom: y + 20,
        };
        let _ = draw_text_in_rect(
            hdc,
            "源文",
            &mut r,
            0x00_88_88_88,
            DT_LEFT | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
        );
        y += 20 + 4;
    }

    // —— 源文正文 ——
    {
        let max_h = 96i32;
        let mut r = RECT {
            left: pad,
            top: y,
            right,
            bottom: (y + max_h).min(bottom),
        };
        let src = if snap.source_text.is_empty() {
            "（暂无源文）"
        } else {
            snap.source_text.as_str()
        };
        let h = draw_text_in_rect(
            hdc,
            src,
            &mut r,
            0x00_33_33_33,
            DT_LEFT | DT_WORDBREAK | DT_NOPREFIX | DT_END_ELLIPSIS,
        );
        y += h.max(18) + gap;
    }

    // 分隔线（简单色带）
    if y + 2 < bottom {
        let sep = RECT {
            left: pad,
            top: y,
            right,
            bottom: y + 1,
        };
        let sep_brush = CreateSolidBrush(COLORREF(0x00_DD_DD_DD));
        FillRect(hdc, &sep, sep_brush);
        let _ = DeleteObject(HGDIOBJ(sep_brush.0));
        y += gap;
    }

    // —— 结果卡 ——
    if snap.cards.is_empty() {
        let mut r = RECT {
            left: pad,
            top: y,
            right,
            bottom: y + 20,
        };
        let _ = draw_text_in_rect(
            hdc,
            "（暂无结果）",
            &mut r,
            0x00_99_99_99,
            DT_LEFT | DT_SINGLELINE | DT_NOPREFIX,
        );
        return;
    }

    for card in &snap.cards {
        if y >= bottom {
            break;
        }

        // 服务名 + 状态
        let header = format!(
            "{}  ·  {}",
            if card.service_name.is_empty() {
                "服务"
            } else {
                card.service_name.as_str()
            },
            status_label(&card.status)
        );
        let status_color = status_color_bgr(&card.status);
        {
            let mut r = RECT {
                left: pad,
                top: y,
                right,
                bottom: y + 20,
            };
            let _ = draw_text_in_rect(
                hdc,
                &header,
                &mut r,
                status_color,
                DT_LEFT | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
            );
            y += 20 + 2;
        }

        // 译文
        if y < bottom {
            let body = if card.text.is_empty() {
                match card.status {
                    PopupCardStatus::Translating | PopupCardStatus::Pending => "…",
                    PopupCardStatus::Failed => {
                        if card.error_message.is_empty() {
                            "翻译失败"
                        } else {
                            card.error_message.as_str()
                        }
                    }
                    _ => "",
                }
            } else {
                card.text.as_str()
            };
            if !body.is_empty() {
                let remain = (bottom - y).max(0);
                let mut r = RECT {
                    left: pad,
                    top: y,
                    right,
                    bottom: y + remain,
                };
                let color = if matches!(card.status, PopupCardStatus::Failed) && card.text.is_empty()
                {
                    status_color_bgr(&PopupCardStatus::Failed)
                } else {
                    0x00_22_22_22
                };
                let h = draw_text_in_rect(
                    hdc,
                    body,
                    &mut r,
                    color,
                    DT_LEFT | DT_WORDBREAK | DT_NOPREFIX | DT_END_ELLIPSIS,
                );
                y += h.max(16) + gap;
            }
        }

        // 失败且已有译文时，另起一行错误摘要
        if matches!(card.status, PopupCardStatus::Failed)
            && !card.error_message.is_empty()
            && !card.text.is_empty()
            && y < bottom
        {
            let mut r = RECT {
                left: pad,
                top: y,
                right,
                bottom: (y + 36).min(bottom),
            };
            let h = draw_text_in_rect(
                hdc,
                &card.error_message,
                &mut r,
                status_color_bgr(&PopupCardStatus::Failed),
                DT_LEFT | DT_WORDBREAK | DT_NOPREFIX | DT_END_ELLIPSIS,
            );
            y += h.max(14) + gap;
        }
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_POPUP_REFRESH => {
            // 在 UI 线程上触发重绘（publish 仅 PostMessage，不持锁等待）
            let _ = InvalidateRect(hwnd, None, BOOL(1));
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            if !hdc.is_invalid() {
                paint_popup(hwnd, hdc);
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

    let hinstance =
        unsafe { GetModuleHandleW(None).map_err(|e| format!("GetModuleHandleW 失败: {e}"))? };

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
                    // 不带 SWP_NOZORDER：与 HWND_TOP 配合置顶
                    SetWindowPos(hwnd, HWND_TOP, x, y, w, h, SWP_SHOWWINDOW)
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
    use crate::app::popup_backend::types::PopupCardVm;

    fn sample_vm() -> PopupViewModel {
        PopupViewModel {
            session_id: Some("s1".into()),
            source_text: "Hello world".into(),
            source_type: "selection".into(),
            source_lang: "en".into(),
            target_lang: "zh".into(),
            is_translating: true,
            cards: vec![PopupCardVm {
                service_instance_id: "svc-1".into(),
                service_name: "Mock".into(),
                service_type: "llm".into(),
                protocol: "mock".into(),
                model_name: "mock".into(),
                status: PopupCardStatus::Translating,
                text: "你好".into(),
                error_message: String::new(),
                usage_input: None,
                usage_output: None,
                detected_source_lang: None,
            }],
        }
    }

    #[test]
    fn store_paint_snapshot_captures_source_and_cards() {
        let vm = sample_vm();
        let snap = store_paint_snapshot(&vm);
        assert_eq!(snap.source_text, "Hello world");
        assert_eq!(snap.cards.len(), 1);
        assert_eq!(snap.cards[0].service_name, "Mock");
        assert_eq!(snap.cards[0].text, "你好");
        assert_eq!(snap.cards[0].status, PopupCardStatus::Translating);

        let loaded = load_paint_snapshot();
        assert_eq!(loaded, snap);
    }

    #[test]
    fn store_paint_snapshot_streaming_overwrite() {
        let mut vm = sample_vm();
        let _ = store_paint_snapshot(&vm);
        vm.cards[0].text = "你好，世界".into();
        vm.cards[0].status = PopupCardStatus::Finished;
        let snap = store_paint_snapshot(&vm);
        assert_eq!(snap.cards[0].text, "你好，世界");
        assert_eq!(snap.cards[0].status, PopupCardStatus::Finished);
        assert_eq!(load_paint_snapshot().cards[0].text, "你好，世界");
    }

    #[test]
    fn enqueue_repaint_zero_hwnd_is_noop_false() {
        // 不测真实 HWND：raw=0 表示未投递
        assert!(!enqueue_repaint(0));
    }

    #[test]
    fn status_color_and_label_mapping() {
        assert_eq!(status_label(&PopupCardStatus::Translating), "翻译中");
        assert_eq!(status_label(&PopupCardStatus::Finished), "完成");
        assert_eq!(status_label(&PopupCardStatus::Failed), "失败");
        // 蓝/灰 vs 绿 vs 红 互不相同
        let t = status_color_bgr(&PopupCardStatus::Translating);
        let f = status_color_bgr(&PopupCardStatus::Finished);
        let e = status_color_bgr(&PopupCardStatus::Failed);
        assert_ne!(t, f);
        assert_ne!(f, e);
        assert_ne!(t, e);
    }

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

    #[test]
    fn publish_view_model_updates_snapshot_without_blocking() {
        let win = create_hidden_popup().expect("create");
        let vm = sample_vm();
        // 应快速返回：写快照 + PostMessage
        publish_view_model(&win, &vm);
        let loaded = load_paint_snapshot();
        assert_eq!(loaded.source_text, "Hello world");
        assert_eq!(loaded.cards[0].text, "你好");
        destroy_popup(&win);
    }
}
