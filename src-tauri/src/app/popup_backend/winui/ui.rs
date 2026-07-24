//! 原生弹窗表面（**路径 B：Win32**）。
//!
//! - `WS_POPUP | WS_CLIPCHILDREN`，无系统厚边框
//! - `WS_EX_TOOLWINDOW`：不进任务栏
//! - 初始 `SW_HIDE`，逻辑尺寸约 420×360
//! - DWM 圆角（best-effort）
//! - GDI 自绘：源文 + 结果卡 + 底部动作条
//! - 用户动作：Esc 关闭、Ctrl+C 复制、工具栏点击、双击标题区关闭
//! - 不依赖 Microsoft.UI.Xaml / WinAppSDK

use std::sync::{Mutex, Once};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{BOOL, COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DWM_WINDOW_CORNER_PREFERENCE,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect, InvalidateRect,
    SetBkMode, SetTextColor, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE,
    DT_VCENTER, DT_WORDBREAK, HGDIOBJ, PAINTSTRUCT, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, SetFocus, VK_CONTROL, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, IsWindow, IsWindowVisible,
    LoadCursorW, PostMessageW, RegisterClassExW, SetWindowPos, ShowWindow, CS_DBLCLKS, CS_HREDRAW,
    CS_VREDRAW, CW_USEDEFAULT, HMENU, HWND_TOP, IDC_ARROW, SWP_SHOWWINDOW, SW_HIDE, SW_SHOW,
    USER_DEFAULT_SCREEN_DPI, WM_CLOSE, WM_DESTROY, WM_KEYDOWN, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN,
    WM_PAINT, WM_USER, WNDCLASSEXW, WS_CLIPCHILDREN, WS_EX_TOOLWINDOW, WS_POPUP,
};

use crate::app::popup_backend::types::{
    PopupCardStatus, PopupPositionMode, PopupUserAction, PopupViewModel,
};
use crate::app::popup_window::{compute_popup_position, LogicalPos, LogicalRect, LogicalSize};
use crate::platform::cursor_logical_context;

/// 用户动作处理器（由 `actions` 在 ensure 时注册）。
///
/// `AppHandle` 仅保存在 `actions` 模块，避免在本 Win32 UI 模块静态持有
/// `AppHandle`（实测会导致测试二进制 STATUS_ENTRYPOINT_NOT_FOUND）。
type ActionHandler = fn(PopupUserAction);

static ACTION_HANDLER: Mutex<Option<ActionHandler>> = Mutex::new(None);

/// 注册动作处理器（`actions::install_action_handler` 调用）。
pub fn set_action_handler(handler: ActionHandler) {
    if let Ok(mut g) = ACTION_HANDLER.lock() {
        *g = Some(handler);
    }
}

fn dispatch_bound_action(action: PopupUserAction) {
    let handler = ACTION_HANDLER.lock().ok().and_then(|g| *g);
    if let Some(h) = handler {
        h(action);
    } else {
        // 未 ensure 绑定前收到的消息：关闭仍 SW_HIDE 由调用方处理
        log::warn!("原生弹窗未注册动作处理器，忽略: {action:?}");
    }
}

/// 弹窗逻辑宽度（与 WebView 弹窗一致）。
pub const POPUP_LOGICAL_WIDTH: f64 = 420.0;
/// 弹窗逻辑高度（与 WebView builder 初始高度一致）。
pub const POPUP_LOGICAL_HEIGHT: f64 = 360.0;

/// 底部动作条高度（物理像素近似；按 DPI 缩放时由布局函数乘 scale）。
const TOOLBAR_LOGICAL_H: i32 = 36;
const BTN_LOGICAL_W: i32 = 56;
const BTN_GAP: i32 = 6;
const TITLE_HIT_LOGICAL_H: i32 = 28;

const CLASS_NAME: PCWSTR = w!("Shizi.NativePopup.B");

/// `WM_USER + N`：任意线程 `publish` 后投递，UI 线程 `InvalidateRect`。
const WM_POPUP_REFRESH: u32 = WM_USER + 0x51;

/// 工具栏按钮（GDI 热区）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarButton {
    Close,
    Cancel,
    Retry,
    Copy,
    Settings,
}

