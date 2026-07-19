use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIcon, TrayIconBuilder, TrayIconEvent},
    Manager,
};

use crate::app::state::AppState;
use crate::app::window::{
    request_show_ocr_window, request_show_settings_window, show_window,
};
use crate::app::popup_window::PopupPositionMode;
use crate::ui::web_popup::{show_translation_error, show_translation_popup_with};

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

// 加速键策略：
// 1) 默认 TrayAccelMode::Native + MenuItem::set_accelerator 原位更新（Tauri 2.11+）
// 2) 禁止 tray 内 GlobalShortcut::register（与 shortcuts.rs 双绑）
// 3) 若实机抢键：将 TRAY_ACCEL_MODE 改为 TextOnly（文案 \t 拼接，accelerator 恒 None）
// 4) 不默认 tray.set_menu 重建；仅当 set_accelerator 在目标环境不可用时再考虑重建

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

/// 按加速键模式生成菜单项展示文案（可测纯函数）。
pub fn format_tray_label(mode: TrayAccelMode, label: &str, accel: Option<&str>) -> String {
    match (mode, accel) {
        (TrayAccelMode::TextOnly, Some(keys)) => format!("{label}\t{keys}"),
        _ => label.to_string(),
    }
}

fn menu_item_text(label: &str, accel: Option<&str>) -> String {
    format_tray_label(TRAY_ACCEL_MODE, label, accel)
}

fn menu_item_accelerator_arg(accel: Option<&str>) -> Option<&str> {
    match TRAY_ACCEL_MODE {
        TrayAccelMode::Native => accel,
        TrayAccelMode::TextOnly => None,
    }
}

fn create_bound_menu_item(
    app: &tauri::App,
    binding: &TrayMenuBinding,
    shortcuts: &HashMap<String, String>,
) -> tauri::Result<MenuItem<tauri::Wry>> {
    let accel = binding
        .shortcut_id
        .and_then(|id| menu_accelerator(shortcuts, id));
    let accel_ref = accel.as_deref();
    let text = menu_item_text(binding.label, accel_ref);
    let accel_arg = menu_item_accelerator_arg(accel_ref);
    MenuItem::with_id(app, binding.menu_id, text, true, accel_arg)
}

/// 配置保存后热更新托盘菜单加速键展示；失败只 warn，不传播错误。
pub fn refresh_tray_menu_accelerators(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    let shortcuts = match state.config_store.get() {
        Ok(c) => c.shortcuts,
        Err(e) => {
            log::warn!("刷新托盘加速键失败：无法读配置: {e}");
            return;
        }
    };
    let handles = app.state::<TrayI18nHandles>();
    if let Err(e) = apply_accelerators_to_handles(&handles, &shortcuts) {
        log::warn!("刷新托盘加速键失败: {e}");
    }
}

fn apply_accelerators_to_handles(
    handles: &TrayI18nHandles,
    shortcuts: &HashMap<String, String>,
) -> Result<(), String> {
    let pairs: [(&MenuItem<tauri::Wry>, &str, &str); 4] = [
        (&handles.selection, TRAY_LABEL_SELECTION, "translate-selection"),
        (
            &handles.screenshot,
            TRAY_LABEL_SCREENSHOT,
            "translate-screenshot",
        ),
        (&handles.ocr, TRAY_LABEL_OCR, "ocr-recognize"),
        (&handles.settings, TRAY_LABEL_SETTINGS, "open-settings"),
    ];
    for (item, label, shortcut_id) in pairs {
        let accel = menu_accelerator(shortcuts, shortcut_id);
        let text = format_tray_label(TRAY_ACCEL_MODE, label, accel.as_deref());
        item.set_text(text)
            .map_err(|e| format!("set_text {shortcut_id}: {e}"))?;
        let native = match TRAY_ACCEL_MODE {
            TrayAccelMode::Native => accel.as_deref(),
            TrayAccelMode::TextOnly => None,
        };
        // set_accelerator 内部 parse().ok()；非法 keys 静默清除。不要直接依赖 muda。
        item.set_accelerator(native)
            .map_err(|e| format!("set_accelerator {shortcut_id}: {e}"))?;
    }
    handles
        .quit
        .set_text(TRAY_LABEL_QUIT)
        .map_err(|e| format!("set_text quit: {e}"))?;
    handles
        .quit
        .set_accelerator(None::<&str>)
        .map_err(|e| format!("set_accelerator quit: {e}"))?;
    Ok(())
}

