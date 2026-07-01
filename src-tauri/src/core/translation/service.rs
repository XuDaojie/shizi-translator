use std::{sync::Arc, sync::Mutex};

use crate::core::llm::{LlmError, LlmProvider, LlmStreamEvent};
use tokio_util::sync::CancellationToken;

use super::{TranslationEvent, TranslationRequest};

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
        cancel: CancellationToken,
        mut emit: F,
    ) -> Result<(), TranslationError>
    where
        F: FnMut(TranslationEvent) + Send,
    {
        let full_text = Arc::new(Mutex::new(String::new()));
        let delta_text = full_text.clone();
        let delta_session_id = request.session_id.clone();

        self.provider
            .stream_translate(&request, &mut |ev| {
                if let LlmStreamEvent::Delta(text) = ev {
                    if let Ok(mut t) = delta_text.lock() {
                        t.push_str(&text);
                    }
                    emit(TranslationEvent::Delta {
                        session_id: delta_session_id.clone(),
                        text,
                    });
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
            });
        } else {
            emit(TranslationEvent::Finished {
                session_id: request.session_id,
                full_text,
                usage: None,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::llm::{LlmProvider, LlmStreamEvent};
    use crate::core::translation::{TranslationInput, TranslationRequest, TranslationSessionId};
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

    fn request() -> TranslationRequest {
        TranslationRequest {
            session_id: TranslationSessionId("test-session".to_string()),
            input: TranslationInput::ManualText("hi".to_string()),
            target_lang: "中文".to_string(),
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
                .translate_with(request(), cancel_for_task, |event| {
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
            .translate_with(request(), cancel, |event| {
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
}
