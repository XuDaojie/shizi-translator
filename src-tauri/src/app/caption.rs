//! 设置窗原生标题栏着色（Windows 11 `DWMWA_CAPTION_COLOR`）。
//!
//! 浅色默认对齐 `--settings-sidebar: oklch(98.5% 0.004 70)`；深色由前端主题切换后下发。
//! Win10 无该属性，调用失败则保持系统默认标题栏。

/// 标题栏底色 / 标题文字色 / 关闭按钮是否用深色模式图标。
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionChrome {
    pub caption: [u8; 3],
    pub text: [u8; 3],
    pub dark_buttons: bool,
}

/// 浅色设置页左栏暖白底（oklch 98.5% 0.004 70 → sRGB）与 `--foreground`。
pub const SETTINGS_LIGHT_CAPTION: CaptionChrome = CaptionChrome {
    caption: [252, 250, 247],
    text: [38, 43, 54],
    dark_buttons: false,
};

/// COLORREF：`0x00BBGGRR`。
pub fn colorref(rgb: [u8; 3]) -> u32 {
    u32::from(rgb[0]) | (u32::from(rgb[1]) << 8) | (u32::from(rgb[2]) << 16)
}

/// 把标题栏底色对齐设置页左栏。失败只记日志，不挡开窗。
pub fn apply_window_caption(window: &tauri::WebviewWindow, chrome: &CaptionChrome) {
    #[cfg(windows)]
    {
        match window.hwnd() {
            Ok(hwnd) => apply_to_hwnd(hwnd.0 as isize, chrome),
            Err(error) => log::warn!("读取窗口 HWND 失败，跳过标题栏着色: {error}"),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (window, chrome);
    }
}

#[cfg(windows)]
fn apply_to_hwnd(hwnd: isize, chrome: &CaptionChrome) {
    #[link(name = "dwmapi")]
    unsafe extern "system" {
        fn DwmSetWindowAttribute(
            hwnd: *mut core::ffi::c_void,
            attr: u32,
            value: *const core::ffi::c_void,
            size: u32,
        ) -> i32;
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn SetWindowPos(
            hwnd: *mut core::ffi::c_void,
            insert_after: *mut core::ffi::c_void,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> i32;
    }

    const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
    const DWMWA_CAPTION_COLOR: u32 = 35;
    const DWMWA_TEXT_COLOR: u32 = 36;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_FRAMECHANGED: u32 = 0x0020;

    let hwnd = hwnd as *mut core::ffi::c_void;
    let caption = colorref(chrome.caption);
    let text = colorref(chrome.text);
    let dark: i32 = i32::from(chrome.dark_buttons);
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_COLOR,
            (&caption as *const u32).cast(),
            4,
        );
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_TEXT_COLOR, (&text as *const u32).cast(), 4);
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&dark as *const i32).cast(),
            4,
        );
        // 强制重算非客户区，否则首帧可能仍是系统深色标题栏。
        let _ = SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colorref_packs_as_bbggrr() {
        assert_eq!(colorref([252, 250, 247]), 0x00F7_FAFC);
        assert_eq!(colorref([19, 16, 13]), 0x000D_1013);
    }

    #[test]
    fn light_caption_matches_settings_sidebar_warm_white() {
        assert_eq!(SETTINGS_LIGHT_CAPTION.caption, [252, 250, 247]);
        assert_eq!(SETTINGS_LIGHT_CAPTION.text, [38, 43, 54]);
        assert!(!SETTINGS_LIGHT_CAPTION.dark_buttons);
    }

    #[test]
    fn caption_chrome_deserializes_camel_case() {
        let chrome: CaptionChrome = serde_json::from_str(
            r#"{"caption":[252,250,247],"text":[38,43,54],"darkButtons":false}"#,
        )
        .expect("IPC 载荷应能反序列化");
        assert_eq!(chrome, SETTINGS_LIGHT_CAPTION);
    }
}
