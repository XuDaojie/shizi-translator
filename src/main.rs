#![windows_subsystem = "windows"]

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use windows::Win32::{
    Foundation::*,
    UI::WindowsAndMessaging::*,
    UI::Shell::*,
};
use windows::core::PCWSTR;
use windows_reactor::*;

const WM_TRAYICON: u32 = WM_USER + 1;
const WM_REG_HOTKEY: u32 = WM_USER + 2;
const WM_HOTKEY_MSG: u32 = 0x0312;
const HOTKEY_ID: i32 = 1;
const TRAY_ICON_ID: u32 = 1001;
const MOD_ALT_V: u32 = 0x0001;

#[link(name = "user32")]
extern "system" {
    fn RegisterHotKey(hWnd: HWND, id: i32, fsModifiers: u32, vk: u32) -> i32;
    fn UnregisterHotKey(hWnd: HWND, id: i32) -> i32;
    fn SendMessageW(hWnd: HWND, Msg: u32, wParam: usize, lParam: isize) -> isize;
}

static HOTKEY_REGISTERED: AtomicBool = AtomicBool::new(false);

fn app(_cx: &mut RenderCx) -> Element {
    let registered = HOTKEY_REGISTERED.load(Ordering::Relaxed);

    vstack((
        text_block("Shizi").font_size(20.0).bold(),
        text_block("Rust + WinUI 3 系统托盘应用").font_size(12.0),
        text_block(" ").font_size(8.0),
        text_block(if registered {
            "全局热键 Alt+T ✓"
        } else {
            "热键注册中..."
        })
        .font_size(12.0),
        text_block(" ").font_size(8.0),
        text_block("双击托盘图标恢复窗口").font_size(11.0),
    ))
    .spacing(4.0)
    .padding(16.0)
    .into()
}

type WndProc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

static mut ORIGINAL_WNDPROC: Option<isize> = None;
static mut HWND_STORED: Option<HWND> = None;

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
            if mouse_msg == 0x0203 {
                _ = ShowWindow(hwnd, SW_SHOW);
                _ = SetForegroundWindow(hwnd);
            }
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

            let original =
                SetWindowLongPtrW(hwnd, GWLP_WNDPROC, tray_wndproc as *const () as isize);
            if original == 0 {
                return;
            }
            ORIGINAL_WNDPROC = Some(original);
            HWND_STORED = Some(hwnd);

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
        .inner_size(320.0, 220.0)
        .render(app)
}
