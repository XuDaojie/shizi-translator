//! 原生弹窗表面（**路径 B：Win32**）。
//!
//! - `WS_POPUP | WS_CLIPCHILDREN`，无系统厚边框
//! - `WS_EX_TOOLWINDOW`：不进任务栏
//! - 初始 `SW_HIDE`，逻辑尺寸约 420×480（与 WebView `present_popup` 定位高一致）
//! - DWM 圆角 + 边框色（best-effort）；`CS_DROPSHADOW` 轻阴影（未用 `WS_EX_LAYERED`）
//! - GDI 自绘：Segoe UI 字体、Fluent 暖色板、源文 + 多服务结果卡（可滚动）+ 底部动作条
//! - chrome：`is_translating` 时显示取消按钮
//! - 用户动作：Esc 关闭、Ctrl+C 复制、滚轮滚动卡片区、工具栏点击、双击标题区关闭
//! - 不依赖 Microsoft.UI.Xaml / WinAppSDK

use std::sync::{Mutex, Once};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{BOOL, COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DWM_WINDOW_CORNER_PREFERENCE,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect,
    IntersectClipRect, InvalidateRect, RestoreDC, SaveDC, SelectObject, SetBkMode, SetTextColor,
    CLEARTYPE_QUALITY, DEFAULT_CHARSET, DEFAULT_PITCH, DT_CALCRECT, DT_CENTER, DT_END_ELLIPSIS,
    DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DT_WORDBREAK, FF_DONTCARE, FW_NORMAL,
    FW_SEMIBOLD, HFONT, HGDIOBJ, PAINTSTRUCT, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, SetFocus, VK_CONTROL, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, IsWindow, IsWindowVisible,
    LoadCursorW, PostMessageW, RegisterClassExW, SetWindowPos, ShowWindow, CS_DBLCLKS,
    CS_DROPSHADOW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, HMENU, HWND_TOP, IDC_ARROW,
    SWP_SHOWWINDOW, SW_HIDE, SW_SHOW, USER_DEFAULT_SCREEN_DPI, WM_CLOSE, WM_DESTROY, WM_KEYDOWN,
    WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_MOUSEWHEEL, WM_PAINT, WM_USER, WNDCLASSEXW,
    WS_CLIPCHILDREN, WS_EX_TOOLWINDOW, WS_POPUP,
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

/// 弹窗逻辑宽度（与 WebView `.popup` / `present_popup` 一致）。
pub const POPUP_LOGICAL_WIDTH: f64 = 420.0;
/// 弹窗逻辑高度上限（与 WebView `present_popup` 的 `POPUP_H` 对齐；多卡区内部滚动）。
pub const POPUP_LOGICAL_HEIGHT: f64 = 480.0;

/// 底部动作条高度（逻辑像素；按 DPI 由布局函数乘 scale）。
const TOOLBAR_LOGICAL_H: i32 = 40;
const BTN_LOGICAL_W: i32 = 60;
const BTN_GAP: i32 = 8;
const TITLE_HIT_LOGICAL_H: i32 = 32;
const PAD_LOGICAL: f64 = 14.0;
const GAP_LOGICAL: f64 = 10.0;

// Fluent / 弹窗 token 近似色（COLORREF = 0x00BBGGRR）
const COL_BG: u32 = 0x00_EC_F2_F5; // #F5F2EC
const COL_CARD_BG: u32 = 0x00_FF_FF_FF;
const COL_FG: u32 = 0x00_1B_1E_1F; // #1F1E1B
const COL_FG_2: u32 = 0x00_4F_58_5B; // #5B584F
const COL_FG_3: u32 = 0x00_70_77_7A; // #7A7770
const COL_BORDER: u32 = 0x00_D8_E2_E6; // #E6E2D8
const COL_BORDER_2: u32 = 0x00_C5_D3_D8; // #D8D3C5
const COL_TOOLBAR_BG: u32 = 0x00_F3_F8_FA; // #FAF8F3
const COL_BTN_BG: u32 = 0x00_FF_FF_FF;
const COL_SCROLL_TRACK: u32 = 0x00_E5_ED_F0;
const COL_SCROLL_THUMB: u32 = 0x00_C5_D3_D8;

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
    /// 协议 id（如 `openai_chat` / `microsoft_edge`），与 Web 结果卡一致。
    pub protocol: String,
    /// 模型名；`microsoft_edge` 绘制时不强调。
    pub model_name: String,
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
                    protocol: c.protocol.clone(),
                    model_name: c.model_name.clone(),
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

/// 卡片列表垂直滚动偏移（像素，向下为正）。
static CARD_SCROLL_Y: Mutex<i32> = Mutex::new(0);

/// 最近一次绘制测得的卡片内容高度 / 视口高度（供滚轮钳制）。
static CARD_SCROLL_METRICS: Mutex<(i32, i32)> = Mutex::new((0, 0));

/// 机器翻译协议（与前端 `resultCardMeta` 一致）：不展示模型。
pub fn is_machine_translate_protocol(protocol: &str) -> bool {
    protocol.trim() == "microsoft_edge"
}

/// 卡片副标题：优先 model；`microsoft_edge` 改用 protocol、不强调模型。
///
/// 空 / 占位 `—` `-` 的 model 回退到 protocol。
pub fn card_detail_label(protocol: &str, model_name: &str) -> String {
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

/// 卡片标题行：`name · detail · status`（detail 空则省略）。
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

/// 卡片区滚动偏移（测试 / 滚轮）。
pub fn card_scroll_offset() -> i32 {
    CARD_SCROLL_Y.lock().map(|g| *g).unwrap_or(0)
}

/// 钳制滚动偏移到 `[0, max(0, content_h - viewport_h)]`。
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

/// 滚轮增量（向下为正像素）；内部按当前 metrics 钳制。
pub fn adjust_card_scroll(delta_px: i32) -> i32 {
    let (content_h, viewport_h) = load_scroll_metrics();
    let next = clamp_card_scroll(card_scroll_offset() + delta_px, content_h, viewport_h);
    set_card_scroll_offset(next);
    next
}

/// 将 ViewModel 写入共享快照（不碰 HWND；任意线程、短路径）。
///
/// 返回写入后的快照副本，便于单测「入队/落盘」而不创建真实窗口。
/// 源文变化时重置卡片区滚动，避免新批次停在旧偏移。
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

/// 状态 → 文本色（COLORREF，0x00BBGGRR；对齐 Fluent success/warning/danger）。
pub fn status_color_bgr(status: &PopupCardStatus) -> u32 {
    match status {
        PopupCardStatus::Pending => COL_FG_3,
        PopupCardStatus::Translating => 0x00_10_50_CA, // #CA5010 warning
        PopupCardStatus::Finished => 0x00_10_7C_10,    // #107C10 success
        PopupCardStatus::Failed => 0x00_18_23_B4,      // #b42318 danger
        PopupCardStatus::Cancelled => COL_FG_3,
    }
}

/// 创建系统 UI 字体（Segoe UI + ClearType）；失败时返回空 HFONT。
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

/// 逻辑字号 → 物理像素高度（最小 11）。
fn font_px(logical_pt: f64, scale: f64) -> i32 {
    ((logical_pt * scale).round() as i32).max(11)
}

struct UiFonts {
    caption: HFONT,
    body: HFONT,
    body_semibold: HFONT,
    button: HFONT,
}

impl UiFonts {
    unsafe fn create(scale: f64) -> Self {
        Self {
            caption: create_ui_font(font_px(12.0, scale), FW_NORMAL.0 as i32),
            body: create_ui_font(font_px(13.0, scale), FW_NORMAL.0 as i32),
            body_semibold: create_ui_font(font_px(13.0, scale), FW_SEMIBOLD.0 as i32),
            button: create_ui_font(font_px(12.0, scale), FW_NORMAL.0 as i32),
        }
    }

    unsafe fn destroy(self) {
        if !self.caption.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(self.caption.0));
        }
        if !self.body.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(self.body.0));
        }
        if !self.body_semibold.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(self.body_semibold.0));
        }
        if !self.button.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(self.button.0));
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
        // CS_DROPSHADOW：轻阴影，无需 WS_EX_LAYERED 自绘
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

