use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{TrayIcon, TrayIconBuilder, TrayIconEvent},
    Manager,
};

use crate::app::state::AppState;
use crate::app::window::{show_ocr_window, show_settings_window, show_window};
use crate::ui::web_popup::{show_translation_error, show_translation_popup};

fn tray_icon_size(scale_factor: f64) -> u32 {
    match (16.0 * scale_factor).round() as u32 {
        0..=16 => 16,
        17..=20 => 20,
        21..=24 => 24,
        25..=28 => 28,
        29..=32 => 32,
        33..=36 => 36,
        37..=40 => 40,
        _ => 48,
    }
}

fn tray_icon_image_for_scale(scale_factor: f64) -> tauri::Result<Image<'static>> {
    let bytes: &[u8] = match tray_icon_size(scale_factor) {
        16 => include_bytes!("../../icons/tray-icon-16.png"),
        20 => include_bytes!("../../icons/tray-icon-20.png"),
        24 => include_bytes!("../../icons/tray-icon-24.png"),
        28 => include_bytes!("../../icons/tray-icon-28.png"),
        32 => include_bytes!("../../icons/tray-icon-32.png"),
        36 => include_bytes!("../../icons/tray-icon-36.png"),
        40 => include_bytes!("../../icons/tray-icon-40.png"),
        _ => include_bytes!("../../icons/tray-icon-48.png"),
    };
    Image::from_bytes(bytes)
}

fn tray_icon_image(app: &tauri::App) -> tauri::Result<Image<'static>> {
    let scale_factor = app
        .primary_monitor()?
        .map(|monitor| monitor.scale_factor())
        .unwrap_or(1.0);
    tray_icon_image_for_scale(scale_factor)
}

/// 托盘加速键展示模式。Native = 系统 MenuItem accelerator；TextOnly = 文案拼接、不注册 accelerator。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAccelMode {
    Native,
    TextOnly,
}

/// 默认 Native。若 Windows 实机发现 accelerator 在菜单未打开时抢键，改为 TextOnly。
const TRAY_ACCEL_MODE: TrayAccelMode = TrayAccelMode::Native;

pub const TRAY_LABEL_SELECTION: &str = "划词翻译";
pub const TRAY_LABEL_SCREENSHOT: &str = "截图翻译";
pub const TRAY_LABEL_OCR: &str = "文字识别";
pub const TRAY_LABEL_SETTINGS: &str = "偏好设置";
pub const TRAY_LABEL_QUIT: &str = "退出 shizi";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrayMenuBinding {
    pub menu_id: &'static str,
    pub label: &'static str,
    pub shortcut_id: Option<&'static str>,
}

/// 有加速键的菜单项（顺序即菜单上半 + 设置项）。quit 与分隔线由 setup 组装。
pub fn tray_menu_bindings() -> &'static [TrayMenuBinding] {
    &[
        TrayMenuBinding {
            menu_id: "selection",
            label: TRAY_LABEL_SELECTION,
            shortcut_id: Some("translate-selection"),
        },
        TrayMenuBinding {
            menu_id: "screenshot",
            label: TRAY_LABEL_SCREENSHOT,
            shortcut_id: Some("translate-screenshot"),
        },
        TrayMenuBinding {
            menu_id: "ocr",
            label: TRAY_LABEL_OCR,
            shortcut_id: Some("ocr-recognize"),
        },
        TrayMenuBinding {
            menu_id: "settings",
            label: TRAY_LABEL_SETTINGS,
            shortcut_id: Some("open-settings"),
        },
    ]
}

