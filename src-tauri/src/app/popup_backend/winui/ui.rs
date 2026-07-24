//! 原生弹窗表面（**路径 B：Win32**）。
//!
//! - `WS_POPUP | WS_CLIPCHILDREN`，无系统厚边框
//! - `WS_EX_TOOLWINDOW`：不进任务栏
//! - 逻辑尺寸约 **468×520**（宽对齐 Open Design WinUI3 原型；高度略抬以减滚动压迫感）
//! - DWM 圆角 + 边框色（best-effort）；`CS_DROPSHADOW` 轻阴影
//! - GDI 精修：`RoundRect` 卡片、几何图标、引擎字标、状态点；非 WinUI XAML 控件
//! - 用户动作：Esc 关闭、Ctrl+C 复制、滚轮滚动、标题栏拖动；语言交换与列表选择
//! - 不依赖 Microsoft.UI.Xaml / WinAppSDK
//!
//! 视觉 SSOT：Open Design `popup/winui3`（见 `docs/superpowers/specs/2026-07-24-winui-popup-fluent-align-design.md`）。

use std::sync::{Mutex, Once};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{BOOL, COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DWM_WINDOW_CORNER_PREFERENCE,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreatePen, CreateSolidBrush, DeleteObject, DrawTextW, Ellipse,
    EndPaint, FillRect, IntersectClipRect, InvalidateRect, LineTo, MoveToEx, RestoreDC, RoundRect,
    SaveDC, ScreenToClient, SelectObject, SetBkMode, SetTextColor, CLEARTYPE_QUALITY,
    DEFAULT_CHARSET, DEFAULT_PITCH, DT_CALCRECT, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX,
    DT_RIGHT, DT_SINGLELINE, DT_VCENTER, DT_WORDBREAK, FF_DONTCARE, FW_NORMAL, FW_SEMIBOLD, HFONT,
    HGDIOBJ, PAINTSTRUCT, PS_SOLID, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, SetActiveWindow, SetFocus, VK_CONTROL, VK_ESCAPE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect,
    GetForegroundWindow, GetWindowThreadProcessId, IsWindow, IsWindowVisible, LoadCursorW,
    PostMessageW, RegisterClassExW, SetForegroundWindow, SetWindowPos, ShowWindow, CS_DBLCLKS,
    CS_DROPSHADOW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, HMENU, HTCAPTION, HTCLIENT,
    HWND_NOTOPMOST, HWND_TOP, HWND_TOPMOST, IDC_ARROW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_SHOWWINDOW, SW_HIDE, SW_SHOW, SW_SHOWNOACTIVATE, USER_DEFAULT_SCREEN_DPI, WM_CLOSE,
    WM_DESTROY, WM_KEYDOWN, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_MOUSEWHEEL, WM_NCLBUTTONDBLCLK,
    WM_NCHITTEST, WM_PAINT, WM_USER, WNDCLASSEXW, WS_CLIPCHILDREN, WS_EX_TOOLWINDOW, WS_POPUP,
};

use crate::app::popup_backend::types::{
    PopupCardStatus, PopupPositionMode, PopupUserAction, PopupViewModel,
};
use crate::app::popup_window::{compute_popup_position, LogicalPos, LogicalRect, LogicalSize};
use crate::platform::cursor_logical_context;

// 语言 / 元信息 / 复制：单一事实来源在 `reactor`（路径 R 与 GDI 共用）。
pub use super::reactor::langs::{lang_codes_for_side, lang_display_name, swap_session_langs};
pub use super::reactor::meta::is_machine_translate_protocol;
use super::reactor::meta::{display_model_name, should_show_tokens};
use super::reactor::state::resolve_copy_fields;

/// 用户动作处理器（由 `actions` 在 ensure 时注册）。
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
        log::warn!("原生弹窗未注册动作处理器，忽略: {action:?}");
    }
}

/// 弹窗逻辑宽度（对齐 Open Design WinUI3 原型 468）。
pub const POPUP_LOGICAL_WIDTH: f64 = 468.0;
/// 弹窗逻辑高度上限（卡片区内部滚动；略高于 v1 以减少首屏压迫）。
pub const POPUP_LOGICAL_HEIGHT: f64 = 520.0;

const TITLEBAR_LOGICAL_H: f64 = 44.0;
const STATUS_LOGICAL_H: f64 = 28.0;
const LANG_BAR_LOGICAL_H: f64 = 36.0;
const SOURCE_MAX_LOGICAL_H: f64 = 118.0;
const PAD_LOGICAL: f64 = 14.0;
const GAP_LOGICAL: f64 = 10.0;
const TITLE_BTN_LOGICAL: f64 = 36.0;
const WIN_BTN_LOGICAL_W: f64 = 46.0;
/// 卡片圆角（原型 8px → RoundRect 直径 16）。
const CARD_RADIUS_LOGICAL: f64 = 8.0;

// Fluent / 原型 token（COLORREF = 0x00BBGGRR）
const COL_BG: u32 = 0x00_F4_F4_F4; // #F4F4F4 Mica 实色近似
const COL_CARD_BG: u32 = 0x00_FF_FF_FF;
const COL_FG: u32 = 0x00_1A_1A_1A; // #1A1A1A
const COL_FG_2: u32 = 0x00_5D_5D_5D; // #5D5D5D
const COL_FG_3: u32 = 0x00_8A_8A_8A; // #8A8A8A
const COL_BORDER: u32 = 0x00_F0_F0_F0; // ≈ rgba(0,0,0,0.06)
const COL_BORDER_2: u32 = 0x00_E0_E0_E0;
const COL_ACCENT: u32 = 0x00_1F_5A_D5; // #D55A1F 柿子橙
const COL_ACCENT_SOFT: u32 = 0x00_E4_EB_F8; // #F8EBE4 ≈ accent 10%
const COL_SUCCESS: u32 = 0x00_10_7C_10;
const COL_WARNING: u32 = 0x00_10_50_CA;
const COL_DANGER: u32 = 0x00_1C_2B_C4; // #C42B1C
const COL_SCROLL_TRACK: u32 = 0x00_EE_EE_EE;
const COL_SCROLL_THUMB: u32 = 0x00_C8_C8_C8;
const COL_HOVER: u32 = 0x00_F5_F5_F5;
const COL_FLYOUT_BG: u32 = 0x00_F9_F9_F9;
const COL_STATUS_ACTION: u32 = COL_ACCENT;
const COL_SHADOW: u32 = 0x00_E8_E8_E8;
const COL_ICON_BADGE: u32 = 0x00_F0_F0_F0;

const CLASS_NAME: PCWSTR = w!("Shizi.NativePopup.B");
const WM_POPUP_REFRESH: u32 = WM_USER + 0x51;

/// 可点击热区（标题栏 / 语言栏 / 状态栏 / 语言列表）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeHit {
    Close,
    Minimize,
    Settings,
    Theme,
    Bookmark,
    Shot,
    Fav,
    Pin,
    Cancel,
    Retry,
    Copy,
    LangSource,
    LangTarget,
    LangSwap,
    /// 语言列表项（索引进 `lang_codes_for_side`）。
    LangItem(usize),
}

/// 单卡渲染快照。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaintCardSnapshot {
    pub service_instance_id: String,
    pub service_name: String,
    pub protocol: String,
    pub model_name: String,
    pub status: PopupCardStatus,
    pub text: String,
    pub error_message: String,
    pub usage_input: Option<u32>,
    pub usage_output: Option<u32>,
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
                    protocol: c.protocol.clone(),
                    model_name: c.model_name.clone(),
                    status: c.status.clone(),
                    text: c.text.clone(),
                    error_message: c.error_message.clone(),
                    usage_input: c.usage_input,
                    usage_output: c.usage_output,
                })
                .collect(),
        }
    }
}

static PAINT_SNAPSHOT: Mutex<PaintSnapshot> = Mutex::new(PaintSnapshot {
    source_text: String::new(),
    source_lang: String::new(),
    target_lang: String::new(),
    is_translating: false,
    cards: Vec::new(),
});

static CARD_SCROLL_Y: Mutex<i32> = Mutex::new(0);
static CARD_SCROLL_METRICS: Mutex<(i32, i32)> = Mutex::new((0, 0));

/// 语言列表：`None` 关闭；`Some(true)` 源语言；`Some(false)` 目标语言。
static LANG_FLYOUT_SOURCE: Mutex<Option<bool>> = Mutex::new(None);
static LANG_FLYOUT_SCROLL: Mutex<i32> = Mutex::new(0);

/// 最近一次 paint 的布局锚点（命中与绘制同源）。
#[derive(Debug, Clone, Copy, Default)]
struct LayoutAnchors {
    lang_bar_top: i32,
    source_block_h: i32,
}

static LAYOUT_ANCHORS: Mutex<LayoutAnchors> = Mutex::new(LayoutAnchors {
    lang_bar_top: 0,
    source_block_h: 0,
});

fn store_layout_anchors(lang_bar_top: i32, source_block_h: i32) {
    if let Ok(mut g) = LAYOUT_ANCHORS.lock() {
        *g = LayoutAnchors {
            lang_bar_top,
            source_block_h,
        };
    }
}

fn load_layout_anchors() -> LayoutAnchors {
    LAYOUT_ANCHORS
        .lock()
        .map(|g| *g)
        .unwrap_or_default()
}

pub fn card_detail_label(protocol: &str, model_name: &str) -> String {
    // GDI 专用：MT 显示协议名（与路径 R `display_model_name` 不同，勿强行统一）。
    if is_machine_translate_protocol(protocol) {
        let p = protocol.trim();
        if p.is_empty() {
            String::new()
        } else {
            p.to_string()
        }
    } else {
        let m = model_name.trim();
        if !m.is_empty() && m != "—" && m != "-" {
            m.to_string()
        } else {
            let p = protocol.trim();
            if p.is_empty() {
                String::new()
            } else {
                p.to_string()
            }
        }
    }
}

pub fn format_card_header(card: &PaintCardSnapshot) -> String {
    let name = if card.service_name.is_empty() {
        "服务"
    } else {
        card.service_name.as_str()
    };
    let detail = card_detail_label(&card.protocol, &card.model_name);
    let status = status_label(&card.status);
    if detail.is_empty() {
        format!("{name}  ·  {status}")
    } else {
        format!("{name}  ·  {detail}  ·  {status}")
    }
}