/// 用 `DT_CALCRECT` 测算文本高度（不实际绘制）。
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

/// 单卡正文展示内容（空 text 时按状态给占位 / 失败信息）。
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

/// 是否在正文之外再画一行失败信息（有译文且另有 error）。
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

/// 估算 / 测算单卡块高度（内边距 + header + body + 可选 error + 底部分隔）。
unsafe fn measure_card_height(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    card: &PaintCardSnapshot,
    content_w: i32,
    scale: f64,
) -> i32 {
    let gap = ((GAP_LOGICAL) * scale).round() as i32;
    let card_pad = ((8.0_f64) * scale).round() as i32;
    let header_h = ((20.0_f64) * scale).round() as i32;
    let text_w = (content_w - card_pad * 2).max(1);
    let mut h = card_pad + header_h + 2;

    let body = card_body_text(card);
    if !body.is_empty() {
        let max_body = ((140.0_f64) * scale).round() as i32;
        let bh = measure_text_height(
            hdc,
            &body,
            text_w,
            DT_LEFT | DT_WORDBREAK | DT_NOPREFIX,
        )
        .max(((16.0_f64) * scale).round() as i32)
        .min(max_body);
        h += bh + gap / 2;
    }

    if let Some(err) = card_extra_error(card) {
        let max_err = ((48.0_f64) * scale).round() as i32;
        let eh = measure_text_height(
            hdc,
            err,
            text_w,
            DT_LEFT | DT_WORDBREAK | DT_NOPREFIX,
        )
        .max(((14.0_f64) * scale).round() as i32)
        .min(max_err);
        h += eh + gap / 2;
    }

    h + card_pad + gap
}

