use tauri::{Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::app::shortcuts::attach_app_shortcut_focus_listener;
use crate::app::tray::TrayI18nHandles;

pub const SETTINGS_LABEL: &str = "settings";
pub const SETTINGS_URL: &str = "settings.html";
pub const SETTINGS_INITIAL_VISIBLE: bool = false;

fn close_to_hide(window: &WebviewWindow) {
    let window_to_hide = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = window_to_hide.hide();
        }
    });
}

pub fn show_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn ensure_settings_window(app: &tauri::AppHandle) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(SETTINGS_LABEL) {
        return Ok(window);
    }

    let window =
        WebviewWindowBuilder::new(app, SETTINGS_LABEL, WebviewUrl::App(SETTINGS_URL.into()))
            .title(
                app.state::<TrayI18nHandles>()
                    .settings_title
                    .read()
                    .map(|title| title.clone())
                    .unwrap_or_else(|_| "Shizi 设置".into()),
            )
            .inner_size(820.0, 600.0)
            .resizable(false)
            .minimizable(false)
            .maximizable(false)
            .center()
            .visible(SETTINGS_INITIAL_VISIBLE)
            .build()
            .map_err(|error| format!("创建设置窗口失败: {error}"))?;
    close_to_hide(&window);
    attach_app_shortcut_focus_listener(&window, app);
    Ok(window)
}

pub fn show_settings_window(app: &tauri::AppHandle) -> Result<(), String> {
    let window = ensure_settings_window(app)?;
    window.show().map_err(|error| error.to_string())?;
    window.unminimize().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;
    Ok(())
}
pub fn setup_close_to_hide(app: &tauri::App) {
    if let Some(window) = app.get_webview_window("main") {
        close_to_hide(&window);
        attach_app_shortcut_focus_listener(&window, app.handle());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_settings_window_starts_hidden() {
        assert!(!SETTINGS_INITIAL_VISIBLE);
    }
}