/// 从 shortcuts map 取展示用加速键：缺 key / trim 空 → None；否则 Some(trimmed)。
pub fn menu_accelerator(shortcuts: &HashMap<String, String>, id: &str) -> Option<String> {
    shortcuts
        .get(id)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

#[allow(dead_code)]
fn menu_item_text(label: &str, accel: Option<&str>) -> String {
    match (TRAY_ACCEL_MODE, accel) {
        (TrayAccelMode::TextOnly, Some(keys)) => format!("{label}\t{keys}"),
        _ => label.to_string(),
    }
}

#[allow(dead_code)]
fn menu_item_accelerator_arg(accel: Option<&str>) -> Option<&str> {
    match TRAY_ACCEL_MODE {
        TrayAccelMode::Native => accel,
        TrayAccelMode::TextOnly => None,
    }
}

#[derive(Clone)]
pub struct TrayI18nHandles {
    pub tray: TrayIcon,
    pub translate: MenuItem<tauri::Wry>,
    pub settings: MenuItem<tauri::Wry>,
    pub quit: MenuItem<tauri::Wry>,
    pub settings_title: Arc<RwLock<String>>,
}

pub fn setup_tray(app: &tauri::App) -> tauri::Result<TrayI18nHandles> {
    let translate_item = MenuItem::with_id(app, "translate", "翻译", true, None::<&str>)?;
    // 不做完整 i18n：固定中文，不接入 TrayI18nHandles / apply_interface_language
    let ocr_item = MenuItem::with_id(app, "ocr", "文字识别", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&translate_item, &ocr_item, &settings_item, &quit_item])?;

    let tray = TrayIconBuilder::new()
        .icon(tray_icon_image(app)?)
        .tooltip("Shizi - 翻译助手")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "translate" => {
                let state = app.state::<AppState>();
                match state.config_store.get() {
                    Ok(config) => {
                        if let Err(e) = show_translation_popup(app, &config) {
                            show_translation_error(app, e);
                        }
                    }
                    Err(e) => show_translation_error(app, e.to_string()),
                }
            }
            "ocr" => {
                // 仅打开文字识别窗口，不启动截图
                if let Err(e) = show_ocr_window(app) {
                    log::warn!("打开文字识别窗口失败: {e}");
                }
            }
            "screenshot" => {
                crate::app::shortcuts::trigger_ocr_translate(app);
            }
            "settings" => {
                let _ = show_settings_window(app);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::DoubleClick { .. } = event {
                show_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(TrayI18nHandles {
        tray,
        translate: translate_item,
        settings: settings_item,
        quit: quit_item,
        settings_title: Arc::new(RwLock::new("Shizi 设置".into())),
    })
}

#[cfg(test)]
mod tests {
    use super::{menu_accelerator, tray_icon_image_for_scale, tray_icon_size, tray_menu_bindings};
    use std::collections::HashMap;

    #[test]
    fn tray_icon_size_matches_common_windows_scale_factors() {
        for (scale_factor, expected_size) in [
            (1.0, 16),
            (1.25, 20),
            (1.5, 24),
            (1.75, 28),
            (2.0, 32),
            (2.25, 36),
            (2.5, 40),
            (3.0, 48),
        ] {
            assert_eq!(tray_icon_size(scale_factor), expected_size);
        }
    }

    #[test]
    fn selected_tray_icons_match_their_physical_size() {
        for scale_factor in [1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 3.0] {
            let expected_size = tray_icon_size(scale_factor);
            let icon =
                tray_icon_image_for_scale(scale_factor).expect("对应 DPI 的专用托盘图标应可解码");

            assert_eq!(
                (icon.width(), icon.height()),
                (expected_size, expected_size)
            );
        }
    }

    #[test]
    fn menu_accelerator_trims_and_skips_empty() {
        let mut map = HashMap::new();
        map.insert("translate-selection".into(), "  Alt+D  ".into());
        map.insert("translate-screenshot".into(), "".into());
        map.insert("ocr-recognize".into(), "   ".into());

        assert_eq!(
            menu_accelerator(&map, "translate-selection").as_deref(),
            Some("Alt+D")
        );
        assert_eq!(menu_accelerator(&map, "translate-screenshot"), None);
        assert_eq!(menu_accelerator(&map, "ocr-recognize"), None);
        assert_eq!(menu_accelerator(&map, "missing-id"), None);
    }

    #[test]
    fn tray_menu_bindings_order_and_shortcut_ids() {
        let rows = tray_menu_bindings();
        let ids: Vec<&str> = rows.iter().map(|r| r.menu_id).collect();
        assert_eq!(ids, ["selection", "screenshot", "ocr", "settings"]);
        assert_eq!(rows[0].shortcut_id, Some("translate-selection"));
        assert_eq!(rows[1].shortcut_id, Some("translate-screenshot"));
        assert_eq!(rows[2].shortcut_id, Some("ocr-recognize"));
        assert_eq!(rows[3].shortcut_id, Some("open-settings"));
        // quit 不在 bindings 表（无加速键）；由 setup 单独追加
    }
}
