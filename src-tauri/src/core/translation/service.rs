use std::{sync::Arc, sync::Mutex};

use tokio_util::sync::CancellationToken;

use super::{
    TokenUsage, TranslationEvent, TranslationRequest,
};
use crate::core::translation::provider::{
    TranslationError, TranslationProvider, TranslationStreamEvent,
};

#[derive(Clone)]
pub struct TranslationService {
    provider: Arc<dyn TranslationProvider>,
}

impl TranslationService {
    pub fn new(provider: Arc<dyn TranslationProvider>) -> Self {
        Self { provider }
    }

    pub async fn translate_with<F>(
        &self,
        request: TranslationRequest,
        collect_usage: bool,
        cancel: CancellationToken,
        mut emit: F,
    ) -> Result<(), TranslationError>
    where
        F: FnMut(TranslationEvent) + Send,
    {
        log::info!(
            "翻译开始: service={} protocol={} session={}",
            request.service.service_name,
            request.service.protocol,
            request.session_id.0
        );

        let full_text = Arc::new(Mutex::new(String::new()));
        let usage: Arc<Mutex<Option<TokenUsage>>> = Arc::new(Mutex::new(None));
        let detected: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let delta_text = full_text.clone();
        let usage_slot = usage.clone();
        let detected_slot = detected.clone();
        let delta_session_id = request.session_id.clone();
        let delta_service = request.service.clone();

        self.provider
            .translate(&request, &mut |ev| match ev {
                TranslationStreamEvent::Delta(text) => {
                    if let Ok(mut t) = delta_text.lock() {
                        t.push_str(&text);
                    }
                    emit(TranslationEvent::Delta {
                        session_id: delta_session_id.clone(),
                        service: delta_service.clone(),
                        text,
                    });
                }
                TranslationStreamEvent::Usage(u) => {
                    if collect_usage {
                        if let Ok(mut slot) = usage_slot.lock() {
                            *slot = Some(u);
                        }
                    }
                }
                TranslationStreamEvent::DetectedSourceLang(lang) => {
                    if let Ok(mut slot) = detected_slot.lock() {
                        *slot = Some(lang);
                    }
                }
            }, &cancel)
            .await?;

        let full_text = full_text.lock().map(|t| t.clone()).unwrap_or_default();
        let detected = detected.lock().map(|d| d.clone()).unwrap_or(None);

        if cancel.is_cancelled() {
            log::warn!(
                "翻译取消: service={} session={}",
                request.service.service_name,
                request.session_id.0
            );
            emit(TranslationEvent::Cancelled {
                session_id: request.session_id,
                service: request.service,
            });
        } else {
            let usage = usage.lock().map(|slot| slot.clone()).unwrap_or(None);
            log::info!(
                "翻译完成: service={} session={} len={}",
                request.service.service_name,
                request.session_id.0,
                full_text.chars().count()
            );
            emit(TranslationEvent::Finished {
                session_id: request.session_id,
                service: request.service,
                full_text,
                usage,
                detected_source_lang: detected,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::translation::provider::{TranslationProvider, TranslationStreamEvent};
    use crate::core::translation::{
        TokenUsage, TranslationInput, TranslationPromptConfig, TranslationRequest,
        TranslationServiceMeta, TranslationSessionId,
    };
    use std::sync::{Arc, Mutex};
    use tokio_util::sync::CancellationToken;

    struct CancelAwareFakeProvider {
        deltas_emitted: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl TranslationProvider for CancelAwareFakeProvider {
        async fn translate(
            &self,
            _request: &TranslationRequest,
            on_event: &mut (dyn FnMut(TranslationStreamEvent) + Send),
            cancel: &CancellationToken,
        ) -> Result<(), TranslationError> {
            let chunks = ["a", "b", "c"];
            for chunk in chunks {
                tokio::select! {
                    _ = cancel.cancelled() => return Ok(()),
                    _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {}
                }
                on_event(TranslationStreamEvent::Delta(chunk.to_string()));
                self.deltas_emitted.lock().unwrap().push(chunk.to_string());
            }
            Ok(())
        }
    }

    struct UsageFakeProvider;

    #[async_trait::async_trait]
    impl TranslationProvider for UsageFakeProvider {
        async fn translate(
            &self,
            _request: &TranslationRequest,
            on_event: &mut (dyn FnMut(TranslationStreamEvent) + Send),
            _cancel: &CancellationToken,
        ) -> Result<(), TranslationError> {
            on_event(TranslationStreamEvent::Delta("你好".to_string()));
            on_event(TranslationStreamEvent::Usage(TokenUsage {
                input_tokens: 27,
                output_tokens: 18,
            }));
            Ok(())
        }
    }

    fn fake_service() -> TranslationServiceMeta {
        TranslationServiceMeta {
            service_instance_id: "test".to_string(),
            service_name: "test".to_string(),
            service_type: "llm".to_string(),
            protocol: "mock".to_string(),
            model_name: "mock-model".to_string(),
        }
    }

    fn request() -> TranslationRequest {
        TranslationRequest {
            session_id: TranslationSessionId("test-session".to_string()),
            input: TranslationInput::ManualText("hi".to_string()),
            source_lang: String::new(),
            target_lang: "中文".to_string(),
            service: fake_service(),
            prompts: Default::default(),
        }
    }

    #[tokio::test]
    async fn emits_cancelled_when_cancelled_before_completion() {
        let emitted = Arc::new(Mutex::new(Vec::new()));
        let provider = CancelAwareFakeProvider {
            deltas_emitted: emitted.clone(),
        };
        let service = TranslationService::new(Arc::new(provider));
        let cancel = CancellationToken::new();
        let cancel_for_task = cancel.clone();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_for_task = events.clone();

        let handle = tokio::spawn(async move {
            service
                .translate_with(request(), true, cancel_for_task, |event| {
                    events_for_task.lock().unwrap().push(event);
                })
                .await
        });

        tokio::time::sleep(std::time::Duration::from_millis(60)).await;
        cancel.cancel();

        handle.await.expect("task 未 panic").expect("应返回 Ok");

        let events = events.lock().unwrap();
        let types: Vec<&str> = events.iter().map(|e| match e {
            TranslationEvent::Started { .. } => "started",
            TranslationEvent::Delta { .. } => "delta",
            TranslationEvent::Finished { .. } => "finished",
            TranslationEvent::Failed { .. } => "failed",
            TranslationEvent::Cancelled { .. } => "cancelled",
        }).collect();

        assert!(types.contains(&"cancelled"), "应 emit Cancelled: {:?}", types);
        assert!(!types.contains(&"finished"), "取消时不应 emit Finished");
    }

    #[tokio::test]
    async fn emits_finished_when_not_cancelled() {
        let emitted = Arc::new(Mutex::new(Vec::new()));
        let provider = CancelAwareFakeProvider {
            deltas_emitted: emitted.clone(),
        };
        let service = TranslationService::new(Arc::new(provider));
        let cancel = CancellationToken::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_for_task = events.clone();

        service
            .translate_with(request(), true, cancel, |event| {
                events_for_task.lock().unwrap().push(event);
            })
            .await
            .expect("应返回 Ok");

        let events = events.lock().unwrap();
        let types: Vec<&str> = events.iter().map(|e| match e {
            TranslationEvent::Started { .. } => "started",
            TranslationEvent::Delta { .. } => "delta",
            TranslationEvent::Finished { .. } => "finished",
            TranslationEvent::Failed { .. } => "failed",
            TranslationEvent::Cancelled { .. } => "cancelled",
        }).collect();

        assert!(types.contains(&"finished"), "未取消应 emit Finished: {:?}", types);
        assert!(!types.contains(&"cancelled"), "未取消不应 emit Cancelled");
    }

    #[tokio::test]
    async fn finished_carries_usage_when_collect_enabled() {
        let service = TranslationService::new(Arc::new(UsageFakeProvider));
        let cancel = CancellationToken::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_for_task = events.clone();

        service
            .translate_with(
                request(),
                true,
                cancel,
                |event| events_for_task.lock().unwrap().push(event),
            )
            .await
            .expect("应返回 Ok");

        let events = events.lock().unwrap();
        let usage = events.iter().find_map(|e| match e {
            TranslationEvent::Finished { usage, .. } => usage.clone(),
            _ => None,
        });
        assert_eq!(
            usage,
            Some(TokenUsage {
                input_tokens: 27,
                output_tokens: 18
            })
        );
    }

    #[tokio::test]
    async fn finished_usage_none_when_collect_disabled() {
        let service = TranslationService::new(Arc::new(UsageFakeProvider));
        let cancel = CancellationToken::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_for_task = events.clone();

        service
            .translate_with(
                request(),
                false,
                cancel,
                |event| events_for_task.lock().unwrap().push(event),
            )
            .await
            .expect("应返回 Ok");

        let events = events.lock().unwrap();
        let usage = events.iter().find_map(|e| match e {
            TranslationEvent::Finished { usage, .. } => usage.clone(),
            _ => None,
        });
        assert_eq!(usage, None);
    }

    /// 可按预设 chunks 输出 Delta 的 fake provider，用于 detected 透传测试。
    struct DetectFakeProvider {
        chunks: Vec<String>,
        detected: Option<String>,
    }

    #[async_trait::async_trait]
    impl TranslationProvider for DetectFakeProvider {
        async fn translate(
            &self,
            _request: &TranslationRequest,
            on_event: &mut (dyn FnMut(TranslationStreamEvent) + Send),
            _cancel: &CancellationToken,
        ) -> Result<(), TranslationError> {
            if let Some(lang) = &self.detected {
                on_event(TranslationStreamEvent::DetectedSourceLang(lang.clone()));
            }
            for chunk in &self.chunks {
                on_event(TranslationStreamEvent::Delta(chunk.clone()));
            }
            Ok(())
        }
    }

    fn request_with_source(source_lang: &str) -> TranslationRequest {
        TranslationRequest {
            session_id: TranslationSessionId("test-session".to_string()),
            input: TranslationInput::ManualText("hello".to_string()),
            source_lang: source_lang.to_string(),
            target_lang: "中文".to_string(),
            service: fake_service(),
            prompts: TranslationPromptConfig::default(),
        }
    }

    fn collect_deltas(events: &[TranslationEvent]) -> String {
        events
            .iter()
            .filter_map(|e| match e {
                TranslationEvent::Delta { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    fn collect_detected(events: &[TranslationEvent]) -> Option<String> {
        events.iter().find_map(|e| match e {
            TranslationEvent::Finished {
                detected_source_lang, ..
            } => detected_source_lang.clone(),
            _ => None,
        })
    }

    async fn run_translate(provider: DetectFakeProvider, source_lang: &str) -> Vec<TranslationEvent> {
        let service = TranslationService::new(Arc::new(provider));
        let cancel = CancellationToken::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_for_task = events.clone();
        service
            .translate_with(request_with_source(source_lang), true, cancel, |event| {
                events_for_task.lock().unwrap().push(event);
            })
            .await
            .expect("应返回 Ok");
        let events = events.lock().unwrap();
        events.clone()
    }

    #[tokio::test]
    async fn finished_carries_detected_source_lang_from_event() {
        let events = run_translate(
            DetectFakeProvider {
                chunks: vec!["译文内容".to_string()],
                detected: Some("英语".to_string()),
            },
            "auto",
        )
        .await;
        assert_eq!(collect_deltas(&events), "译文内容");
        assert_eq!(collect_detected(&events), Some("英语".to_string()));
    }

    #[tokio::test]
    async fn finished_detected_none_when_provider_does_not_emit() {
        let events = run_translate(
            DetectFakeProvider {
                chunks: vec!["译文".to_string()],
                detected: None,
            },
            "auto",
        )
        .await;
        assert_eq!(collect_deltas(&events), "译文");
        assert_eq!(collect_detected(&events), None);
    }
}