pub fn card_scroll_offset() -> i32 {
    CARD_SCROLL_Y.lock().map(|g| *g).unwrap_or(0)
}

pub fn clamp_card_scroll(offset: i32, content_h: i32, viewport_h: i32) -> i32 {
    let max = (content_h - viewport_h).max(0);
    offset.clamp(0, max)
}

fn set_card_scroll_offset(y: i32) {
    if let Ok(mut g) = CARD_SCROLL_Y.lock() {
        *g = y.max(0);
    }
}

fn reset_card_scroll() {
    set_card_scroll_offset(0);
}

fn store_scroll_metrics(content_h: i32, viewport_h: i32) {
    if let Ok(mut g) = CARD_SCROLL_METRICS.lock() {
        *g = (content_h, viewport_h);
    }
}

fn load_scroll_metrics() -> (i32, i32) {
    CARD_SCROLL_METRICS
        .lock()
        .map(|g| *g)
        .unwrap_or((0, 0))
}

pub fn adjust_card_scroll(delta_px: i32) -> i32 {
    let (content_h, viewport_h) = load_scroll_metrics();
    let next = clamp_card_scroll(card_scroll_offset() + delta_px, content_h, viewport_h);
    set_card_scroll_offset(next);
    next
}

/// 语言列表是否打开；`true`=源，`false`=目标。
pub fn lang_flyout_side() -> Option<bool> {
    LANG_FLYOUT_SOURCE.lock().ok().and_then(|g| *g)
}

pub fn set_lang_flyout(side: Option<bool>) {
    if let Ok(mut g) = LANG_FLYOUT_SOURCE.lock() {
        *g = side;
    }
    if let Ok(mut s) = LANG_FLYOUT_SCROLL.lock() {
        *s = 0;
    }
}

fn lang_flyout_scroll() -> i32 {
    LANG_FLYOUT_SCROLL.lock().map(|g| *g).unwrap_or(0)
}

fn set_lang_flyout_scroll(y: i32) {
    if let Ok(mut g) = LANG_FLYOUT_SCROLL.lock() {
        *g = y.max(0);
    }
}

pub fn store_paint_snapshot(vm: &PopupViewModel) -> PaintSnapshot {
    let snap = PaintSnapshot::from_view_model(vm);
    if let Ok(mut guard) = PAINT_SNAPSHOT.lock() {
        let source_changed = guard.source_text != snap.source_text;
        let cards_identity_changed = guard.cards.len() != snap.cards.len()
            || guard
                .cards
                .iter()
                .zip(snap.cards.iter())
                .any(|(a, b)| a.service_instance_id != b.service_instance_id);
        if source_changed || cards_identity_changed {
            reset_card_scroll();
        }
        *guard = snap.clone();
    }
    snap
}

pub fn load_paint_snapshot() -> PaintSnapshot {
    PAINT_SNAPSHOT
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default()
}

/// GDI PaintSnapshot 薄适配 → `reactor::state` 字段级纯函数。
pub fn resolve_copy_text(snap: &PaintSnapshot, service_instance_id: &str) -> Option<String> {
    let card = snap
        .cards
        .iter()
        .find(|c| c.service_instance_id == service_instance_id)?;
    resolve_copy_fields(&card.text, &card.error_message)
}

/// GDI PaintSnapshot 薄适配（语义与 `reactor::state::first_copyable_service_id` 对齐）。
pub fn first_copyable_service_id(snap: &PaintSnapshot) -> Option<String> {
    snap.cards.iter().find_map(|c| {
        if !c.text.trim().is_empty() {
            Some(c.service_instance_id.clone())
        } else {
            None
        }
    })
}

pub fn status_label(status: &PopupCardStatus) -> &'static str {
    match status {
        PopupCardStatus::Pending => "等待",
        PopupCardStatus::Translating => "翻译中",
        PopupCardStatus::Finished => "完成",
        PopupCardStatus::Failed => "失败",
        PopupCardStatus::Cancelled => "已取消",
    }
}

pub fn status_color_bgr(status: &PopupCardStatus) -> u32 {
    match status {
        PopupCardStatus::Pending => COL_FG_3,
        PopupCardStatus::Translating => COL_WARNING,
        PopupCardStatus::Finished => COL_SUCCESS,
        PopupCardStatus::Failed => COL_DANGER,
        PopupCardStatus::Cancelled => COL_FG_3,
    }
}

unsafe fn create_ui_font(height_px: i32, weight: i32) -> HFONT {
    CreateFontW(
        -height_px.abs(),
        0,
        0,
        0,
        weight,
        0,
        0,
        0,
        DEFAULT_CHARSET.0 as u32,
        0,
        0,
        CLEARTYPE_QUALITY.0 as u32,
        DEFAULT_PITCH.0 as u32 | (FF_DONTCARE.0 as u32),
        w!("Segoe UI"),
    )
}

fn font_px(logical_pt: f64, scale: f64) -> i32 {
    ((logical_pt * scale).round() as i32).max(11)
}

struct UiFonts {
    caption: HFONT,
    body: HFONT,
    body_semibold: HFONT,
    small: HFONT,
    /// ~10.3px 引擎名 / 状态栏（对齐原型 0.6875rem）
    micro: HFONT,
}

impl UiFonts {
    unsafe fn create(scale: f64) -> Self {
        Self {
            caption: create_ui_font(font_px(12.0, scale), FW_NORMAL.0 as i32),
            body: create_ui_font(font_px(12.2, scale), FW_NORMAL.0 as i32),
            body_semibold: create_ui_font(font_px(13.0, scale), FW_SEMIBOLD.0 as i32),
            small: create_ui_font(font_px(11.0, scale), FW_NORMAL.0 as i32),
            micro: create_ui_font(font_px(10.3, scale), FW_NORMAL.0 as i32),
        }
    }

    unsafe fn destroy(self) {
        for f in [
            self.caption,
            self.body,
            self.body_semibold,
            self.small,
            self.micro,
        ] {
            if !f.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(f.0));
            }
        }
    }
}

unsafe fn select_font(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    font: HFONT,
) -> HGDIOBJ {
    if font.is_invalid() {
        return HGDIOBJ(std::ptr::null_mut());
    }
    SelectObject(hdc, HGDIOBJ(font.0))
}

unsafe fn fill_solid_rect(hdc: windows::Win32::Graphics::Gdi::HDC, rect: &RECT, color: u32) {
    let brush = CreateSolidBrush(COLORREF(color));
    FillRect(hdc, rect, brush);
    let _ = DeleteObject(HGDIOBJ(brush.0));
}

/// 圆角填充 + 1px 描边（对齐原型 8px 圆角卡片）。
unsafe fn fill_round_card(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    rect: &RECT,
    fill: u32,
    border: u32,
    scale: f64,
) {
    let dia = ((CARD_RADIUS_LOGICAL * 2.0) * scale).round().max(4.0) as i32;
    // 轻阴影：下移 1px 灰底（近似 box-shadow 0 1.6px）
    let shadow = RECT {
        left: rect.left + 1,
        top: rect.top + 1,
        right: rect.right + 1,
        bottom: rect.bottom + 1,
    };
    let brush_s = CreateSolidBrush(COLORREF(COL_SHADOW));
    let pen_null = CreatePen(PS_SOLID, 1, COLORREF(COL_SHADOW));
    let old_b = SelectObject(hdc, HGDIOBJ(brush_s.0));
    let old_p = SelectObject(hdc, HGDIOBJ(pen_null.0));
    let _ = RoundRect(
        hdc,
        shadow.left,
        shadow.top,
        shadow.right,
        shadow.bottom,
        dia,
        dia,
    );
    let _ = SelectObject(hdc, old_b);
    let _ = SelectObject(hdc, old_p);
    let _ = DeleteObject(HGDIOBJ(brush_s.0));
    let _ = DeleteObject(HGDIOBJ(pen_null.0));

    let brush = CreateSolidBrush(COLORREF(fill));
    let pen = CreatePen(PS_SOLID, 1, COLORREF(border));
    let old_b = SelectObject(hdc, HGDIOBJ(brush.0));
    let old_p = SelectObject(hdc, HGDIOBJ(pen.0));
    let _ = RoundRect(hdc, rect.left, rect.top, rect.right, rect.bottom, dia, dia);
    let _ = SelectObject(hdc, old_b);
    let _ = SelectObject(hdc, old_p);
    let _ = DeleteObject(HGDIOBJ(brush.0));
    let _ = DeleteObject(HGDIOBJ(pen.0));
}

unsafe fn with_stroke_pen<F>(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    color: u32,
    width: i32,
    f: F,
) where
    F: FnOnce(),
{
    let pen = CreatePen(PS_SOLID, width.max(1), COLORREF(color));
    let old = SelectObject(hdc, HGDIOBJ(pen.0));
    f();
    let _ = SelectObject(hdc, old);
    let _ = DeleteObject(HGDIOBJ(pen.0));
}

unsafe fn line_to(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
) {
    let _ = MoveToEx(hdc, x1, y1, None);
    let _ = LineTo(hdc, x2, y2);
}

unsafe fn fill_ellipse(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    color: u32,
) {
    let brush = CreateSolidBrush(COLORREF(color));
    let pen = CreatePen(PS_SOLID, 1, COLORREF(color));
    let old_b = SelectObject(hdc, HGDIOBJ(brush.0));
    let old_p = SelectObject(hdc, HGDIOBJ(pen.0));
    let _ = Ellipse(hdc, left, top, right, bottom);
    let _ = SelectObject(hdc, old_b);
    let _ = SelectObject(hdc, old_p);
    let _ = DeleteObject(HGDIOBJ(brush.0));
    let _ = DeleteObject(HGDIOBJ(pen.0));
}