impl ToolbarButton {
    pub fn label(self) -> &'static str {
        match self {
            Self::Close => "关闭",
            Self::Cancel => "取消",
            Self::Retry => "重试",
            Self::Copy => "复制",
            Self::Settings => "设置",
        }
    }

    pub fn to_action(self, snap: &PaintSnapshot) -> PopupUserAction {
        match self {
            Self::Close => PopupUserAction::Close,
            Self::Cancel => PopupUserAction::CancelTranslation,
            Self::Retry => PopupUserAction::Retry {
                service_instance_id: None,
            },
            Self::Copy => {
                let id = first_copyable_service_id(snap).unwrap_or_default();
                PopupUserAction::CopyResult {
                    service_instance_id: id,
                }
            }
            Self::Settings => PopupUserAction::OpenSettings,
        }
    }
}

/// 单卡渲染快照（与 ViewModel 解耦，便于锁内拷贝）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaintCardSnapshot {
    pub service_instance_id: String,
    pub service_name: String,
    pub status: PopupCardStatus,
    pub text: String,
    pub error_message: String,
}

/// 整窗 GDI 绘制快照。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PaintSnapshot {
    pub source_text: String,
    pub source_lang: String,
    pub target_lang: String,
    pub is_translating: bool,
    pub cards: Vec<PaintCardSnapshot>,
}

