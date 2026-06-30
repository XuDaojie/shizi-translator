use std::{thread, time::Duration};

use tauri::{Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::{
    app::state::AppState,
    core::{selection::copy_selected_text, translation::TranslationInput},
    ui::{
        ocr_popup::start_translation_from_ocr,
        web_popup::{show_translation_error, start_translation_from_input},
    },
};

pub fn register_global_shortcuts(app: &tauri::App) -> Result<(), tauri_plugin_global_shortcut::Error> {
    app.global_shortcut().register("Alt+T")?;
    app.global_shortcut().register("Alt+O")
}

pub fn handle_global_shortcut(
    app: &tauri::AppHandle,
    shortcut: &tauri_plugin_global_shortcut::Shortcut,
    event: tauri_plugin_global_shortcut::ShortcutEvent,
) {
    if event.state != ShortcutState::Released {
        return;
    }

    match shortcut.to_string().as_str() {
        "Alt+O" => {
            let app_handle = app.clone();
            let state: State<'_, AppState> = app_handle.state();
            let state = state.inner().clone();
            tauri::async_runtime::spawn(async move {
                start_translation_from_ocr(app_handle, state).await;
            });
        }
        _ => handle_selection_translate(app),
    }
}

fn handle_selection_translate(app: &tauri::AppHandle) {
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

        if let Err(error) = start_translation_from_input(
            TranslationInput::SelectedText(selected_text),
            app_handle.clone(),
            state.inner(),
        ) {
            show_translation_error(&app_handle, error);
        }
    });
}