/// 在按钮矩形中心绘制 18px 逻辑图标（几何线段，非文字）。
unsafe fn paint_chrome_icon(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    hit: ChromeHit,
    rect: &RECT,
    scale: f64,
    color: u32,
) {
    let cx = (rect.left + rect.right) / 2;
    let cy = (rect.top + rect.bottom) / 2;
    let s = ((9.0_f64) * scale).round().max(7.0) as i32; // half-size
    let w = ((1.6_f64) * scale).round().max(1.0) as i32;

    with_stroke_pen(hdc, color, w, || match hit {
        ChromeHit::Close => {
            line_to(hdc, cx - s + 2, cy - s + 2, cx + s - 2, cy + s - 2);
            line_to(hdc, cx + s - 2, cy - s + 2, cx - s + 2, cy + s - 2);
        }
        ChromeHit::Minimize => {
            line_to(hdc, cx - s + 1, cy, cx + s - 1, cy);
        }
        ChromeHit::Settings => {
            // 齿轮简化：十字 + 对角 + 中心点
            let r = s - 1;
            line_to(hdc, cx, cy - r, cx, cy + r);
            line_to(hdc, cx - r, cy, cx + r, cy);
            line_to(
                hdc,
                cx - r * 7 / 10,
                cy - r * 7 / 10,
                cx + r * 7 / 10,
                cy + r * 7 / 10,
            );
            line_to(
                hdc,
                cx + r * 7 / 10,
                cy - r * 7 / 10,
                cx - r * 7 / 10,
                cy + r * 7 / 10,
            );
        }
        ChromeHit::Theme => {
            // 新月：大半圆弦
            line_to(hdc, cx + s / 3, cy - s + 1, cx + s / 3, cy + s - 1);
            line_to(hdc, cx + s / 3, cy - s + 1, cx - s + 2, cy);
            line_to(hdc, cx + s / 3, cy + s - 1, cx - s + 2, cy);
            line_to(hdc, cx - s + 2, cy - s / 2, cx - s + 2, cy + s / 2);
        }
        ChromeHit::Bookmark => {
            // 书签：竖折
            line_to(hdc, cx - s + 2, cy - s + 1, cx - s + 2, cy + s - 1);
            line_to(hdc, cx + s - 2, cy - s + 1, cx + s - 2, cy + s - 1);
            line_to(hdc, cx - s + 2, cy - s + 1, cx + s - 2, cy - s + 1);
            line_to(hdc, cx - s + 2, cy + s - 1, cx, cy + s / 3);
            line_to(hdc, cx + s - 2, cy + s - 1, cx, cy + s / 3);
        }
        ChromeHit::Shot => {
            // 框选四角
            let a = s - 1;
            let t = (s / 2).max(3);
            line_to(hdc, cx - a, cy - a, cx - a + t, cy - a);
            line_to(hdc, cx - a, cy - a, cx - a, cy - a + t);
            line_to(hdc, cx + a, cy - a, cx + a - t, cy - a);
            line_to(hdc, cx + a, cy - a, cx + a, cy - a + t);
            line_to(hdc, cx - a, cy + a, cx - a + t, cy + a);
            line_to(hdc, cx - a, cy + a, cx - a, cy + a - t);
            line_to(hdc, cx + a, cy + a, cx + a - t, cy + a);
            line_to(hdc, cx + a, cy + a, cx + a, cy + a - t);
            line_to(hdc, cx - a / 2, cy, cx + a / 2, cy);
        }
        ChromeHit::Fav => {
            // 五角星简化为菱形 + 顶
            line_to(hdc, cx, cy - s + 1, cx + s - 2, cy);
            line_to(hdc, cx + s - 2, cy, cx, cy + s - 1);
            line_to(hdc, cx, cy + s - 1, cx - s + 2, cy);
            line_to(hdc, cx - s + 2, cy, cx, cy - s + 1);
            line_to(hdc, cx - s / 2, cy - 1, cx + s / 2, cy - 1);
        }
        ChromeHit::Pin => {
            // 图钉：头 + 针
            line_to(hdc, cx - s + 3, cy - s / 3, cx + s - 3, cy - s / 3);
            line_to(hdc, cx - s + 3, cy - s / 3, cx - s + 3, cy + s / 4);
            line_to(hdc, cx + s - 3, cy - s / 3, cx + s - 3, cy + s / 4);
            line_to(hdc, cx - s + 3, cy + s / 4, cx + s - 3, cy + s / 4);
            line_to(hdc, cx, cy + s / 4, cx, cy + s - 1);
        }
        _ => {}
    });
}

unsafe fn paint_chevron_down(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    cx: i32,
    cy: i32,
    scale: f64,
    color: u32,
) {
    let s = ((5.0_f64) * scale).round().max(3.0) as i32;
    let w = ((1.5_f64) * scale).round().max(1.0) as i32;
    with_stroke_pen(hdc, color, w, || {
        line_to(hdc, cx - s, cy - s / 2, cx, cy + s / 2);
        line_to(hdc, cx, cy + s / 2, cx + s, cy - s / 2);
    });
}

unsafe fn paint_swap_icon(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    cx: i32,
    cy: i32,
    scale: f64,
    color: u32,
) {
    let s = ((7.0_f64) * scale).round().max(5.0) as i32;
    let w = ((1.5_f64) * scale).round().max(1.0) as i32;
    with_stroke_pen(hdc, color, w, || {
        // 左箭头
        line_to(hdc, cx - s, cy, cx + s / 3, cy);
        line_to(hdc, cx - s, cy, cx - s / 2, cy - s / 2);
        line_to(hdc, cx - s, cy, cx - s / 2, cy + s / 2);
        // 右箭头
        line_to(hdc, cx + s, cy, cx - s / 3, cy);
        line_to(hdc, cx + s, cy, cx + s / 2, cy - s / 2);
        line_to(hdc, cx + s, cy, cx + s / 2, cy + s / 2);
    });
}

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

pub fn publish_view_model(window: &NativePopupHwnd, vm: &PopupViewModel) {
    let _ = store_paint_snapshot(vm);
    if window.is_valid() {
        let _ = enqueue_repaint(window.raw);
    }
}

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
        style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS | CS_DROPSHADOW,
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

unsafe fn measure_text_height(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    text: &str,
    width: i32,
    flags: windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT,
) -> i32 {
    if text.is_empty() || width <= 0 {
        return 0;
    }
    let mut r = RECT {
        left: 0,
        top: 0,
        right: width,
        bottom: 0,
    };
    let mut buf = to_utf16(text);
    let h = DrawTextW(hdc, &mut buf, &mut r, flags | DT_CALCRECT);
    if h > 0 {
        h
    } else {
        (r.bottom - r.top).max(0)
    }
}

pub fn card_body_text(card: &PaintCardSnapshot) -> String {
    if !card.text.is_empty() {
        return card.text.clone();
    }
    match card.status {
        PopupCardStatus::Translating | PopupCardStatus::Pending => "…".to_string(),
        PopupCardStatus::Failed => {
            if card.error_message.is_empty() {
                "翻译失败".to_string()
            } else {
                card.error_message.clone()
            }
        }
        _ => String::new(),
    }
}

pub fn card_extra_error(card: &PaintCardSnapshot) -> Option<&str> {
    if matches!(card.status, PopupCardStatus::Failed)
        && !card.error_message.is_empty()
        && !card.text.is_empty()
    {
        Some(card.error_message.as_str())
    } else {
        None
    }
}

fn model_tag_for_card(card: &PaintCardSnapshot) -> String {
    display_model_name(&card.protocol, &card.model_name)
}

fn tokens_label(card: &PaintCardSnapshot) -> String {
    let has_usage = card.usage_input.is_some() || card.usage_output.is_some();
    if !should_show_tokens(&card.protocol, has_usage) {
        return String::new();
    }
    match (card.usage_input, card.usage_output) {
        (Some(i), Some(o)) => format!("↑{i} ↓{o}"),
        (Some(i), None) => format!("↑{i}"),
        (None, Some(o)) => format!("↓{o}"),
        _ => String::new(),
    }
}

unsafe fn measure_card_height(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    card: &PaintCardSnapshot,
    content_w: i32,
    scale: f64,
) -> i32 {
    let gap = ((GAP_LOGICAL) * scale).round() as i32;
    let card_pad_x = ((12.0_f64) * scale).round() as i32;
    let card_pad_y = ((8.0_f64) * scale).round() as i32;
    let header_h = ((22.0_f64) * scale).round() as i32;
    let text_w = (content_w - card_pad_x * 2).max(1);
    let mut h = card_pad_y + header_h + 2;

    let body = card_body_text(card);
    if !body.is_empty() {
        let max_body = ((120.0_f64) * scale).round() as i32;
        let bh = measure_text_height(hdc, &body, text_w, DT_LEFT | DT_WORDBREAK | DT_NOPREFIX)
            .max(((16.0_f64) * scale).round() as i32)
            .min(max_body);
        h += bh + gap / 2;
    }

    if let Some(err) = card_extra_error(card) {
        let max_err = ((40.0_f64) * scale).round() as i32;
        let eh = measure_text_height(hdc, err, text_w, DT_LEFT | DT_WORDBREAK | DT_NOPREFIX)
            .max(((14.0_f64) * scale).round() as i32)
            .min(max_err);
        h += eh + gap / 2;
    }

    // meta 行（model / tokens）
    if !model_tag_for_card(card).is_empty() || !tokens_label(card).is_empty() {
        h += ((16.0_f64) * scale).round() as i32;
    }

    h + card_pad_y + gap
}

fn content_pad(scale: f64) -> i32 {
    ((PAD_LOGICAL) * scale).round() as i32
}

fn content_gap(scale: f64) -> i32 {
    ((GAP_LOGICAL) * scale).round() as i32
}

fn titlebar_h(scale: f64) -> i32 {
    (TITLEBAR_LOGICAL_H * scale).round() as i32
}

fn status_h(scale: f64) -> i32 {
    (STATUS_LOGICAL_H * scale).round() as i32
}

fn lang_bar_h(scale: f64) -> i32 {
    (LANG_BAR_LOGICAL_H * scale).round() as i32
}

/// 标题栏按钮布局（从右到左：关闭、最小化 | 主题、设置、书签、截图、收藏；左侧钉）。
pub fn layout_titlebar_buttons(client: &RECT, scale: f64) -> Vec<(ChromeHit, RECT)> {
    let th = titlebar_h(scale);
    let btn = (TITLE_BTN_LOGICAL * scale).round() as i32;
    let win_w = (WIN_BTN_LOGICAL_W * scale).round() as i32;
    let left_pad = content_pad(scale);
    let brand_w = ((72.0_f64) * scale).round() as i32;

    let mut out = Vec::new();
    // 关闭
    let mut x = client.right - win_w;
    out.push((
        ChromeHit::Close,
        RECT {
            left: x,
            top: 0,
            right: client.right,
            bottom: th,
        },
    ));
    // 最小化
    x -= win_w;
    out.push((
        ChromeHit::Minimize,
        RECT {
            left: x,
            top: 0,
            right: x + win_w,
            bottom: th,
        },
    ));
    // 右侧工具：主题 设置 书签 截图 收藏
    let tools = [
        ChromeHit::Theme,
        ChromeHit::Settings,
        ChromeHit::Bookmark,
        ChromeHit::Shot,
        ChromeHit::Fav,
    ];
    x -= ((4.0_f64) * scale).round() as i32;
    for hit in tools {
        x -= btn;
        out.push((
            hit,
            RECT {
                left: x,
                top: (th - btn) / 2,
                right: x + btn,
                bottom: (th - btn) / 2 + btn,
            },
        ));
    }
    // 钉在品牌右侧
    let pin_left = left_pad + brand_w;
    out.push((
        ChromeHit::Pin,
        RECT {
            left: pin_left,
            top: (th - btn) / 2,
            right: pin_left + btn,
            bottom: (th - btn) / 2 + btn,
        },
    ));
    out
}

