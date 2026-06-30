use std::{str::FromStr, thread, time::Duration};

use tauri::{Manager, State};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

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

#[derive(Debug, PartialEq, Eq)]
enum ShortcutAction {
    OcrTranslate,
    SelectionTranslate,
}

// ponytail: 用 Shortcut 结构体相等比较，而非 to_string() 字符串比较——
// HotKey::into_string() 输出小写修饰键 + "KeyO"（"alt+KeyO"），与 "Alt+O" 不匹配。
fn classify_shortcut(shortcut: &Shortcut) -> ShortcutAction {
    let ocr = Shortcut::new(Some(Modifiers::ALT), Code::KeyO);
    if shortcut == &ocr {
        ShortcutAction::OcrTranslate
    } else {
        ShortcutAction::SelectionTranslate
    }
}

pub fn handle_global_shortcut(
    app: &tauri::AppHandle,
    shortcut: &tauri_plugin_global_shortcut::Shortcut,
    event: tauri_plugin_global_shortcut::ShortcutEvent,
) {
    if event.state != ShortcutState::Released {
        return;
    }

    match classify_shortcut(shortcut) {
        ShortcutAction::OcrTranslate => {
            let app_handle = app.clone();
            let state: State<'_, AppState> = app_handle.state();
            let state = state.inner().clone();
            tauri::async_runtime::spawn(async move {
                start_translation_from_ocr(app_handle, state).await;
            });
        }
        ShortcutAction::SelectionTranslate => handle_selection_translate(app),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_alt_o_as_ocr() {
        // 复现 register("Alt+O") 的真实路径：插件内部用 FromStr 解析字符串为 Shortcut。
        let shortcut = "Alt+O".parse::<Shortcut>().expect("Alt+O 应可解析");
        assert_eq!(classify_shortcut(&shortcut), ShortcutAction::OcrTranslate);
    }

    #[test]
    fn classify_alt_t_as_selection() {
        let shortcut = "Alt+T".parse::<Shortcut>().expect("Alt+T 应可解析");
        assert_eq!(
            classify_shortcut(&shortcut),
            ShortcutAction::SelectionTranslate
        );
    }
}