mod app;
mod core;
mod platform;
mod ui;

use app::{
    shortcuts::{handle_global_shortcut, register_global_shortcuts},
    state::AppState,
    tray::setup_tray,
    window::setup_close_to_hide,
    popup_window::ensure_popup_window,
};
use core::config::ConfigStore;
use tauri::Manager;
use ui::{
    config::{get_app_config, save_app_config, open_settings},
    ocr_popup::trigger_ocr_translation,
    overlay::{
        cancel_capture, ensure_overlay, get_capture_frame_bytes, get_capture_frame_meta,
        show_overlay, submit_capture_region,
    },
    web_popup::{
        cancel_translation, retry_translation, start_translation, take_pending_source_text,
    },
};

use crate::core::config::AppConfig;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    handle_global_shortcut(app, shortcut, event);
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            start_translation,
            trigger_ocr_translation,
            cancel_translation,
            retry_translation,
            take_pending_source_text,
            get_app_config,
            save_app_config,
            open_settings,
            get_capture_frame_meta,
            get_capture_frame_bytes,
            submit_capture_region,
            cancel_capture,
            show_overlay,
        ])
        .setup(|app| {
            let config_store = ConfigStore::load(app.handle())
                .map_err(|error| tauri::Error::Anyhow(error.into()))?;
            app.manage(AppState::new(config_store));
            setup_tray(app)?;
            setup_close_to_hide(app);
            register_global_shortcuts(app)
                .map_err(|error| tauri::Error::Anyhow(error.into()))?;

            // 按窗口策略预创建弹窗与 overlay
            let config = app.state::<AppState>().config_store.get().unwrap_or_else(|_| AppConfig::from_env());
            let _ = ensure_popup_window(app.handle(), &config);
            let _ = ensure_overlay(app.handle());

            // 按 is_configured 决定主窗口显隐
            if config.is_configured() {
                // 已配置：隐藏主窗口（驻留托盘）
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            } else {
                // 未配置：显示主窗口（设置页引导）
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("启动应用失败");
}