/// 绘制单张服务卡（浅底 + 边框 + name/model/状态/正文）。
/// `block_h` 须与滚动测高同一字体上下文下的 `measure_card_height` 结果一致。
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
    let card_pad = ((8.0_f64) * scale).round() as i32;
    // 块底含 gap，卡片面略短一截，露出分隔呼吸感
    let face_bottom = (top + block_h - gap / 2).max(top + 1);
    let face = RECT {
        left,
        top,
        right,
        bottom: face_bottom,
    };
    fill_solid_rect(hdc, &face, COL_CARD_BG);
    // 顶边与左右细边（底边用分隔区留白代替）
    let border_top = RECT {
        left,
        top,
        right,
        bottom: top + 1,
    };
    fill_solid_rect(hdc, &border_top, COL_BORDER);
    let border_left = RECT {
        left,
        top,
        right: left + 1,
        bottom: face_bottom,
    };
    fill_solid_rect(hdc, &border_left, COL_BORDER);
    let border_right = RECT {
        left: right - 1,
        top,
        right,
        bottom: face_bottom,
    };
    fill_solid_rect(hdc, &border_right, COL_BORDER);
    let border_bottom = RECT {
        left,
        top: face_bottom - 1,
        right,
        bottom: face_bottom,
    };
    fill_solid_rect(hdc, &border_bottom, COL_BORDER);

    let text_left = left + card_pad;
    let text_right = right - card_pad;
    let mut y = top + card_pad;
    let status_color = status_color_bgr(&card.status);
    let header = format_card_header(card);
    let header_h = ((20.0_f64) * scale).round() as i32;
    {
        let old = select_font(hdc, fonts.body_semibold);
        let mut r = RECT {
            left: text_left,
            top: y,
            right: text_right,
            bottom: y + header_h,
        };
        let _ = draw_text_in_rect(
            hdc,
            &header,
            &mut r,
            status_color,
            DT_LEFT | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
        );
        if !old.0.is_null() {
            let _ = SelectObject(hdc, old);
        }
        y += header_h + 2;
    }

    let body = card_body_text(card);
    if !body.is_empty() {
        let max_body = ((140.0_f64) * scale).round() as i32;
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
        let max_err = ((48.0_f64) * scale).round() as i32;
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
    }
}

/// 绘制简易竖向滚动条（内容超出视口时）。
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

