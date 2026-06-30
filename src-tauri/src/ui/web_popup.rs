use std::{sync::Arc, time::{SystemTime, UNIX_EPOCH}};

use tauri::{Emitter, Manager};

use crate::{
    app::state::AppState,
    core::{
        llm::{LlmProvider, MockLlmProvider, OpenAiCompatibleConfig, OpenAiCompatibleProvider},
        translation::{TranslationEvent, TranslationRequest, TranslationService, TranslationSessionId},
    },
};

pub const TRANSLATION_EVENT: &str = "translation:event";

pub fn emit_translation_event(
    app: &tauri::AppHandle,
    event: TranslationEvent,
) -> Result<(), tauri::Error> {
    app.emit(TRANSLATION_EVENT, event)
}

#[tauri::command]
pub async fn start_translation(
    text: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let source_text = text.trim().to_string();
    if source_text.is_empty() {
        return Err("请输入要翻译的文本".to_string());
    }

    let config = state.config_store.get().map_err(|error| error.to_string())?;
    let provider: Arc<dyn LlmProvider> = match config.provider.as_str() {
        "mock" => Arc::new(MockLlmProvider),
        _ => Arc::new(OpenAiCompatibleProvider::new(OpenAiCompatibleConfig::from(
            config.openai_compatible,
        ))),
    };
    let translation_service = TranslationService::new(provider);

    let session_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "无法创建翻译会话".to_string())?
        .as_millis()
        .to_string();

    let request = TranslationRequest {
        session_id: TranslationSessionId(session_id.clone()),
        source_text,
        target_lang: config.target_lang,
    };

    let app_handle = app.clone();

    tauri::async_runtime::spawn(async move {
        let failed_session_id = request.session_id.clone();
        let result = translation_service
            .translate_with(request, |event| {
                let _ = emit_translation_event(&app_handle, event);
            })
            .await;

        if let Err(error) = result {
            let retryable = error.retryable();
            let _ = emit_translation_event(
                &app_handle,
                TranslationEvent::Failed {
                    session_id: failed_session_id,
                    message: error.to_string(),
                    retryable,
                },
            );
        }
    });

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }

    Ok(session_id)
}
