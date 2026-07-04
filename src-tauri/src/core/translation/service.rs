use std::{sync::Arc, sync::Mutex};

use crate::core::llm::{LlmError, LlmProvider, LlmStreamEvent};
use tokio_util::sync::CancellationToken;

use super::{
    TokenUsage, TranslationEvent, TranslationRequest,
};

#[derive(Debug, thiserror::Error)]
pub enum TranslationError {
    #[error(transparent)]
    Llm(#[from] LlmError),
}

impl TranslationError {
    pub fn retryable(&self) -> bool {
        match self {
            Self::Llm(error) => error.retryable(),
        }
    }
}

#[derive(Clone)]
pub struct TranslationService {
    provider: Arc<dyn LlmProvider>,
}

impl TranslationService {
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
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
        let full_text = Arc::new(Mutex::new(String::new()));
        let usage: Arc<Mutex<Option<TokenUsage>>> = Arc::new(Mutex::new(None));
        let delta_text = full_text.clone();
        let usage_slot = usage.clone();
        let delta_session_id = request.session_id.clone();
        let delta_service = request.service.clone();

        self.provider
            .stream_translate(&request, &mut |ev| {
                match ev {
                    LlmStreamEvent::Delta(text) => {
                        if let Ok(mut t) = delta_text.lock() {
                            t.push_str(&text);
                        }
                        emit(TranslationEvent::Delta {
                            session_id: delta_session_id.clone(),
                            service: delta_service.clone(),
                            text,
                        });
                    }
                    LlmStreamEvent::Usage(u) => {
                        if collect_usage {
                            if let Ok(mut slot) = usage_slot.lock() {
                                *slot = Some(u);
                            }
                        }
                    }
                }
            }, &cancel)
            .await?;

        let full_text = full_text
            .lock()
            .map(|text| text.clone())
            .unwrap_or_default();

        if cancel.is_cancelled() {
            emit(TranslationEvent::Cancelled {
                session_id: request.session_id,
                service: request.service,
            });
        } else {
            let usage = usage
                .lock()
                .map(|slot| slot.clone())
                .unwrap_or(None);
            emit(TranslationEvent::Finished {
                session_id: request.session_id,
                service: request.service,
                full_text,
                usage,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::llm::{LlmProvider, LlmStreamEvent};
    use crate::core::translation::{
        TokenUsage, TranslationInput, TranslationRequest,
        TranslationServiceMeta,
        TranslationSessionId,
    };
    use std::sync::{Arc, Mutex};
    use tokio_util::sync::CancellationToken;

    struct CancelAwareFakeProvider {
        deltas_emitted: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl LlmProvider for CancelAwareFakeProvider {
        async fn stream_translate(
            &self,
            _request: &TranslationRequest,
            on_event: &mut (dyn FnMut(LlmStreamEvent) + Send),
            cancel: &CancellationToken,
        ) -> Result<(), LlmError> {
            let chunks = ["a", "b", "c"];
            for chunk in chunks {
                tokio::select! {
                    _ = cancel.cancelled() => return Ok(()),
                    _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {}
                }
                on_event(LlmStreamEvent::Delta(chunk.to_string()));
                self.deltas_emitted.lock().unwrap().push(chunk.to_string());
            }
            Ok(())
        }
    }

    struct UsageFakeProvider;

    #[async_trait::async_trait]
    impl LlmProvider for UsageFakeProvider {
        async fn stream_translate(
            &self,
            _request: &TranslationRequest,
            on_event: &mut (dyn FnMut(LlmStreamEvent) + Send),
            _cancel: &CancellationToken,
        ) -> Result<(), LlmError> {
            on_event(LlmStreamEvent::Delta("你好".to_string()));
            on_event(LlmStreamEvent::Usage(TokenUsage {
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
        }
    }

    fn request() -> TranslationRequest {
        TranslationRequest {
            session_id: TranslationSessionId("test-session".to_string()),
            input: TranslationInput::ManualText("hi".to_string()),
            target_lang: "中文".to_string(),
            service: fake_service(),
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
}


