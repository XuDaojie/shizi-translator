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
    web_popup::{start_translation, take_pending_source_text},
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    handle_global_shortcut(app, event);
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            start_translation,
            take_pending_source_text,
            get_app_config,
            save_app_config,
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