/// 语言栏三区：源 | 交换 | 目标。
pub fn layout_lang_bar(client: &RECT, scale: f64, bar_top: i32) -> Vec<(ChromeHit, RECT)> {
    let pad = content_pad(scale);
    let h = lang_bar_h(scale);
    let swap_w = ((38.0_f64) * scale).round() as i32;
    let left = pad;
    let right = client.right - pad;
    let side_w = ((right - left - swap_w) / 2).max(1);
    let swap_left = left + side_w;
    let target_left = swap_left + swap_w;
    vec![
        (
            ChromeHit::LangSource,
            RECT {
                left,
                top: bar_top,
                right: swap_left,
                bottom: bar_top + h,
            },
        ),
        (
            ChromeHit::LangSwap,
            RECT {
                left: swap_left,
                top: bar_top,
                right: target_left,
                bottom: bar_top + h,
            },
        ),
        (
            ChromeHit::LangTarget,
            RECT {
                left: target_left,
                top: bar_top,
                right,
                bottom: bar_top + h,
            },
        ),
    ]
}

/// 状态栏动作（取消 / 重试 / 复制）。
pub fn layout_status_actions(
    client: &RECT,
    is_translating: bool,
    has_failure: bool,
    scale: f64,
) -> Vec<(ChromeHit, RECT)> {
    let sh = status_h(scale);
    let pad = content_pad(scale);
    let top = client.bottom - sh;
    let btn_w = ((40.0_f64) * scale).round() as i32;
    let gap = ((8.0_f64) * scale).round() as i32;
    let mut x = pad + ((72.0_f64) * scale).round() as i32; // 状态文案右侧
    let mut out = Vec::new();
    if is_translating {
        out.push((
            ChromeHit::Cancel,
            RECT {
                left: x,
                top,
                right: x + btn_w,
                bottom: client.bottom,
            },
        ));
        x += btn_w + gap;
    } else if has_failure {
        out.push((
            ChromeHit::Retry,
            RECT {
                left: x,
                top,
                right: x + btn_w,
                bottom: client.bottom,
            },
        ));
        x += btn_w + gap;
    }
    out.push((
        ChromeHit::Copy,
        RECT {
            left: x,
            top,
            right: x + btn_w,
            bottom: client.bottom,
        },
    ));
    out
}

/// 语言列表矩形与条目高度。
pub fn flyout_metrics(client: &RECT, scale: f64, lang_bar_top: i32) -> (RECT, i32) {
    let pad = content_pad(scale);
    let item_h = ((28.0_f64) * scale).round() as i32;
    let max_h = ((280.0_f64) * scale).round() as i32;
    let w = ((252.0_f64) * scale).round() as i32;
    let gap = ((6.0_f64) * scale).round() as i32;
    let top = lang_bar_top + lang_bar_h(scale) + gap;
    let left = if lang_flyout_side() == Some(false) {
        (client.right - pad - w).max(pad)
    } else {
        pad
    };
    let bottom = (top + max_h).min(client.bottom - status_h(scale) - 4);
    (
        RECT {
            left,
            top,
            right: left + w,
            bottom,
        },
        item_h,
    )
}

pub fn layout_flyout_items(
    client: &RECT,
    scale: f64,
    lang_bar_top: i32,
    is_source: bool,
) -> Vec<(ChromeHit, RECT)> {
    let codes = lang_codes_for_side(is_source);
    let (fly, item_h) = flyout_metrics(client, scale, lang_bar_top);
    let scroll = lang_flyout_scroll();
    let mut out = Vec::new();
    let mut y = fly.top + 4 - scroll;
    for (idx, _) in codes.iter().enumerate() {
        let r = RECT {
            left: fly.left + 4,
            top: y,
            right: fly.right - 4,
            bottom: y + item_h,
        };
        if r.bottom > fly.top && r.top < fly.bottom {
            out.push((ChromeHit::LangItem(idx), r));
        }
        y += item_h;
    }
    out
}

/// 计算语言栏 top（与 paint 一致；无锚点缓存时用估算）。
pub fn compute_lang_bar_top(scale: f64, source_block_h: i32) -> i32 {
    let th = titlebar_h(scale);
    let pad = content_pad(scale);
    let gap = content_gap(scale);
    th + pad + source_block_h + gap
}

fn resolve_lang_bar_top(scale: f64, source_text: &str) -> i32 {
    let anchors = load_layout_anchors();
    if anchors.lang_bar_top > 0 {
        anchors.lang_bar_top
    } else {
        let source_h = if anchors.source_block_h > 0 {
            anchors.source_block_h
        } else {
            estimate_source_block_h(scale, source_text)
        };
        compute_lang_bar_top(scale, source_h)
    }
}

/// 估算源文块高度（无 HDC 时用行数近似；有 HDC 时在 paint 内实测）。
fn estimate_source_block_h(scale: f64, source_text: &str) -> i32 {
    let max_h = (SOURCE_MAX_LOGICAL_H * scale).round() as i32;
    let min_h = ((48.0_f64) * scale).round() as i32;
    let lines = source_text.lines().count().max(1).min(6);
    let h = ((lines as f64 * 18.0 + 36.0) * scale).round() as i32;
    h.clamp(min_h, max_h)
}

fn pt_in_rect(x: i32, y: i32, r: &RECT) -> bool {
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}

/// 综合命中测试。
pub fn hit_test_chrome(
    x: i32,
    y: i32,
    client: &RECT,
    snap: &PaintSnapshot,
    scale: f64,
) -> Option<ChromeHit> {
    let bar_top = resolve_lang_bar_top(scale, &snap.source_text);

    // 语言列表优先
    if let Some(is_source) = lang_flyout_side() {
        let (fly, _) = flyout_metrics(client, scale, bar_top);
        if pt_in_rect(x, y, &fly) {
            for (hit, r) in layout_flyout_items(client, scale, bar_top, is_source) {
                if pt_in_rect(x, y, &r) {
                    return Some(hit);
                }
            }
            return None; // 列表空白区吞点击
        }
    }

    for (hit, r) in layout_titlebar_buttons(client, scale) {
        if pt_in_rect(x, y, &r) {
            return Some(hit);
        }
    }

    for (hit, r) in layout_lang_bar(client, scale, bar_top) {
        if pt_in_rect(x, y, &r) {
            return Some(hit);
        }
    }

    let has_failure = snap
        .cards
        .iter()
        .any(|c| matches!(c.status, PopupCardStatus::Failed));
    for (hit, r) in layout_status_actions(client, snap.is_translating, has_failure, scale) {
        if pt_in_rect(x, y, &r) {
            return Some(hit);
        }
    }

    None
}

/// 标题拖动区：顶部条且未落在按钮上。
pub fn hit_test_title_bar_drag(x: i32, y: i32, client: &RECT, scale: f64) -> bool {
    let th = titlebar_h(scale);
    if y < 0 || y >= th {
        return false;
    }
    for (_, r) in layout_titlebar_buttons(client, scale) {
        if pt_in_rect(x, y, &r) {
            return false;
        }
    }
    true
}

pub fn hit_test_title_bar(y: i32, scale: f64) -> bool {
    y >= 0 && y < titlebar_h(scale)
}

pub fn hit_test_title_bar_screen(hwnd: HWND, screen_x: i32, screen_y: i32) -> bool {
    let mut pt = POINT {
        x: screen_x,
        y: screen_y,
    };
    unsafe {
        if !ScreenToClient(hwnd, &mut pt).as_bool() {
            return false;
        }
    }
    let mut client = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut client);
    }
    hit_test_title_bar_drag(pt.x, pt.y, &client, window_scale(hwnd))
}

/// 兼容旧测试命名：翻译中状态栏含 Cancel。
pub fn layout_toolbar_buttons(
    client: &RECT,
    is_translating: bool,
    scale: f64,
) -> Vec<(ChromeHit, RECT)> {
    layout_status_actions(client, is_translating, !is_translating, scale)
}

pub fn hit_test_toolbar(
    x: i32,
    y: i32,
    client: &RECT,
    is_translating: bool,
    scale: f64,
) -> Option<ChromeHit> {
    layout_toolbar_buttons(client, is_translating, scale)
        .into_iter()
        .find(|(_, r)| pt_in_rect(x, y, r))
        .map(|(h, _)| h)
}

