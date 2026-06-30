mod app;
mod core;
mod ui;

use std::sync::Arc;

use app::{
    shortcuts::register_global_shortcuts,
    state::AppState,
    tray::setup_tray,
    window::{setup_close_to_hide, toggle_window},
};
use core::{llm::MockLlmProvider, translation::TranslationService};
use ui::web_popup::start_mock_translation;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let translation_service = TranslationService::new(Arc::new(MockLlmProvider));

    tauri::Builder::default()
        .manage(AppState::new(translation_service))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        toggle_window(app);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![start_mock_translation])
        .setup(|app| {
            setup_tray(app)?;
            setup_close_to_hide(app);
            register_global_shortcuts(app).map_err(|error| tauri::Error::Anyhow(error.into()))?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("启动应用失败");
}
