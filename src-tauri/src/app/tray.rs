use std::sync::{Arc, RwLock};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{TrayIcon, TrayIconBuilder, TrayIconEvent},
    Manager,
};

use crate::app::state::AppState;
use crate::app::window::{show_settings_window, show_window};
use crate::ui::web_popup::{show_translation_error, show_translation_popup};

fn tray_icon_image() -> tauri::Result<Image<'static>> {
    Image::from_bytes(include_bytes!("../../icons/tray-icon.png"))
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
    let settings_item = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&translate_item, &settings_item, &quit_item])?;

    let tray = TrayIconBuilder::new()
        .icon(tray_icon_image()?)
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
    use super::tray_icon_image;

    #[test]
    fn dedicated_tray_icon_is_16px() {
        let icon = tray_icon_image().expect("专用托盘图标应可解码");

        assert_eq!((icon.width(), icon.height()), (16, 16));
    }
}
