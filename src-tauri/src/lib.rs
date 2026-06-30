mod app;
mod core;
mod ui;

use std::{env, sync::Arc};

use app::{
    shortcuts::register_global_shortcuts,
    state::AppState,
    tray::setup_tray,
    window::{setup_close_to_hide, toggle_window},
};
use core::{
    llm::{LlmProvider, MockLlmProvider, OpenAiCompatibleProvider},
    translation::TranslationService,
};
use ui::web_popup::start_translation;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let provider_name = env::var("SHIZI_LLM_PROVIDER")
        .unwrap_or_else(|_| "openai-compatible".to_string());
    let provider: Arc<dyn LlmProvider> = match provider_name.as_str() {
        "mock" => Arc::new(MockLlmProvider),
        _ => Arc::new(OpenAiCompatibleProvider::from_env()),
    };
    let translation_service = TranslationService::new(provider);

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
        .invoke_handler(tauri::generate_handler![start_translation])
        .setup(|app| {
            setup_tray(app)?;
            setup_close_to_hide(app);
            register_global_shortcuts(app).map_err(|error| tauri::Error::Anyhow(error.into()))?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("启动应用失败");
}
