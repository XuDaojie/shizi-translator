#![windows_subsystem = "windows"]

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use windows::Win32::{
    Foundation::*,
    Graphics::Dwm::*,
    UI::WindowsAndMessaging::*,
    UI::Shell::*,
    UI::Controls::MARGINS,
};
use windows::core::PCWSTR;
use windows_reactor::*;

const WM_TRAYICON: u32 = WM_USER + 1;
const WM_REG_HOTKEY: u32 = WM_USER + 2;
const WM_RESTORE_STYLE: u32 = WM_USER + 3;
const WM_HIDE_STYLE: u32 = WM_USER + 4;
const WM_HOTKEY_MSG: u32 = 0x0312;
const HOTKEY_ID: i32 = 1;
const TRAY_ICON_ID: u32 = 1001;
const MOD_ALT_V: u32 = 0x0001;
const IDM_EXIT: u32 = 1001;
const GWL_STYLE: WINDOW_LONG_PTR_INDEX = WINDOW_LONG_PTR_INDEX(-16);

#[link(name = "user32")]
extern "system" {
    fn RegisterHotKey(hWnd: HWND, id: i32, fsModifiers: u32, vk: u32) -> i32;
    fn UnregisterHotKey(hWnd: HWND, id: i32) -> i32;
    fn SendMessageW(hWnd: HWND, Msg: u32, wParam: usize, lParam: isize) -> isize;
    fn GetWindowLongW(hWnd: HWND, nIndex: WINDOW_LONG_PTR_INDEX) -> i32;
    fn SetWindowLongW(hWnd: HWND, nIndex: WINDOW_LONG_PTR_INDEX, dwNewLong: i32) -> i32;
    fn CreatePopupMenu() -> HMENU;
    fn AppendMenuW(hMenu: HMENU, uFlags: u32, uIDNewItem: usize, lpNewItem: PCWSTR) -> i32;
    fn TrackPopupMenu(hMenu: HMENU, uFlags: u32, x: i32, y: i32, nReserved: i32, hWnd: HWND, prc: *const RECT) -> i32;
    fn PostMessageW(hWnd: HWND, Msg: u32, wParam: usize, lParam: isize) -> i32;
    fn GetCursorPos(lpPoint: *mut POINT) -> i32;
    fn DestroyMenu(hMenu: HMENU) -> i32;
    fn ReleaseCapture() -> i32;
    fn SetCapture(hWnd: HWND) -> HWND;
}

static HOTKEY_REGISTERED: AtomicBool = AtomicBool::new(false);
static mut HWND_STORED: Option<HWND> = None;
static mut DRAGGING: bool = false;
static mut DRAG_LAST_X: i32 = 0;
static mut DRAG_LAST_Y: i32 = 0;

fn app(cx: &mut RenderCx) -> Element {
    let (input, set_input) = cx.use_state(String::new());
    let (titlebar_visible, set_titlebar_visible) = cx.use_state(false);
    let registered = HOTKEY_REGISTERED.load(Ordering::Relaxed);

    vstack((
        text_block(" ")
            .height(36.0)
            .on_pointer_pressed(move |_: PointerEventInfo| {
                unsafe {
                    if let Some(_hwnd) = HWND_STORED {
                        let mut pt = POINT::default();
                        GetCursorPos(&mut pt);
                        DRAGGING = false;
                        DRAG_LAST_X = pt.x;
                        DRAG_LAST_Y = pt.y;
                    }
                }
            })
            .on_pointer_moved({
                move |e: PointerEventInfo| {
                    unsafe {
                        if DRAGGING || !e.is_left_button_pressed {
                            return;
                        }
                        if let Some(hwnd) = HWND_STORED {
                            let mut pt = POINT::default();
                            GetCursorPos(&mut pt);
                            let dy = (pt.y - DRAG_LAST_Y).abs();
                            if dy > 5 {
                                DRAGGING = true;
                                SetCapture(hwnd);
                            }
                        }
                    }
                }
            })
            .on_pointer_released(move |_: PointerEventInfo| {
                unsafe {
                    DRAGGING = false;
                    ReleaseCapture();
                }
            }),
        text_box(&input)
            .placeholder_text("输入搜索内容...")
            .on_text_changed(move |v| set_input.call(v)),
        Expander::new(vstack((
            text_block("log").font_size(24.0).bold(),
            text_block("释义: 日志").font_size(14.0),
            text_block("n. 原木, 圆材; 正式记录, 航海日志; 对数; 观察记录; 船舶测速仪")
                .font_size(12.0),
        )).spacing(4.0))
        .header("有道词典")
        .expanded(true),
        Expander::new(vstack((
            text_block("log → 日志").font_size(14.0),
        )).spacing(4.0))
        .header("OpenAI 翻译"),
        text_block(" ").font_size(6.0),
        text_block(if registered {
            "全局热键 Alt+T"
        } else {
            "热键注册中..."
        })
        .font_size(11.0),
        button(if titlebar_visible { "隐藏标题栏" } else { "恢复标题栏" }).on_click(move || {
            let next = !titlebar_visible;
            set_titlebar_visible.call(next);
            unsafe {
                if let Some(hwnd) = HWND_STORED {
                    let msg = if next { WM_RESTORE_STYLE } else { WM_HIDE_STYLE };
                    _ = PostMessageW(hwnd, msg, 0, 0);
                }
            }
        }),
        button("打开测试窗口").on_click(|| {
            open_test_xml_window();
        }),
    ))
    .spacing(6.0)
    .padding(16.0)
    .into()
}

