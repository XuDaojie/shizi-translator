use std::{thread, time::Duration};

use tauri::{Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::{
    app::state::AppState,
    core::selection::copy_selected_text,
    ui::web_popup::{show_translation_error, start_translation_from_text},
};

pub fn register_global_shortcuts(app: &tauri::App) -> Result<(), tauri_plugin_global_shortcut::Error> {
    app.global_shortcut().register("Alt+T")
}

pub fn handle_global_shortcut(
    app: &tauri::AppHandle,
    event: tauri_plugin_global_shortcut::ShortcutEvent,
) {
    if event.state != ShortcutState::Released {
        return;
    }

    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        thread::sleep(Duration::from_millis(40));

        let selected_text = match copy_selected_text() {
            Ok(text) => text,
            Err(error) => {
                show_translation_error(&app_handle, error.to_string());
                return;
            }
        };

        let state: State<'_, AppState> = app_handle.state();
        if let Err(error) = state.set_pending_source_text(selected_text.clone()) {
            show_translation_error(&app_handle, error);
            return;
        }

        if let Err(error) = start_translation_from_text(selected_text, app_handle.clone(), state.inner()) {
            show_translation_error(&app_handle, error);
        }
    });
}