#[derive(Clone)]
pub struct TrayI18nHandles {
    pub tray: TrayIcon,
    pub selection: MenuItem<tauri::Wry>,
    pub screenshot: MenuItem<tauri::Wry>,
    pub ocr: MenuItem<tauri::Wry>,
    pub settings: MenuItem<tauri::Wry>,
    pub quit: MenuItem<tauri::Wry>,
    pub settings_title: Arc<RwLock<String>>,
}

pub fn setup_tray(app: &tauri::App) -> tauri::Result<TrayI18nHandles> {
    let shortcuts = app
        .state::<AppState>()
        .config_store
        .get()
        .map(|c| c.shortcuts)
        .unwrap_or_default();

    let bindings = tray_menu_bindings();
    let selection_item = create_bound_menu_item(app, &bindings[0], &shortcuts)?;
    let screenshot_item = create_bound_menu_item(app, &bindings[1], &shortcuts)?;
    let ocr_item = create_bound_menu_item(app, &bindings[2], &shortcuts)?;
    let settings_item = create_bound_menu_item(app, &bindings[3], &shortcuts)?;
    let quit_item = MenuItem::with_id(app, "quit", TRAY_LABEL_QUIT, true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    // 顺序：划词翻译 / 截图翻译 / 文字识别 → 分隔 → 偏好设置 → 分隔 → 退出 shizi
    let menu = Menu::with_items(
        app,
        &[
            &selection_item,
            &screenshot_item,
            &ocr_item,
            &sep1,
            &settings_item,
            &sep2,
            &quit_item,
        ],
    )?;

    let tray = TrayIconBuilder::new()
        .icon(tray_icon_image(app)?)
        .tooltip("Shizi - 翻译助手")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "selection" => {
                // 托盘打开：保留上次位置（首次为 conf 居中），不跟随鼠标到托盘角。
                let state = app.state::<AppState>();
                match state.config_store.get() {
                    Ok(config) => {
                        if let Err(e) =
                            show_translation_popup_with(app, &config, PopupPositionMode::Restore)
                        {
                            show_translation_error(app, e);
                        }
                    }
                    Err(e) => show_translation_error(app, e.to_string()),
                }
            }
            "screenshot" => {
                crate::app::shortcuts::trigger_ocr_translate(app);
            }
            "ocr" => {
                // 仅打开文字识别窗口，不启动截图。
                // 独立线程：托盘菜单回调里同步 build WebView 会在 Windows 上死锁。
                request_show_ocr_window(app);
            }
            "settings" => {
                request_show_settings_window(app);
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
        selection: selection_item,
        screenshot: screenshot_item,
        ocr: ocr_item,
        settings: settings_item,
        quit: quit_item,
        settings_title: Arc::new(RwLock::new("Shizi 设置".into())),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        format_tray_label, menu_accelerator, tray_icon_image_for_scale, tray_icon_size,
        tray_menu_bindings, TrayAccelMode,
    };
    use std::collections::HashMap;

    #[test]
    fn menu_item_text_respects_accel_mode_shape() {
        assert_eq!(
            format_tray_label(TrayAccelMode::Native, "划词翻译", Some("Alt+D")),
            "划词翻译"
        );
        assert_eq!(
            format_tray_label(TrayAccelMode::TextOnly, "划词翻译", Some("Alt+D")),
            "划词翻译\tAlt+D"
        );
        assert_eq!(
            format_tray_label(TrayAccelMode::TextOnly, "划词翻译", None),
            "划词翻译"
        );
    }

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