struct TestXmlWindow;

impl Component for TestXmlWindow {
    fn render(&self, _props: &(), _cx: &mut RenderCx) -> Element {
        let mut search = auto_suggest_box("");
        search.placeholder_text = "搜索...".into();
        search.modifiers.width = Some(200.0);

        let mut tb = TitleBar::new("我的应用").subtitle("v1.0");
        tb.content_element = Some(Box::new(search.into()));

        vstack((tb,)).spacing(0.0).into()
    }
}

fn open_test_xml_window() {
    if let Ok(host) = ReactorHost::new_with_window_options(
        "测试窗口",
        Some(WindowSize {
            width: 480.0,
            height: 320.0,
        }),
        InnerConstraints::default(),
        Box::new(TestXmlWindow),
        |_| {},
    ) {
        let _ = host.activate();
    }
}

type WndProc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

static mut ORIGINAL_WNDPROC: Option<isize> = None;

unsafe fn show_tray_menu(hwnd: HWND) {
    let menu = CreatePopupMenu();
    if menu.0.is_null() {
        return;
    }
    let exit_text: Vec<u16> = "退出\0".encode_utf16().collect();
    AppendMenuW(menu, MF_STRING.0, IDM_EXIT as usize, PCWSTR::from_raw(exit_text.as_ptr()));

    let mut pt = POINT::default();
    GetCursorPos(&mut pt);
    TrackPopupMenu(menu, TPM_RIGHTBUTTON.0, pt.x, pt.y, 0, hwnd, std::ptr::null());
    PostMessageW(hwnd, WM_NULL, 0, 0);
    DestroyMenu(menu);
}

