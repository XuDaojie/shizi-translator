use std::{
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures_util::future::join_all;
use tauri::Emitter;
use tauri::Manager;
use tokio_util::sync::CancellationToken;

use crate::{
    app::{popup_window, state::AppState},
    core::{
        config::{AppConfig, ServiceInstanceConfig},
        llm::provider_for_service,
        logging::redact_text,
        translation::{
            batch, TranslationEvent, TranslationInput, TranslationService,
            TranslationServiceMeta, TranslationSessionId,
        },
    },
};

pub const TRANSLATION_EVENT: &str = "translation:event";

pub fn emit_translation_event(
    app: &tauri::AppHandle,
    event: TranslationEvent,
) -> Result<(), tauri::Error> {
    app.emit(TRANSLATION_EVENT, event)
}

/// 唤起翻译弹窗（show + 光标定位）。触发翻译前调用，修正旧版依赖窗口已可见的缺陷。
pub fn show_translation_popup(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    popup_window::show_popup(app, config)
}

pub fn start_translation_from_text(
    text: String,
    app: tauri::AppHandle,
    state: &AppState,
) -> Result<String, String> {
    start_translation_from_input(TranslationInput::ManualText(text), app, state)
}

fn cache_automatic_source_text_for_popup(
    input: &TranslationInput,
    source_text: &str,
    state: &AppState,
) -> Result<(), String> {
    match input {
        TranslationInput::ManualText(_) => Ok(()),
        TranslationInput::SelectedText(_) | TranslationInput::OcrText { .. } => {
            state.set_pending_source_text(source_text.to_string())
        }
    }
}

pub fn start_translation_from_input(
    input: TranslationInput,
    app: tauri::AppHandle,
    state: &AppState,
) -> Result<String, String> {
    let source_text = input.text().trim().to_string();
    if source_text.is_empty() {
        return Err("请输入要翻译的文本".to_string());
    }
    let input = match input {
        TranslationInput::ManualText(_) => TranslationInput::ManualText(source_text),
        TranslationInput::SelectedText(_) => TranslationInput::SelectedText(source_text),
        TranslationInput::OcrText { image_id, .. } => TranslationInput::OcrText {
            text: source_text,
            image_id,
        },
    };

    let config = state
        .config_store
        .get()
        .map_err(|error| error.to_string())?;

    let batch_id = create_session_id()?;

    let requests = batch::build_batch_requests(
        input.clone(),
        config.target_lang.clone(),
        config.default_source_lang.clone(),
        &config.services,
        &batch_id,
    )?;

    let log_level = config.log_level.clone();
    log::info!(
        "翻译入口: source_type={} {}",
        input.kind(),
        redact_text(&input.text(), &log_level)
    );

    let enabled_services: Vec<ServiceInstanceConfig> = config
        .services
        .iter()
        .filter(|s| s.enabled)
        .cloned()
        .collect();

    let cancel_token = CancellationToken::new();
    let generation = state.begin_translation_overriding(cancel_token.clone())?;

    if let Err(error) = state.set_last_translation_input(input.clone()) {
        let _ = state.finish_translation_if_current(generation);
        return Err(error);
    }
    if let Err(error) =
        cache_automatic_source_text_for_popup(&input, input.text(), state)
    {
        let _ = state.finish_translation_if_current(generation);
        return Err(error);
    }

    // ponytail: 短暂延迟让弹窗渲染完成
    thread::sleep(Duration::from_millis(120));

    // 为每个服务发送 Started 事件
    for request in &requests {
        emit_translation_event(
            &app,
            TranslationEvent::Started {
                session_id: request.session_id.clone(),
                service: request.service.clone(),
                source_text: request.source_text().to_string(),
                source_type: request.input.kind().to_string(),
            },
        )
        .map_err(|error| {
            let _ = state.finish_translation_if_current(generation);
            error.to_string()
        })?;
    }

    let app_handle = app.clone();
    let state_for_task = state.clone();
    let collect_usage = config.collect_usage;

    tauri::async_runtime::spawn(async move {
        let jobs = requests.into_iter().zip(enabled_services).map(|(request, service_config)| {
            let app_handle = app_handle.clone();
            let cancel = cancel_token.clone();
            async move {
                let failed_session_id = request.session_id.clone();
                let failed_service = request.service.clone();

                match provider_for_service(&service_config) {
                    Ok(provider) => {
                        let translation_service = TranslationService::new(provider);
                        let result = translation_service
                            .translate_with(request, collect_usage, cancel, |event| {
                                let _ = emit_translation_event(&app_handle, event);
                            })
                            .await;

                        if let Err(error) = result {
                            log::error!(
                                "翻译失败: service={} session={} retryable={} err={}",
                                failed_service.service_name,
                                failed_session_id.0,
                                error.retryable(),
                                error
                            );
                            let _ = emit_translation_event(
                                &app_handle,
                                TranslationEvent::Failed {
                                    session_id: failed_session_id,
                                    service: failed_service,
                                    message: error.to_string(),
                                    retryable: error.retryable(),
                                },
                            );
                        }
                    }
                    Err(message) => {
                        log::error!(
                            "provider 初始化失败: service={} err={}",
                            failed_service.service_name,
                            message
                        );
                        let _ = emit_translation_event(
                            &app_handle,
                            TranslationEvent::Failed {
                                session_id: failed_session_id,
                                service: failed_service,
                                message,
                                retryable: false,
                            },
                        );
                    }
                }
            }
        });

        join_all(jobs).await;
        let _ = state_for_task.finish_translation_if_current(generation);
    });

    Ok(batch_id)
}

pub fn show_translation_error(app: &tauri::AppHandle, message: impl Into<String>) {
    let session_id = create_session_id().unwrap_or_else(|_| "selection-error".to_string());
    let config = app
        .state::<AppState>()
        .config_store
        .get()
        .ok();
    if let Some(config) = config {
        let _ = show_translation_popup(app, &config);
    }
    let _ = emit_translation_event(
        app,
        TranslationEvent::Failed {
            session_id: TranslationSessionId(session_id),
            service: TranslationServiceMeta::default(),
            message: message.into(),
            retryable: false,
        },
    );
}

#[tauri::command]
pub async fn take_pending_source_text(
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    state.take_pending_source_text()
}

#[tauri::command]
pub async fn start_translation(
    text: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    start_translation_from_text(text, app, state.inner())
}

#[tauri::command]
pub async fn cancel_translation(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.cancel_current_translation()
}

#[tauri::command]
pub async fn retry_translation(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let input = state
        .take_last_translation_input()?
        .ok_or_else(|| "没有可重试的翻译".to_string())?;
    start_translation_from_input(input, app, state.inner())
}

fn create_session_id() -> Result<String, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "无法创建翻译会话".to_string())
        .map(|duration| duration.as_millis().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::{AppConfig, ConfigStore};
    use std::{
        path::PathBuf,
        sync::{Arc, RwLock},
    };

    fn app_state() -> AppState {
        AppState::new(ConfigStore::from_parts_for_test(
            PathBuf::from("unused-config.json"),
            Arc::new(RwLock::new(AppConfig::from_env())),
        ))
    }

    #[test]
    fn automatic_translation_source_text_is_cached_for_popup_refill() {
        let state = app_state();
        let input = TranslationInput::OcrText {
            text: " OCR 原文 ".to_string(),
            image_id: None,
        };

        cache_automatic_source_text_for_popup(&input, "OCR 原文", &state).expect("缓存 OCR 原文");

        assert_eq!(
            state.take_pending_source_text().expect("读取待回填原文"),
            Some("OCR 原文".to_string())
        );
    }

    #[test]
    fn manual_translation_source_text_is_not_cached_for_popup_refill() {
        let state = app_state();
        let input = TranslationInput::ManualText("手动输入".to_string());

        cache_automatic_source_text_for_popup(&input, "手动输入", &state).expect("手动输入不需要缓存");

        assert_eq!(
            state.take_pending_source_text().expect("读取待回填原文"),
            None
        );
    }
}