impl PaintSnapshot {
    pub fn from_view_model(vm: &PopupViewModel) -> Self {
        Self {
            source_text: vm.source_text.clone(),
            source_lang: vm.source_lang.clone(),
            target_lang: vm.target_lang.clone(),
            is_translating: vm.is_translating,
            cards: vm
                .cards
                .iter()
                .map(|c| PaintCardSnapshot {
                    service_instance_id: c.service_instance_id.clone(),
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
    source_lang: String::new(),
    target_lang: String::new(),
    is_translating: false,
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

/// 读取当前绘制快照（测试 / WM_PAINT / 复制）。
pub fn load_paint_snapshot() -> PaintSnapshot {
    PAINT_SNAPSHOT
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default()
}

/// 按 `service_instance_id` 解析复制文本：优先 `text`，否则 `error_message`。
pub fn resolve_copy_text(snap: &PaintSnapshot, service_instance_id: &str) -> Option<String> {
    let card = snap
        .cards
        .iter()
        .find(|c| c.service_instance_id == service_instance_id)?;
    if !card.text.is_empty() {
        Some(card.text.clone())
    } else if !card.error_message.is_empty() {
        Some(card.error_message.clone())
    } else {
        None
    }
}

/// 首张有可复制正文的服务 id（Ctrl+C）。
pub fn first_copyable_service_id(snap: &PaintSnapshot) -> Option<String> {
    snap.cards
        .iter()
        .find(|c| !c.text.is_empty())
        .map(|c| c.service_instance_id.clone())
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
        PopupCardStatus::Pending => 0x00_88_88_88,
        PopupCardStatus::Translating => 0x00_C0_70_20,
        PopupCardStatus::Finished => 0x00_2E_8B_2E,
        PopupCardStatus::Failed => 0x00_22_22_CC,
        PopupCardStatus::Cancelled => 0x00_88_88_88,
    }
}

/// 请求 UI 线程重绘：`PostMessage(WM_POPUP_REFRESH)`（非阻塞）。
///
/// 返回是否尝试了投递（`hwnd_raw != 0`）。
pub fn enqueue_repaint(hwnd_raw: isize) -> bool {
    if hwnd_raw == 0 {
        return false;
    }
    let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
    unsafe {
        if PostMessageW(hwnd, WM_POPUP_REFRESH, WPARAM(0), LPARAM(0)).is_err() {
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
#[derive(Debug)]
pub struct NativePopupHwnd {
    raw: isize,
}

// SAFETY: 原生 HWND 为内核句柄值，跨线程传递句柄值是常见模式。
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

    pub fn is_valid(&self) -> bool {
        unsafe { IsWindow(self.hwnd()).as_bool() }
    }

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
        style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: windows::Win32::Foundation::HINSTANCE(hinstance.0),
        hIcon: Default::default(),
        hCursor: LoadCursorW(None, IDC_ARROW).map_err(|e| format!("LoadCursorW 失败: {e}"))?,
        hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH(std::ptr::null_mut()),
        lpszMenuName: PCWSTR::null(),
        lpszClassName: CLASS_NAME,
        hIconSm: Default::default(),
    };

    let atom = RegisterClassExW(&wc);
    if atom == 0 {
        let err = windows::Win32::Foundation::GetLastError();
        if err == windows::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS {
            CLASS_ATOM = 1;
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
    let h = DrawTextW(hdc, &mut buf, rect, flags);
    if h > 0 {
        h
    } else {
        16
    }
}

/// 计算底部工具栏按钮布局（客户区坐标）。
pub fn layout_toolbar_buttons(
    client: &RECT,
    is_translating: bool,
    scale: f64,
) -> Vec<(ToolbarButton, RECT)> {
    let toolbar_h = ((TOOLBAR_LOGICAL_H as f64) * scale).round() as i32;
    let btn_w = ((BTN_LOGICAL_W as f64) * scale).round() as i32;
    let gap = ((BTN_GAP as f64) * scale).round() as i32;
    let pad = ((8.0_f64) * scale).round() as i32;

    let mut buttons = vec![ToolbarButton::Close];
    if is_translating {
        buttons.push(ToolbarButton::Cancel);
    }
    buttons.push(ToolbarButton::Retry);
    buttons.push(ToolbarButton::Copy);
    buttons.push(ToolbarButton::Settings);

    let top = (client.bottom - toolbar_h + pad / 2).max(client.top);
    let bottom = (client.bottom - pad / 2).max(top + 1);
    let mut x = pad;
    let mut out = Vec::with_capacity(buttons.len());
    for btn in buttons {
        let right = (x + btn_w).min(client.right - pad);
        if right <= x {
            break;
        }
        out.push((
            btn,
            RECT {
                left: x,
                top,
                right,
                bottom,
            },
        ));
        x = right + gap;
    }
    out
}

/// 命中测试：点是否落在工具栏按钮上。
pub fn hit_test_toolbar(
    x: i32,
    y: i32,
    client: &RECT,
    is_translating: bool,
    scale: f64,
) -> Option<ToolbarButton> {
    for (btn, r) in layout_toolbar_buttons(client, is_translating, scale) {
        if x >= r.left && x < r.right && y >= r.top && y < r.bottom {
            return Some(btn);
        }
    }
    None
}

/// 标题区（双击关闭）命中。
pub fn hit_test_title_bar(y: i32, scale: f64) -> bool {
    let h = ((TITLE_HIT_LOGICAL_H as f64) * scale).round() as i32;
    y >= 0 && y < h
}

fn lparam_to_point(lparam: LPARAM) -> POINT {
    let v = lparam.0 as u32;
    let x = (v & 0xFFFF) as i16 as i32;
    let y = ((v >> 16) & 0xFFFF) as i16 as i32;
    POINT { x, y }
}

unsafe fn paint_popup(hwnd: HWND, hdc: windows::Win32::Graphics::Gdi::HDC) {
    let mut client = RECT::default();
    let _ = GetClientRect(hwnd, &mut client);

    let brush = CreateSolidBrush(COLORREF(0x00F5F5F5));
    FillRect(hdc, &client, brush);
    let _ = DeleteObject(HGDIOBJ(brush.0));

    SetBkMode(hdc, TRANSPARENT);

    let snap = load_paint_snapshot();
    let scale = window_scale(hwnd);
    let pad = ((12.0_f64) * scale).round() as i32;
    let gap = ((8.0_f64) * scale).round() as i32;
    let toolbar_h = ((TOOLBAR_LOGICAL_H as f64) * scale).round() as i32;
    let mut y = pad;
    let right = client.right - pad;
    let bottom = (client.bottom - toolbar_h - pad).max(y + 1);

    // 源文标题 + 语言摘要
    {
        let lang_hint = if snap.source_lang.is_empty() && snap.target_lang.is_empty() {
            "源文".to_string()
        } else {
            format!(
                "源文  {} → {}",
                if snap.source_lang.is_empty() {
                    "?"
                } else {
                    snap.source_lang.as_str()
                },
                if snap.target_lang.is_empty() {
                    "?"
                } else {
                    snap.target_lang.as_str()
                }
            )
        };
        let mut r = RECT {
            left: pad,
            top: y,
            right,
            bottom: y + 20,
        };
        let _ = draw_text_in_rect(
            hdc,
            &lang_hint,
            &mut r,
            0x00_88_88_88,
            DT_LEFT | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
        );
        y += 20 + 4;
    }

    // 源文正文
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
    } else {
        for card in &snap.cards {
            if y >= bottom {
                break;
            }

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
                    let color =
                        if matches!(card.status, PopupCardStatus::Failed) && card.text.is_empty() {
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

    paint_toolbar(hdc, &client, &snap, scale);
}

unsafe fn paint_toolbar(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    client: &RECT,
    snap: &PaintSnapshot,
    scale: f64,
) {
    let toolbar_h = ((TOOLBAR_LOGICAL_H as f64) * scale).round() as i32;
    let bar = RECT {
        left: client.left,
        top: (client.bottom - toolbar_h).max(client.top),
        right: client.right,
        bottom: client.bottom,
    };
    let bar_brush = CreateSolidBrush(COLORREF(0x00_EE_EE_EE));
    FillRect(hdc, &bar, bar_brush);
    let _ = DeleteObject(HGDIOBJ(bar_brush.0));

    let sep = RECT {
        left: bar.left,
        top: bar.top,
        right: bar.right,
        bottom: bar.top + 1,
    };
    let sep_brush = CreateSolidBrush(COLORREF(0x00_CC_CC_CC));
    FillRect(hdc, &sep, sep_brush);
    let _ = DeleteObject(HGDIOBJ(sep_brush.0));

    for (btn, mut r) in layout_toolbar_buttons(client, snap.is_translating, scale) {
        let btn_brush = CreateSolidBrush(COLORREF(0x00_E0_E0_E0));
        FillRect(hdc, &r, btn_brush);
        let _ = DeleteObject(HGDIOBJ(btn_brush.0));
        let _ = draw_text_in_rect(
            hdc,
            btn.label(),
            &mut r,
            0x00_22_22_22,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
    }
}

fn handle_mouse_click(hwnd: HWND, lparam: LPARAM, double_click: bool) {
    let pt = lparam_to_point(lparam);
    let mut client = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut client);
    }
    let scale = window_scale(hwnd);
    let snap = load_paint_snapshot();

    if double_click && hit_test_title_bar(pt.y, scale) {
        dispatch_bound_action(PopupUserAction::Close);
        return;
    }

    if let Some(btn) = hit_test_toolbar(pt.x, pt.y, &client, snap.is_translating, scale) {
        let action = btn.to_action(&snap);
        if matches!(
            &action,
            PopupUserAction::CopyResult {
                service_instance_id
            } if service_instance_id.is_empty()
        ) {
            log::debug!("复制：当前无结果卡文本");
            return;
        }
        dispatch_bound_action(action);
    }
}

fn handle_keydown(wparam: WPARAM) {
    let vk = wparam.0 as i32;
    if vk == i32::from(VK_ESCAPE.0) {
        dispatch_bound_action(PopupUserAction::Close);
        return;
    }
    if (vk == b'C' as i32 || vk == b'c' as i32)
        && unsafe { GetKeyState(i32::from(VK_CONTROL.0)) } as u16 & 0x8000 != 0
    {
        let snap = load_paint_snapshot();
        if let Some(id) = first_copyable_service_id(&snap) {
            dispatch_bound_action(PopupUserAction::CopyResult {
                service_instance_id: id,
            });
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
        WM_LBUTTONDOWN => {
            handle_mouse_click(hwnd, lparam, false);
            LRESULT(0)
        }
        WM_LBUTTONDBLCLK => {
            handle_mouse_click(hwnd, lparam, true);
            LRESULT(0)
        }
        WM_KEYDOWN => {
            handle_keydown(wparam);
            LRESULT(0)
        }
        WM_CLOSE => {
            // 托盘驻留：关 = hide，不 Destroy
            dispatch_bound_action(PopupUserAction::Close);
            // 保底：handler 未注册或 Host 锁失败时仍隐藏
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
                    SetWindowPos(hwnd, HWND_TOP, x, y, w, h, SWP_SHOWWINDOW)
                        .map_err(|e| format!("SetWindowPos 失败: {e}"))?;
                }
            } else {
                unsafe {
                    let _ = ShowWindow(hwnd, SW_SHOW);
                }
            }
        }
        PopupPositionMode::Restore => unsafe {
            let _ = ShowWindow(hwnd, SW_SHOW);
        },
    }

    // 键盘快捷键需要焦点
    unsafe {
        let _ = SetFocus(hwnd);
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
    use std::sync::Mutex;

    static SNAPSHOT_TEST_LOCK: Mutex<()> = Mutex::new(());

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
        let _guard = SNAPSHOT_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let vm = sample_vm();
        let snap = store_paint_snapshot(&vm);
        assert_eq!(snap.source_text, "Hello world");
        assert_eq!(snap.source_lang, "en");
        assert_eq!(snap.target_lang, "zh");
        assert!(snap.is_translating);
        assert_eq!(snap.cards.len(), 1);
        assert_eq!(snap.cards[0].service_name, "Mock");
        assert_eq!(snap.cards[0].service_instance_id, "svc-1");
        assert_eq!(snap.cards[0].text, "你好");
        assert_eq!(snap.cards[0].status, PopupCardStatus::Translating);

        let loaded = load_paint_snapshot();
        assert_eq!(loaded, snap);
    }

    #[test]
    fn store_paint_snapshot_streaming_overwrite() {
        let _guard = SNAPSHOT_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
        assert!(!enqueue_repaint(0));
    }

    #[test]
    fn status_color_and_label_mapping() {
        assert_eq!(status_label(&PopupCardStatus::Translating), "翻译中");
        assert_eq!(status_label(&PopupCardStatus::Finished), "完成");
        assert_eq!(status_label(&PopupCardStatus::Failed), "失败");
        let t = status_color_bgr(&PopupCardStatus::Translating);
        let f = status_color_bgr(&PopupCardStatus::Finished);
        let e = status_color_bgr(&PopupCardStatus::Failed);
        assert_ne!(t, f);
        assert_ne!(f, e);
        assert_ne!(t, e);
    }

    #[test]
    fn toolbar_includes_cancel_only_when_translating() {
        let client = RECT {
            left: 0,
            top: 0,
            right: 420,
            bottom: 360,
        };
        let idle = layout_toolbar_buttons(&client, false, 1.0);
        assert!(idle.iter().all(|(b, _)| *b != ToolbarButton::Cancel));
        assert!(idle.iter().any(|(b, _)| *b == ToolbarButton::Close));
        assert!(idle.iter().any(|(b, _)| *b == ToolbarButton::Retry));
        assert!(idle.iter().any(|(b, _)| *b == ToolbarButton::Copy));
        assert!(idle.iter().any(|(b, _)| *b == ToolbarButton::Settings));

        let busy = layout_toolbar_buttons(&client, true, 1.0);
        assert!(busy.iter().any(|(b, _)| *b == ToolbarButton::Cancel));
        assert_eq!(busy.len(), idle.len() + 1);
    }

    #[test]
    fn hit_test_toolbar_finds_close() {
        let client = RECT {
            left: 0,
            top: 0,
            right: 420,
            bottom: 360,
        };
        let layout = layout_toolbar_buttons(&client, false, 1.0);
        let (btn, r) = layout
            .iter()
            .find(|(b, _)| *b == ToolbarButton::Close)
            .expect("close");
        assert_eq!(
            hit_test_toolbar(r.left + 2, r.top + 2, &client, false, 1.0),
            Some(*btn)
        );
        assert_eq!(hit_test_toolbar(0, 0, &client, false, 1.0), None);
    }

    #[test]
    fn hit_test_title_bar_top_strip() {
        assert!(hit_test_title_bar(0, 1.0));
        assert!(hit_test_title_bar(20, 1.0));
        assert!(!hit_test_title_bar(40, 1.0));
    }

    #[test]
    fn resolve_copy_and_first_id() {
        let snap = PaintSnapshot {
            source_text: "x".into(),
            source_lang: "en".into(),
            target_lang: "zh".into(),
            is_translating: false,
            cards: vec![
                PaintCardSnapshot {
                    service_instance_id: "a".into(),
                    service_name: "A".into(),
                    status: PopupCardStatus::Pending,
                    text: String::new(),
                    error_message: String::new(),
                },
                PaintCardSnapshot {
                    service_instance_id: "b".into(),
                    service_name: "B".into(),
                    status: PopupCardStatus::Finished,
                    text: "译文".into(),
                    error_message: String::new(),
                },
            ],
        };
        assert_eq!(first_copyable_service_id(&snap).as_deref(), Some("b"));
        assert_eq!(resolve_copy_text(&snap, "b").as_deref(), Some("译文"));
    }

    #[test]
    fn create_hide_destroy_lifecycle() {
        let win = create_hidden_popup().expect("create");
        assert!(win.is_valid());
        assert!(!win.is_visible());

        show_popup(&win, PopupPositionMode::Restore).expect("show restore");
        assert!(win.is_valid());

        hide_popup(&win);
        hide_popup(&win);
        assert!(win.is_valid());

        destroy_popup(&win);
        assert!(!win.is_valid());
        destroy_popup(&win);
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
        let _guard = SNAPSHOT_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let win = create_hidden_popup().expect("create");
        let vm = sample_vm();
        publish_view_model(&win, &vm);
        let loaded = load_paint_snapshot();
        assert_eq!(loaded.source_text, "Hello world");
        assert_eq!(loaded.cards[0].text, "你好");
        assert_eq!(loaded.cards[0].service_instance_id, "svc-1");
        destroy_popup(&win);
    }
}