unsafe extern "system" fn tray_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CLOSE => {
            _ = ShowWindow(hwnd, SW_HIDE);
            LRESULT(0)
        }
        WM_KILLFOCUS => {
            _ = ShowWindow(hwnd, SW_HIDE);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            if DRAGGING {
                let mut pt = POINT::default();
                GetCursorPos(&mut pt);
                let dx = pt.x - DRAG_LAST_X;
                let dy = pt.y - DRAG_LAST_Y;
                DRAG_LAST_X = pt.x;
                DRAG_LAST_Y = pt.y;
                let mut rect = RECT::default();
                if GetWindowRect(hwnd, &mut rect).is_ok() {
                    _ = SetWindowPos(hwnd, Some(HWND(std::ptr::null_mut())), rect.left + dx, rect.top + dy, 0, 0, SWP_NOSIZE | SWP_NOZORDER);
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            DRAGGING = false;
            ReleaseCapture();
            LRESULT(0)
        }
        WM_HOTKEY_MSG => {
            if (wparam.0 as u32) & 0xFFFF == HOTKEY_ID as u32 {
                if IsWindowVisible(hwnd).as_bool() {
                    _ = ShowWindow(hwnd, SW_HIDE);
                } else {
                    _ = ShowWindow(hwnd, SW_SHOW);
                    _ = SetForegroundWindow(hwnd);
                }
            }
            LRESULT(0)
        }
        msg if msg == WM_TRAYICON => {
            let mouse_msg = (lparam.0 as u32) & 0xFFFF;
            match mouse_msg {
                0x0203 => {
                    _ = ShowWindow(hwnd, SW_SHOW);
                    _ = SetForegroundWindow(hwnd);
                }
                0x0205 => {
                    show_tray_menu(hwnd);
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 as u32) & 0xFFFF;
            if id == IDM_EXIT {
                _ = ShowWindow(hwnd, SW_HIDE);
                let nid = NOTIFYICONDATAW {
                    cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                    hWnd: hwnd,
                    uID: TRAY_ICON_ID,
                    uFlags: NIF_MESSAGE,
                    uCallbackMessage: WM_TRAYICON,
                    ..Default::default()
                };
                _ = Shell_NotifyIconW(NIM_DELETE, &nid);
                UnregisterHotKey(hwnd, HOTKEY_ID);
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        WM_RESTORE_STYLE => {
            let style = GetWindowLongW(hwnd, GWL_STYLE);
            let style = style | (WS_CAPTION.0 as i32) | (WS_THICKFRAME.0 as i32);
            SetWindowLongW(hwnd, GWL_STYLE, style);
            let pref: i32 = DWMWCP_DEFAULT.0;
            _ = DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &pref as *const _ as *const _, std::mem::size_of::<i32>() as u32);
            let margins = MARGINS {
                cxLeftWidth: 0,
                cxRightWidth: 0,
                cyTopHeight: 0,
                cyBottomHeight: 0,
            };
            _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
            _ = SetWindowPos(hwnd, Some(HWND(std::ptr::null_mut())), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED);
            LRESULT(0)
        }
        WM_HIDE_STYLE => {
            let style = GetWindowLongW(hwnd, GWL_STYLE);
            let style = style & !(WS_CAPTION.0 as i32) & !(WS_THICKFRAME.0 as i32);
            SetWindowLongW(hwnd, GWL_STYLE, style);
            let pref: i32 = DWMWCP_ROUND.0;
            _ = DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &pref as *const _ as *const _, std::mem::size_of::<i32>() as u32);
            let margins = MARGINS {
                cxLeftWidth: 1,
                cxRightWidth: 1,
                cyTopHeight: 1,
                cyBottomHeight: 0,
            };
            _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
            _ = SetWindowPos(hwnd, Some(HWND(std::ptr::null_mut())), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED);
            LRESULT(0)
        }
        WM_REG_HOTKEY => {
            let ok = RegisterHotKey(hwnd, HOTKEY_ID, MOD_ALT_V, b'T' as u32);
            HOTKEY_REGISTERED.store(ok != 0, Ordering::Relaxed);
            LRESULT(ok as isize)
        }
        WM_DESTROY => {
            let nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: TRAY_ICON_ID,
                uFlags: NIF_MESSAGE,
                uCallbackMessage: WM_TRAYICON,
                ..Default::default()
            };
            _ = Shell_NotifyIconW(NIM_DELETE, &nid);
            UnregisterHotKey(hwnd, HOTKEY_ID);
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => {
            if let Some(orig) = ORIGINAL_WNDPROC {
                let proc: WndProc = std::mem::transmute(orig);
                proc(hwnd, msg, wparam, lparam)
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
    }
}

fn setup_tray_and_hotkey() {
    thread::spawn(|| {
        thread::sleep(Duration::from_secs(2));

        unsafe {
            let title: Vec<u16> = "Shizi\0".encode_utf16().collect();
            let hwnd = FindWindowW(None, PCWSTR::from_raw(title.as_ptr()));
            let hwnd = match hwnd {
                Ok(h) if h.0 != std::ptr::null_mut() => h,
                _ => return,
            };

            let style = GetWindowLongW(hwnd, GWL_STYLE);
            let style = style & !(WS_CAPTION.0 as i32) & !(WS_THICKFRAME.0 as i32);
            SetWindowLongW(hwnd, GWL_STYLE, style);

            let pref: i32 = DWMWCP_ROUND.0;
            _ = DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &pref as *const _ as *const _, std::mem::size_of::<i32>() as u32);

            let margins = MARGINS {
                cxLeftWidth: 1,
                cxRightWidth: 1,
                cyTopHeight: 1,
                cyBottomHeight: 0,
            };
            _ = DwmExtendFrameIntoClientArea(hwnd, &margins);

            HWND_STORED = Some(hwnd);

            let original =
                SetWindowLongPtrW(hwnd, GWLP_WNDPROC, tray_wndproc as *const () as isize);
            if original == 0 {
                return;
            }
            ORIGINAL_WNDPROC = Some(original);

            let icon = LoadIconW(None, IDI_APPLICATION);
            if let Err(_) = icon {
                return;
            }

            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: TRAY_ICON_ID,
                uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
                uCallbackMessage: WM_TRAYICON,
                hIcon: icon.unwrap(),
                ..Default::default()
            };
            let tip: Vec<u16> = "Shizi\0".encode_utf16().collect();
            let tip_len = tip.len().min(128);
            nid.szTip[..tip_len].copy_from_slice(&tip[..tip_len]);

            _ = Shell_NotifyIconW(NIM_ADD, &nid);

            let result = SendMessageW(hwnd, WM_REG_HOTKEY, 0, 0);
            HOTKEY_REGISTERED.store(result != 0, Ordering::Relaxed);
        }
    });
}

fn main() -> Result<()> {
    setup_tray_and_hotkey();
    App::new()
        .title("Shizi")
        .inner_size(320.0, 480.0)
        .render(app)
}