/// 计算底部工具栏按钮布局（客户区坐标）。
pub fn layout_toolbar_buttons(
    client: &RECT,
    is_translating: bool,
    scale: f64,
) -> Vec<(ToolbarButton, RECT)> {
    let toolbar_h = ((TOOLBAR_LOGICAL_H as f64) * scale).round() as i32;
    let btn_w = ((BTN_LOGICAL_W as f64) * scale).round() as i32;
    let gap = ((BTN_GAP as f64) * scale).round() as i32;
    let pad = ((PAD_LOGICAL) * scale).round() as i32;
    let v_inset = ((6.0_f64) * scale).round() as i32;

    let mut buttons = vec![ToolbarButton::Close];
    if is_translating {
        buttons.push(ToolbarButton::Cancel);
    }
    buttons.push(ToolbarButton::Retry);
    buttons.push(ToolbarButton::Copy);
    buttons.push(ToolbarButton::Settings);

    let top = (client.bottom - toolbar_h + v_inset).max(client.top);
    let bottom = (client.bottom - v_inset).max(top + 1);
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

    fill_solid_rect(hdc, &client, COL_BG);
    SetBkMode(hdc, TRANSPARENT);

    let snap = load_paint_snapshot();
    let scale = window_scale(hwnd);
    let fonts = UiFonts::create(scale);
    let pad = ((PAD_LOGICAL) * scale).round() as i32;
    let gap = ((GAP_LOGICAL) * scale).round() as i32;
    let toolbar_h = ((TOOLBAR_LOGICAL_H as f64) * scale).round() as i32;
    let mut y = pad;
    let right = client.right - pad;
    let bottom = (client.bottom - toolbar_h - pad / 2).max(y + 1);

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
        let caption_h = ((18.0_f64) * scale).round() as i32;
        let mut r = RECT {
            left: pad,
            top: y,
            right,
            bottom: y + caption_h,
        };
        let old = select_font(hdc, fonts.caption);
        let _ = draw_text_in_rect(
            hdc,
            &lang_hint,
            &mut r,
            COL_FG_3,
            DT_LEFT | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
        );
        if !old.0.is_null() {
            let _ = SelectObject(hdc, old);
        }
        y += caption_h + ((4.0_f64) * scale).round() as i32;
    }

    // 源文正文
    {
        let max_h = ((96.0_f64) * scale).round() as i32;
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
        let old = select_font(hdc, fonts.body);
        let h = draw_text_in_rect(
            hdc,
            src,
            &mut r,
            COL_FG,
            DT_LEFT | DT_WORDBREAK | DT_NOPREFIX | DT_END_ELLIPSIS,
        );
        if !old.0.is_null() {
            let _ = SelectObject(hdc, old);
        }
        y += h.max(((18.0_f64) * scale).round() as i32) + gap;
    }

    if y + 2 < bottom {
        let sep = RECT {
            left: pad,
            top: y,
            right,
            bottom: y + 1,
        };
        fill_solid_rect(hdc, &sep, COL_BORDER);
        y += gap;
    }

    // 多服务卡片区：裁剪 + 简易滚动条
    let cards_top = y;
    let cards_bottom = bottom;
    let viewport_h = (cards_bottom - cards_top).max(0);
    let sb_w = ((8.0_f64) * scale).round() as i32;
    let content_right = if snap.cards.len() > 1 {
        (right - sb_w - 2).max(pad + 1)
    } else {
        right
    };
    let content_w = (content_right - pad).max(1);

    // 测算前选 body 字体，保证高度与绘制一致
    let old_measure = select_font(hdc, fonts.body);

    if snap.cards.is_empty() {
        let mut r = RECT {
            left: pad,
            top: y,
            right,
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
        // 统一用 body 字体测高，避免绘制过程中切字体导致滚动与块高不一致
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
        let _ = IntersectClipRect(hdc, pad, cards_top, content_right, cards_bottom);

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
                pad,
                content_right,
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
                left: content_right + 2,
                top: cards_top,
                right,
                bottom: cards_bottom,
            };
            paint_simple_scrollbar(hdc, &track, content_h, viewport_h, scroll_y);
        }
    }

    if !old_measure.0.is_null() {
        let _ = SelectObject(hdc, old_measure);
    }

    paint_toolbar(hdc, &client, &snap, scale, &fonts);
    fonts.destroy();
}