unsafe fn paint_one_card(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    card: &PaintCardSnapshot,
    left: i32,
    right: i32,
    top: i32,
    block_h: i32,
    scale: f64,
    gap: i32,
    fonts: &UiFonts,
) {
    let card_pad_x = ((12.0_f64) * scale).round() as i32;
    let card_pad_y = ((8.0_f64) * scale).round() as i32;
    let face_bottom = (top + block_h - gap / 2).max(top + 1);
    let face = RECT {
        left,
        top,
        right,
        bottom: face_bottom,
    };
    fill_round_card(hdc, &face, COL_CARD_BG, COL_BORDER, scale);

    let text_left = left + card_pad_x;
    let text_right = right - card_pad_x;
    let mut y = top + card_pad_y;

    let name = if card.service_name.is_empty() {
        "服务"
    } else {
        card.service_name.as_str()
    };
    let header_h = ((22.0_f64) * scale).round() as i32;
    let badge = ((14.0_f64) * scale).round() as i32;
    // 引擎字标圆角方块
    {
        let letter = name
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_else(|| "?".into());
        let br = RECT {
            left: text_left,
            top: y + (header_h - badge) / 2,
            right: text_left + badge,
            bottom: y + (header_h - badge) / 2 + badge,
        };
        fill_ellipse(hdc, br.left, br.top, br.right, br.bottom, COL_ICON_BADGE);
        let old = select_font(hdc, fonts.micro);
        let mut tr = br;
        let _ = draw_text_in_rect(
            hdc,
            &letter,
            &mut tr,
            COL_FG_2,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        if !old.0.is_null() {
            let _ = SelectObject(hdc, old);
        }
    }
    {
        let name_left = text_left + badge + ((6.0_f64) * scale).round() as i32;
        let old = select_font(hdc, fonts.micro);
        let mut r = RECT {
            left: name_left,
            top: y,
            right: text_right - ((48.0_f64) * scale).round() as i32,
            bottom: y + header_h,
        };
        let _ = draw_text_in_rect(
            hdc,
            name,
            &mut r,
            COL_FG_2,
            DT_LEFT | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS | DT_VCENTER,
        );
        // 翻译中：橙点；失败：红点；完成不画点
        match card.status {
            PopupCardStatus::Translating | PopupCardStatus::Pending => {
                let d = ((6.0_f64) * scale).round() as i32;
                let dx = text_right - d - 2;
                let dy = y + (header_h - d) / 2;
                fill_ellipse(hdc, dx, dy, dx + d, dy + d, COL_ACCENT);
            }
            PopupCardStatus::Failed => {
                let d = ((6.0_f64) * scale).round() as i32;
                let dx = text_right - d - 2;
                let dy = y + (header_h - d) / 2;
                fill_ellipse(hdc, dx, dy, dx + d, dy + d, COL_DANGER);
            }
            _ => {
                let st = status_label(&card.status);
                if matches!(card.status, PopupCardStatus::Cancelled) {
                    let mut rr = RECT {
                        left: text_left,
                        top: y,
                        right: text_right,
                        bottom: y + header_h,
                    };
                    let _ = draw_text_in_rect(
                        hdc,
                        st,
                        &mut rr,
                        COL_FG_3,
                        DT_RIGHT | DT_SINGLELINE | DT_NOPREFIX | DT_VCENTER,
                    );
                }
            }
        }
        if !old.0.is_null() {
            let _ = SelectObject(hdc, old);
        }
        y += header_h + ((2.0_f64) * scale).round() as i32;
    }

    let body = card_body_text(card);
    if !body.is_empty() {
        let max_body = ((120.0_f64) * scale).round() as i32;
        let mut r = RECT {
            left: text_left,
            top: y,
            right: text_right,
            bottom: y + max_body,
        };
        let color = if matches!(card.status, PopupCardStatus::Failed) && card.text.is_empty() {
            status_color_bgr(&PopupCardStatus::Failed)
        } else {
            COL_FG
        };
        let old = select_font(hdc, fonts.body);
        let h = draw_text_in_rect(
            hdc,
            &body,
            &mut r,
            color,
            DT_LEFT | DT_WORDBREAK | DT_NOPREFIX | DT_END_ELLIPSIS,
        );
        if !old.0.is_null() {
            let _ = SelectObject(hdc, old);
        }
        y += h.max(((16.0_f64) * scale).round() as i32) + gap / 2;
    }

    if let Some(err) = card_extra_error(card) {
        let max_err = ((40.0_f64) * scale).round() as i32;
        let mut r = RECT {
            left: text_left,
            top: y,
            right: text_right,
            bottom: y + max_err,
        };
        let old = select_font(hdc, fonts.caption);
        let _ = draw_text_in_rect(
            hdc,
            err,
            &mut r,
            status_color_bgr(&PopupCardStatus::Failed),
            DT_LEFT | DT_WORDBREAK | DT_NOPREFIX | DT_END_ELLIPSIS,
        );
        if !old.0.is_null() {
            let _ = SelectObject(hdc, old);
        }
        y += ((14.0_f64) * scale).round() as i32;
    }

    let model = model_tag_for_card(card);
    let tokens = tokens_label(card);
    if !model.is_empty() || !tokens.is_empty() {
        let meta = if model.is_empty() {
            tokens
        } else if tokens.is_empty() {
            model
        } else {
            format!("{model}  {tokens}")
        };
        let mut r = RECT {
            left: text_left,
            top: y,
            right: text_right,
            bottom: y + ((16.0_f64) * scale).round() as i32,
        };
        let old = select_font(hdc, fonts.micro);
        let _ = draw_text_in_rect(
            hdc,
            &meta,
            &mut r,
            COL_FG_3,
            DT_RIGHT | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS | DT_VCENTER,
        );
        if !old.0.is_null() {
            let _ = SelectObject(hdc, old);
        }
    }
}

unsafe fn paint_simple_scrollbar(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    track: &RECT,
    content_h: i32,
    viewport_h: i32,
    scroll_y: i32,
) {
    if content_h <= viewport_h || viewport_h <= 0 {
        return;
    }
    let track_h = (track.bottom - track.top).max(1);
    fill_solid_rect(hdc, track, COL_SCROLL_TRACK);

    let thumb_h = ((viewport_h as f64 / content_h as f64) * track_h as f64)
        .round()
        .max(12.0) as i32;
    let thumb_h = thumb_h.min(track_h);
    let max_scroll = (content_h - viewport_h).max(1);
    let travel = (track_h - thumb_h).max(0);
    let thumb_top =
        track.top + ((scroll_y as f64 / max_scroll as f64) * travel as f64).round() as i32;
    let thumb = RECT {
        left: track.left,
        top: thumb_top,
        right: track.right,
        bottom: (thumb_top + thumb_h).min(track.bottom),
    };
    fill_solid_rect(hdc, &thumb, COL_SCROLL_THUMB);
}

unsafe fn paint_titlebar(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    client: &RECT,
    scale: f64,
    fonts: &UiFonts,
) {
    let th = titlebar_h(scale);
    let pad = content_pad(scale);
    // 品牌：柿子橙圆角方 +「文」
    let icon = ((20.0_f64) * scale).round() as i32;
    let icon_r = RECT {
        left: pad,
        top: (th - icon) / 2,
        right: pad + icon,
        bottom: (th - icon) / 2 + icon,
    };
    let dia = ((10.0_f64) * scale).round() as i32;
    let brush = CreateSolidBrush(COLORREF(COL_ACCENT));
    let pen = CreatePen(PS_SOLID, 1, COLORREF(COL_ACCENT));
    let old_b = SelectObject(hdc, HGDIOBJ(brush.0));
    let old_p = SelectObject(hdc, HGDIOBJ(pen.0));
    let _ = RoundRect(
        hdc,
        icon_r.left,
        icon_r.top,
        icon_r.right,
        icon_r.bottom,
        dia,
        dia,
    );
    let _ = SelectObject(hdc, old_b);
    let _ = SelectObject(hdc, old_p);
    let _ = DeleteObject(HGDIOBJ(brush.0));
    let _ = DeleteObject(HGDIOBJ(pen.0));
    {
        let old = select_font(hdc, fonts.small);
        let mut tr = icon_r;
        let _ = draw_text_in_rect(
            hdc,
            "文",
            &mut tr,
            0x00_FF_FF_FF,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        if !old.0.is_null() {
            let _ = SelectObject(hdc, old);
        }
    }
    {
        let old = select_font(hdc, fonts.body_semibold);
        let mut r = RECT {
            left: pad + icon + ((8.0_f64) * scale).round() as i32,
            top: 0,
            right: pad + ((90.0_f64) * scale).round() as i32,
            bottom: th,
        };
        let _ = draw_text_in_rect(
            hdc,
            "shizi",
            &mut r,
            COL_FG,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        if !old.0.is_null() {
            let _ = SelectObject(hdc, old);
        }
    }

    // 窗口控件与工具按钮：几何图标
    for (hit, r) in layout_titlebar_buttons(client, scale) {
        let color = COL_FG_2;
        paint_chrome_icon(hdc, hit, &r, scale, color);
    }

    // 关闭 / 最小化 与工具区分隔线
    let win_w = ((WIN_BTN_LOGICAL_W) * scale).round() as i32;
    let sep_x = client.right - win_w * 2 - ((4.0_f64) * scale).round() as i32;
    let sep_h = ((16.0_f64) * scale).round() as i32;
    fill_solid_rect(
        hdc,
        &RECT {
            left: sep_x,
            top: (th - sep_h) / 2,
            right: sep_x + 1,
            bottom: (th - sep_h) / 2 + sep_h,
        },
        COL_BORDER,
    );
}

unsafe fn paint_source_card(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    snap: &PaintSnapshot,
    left: i32,
    right: i32,
    top: i32,
    scale: f64,
    fonts: &UiFonts,
) -> i32 {
    let max_h = (SOURCE_MAX_LOGICAL_H * scale).round() as i32;
    let pad_x = ((12.0_f64) * scale).round() as i32;
    let pad_y = ((10.0_f64) * scale).round() as i32;
    let meta_h = ((22.0_f64) * scale).round() as i32;
    let text_w = (right - left - pad_x * 2).max(1);
    let src = if snap.source_text.is_empty() {
        "输入要翻译的文本…"
    } else {
        snap.source_text.as_str()
    };
    let old = select_font(hdc, fonts.body);
    let text_h = measure_text_height(hdc, src, text_w, DT_LEFT | DT_WORDBREAK | DT_NOPREFIX)
        .max(((36.0_f64) * scale).round() as i32)
        .min(max_h - pad_y * 2 - meta_h);
    let block_h = (pad_y + text_h + meta_h + pad_y).min(max_h);
    let face = RECT {
        left,
        top,
        right,
        bottom: top + block_h,
    };
    fill_round_card(hdc, &face, COL_CARD_BG, COL_BORDER, scale);

    let mut r = RECT {
        left: left + pad_x,
        top: top + pad_y,
        right: right - pad_x,
        bottom: top + pad_y + text_h,
    };
    let _ = draw_text_in_rect(
        hdc,
        src,
        &mut r,
        if snap.source_text.is_empty() {
            COL_FG_3
        } else {
            COL_FG
        },
        DT_LEFT | DT_WORDBREAK | DT_NOPREFIX | DT_END_ELLIPSIS,
    );
    if !old.0.is_null() {
        let _ = SelectObject(hdc, old);
    }

    // meta 顶部分割线
    let meta_top = top + block_h - pad_y - meta_h + 2;
    fill_solid_rect(
        hdc,
        &RECT {
            left: left + pad_x,
            top: meta_top,
            right: right - pad_x,
            bottom: meta_top + 1,
        },
        COL_BORDER,
    );

    // 语言 badge 胶囊（右对齐）
    let badge = lang_display_name(&snap.source_lang);
    let old = select_font(hdc, fonts.micro);
    let badge_h = ((18.0_f64) * scale).round() as i32;
    let badge_w = ((badge.chars().count() as f64 * 11.0 + 16.0) * scale)
        .round()
        .max(36.0) as i32;
    let bx = right - pad_x - badge_w;
    let by = top + block_h - pad_y - badge_h + 2;
    let badge_r = RECT {
        left: bx,
        top: by,
        right: bx + badge_w,
        bottom: by + badge_h,
    };
    let dia = badge_h;
    let brush = CreateSolidBrush(COLORREF(COL_ACCENT_SOFT));
    let pen = CreatePen(PS_SOLID, 1, COLORREF(COL_ACCENT_SOFT));
    let old_b = SelectObject(hdc, HGDIOBJ(brush.0));
    let old_p = SelectObject(hdc, HGDIOBJ(pen.0));
    let _ = RoundRect(
        hdc,
        badge_r.left,
        badge_r.top,
        badge_r.right,
        badge_r.bottom,
        dia,
        dia,
    );
    let _ = SelectObject(hdc, old_b);
    let _ = SelectObject(hdc, old_p);
    let _ = DeleteObject(HGDIOBJ(brush.0));
    let _ = DeleteObject(HGDIOBJ(pen.0));
    let mut br = badge_r;
    let _ = draw_text_in_rect(
        hdc,
        badge,
        &mut br,
        COL_ACCENT,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
    );
    if !old.0.is_null() {
        let _ = SelectObject(hdc, old);
    }
    block_h
}

unsafe fn paint_lang_bar(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    snap: &PaintSnapshot,
    client: &RECT,
    bar_top: i32,
    scale: f64,
    fonts: &UiFonts,
) {
    let parts = layout_lang_bar(client, scale, bar_top);
    let full = RECT {
        left: parts[0].1.left,
        top: bar_top,
        right: parts[2].1.right,
        bottom: bar_top + lang_bar_h(scale),
    };
    fill_round_card(hdc, &full, COL_CARD_BG, COL_BORDER, scale);

    let old = select_font(hdc, fonts.body_semibold);
    let src_label = lang_display_name(&snap.source_lang);
    let tgt_label = lang_display_name(&snap.target_lang);
    for (hit, r) in &parts {
        match *hit {
            ChromeHit::LangSource | ChromeHit::LangTarget => {
                let label = if *hit == ChromeHit::LangSource {
                    src_label
                } else {
                    tgt_label
                };
                // 文字略偏左，右侧画 chevron
                let mut tr = RECT {
                    left: r.left + ((8.0_f64) * scale).round() as i32,
                    top: r.top,
                    right: r.right - ((20.0_f64) * scale).round() as i32,
                    bottom: r.bottom,
                };
                let _ = draw_text_in_rect(
                    hdc,
                    label,
                    &mut tr,
                    COL_FG,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
                );
                paint_chevron_down(
                    hdc,
                    r.right - ((12.0_f64) * scale).round() as i32,
                    (r.top + r.bottom) / 2,
                    scale,
                    COL_FG_3,
                );
            }
            ChromeHit::LangSwap => {
                paint_swap_icon(
                    hdc,
                    (r.left + r.right) / 2,
                    (r.top + r.bottom) / 2,
                    scale,
                    COL_FG_2,
                );
            }
            _ => {}
        }
    }
    if !old.0.is_null() {
        let _ = SelectObject(hdc, old);
    }
}

unsafe fn paint_flyout(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    snap: &PaintSnapshot,
    client: &RECT,
    bar_top: i32,
    scale: f64,
    fonts: &UiFonts,
) {
    let Some(is_source) = lang_flyout_side() else {
        return;
    };
    let codes = lang_codes_for_side(is_source);
    let (fly, item_h) = flyout_metrics(client, scale, bar_top);
    fill_round_card(hdc, &fly, COL_FLYOUT_BG, COL_BORDER_2, scale);

    let selected = if is_source {
        snap.source_lang.as_str()
    } else {
        snap.target_lang.as_str()
    };
    let scroll = lang_flyout_scroll();
    let content_h = (codes.len() as i32) * item_h + 8;
    let viewport = (fly.bottom - fly.top).max(1);
    let max_scroll = (content_h - viewport).max(0);
    if scroll > max_scroll {
        set_lang_flyout_scroll(max_scroll);
    }

    let saved = SaveDC(hdc);
    let _ = IntersectClipRect(hdc, fly.left, fly.top, fly.right, fly.bottom);
    let old = select_font(hdc, fonts.caption);
    let mut y = fly.top + 4 - lang_flyout_scroll();
    for code in codes {
        let r = RECT {
            left: fly.left + 4,
            top: y,
            right: fly.right - 4,
            bottom: y + item_h,
        };
        if r.bottom > fly.top && r.top < fly.bottom {
            if code == selected {
                fill_solid_rect(hdc, &r, COL_ACCENT_SOFT);
            }
            let mut tr = RECT {
                left: r.left + 8,
                top: r.top,
                right: r.right - 8,
                bottom: r.bottom,
            };
            let color = if code == selected {
                COL_ACCENT
            } else {
                COL_FG
            };
            let _ = draw_text_in_rect(
                hdc,
                lang_display_name(code),
                &mut tr,
                color,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
            );
        }
        y += item_h;
    }
    if !old.0.is_null() {
        let _ = SelectObject(hdc, old);
    }
    let _ = RestoreDC(hdc, saved);

    if content_h > viewport {
        let track = RECT {
            left: fly.right - 6,
            top: fly.top + 4,
            right: fly.right - 2,
            bottom: fly.bottom - 4,
        };
        paint_simple_scrollbar(hdc, &track, content_h, viewport, lang_flyout_scroll());
    }
}

unsafe fn paint_status_bar(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    client: &RECT,
    snap: &PaintSnapshot,
    scale: f64,
    fonts: &UiFonts,
) {
    let sh = status_h(scale);
    let top = client.bottom - sh;
    let bar = RECT {
        left: client.left,
        top,
        right: client.right,
        bottom: client.bottom,
    };
    // 顶部分割线
    fill_solid_rect(
        hdc,
        &RECT {
            left: bar.left,
            top: bar.top,
            right: bar.right,
            bottom: bar.top + 1,
        },
        COL_BORDER,
    );

    let pad = content_pad(scale);
    let status_text = if snap.is_translating {
        "翻译中…"
    } else if snap.cards.iter().any(|c| matches!(c.status, PopupCardStatus::Failed)) {
        "部分失败"
    } else if snap.cards.is_empty() {
        "就绪"
    } else if snap
        .cards
        .iter()
        .all(|c| matches!(c.status, PopupCardStatus::Finished | PopupCardStatus::Cancelled))
    {
        "翻译完成"
    } else {
        "就绪"
    };
    let chars = snap.source_text.chars().count();
    let count = format!("{chars} 字");

    // 状态点（原型 status-dot）
    let d = ((6.0_f64) * scale).round() as i32;
    let dy = top + (sh - d) / 2;
    let dot_color = if snap.is_translating {
        COL_ACCENT
    } else if snap
        .cards
        .iter()
        .any(|c| matches!(c.status, PopupCardStatus::Failed))
    {
        COL_DANGER
    } else {
        COL_SUCCESS
    };
    fill_ellipse(hdc, pad, dy, pad + d, dy + d, dot_color);

    let old = select_font(hdc, fonts.micro);
    let mut left_r = RECT {
        left: pad + d + ((6.0_f64) * scale).round() as i32,
        top,
        right: client.right / 2,
        bottom: client.bottom,
    };
    let _ = draw_text_in_rect(
        hdc,
        status_text,
        &mut left_r,
        COL_FG_2,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
    );

    let has_failure = snap
        .cards
        .iter()
        .any(|c| matches!(c.status, PopupCardStatus::Failed));
    for (hit, mut r) in layout_status_actions(client, snap.is_translating, has_failure, scale) {
        let label = match hit {
            ChromeHit::Cancel => "取消",
            ChromeHit::Retry => "重试",
            ChromeHit::Copy => "复制",
            _ => "",
        };
        let _ = draw_text_in_rect(
            hdc,
            label,
            &mut r,
            COL_STATUS_ACTION,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
    }

    let mut right_r = RECT {
        left: client.right / 2,
        top,
        right: client.right - pad,
        bottom: client.bottom,
    };
    let _ = draw_text_in_rect(
        hdc,
        &count,
        &mut right_r,
        COL_FG_3,
        DT_RIGHT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
    );
    if !old.0.is_null() {
        let _ = SelectObject(hdc, old);
    }
}

unsafe fn paint_popup(hwnd: HWND, hdc: windows::Win32::Graphics::Gdi::HDC) {
    let mut client = RECT::default();
    let _ = GetClientRect(hwnd, &mut client);

    fill_solid_rect(hdc, &client, COL_BG);
    SetBkMode(hdc, TRANSPARENT);

    let snap = load_paint_snapshot();
    let scale = window_scale(hwnd);
    let fonts = UiFonts::create(scale);
    let pad = content_pad(scale);
    let gap = content_gap(scale);
    let th = titlebar_h(scale);
    let sh = status_h(scale);

    paint_titlebar(hdc, &client, scale, &fonts);

    let mut y = th + pad;
    let content_left = pad;
    let content_right = client.right - pad;

    // 源文卡
    let source_h = paint_source_card(
        hdc,
        &snap,
        content_left,
        content_right,
        y,
        scale,
        &fonts,
    );
    y += source_h + gap;

    // 语言栏
    let bar_top = y;
    store_layout_anchors(bar_top, source_h);
    paint_lang_bar(hdc, &snap, &client, bar_top, scale, &fonts);
    y += lang_bar_h(scale) + gap;

    // 结果卡区
    let cards_top = y;
    let cards_bottom = (client.bottom - sh - pad / 2).max(cards_top + 1);
    let viewport_h = (cards_bottom - cards_top).max(0);
    let sb_w = ((8.0_f64) * scale).round() as i32;
    let content_right_cards = if snap.cards.len() > 1 {
        (content_right - sb_w - 2).max(content_left + 1)
    } else {
        content_right
    };
    let content_w = (content_right_cards - content_left).max(1);

    let old_measure = select_font(hdc, fonts.body);
    if snap.cards.is_empty() {
        let mut r = RECT {
            left: content_left,
            top: y,
            right: content_right,
            bottom: y + 20,
        };
        let old = select_font(hdc, fonts.caption);
        let _ = draw_text_in_rect(
            hdc,
            "（暂无结果）",
            &mut r,
            COL_FG_3,
            DT_LEFT | DT_SINGLELINE | DT_NOPREFIX,
        );
        if !old.0.is_null() {
            let _ = SelectObject(hdc, old);
        }
        store_scroll_metrics(0, viewport_h);
    } else {
        let _ = select_font(hdc, fonts.body);
        let card_heights: Vec<i32> = snap
            .cards
            .iter()
            .map(|card| measure_card_height(hdc, card, content_w, scale))
            .collect();
        let content_h: i32 = card_heights.iter().sum();
        store_scroll_metrics(content_h, viewport_h);
        let scroll_y = clamp_card_scroll(card_scroll_offset(), content_h, viewport_h);
        set_card_scroll_offset(scroll_y);

        let saved = SaveDC(hdc);
        let _ = IntersectClipRect(hdc, content_left, cards_top, content_right_cards, cards_bottom);

        let mut content_offset = 0i32;
        for (card, &block_h) in snap.cards.iter().zip(card_heights.iter()) {
            let block_top = cards_top - scroll_y + content_offset;
            content_offset += block_h;
            if block_top + block_h < cards_top {
                continue;
            }
            if block_top >= cards_bottom {
                break;
            }
            paint_one_card(
                hdc,
                card,
                content_left,
                content_right_cards,
                block_top,
                block_h,
                scale,
                gap,
                &fonts,
            );
        }
        let _ = RestoreDC(hdc, saved);

        if content_h > viewport_h && viewport_h > 0 {
            let track = RECT {
                left: content_right_cards + 2,
                top: cards_top,
                right: content_right,
                bottom: cards_bottom,
            };
            paint_simple_scrollbar(hdc, &track, content_h, viewport_h, scroll_y);
        }
    }
    if !old_measure.0.is_null() {
        let _ = SelectObject(hdc, old_measure);
    }

    paint_status_bar(hdc, &client, &snap, scale, &fonts);

    // 语言列表盖在最上层
    paint_flyout(hdc, &snap, &client, bar_top, scale, &fonts);

    fonts.destroy();
}

fn lparam_to_point(lparam: LPARAM) -> POINT {
    let v = lparam.0 as u32;
    let x = (v & 0xFFFF) as i16 as i32;
    let y = ((v >> 16) & 0xFFFF) as i16 as i32;
    POINT { x, y }
}

fn chrome_hit_to_action(hit: ChromeHit, snap: &PaintSnapshot) -> Option<PopupUserAction> {
    match hit {
        ChromeHit::Close => Some(PopupUserAction::Close),
        ChromeHit::Settings => Some(PopupUserAction::OpenSettings),
        ChromeHit::Cancel => Some(PopupUserAction::CancelTranslation),
        ChromeHit::Retry => Some(PopupUserAction::Retry {
            service_instance_id: None,
        }),
        ChromeHit::Copy => {
            let id = first_copyable_service_id(snap).unwrap_or_default();
            Some(PopupUserAction::CopyResult {
                service_instance_id: id,
            })
        }
        ChromeHit::LangSwap => {
            let (s, t) = swap_session_langs(&snap.source_lang, &snap.target_lang);
            Some(PopupUserAction::SetSessionLanguages {
                source_lang: s,
                target_lang: t,
            })
        }
        ChromeHit::LangItem(idx) => {
            let is_source = lang_flyout_side()?;
            let codes = lang_codes_for_side(is_source);
            let code = codes.get(idx)?;
            let (source_lang, target_lang) = if is_source {
                ((*code).to_string(), snap.target_lang.clone())
            } else {
                (snap.source_lang.clone(), (*code).to_string())
            };
            Some(PopupUserAction::SetSessionLanguages {
                source_lang,
                target_lang,
            })
        }
        ChromeHit::Minimize
        | ChromeHit::Theme
        | ChromeHit::Bookmark
        | ChromeHit::Shot
        | ChromeHit::Fav
        | ChromeHit::Pin
        | ChromeHit::LangSource
        | ChromeHit::LangTarget => None,
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

    if double_click && hit_test_title_bar_drag(pt.x, pt.y, &client, scale) {
        dispatch_bound_action(PopupUserAction::Close);
        return;
    }

    // 打开列表时点外部关闭
    if lang_flyout_side().is_some() {
        let bar_top = resolve_lang_bar_top(scale, &snap.source_text);
        let (fly, _) = flyout_metrics(&client, scale, bar_top);
        let on_lang_bar = layout_lang_bar(&client, scale, bar_top)
            .iter()
            .any(|(_, r)| pt_in_rect(pt.x, pt.y, r));
        if !pt_in_rect(pt.x, pt.y, &fly) && !on_lang_bar {
            set_lang_flyout(None);
            unsafe {
                let _ = InvalidateRect(hwnd, None, BOOL(1));
            }
            return;
        }
    }

    if let Some(hit) = hit_test_chrome(pt.x, pt.y, &client, &snap, scale) {
        match hit {
            ChromeHit::LangSource => {
                if lang_flyout_side() == Some(true) {
                    set_lang_flyout(None);
                } else {
                    set_lang_flyout(Some(true));
                }
                unsafe {
                    let _ = InvalidateRect(hwnd, None, BOOL(1));
                }
                return;
            }
            ChromeHit::LangTarget => {
                if lang_flyout_side() == Some(false) {
                    set_lang_flyout(None);
                } else {
                    set_lang_flyout(Some(false));
                }
                unsafe {
                    let _ = InvalidateRect(hwnd, None, BOOL(1));
                }
                return;
            }
            ChromeHit::LangItem(_) => {
                if let Some(action) = chrome_hit_to_action(hit, &snap) {
                    set_lang_flyout(None);
                    dispatch_bound_action(action);
                    unsafe {
                        let _ = InvalidateRect(hwnd, None, BOOL(1));
                    }
                }
                return;
            }
            ChromeHit::LangSwap => {
                set_lang_flyout(None);
                if let Some(action) = chrome_hit_to_action(hit, &snap) {
                    dispatch_bound_action(action);
                }
                return;
            }
            ChromeHit::Minimize
            | ChromeHit::Theme
            | ChromeHit::Bookmark
            | ChromeHit::Shot
            | ChromeHit::Fav
            | ChromeHit::Pin => {
                // A 阶段占位
                log::debug!("原生弹窗占位动作: {hit:?}");
                return;
            }
            other => {
                if let Some(action) = chrome_hit_to_action(other, &snap) {
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
        }
    }
}

fn handle_keydown(wparam: WPARAM) {
    let vk = wparam.0 as i32;
    if vk == i32::from(VK_ESCAPE.0) {
        if lang_flyout_side().is_some() {
            set_lang_flyout(None);
            return;
        }
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
        WM_NCHITTEST => {
            let pt = lparam_to_point(lparam);
            if hit_test_title_bar_screen(hwnd, pt.x, pt.y) {
                LRESULT(HTCAPTION as isize)
            } else {
                LRESULT(HTCLIENT as isize)
            }
        }
        WM_NCLBUTTONDBLCLK => {
            if wparam.0 == HTCAPTION as usize {
                dispatch_bound_action(PopupUserAction::Close);
                LRESULT(0)
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
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
        WM_MOUSEWHEEL => {
            let delta = ((wparam.0 as u32) >> 16) as i16 as i32;
            if delta != 0 {
                let scale = window_scale(hwnd);
                let step = ((48.0_f64) * scale).round().max(1.0) as i32;
                let notches = {
                    let n = delta / 120;
                    if n != 0 {
                        n.clamp(-5, 5)
                    } else if delta > 0 {
                        1
                    } else {
                        -1
                    }
                };
                if lang_flyout_side().is_some() {
                    let next = (lang_flyout_scroll() - notches * step).max(0);
                    set_lang_flyout_scroll(next);
                } else {
                    let _ = adjust_card_scroll(-notches * step);
                }
                let _ = InvalidateRect(hwnd, None, BOOL(1));
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            set_lang_flyout(None);
            dispatch_bound_action(PopupUserAction::Close);
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

fn apply_dwm_chrome(hwnd: HWND) {
    let pref = DWMWCP_ROUND;
    let _ = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const DWM_WINDOW_CORNER_PREFERENCE as *const core::ffi::c_void,
            std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        )
    };
    let border = COLORREF(COL_BORDER);
    let _ = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &border as *const COLORREF as *const core::ffi::c_void,
            std::mem::size_of::<COLORREF>() as u32,
        )
    };
}

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

    apply_dwm_chrome(hwnd);

    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }

    Ok(NativePopupHwnd::from_hwnd(hwnd))
}

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
                    SetWindowPos(
                        hwnd,
                        HWND_TOP,
                        x,
                        y,
                        w,
                        h,
                        SWP_SHOWWINDOW | SWP_NOACTIVATE,
                    )
                    .map_err(|e| format!("SetWindowPos 失败: {e}"))?;
                }
            } else {
                unsafe {
                    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                }
            }
        }
        PopupPositionMode::Restore => unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        },
    }

    bring_popup_to_foreground(hwnd);
    Ok(())
}

fn bring_popup_to_foreground(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );
        let _ = SetWindowPos(
            hwnd,
            HWND_NOTOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );
        let _ = BringWindowToTop(hwnd);

        let fg = GetForegroundWindow();
        let target_tid = GetWindowThreadProcessId(hwnd, None);
        let fg_tid = if fg.0.is_null() {
            0
        } else {
            GetWindowThreadProcessId(fg, None)
        };
        let cur_tid = GetCurrentThreadId();
        let mut attached_fg = false;
        let mut attached_target = false;
        if fg_tid != 0 && fg_tid != cur_tid {
            attached_fg = AttachThreadInput(cur_tid, fg_tid, true).as_bool();
        }
        if target_tid != 0 && target_tid != cur_tid {
            attached_target = AttachThreadInput(cur_tid, target_tid, true).as_bool();
        }

        let _ = SetForegroundWindow(hwnd);
        let _ = SetActiveWindow(hwnd);
        let _ = SetFocus(hwnd);

        if attached_target {
            let _ = AttachThreadInput(cur_tid, target_tid, false);
        }
        if attached_fg {
            let _ = AttachThreadInput(cur_tid, fg_tid, false);
        }
    }
}

