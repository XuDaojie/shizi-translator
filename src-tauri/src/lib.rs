mod app;
mod core;
mod platform;
mod ui;

use app::{
    popup_window::ensure_popup_window,
    shortcuts::{handle_global_shortcut, register_global_shortcuts_at_startup},
    state::AppState,
    tray::setup_tray,
    window::{ensure_settings_window, setup_close_to_hide},
};
use core::config::ConfigStore;
use tauri::Manager;
use ui::{
    config::{get_app_config, get_shortcut_conflicts, open_settings, save_app_config},
    ocr_popup::trigger_ocr_translation,
    overlay::{
        cancel_capture, ensure_overlay, get_capture_frame_bytes, get_capture_frame_meta,
        show_overlay, submit_capture_region,
    },
    service_probe::{list_service_models, validate_service_credential},
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
            get_shortcut_conflicts,
            open_settings,
            list_service_models,
            validate_service_credential,
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

            let config = app
                .state::<AppState>()
                .config_store
                .get()
                .unwrap_or_else(|_| AppConfig::from_env());
            let shortcut_conflicts = register_global_shortcuts_at_startup(app.handle(), &config);
            let _ = app
                .state::<AppState>()
                .set_shortcut_conflicts(shortcut_conflicts);

            // 按窗口策略预创建弹窗与 overlay
            let _ = ensure_popup_window(app.handle(), &config);
            let _ = ensure_settings_window(app.handle());
            let _ = ensure_overlay(app.handle());

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("启动应用失败");
}