unsafe fn paint_toolbar(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    client: &RECT,
    snap: &PaintSnapshot,
    scale: f64,
    fonts: &UiFonts,
) {
    let toolbar_h = ((TOOLBAR_LOGICAL_H as f64) * scale).round() as i32;
    let bar = RECT {
        left: client.left,
        top: (client.bottom - toolbar_h).max(client.top),
        right: client.right,
        bottom: client.bottom,
    };
    fill_solid_rect(hdc, &bar, COL_TOOLBAR_BG);

    let sep = RECT {
        left: bar.left,
        top: bar.top,
        right: bar.right,
        bottom: bar.top + 1,
    };
    fill_solid_rect(hdc, &sep, COL_BORDER);

    let old = select_font(hdc, fonts.button);
    for (btn, mut r) in layout_toolbar_buttons(client, snap.is_translating, scale) {
        fill_solid_rect(hdc, &r, COL_BTN_BG);
        // 按钮外框
        let edge = RECT {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.top + 1,
        };
        fill_solid_rect(hdc, &edge, COL_BORDER_2);
        let edge_b = RECT {
            left: r.left,
            top: r.bottom - 1,
            right: r.right,
            bottom: r.bottom,
        };
        fill_solid_rect(hdc, &edge_b, COL_BORDER_2);
        let edge_l = RECT {
            left: r.left,
            top: r.top,
            right: r.left + 1,
            bottom: r.bottom,
        };
        fill_solid_rect(hdc, &edge_l, COL_BORDER_2);
        let edge_r = RECT {
            left: r.right - 1,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        };
        fill_solid_rect(hdc, &edge_r, COL_BORDER_2);

        let label_color = if matches!(btn, ToolbarButton::Cancel) {
            status_color_bgr(&PopupCardStatus::Failed)
        } else {
            COL_FG_2
        };
        let _ = draw_text_in_rect(
            hdc,
            btn.label(),
            &mut r,
            label_color,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
    }
    if !old.0.is_null() {
        let _ = SelectObject(hdc, old);
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
        WM_MOUSEWHEEL => {
            // HIWORD(wParam) = wheel delta（上滚为正）；向下浏览 → 增大 scroll offset
            let delta = ((wparam.0 as u32) >> 16) as i16 as i32;
            if delta != 0 {
                let scale = window_scale(hwnd);
                let step = ((48.0_f64) * scale).round().max(1.0) as i32;
                // 高分辨率触控板：按 120 归一，至少滚动一档
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
                let _ = adjust_card_scroll(-notches * step);
                let _ = InvalidateRect(hwnd, None, BOOL(1));
            }
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
    // 边框色接近 token `--popup-border`（COLORREF BGR）；旧系统忽略即可
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

/// 创建隐藏的原生弹窗（约 420×480 逻辑像素，与 WebView NearCursor 定位高一致）。
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
            usage_input: None,
            usage_output: None,
            detected_source_lang: None,
        }
    }

    fn sample_vm() -> PopupViewModel {
        PopupViewModel {
            session_id: Some("s1".into()),
            source_text: "Hello world".into(),
            source_type: "selection".into(),
            source_lang: "en".into(),
            target_lang: "zh".into(),
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
        assert_eq!(snap.cards[0].protocol, "mock");
        assert_eq!(snap.cards[0].model_name, "mock");
        assert_eq!(snap.cards[0].text, "你好");
        assert_eq!(snap.cards[0].status, PopupCardStatus::Translating);

        let loaded = load_paint_snapshot();
        assert_eq!(loaded, snap);
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
            target_lang: "zh".into(),
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
        assert_eq!(snap.cards[0].model_name, "gpt-4o-mini");
        assert_eq!(snap.cards[1].protocol, "microsoft_edge");
        assert_eq!(snap.cards[2].error_message, "超时");
        assert!(snap.is_translating);
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
        assert_eq!(card_detail_label("openai_chat", "—"), "openai_chat");
        assert_eq!(card_detail_label("openai_chat", ""), "openai_chat");
        assert!(is_machine_translate_protocol("microsoft_edge"));
        assert!(!is_machine_translate_protocol("openai_chat"));
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

        let edge = paint_card(
            "2",
            "Edge",
            "microsoft_edge",
            "hidden-model",
            PopupCardStatus::Translating,
            "",
            "",
        );
        let he = format_card_header(&edge);
        assert!(he.contains("Edge"));
        assert!(he.contains("microsoft_edge"));
        assert!(!he.contains("hidden-model"));
        assert!(he.contains("翻译中"));
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
        assert_eq!(card_extra_error(&fail_only), None);

        let fail_both = paint_card(
            "3",
            "C",
            "mock",
            "m",
            PopupCardStatus::Failed,
            "部分译文",
            "截断",
        );
        assert_eq!(card_body_text(&fail_both), "部分译文");
        assert_eq!(card_extra_error(&fail_both), Some("截断"));

        let pending = paint_card(
            "4",
            "D",
            "mock",
            "m",
            PopupCardStatus::Pending,
            "",
            "",
        );
        assert_eq!(card_body_text(&pending), "…");
    }

    #[test]
    fn clamp_and_adjust_card_scroll() {
        let _guard = SNAPSHOT_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        assert_eq!(clamp_card_scroll(-10, 500, 200), 0);
        assert_eq!(clamp_card_scroll(100, 500, 200), 100);
        assert_eq!(clamp_card_scroll(999, 500, 200), 300);
        assert_eq!(clamp_card_scroll(50, 100, 200), 0);

        store_scroll_metrics(500, 200);
        set_card_scroll_offset(0);
        assert_eq!(adjust_card_scroll(100), 100);
        assert_eq!(adjust_card_scroll(250), 300);
        assert_eq!(adjust_card_scroll(-50), 250);
        assert_eq!(card_scroll_offset(), 250);
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
        assert_eq!(card_scroll_offset(), 120);

        // 同源、同服务 id 仅改译文 → 保留滚动
        vm.cards[0].text = "新译文".into();
        let _ = store_paint_snapshot(&vm);
        assert_eq!(card_scroll_offset(), 120);

        // 源文变化 → 重置
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
                paint_card("a", "A", "mock", "m", PopupCardStatus::Pending, "", ""),
                paint_card("b", "B", "mock", "m", PopupCardStatus::Finished, "译文", ""),
            ],
        };
        assert_eq!(first_copyable_service_id(&snap).as_deref(), Some("b"));
        assert_eq!(resolve_copy_text(&snap, "b").as_deref(), Some("译文"));
    }

    #[test]
    fn toolbar_cancel_hidden_when_idle_shown_when_translating() {
        let client = RECT {
            left: 0,
            top: 0,
            right: 420,
            bottom: 360,
        };
        let idle = layout_toolbar_buttons(&client, false, 1.0);
        assert!(
            idle.iter().all(|(b, _)| *b != ToolbarButton::Cancel),
            "空闲时不得显示取消"
        );
        let busy = layout_toolbar_buttons(&client, true, 1.0);
        assert!(
            busy.iter().any(|(b, _)| *b == ToolbarButton::Cancel),
            "翻译中应显示取消"
        );
        // busy 布局能命中取消；idle 布局中不存在 Cancel 热区
        let (_, r) = busy
            .iter()
            .find(|(b, _)| *b == ToolbarButton::Cancel)
            .expect("cancel rect");
        assert_eq!(
            hit_test_toolbar(r.left + 1, r.top + 1, &client, true, 1.0),
            Some(ToolbarButton::Cancel)
        );
        assert_ne!(
            hit_test_toolbar(r.left + 1, r.top + 1, &client, false, 1.0),
            Some(ToolbarButton::Cancel)
        );
    }

    #[test]
    fn popup_logical_size_matches_webview_near_cursor() {
        // WebView present_popup 使用 420×480 参与 compute_popup_position
        assert!((POPUP_LOGICAL_WIDTH - 420.0).abs() < f64::EPSILON);
        assert!((POPUP_LOGICAL_HEIGHT - 480.0).abs() < f64::EPSILON);
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