pub fn hide_popup(window: &NativePopupHwnd) {
    if window.is_valid() {
        set_lang_flyout(None);
        unsafe {
            let _ = ShowWindow(window.hwnd(), SW_HIDE);
        }
    }
}

pub fn destroy_popup(window: &NativePopupHwnd) {
    if window.is_valid() {
        set_lang_flyout(None);
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

    fn sample_card(
        id: &str,
        name: &str,
        protocol: &str,
        model: &str,
        status: PopupCardStatus,
        text: &str,
        error: &str,
    ) -> PopupCardVm {
        PopupCardVm {
            service_instance_id: id.into(),
            service_name: name.into(),
            service_type: if protocol == "microsoft_edge" {
                "mt".into()
            } else {
                "llm".into()
            },
            protocol: protocol.into(),
            model_name: model.into(),
            status,
            text: text.into(),
            error_message: error.into(),
            usage_input: Some(10),
            usage_output: Some(20),
            detected_source_lang: None,
        }
    }

    fn sample_vm() -> PopupViewModel {
        PopupViewModel {
            session_id: Some("s1".into()),
            source_text: "Hello world".into(),
            source_type: "selection".into(),
            source_lang: "en".into(),
            target_lang: "zh-CN".into(),
            is_translating: true,
            cards: vec![sample_card(
                "svc-1",
                "Mock",
                "mock",
                "mock",
                PopupCardStatus::Translating,
                "你好",
                "",
            )],
        }
    }

    fn paint_card(
        id: &str,
        name: &str,
        protocol: &str,
        model: &str,
        status: PopupCardStatus,
        text: &str,
        error: &str,
    ) -> PaintCardSnapshot {
        PaintCardSnapshot {
            service_instance_id: id.into(),
            service_name: name.into(),
            protocol: protocol.into(),
            model_name: model.into(),
            status,
            text: text.into(),
            error_message: error.into(),
            usage_input: None,
            usage_output: None,
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
        assert_eq!(snap.target_lang, "zh-CN");
        assert!(snap.is_translating);
        assert_eq!(snap.cards.len(), 1);
        assert_eq!(snap.cards[0].service_name, "Mock");
        assert_eq!(snap.cards[0].usage_input, Some(10));
        assert_eq!(snap.cards[0].usage_output, Some(20));
    }

    #[test]
    fn multi_card_snapshot_preserves_protocol_and_model() {
        let _guard = SNAPSHOT_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let vm = PopupViewModel {
            session_id: Some("batch".into()),
            source_text: "hi".into(),
            source_type: "selection".into(),
            source_lang: "en".into(),
            target_lang: "zh-CN".into(),
            is_translating: true,
            cards: vec![
                sample_card(
                    "llm-1",
                    "GPT",
                    "openai_chat",
                    "gpt-4o-mini",
                    PopupCardStatus::Translating,
                    "你好",
                    "",
                ),
                sample_card(
                    "mt-1",
                    "Edge",
                    "microsoft_edge",
                    "should-hide",
                    PopupCardStatus::Finished,
                    "嗨",
                    "",
                ),
                sample_card(
                    "fail-1",
                    "Claude",
                    "claude_messages",
                    "claude-3",
                    PopupCardStatus::Failed,
                    "",
                    "超时",
                ),
            ],
        };
        let snap = store_paint_snapshot(&vm);
        assert_eq!(snap.cards.len(), 3);
        assert_eq!(snap.cards[0].protocol, "openai_chat");
        assert_eq!(snap.cards[1].protocol, "microsoft_edge");
        assert_eq!(snap.cards[2].error_message, "超时");
    }

    #[test]
    fn card_detail_label_hides_model_for_microsoft_edge() {
        assert_eq!(
            card_detail_label("microsoft_edge", "gpt-x"),
            "microsoft_edge"
        );
        assert_eq!(
            card_detail_label("openai_chat", "gpt-4o-mini"),
            "gpt-4o-mini"
        );
        assert!(is_machine_translate_protocol("microsoft_edge"));
    }

    #[test]
    fn format_card_header_includes_name_detail_status() {
        let llm = paint_card(
            "1",
            "GPT",
            "openai_chat",
            "gpt-4o-mini",
            PopupCardStatus::Finished,
            "ok",
            "",
        );
        let h = format_card_header(&llm);
        assert!(h.contains("GPT"));
        assert!(h.contains("gpt-4o-mini"));
        assert!(h.contains("完成"));
    }

    #[test]
    fn card_body_and_extra_error() {
        let ok = paint_card(
            "1",
            "A",
            "mock",
            "m",
            PopupCardStatus::Finished,
            "译文",
            "",
        );
        assert_eq!(card_body_text(&ok), "译文");
        assert_eq!(card_extra_error(&ok), None);

        let fail_only = paint_card(
            "2",
            "B",
            "mock",
            "m",
            PopupCardStatus::Failed,
            "",
            "网络错误",
        );
        assert_eq!(card_body_text(&fail_only), "网络错误");
    }

    #[test]
    fn clamp_and_adjust_card_scroll() {
        let _guard = SNAPSHOT_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        assert_eq!(clamp_card_scroll(-10, 500, 200), 0);
        store_scroll_metrics(500, 200);
        set_card_scroll_offset(0);
        assert_eq!(adjust_card_scroll(100), 100);
        reset_card_scroll();
        assert_eq!(card_scroll_offset(), 0);
    }

    #[test]
    fn store_snapshot_resets_scroll_on_source_change() {
        let _guard = SNAPSHOT_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut vm = sample_vm();
        let _ = store_paint_snapshot(&vm);
        store_scroll_metrics(800, 200);
        set_card_scroll_offset(120);
        vm.cards[0].text = "新译文".into();
        let _ = store_paint_snapshot(&vm);
        assert_eq!(card_scroll_offset(), 120);
        vm.source_text = "Other".into();
        let _ = store_paint_snapshot(&vm);
        assert_eq!(card_scroll_offset(), 0);
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
    }

    #[test]
    fn enqueue_repaint_zero_hwnd_is_noop_false() {
        assert!(!enqueue_repaint(0));
    }

    #[test]
    fn status_color_and_label_mapping() {
        assert_eq!(status_label(&PopupCardStatus::Translating), "翻译中");
        assert_ne!(
            status_color_bgr(&PopupCardStatus::Translating),
            status_color_bgr(&PopupCardStatus::Finished)
        );
    }

    #[test]
    fn toolbar_includes_cancel_only_when_translating() {
        let client = RECT {
            left: 0,
            top: 0,
            right: 468,
            bottom: 360,
        };
        let idle = layout_toolbar_buttons(&client, false, 1.0);
        assert!(idle.iter().all(|(b, _)| *b != ChromeHit::Cancel));
        let busy = layout_toolbar_buttons(&client, true, 1.0);
        assert!(busy.iter().any(|(b, _)| *b == ChromeHit::Cancel));
    }

    #[test]
    fn hit_test_toolbar_finds_cancel_when_busy() {
        let client = RECT {
            left: 0,
            top: 0,
            right: 468,
            bottom: 360,
        };
        let busy = layout_toolbar_buttons(&client, true, 1.0);
        let (_, r) = busy
            .iter()
            .find(|(b, _)| *b == ChromeHit::Cancel)
            .expect("cancel");
        assert_eq!(
            hit_test_toolbar(r.left + 2, r.top + 2, &client, true, 1.0),
            Some(ChromeHit::Cancel)
        );
    }

    #[test]
    fn hit_test_title_bar_top_strip() {
        assert!(hit_test_title_bar(0, 1.0));
        assert!(hit_test_title_bar(20, 1.0));
        assert!(!hit_test_title_bar(50, 1.0));
    }

    #[test]
    fn titlebar_contains_close_and_settings() {
        let client = RECT {
            left: 0,
            top: 0,
            right: 468,
            bottom: 480,
        };
        let layout = layout_titlebar_buttons(&client, 1.0);
        assert!(layout.iter().any(|(h, _)| *h == ChromeHit::Close));
        assert!(layout.iter().any(|(h, _)| *h == ChromeHit::Settings));
        assert!(layout.iter().any(|(h, _)| *h == ChromeHit::Pin));
    }

    #[test]
    fn lang_bar_three_zones() {
        let client = RECT {
            left: 0,
            top: 0,
            right: 468,
            bottom: 480,
        };
        let parts = layout_lang_bar(&client, 1.0, 100);
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].0, ChromeHit::LangSource);
        assert_eq!(parts[1].0, ChromeHit::LangSwap);
        assert_eq!(parts[2].0, ChromeHit::LangTarget);
    }

    #[test]
    fn lang_display_and_codes() {
        assert_eq!(lang_display_name("zh-CN"), "简体中文");
        assert_eq!(lang_display_name("auto"), "自动检测");
        let src = lang_codes_for_side(true);
        let tgt = lang_codes_for_side(false);
        assert!(src.contains(&"auto"));
        assert!(!tgt.contains(&"auto"));
        assert!(tgt.contains(&"en"));
    }

    #[test]
    fn swap_session_langs_auto_rules() {
        assert_eq!(
            swap_session_langs("auto", "zh-CN"),
            ("zh-CN".into(), "en".into())
        );
        assert_eq!(
            swap_session_langs("en", "zh-CN"),
            ("zh-CN".into(), "en".into())
        );
    }

    #[test]
    fn resolve_copy_and_first_id() {
        let snap = PaintSnapshot {
            source_text: "x".into(),
            source_lang: "en".into(),
            target_lang: "zh-CN".into(),
            is_translating: false,
            cards: vec![
                paint_card("a", "A", "mock", "m", PopupCardStatus::Pending, "", ""),
                paint_card("b", "B", "mock", "m", PopupCardStatus::Finished, "译文", ""),
            ],
        };
        assert_eq!(first_copyable_service_id(&snap).as_deref(), Some("b"));
        assert_eq!(resolve_copy_text(&snap, "b").as_deref(), Some("译文"));
    }

    #[test]
    fn popup_logical_size_matches_winui3_prototype_width() {
        assert!((POPUP_LOGICAL_WIDTH - 468.0).abs() < f64::EPSILON);
        assert!((POPUP_LOGICAL_HEIGHT - 520.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fluent_accent_is_persimmon() {
        // #D55A1F → BGR 0x001F5AD5
        assert_eq!(COL_ACCENT, 0x00_1F_5A_D5);
        assert_eq!(COL_BG, 0x00_F4_F4_F4);
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
}
