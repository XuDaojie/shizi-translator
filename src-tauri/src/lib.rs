mod app;
mod core;
mod platform;
mod ui;

use app::{
    shortcuts::{handle_global_shortcut, register_global_shortcuts},
    state::AppState,
    tray::setup_tray,
    window::setup_close_to_hide,
};
use core::config::ConfigStore;
use tauri::Manager;
use ui::{
    config::{get_app_config, save_app_config},
    overlay::{
        cancel_capture, get_capture_frame_bytes, get_capture_frame_meta, show_overlay,
        submit_capture_region,
    },
    web_popup::{
        cancel_translation, retry_translation, start_translation, take_pending_source_text,
    },
};

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
            cancel_translation,
            retry_translation,
            take_pending_source_text,
            get_app_config,
            save_app_config,
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
            register_global_shortcuts(app).map_err(|error| tauri::Error::Anyhow(error.into()))?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("启动应用失败");
}
