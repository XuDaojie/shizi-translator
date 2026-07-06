use tauri::{Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

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
            .title("Shizi - 设置")
            .inner_size(820.0, 600.0)
            .resizable(false)
            .minimizable(false)
            .maximizable(false)
            .center()
            .visible(SETTINGS_INITIAL_VISIBLE)
            .build()
            .map_err(|error| format!("创建设置窗口失败: {error}"))?;
    close_to_hide(&window);
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
