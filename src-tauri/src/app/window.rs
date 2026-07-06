use tauri::{Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

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

pub fn show_settings_window(app: &tauri::AppHandle) -> Result<(), String> {
    let window = match app.get_webview_window("settings") {
        Some(window) => window,
        None => {
            let window =
                WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
                    .title("Shizi - 设置")
                    .inner_size(820.0, 600.0)
                    .resizable(false)
                    .minimizable(false)
                    .maximizable(false)
                    .center()
                    .build()
                    .map_err(|error| format!("创建设置窗口失败: {error}"))?;
            close_to_hide(&window);
            window
        }
    };
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
