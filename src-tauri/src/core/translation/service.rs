use std::{sync::Arc, sync::Mutex};

use crate::core::llm::{LlmError, LlmProvider};

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
        mut emit: F,
    ) -> Result<(), TranslationError>
    where
        F: FnMut(TranslationEvent) + Send,
    {
        let full_text = Arc::new(Mutex::new(String::new()));
        let delta_text = full_text.clone();
        let delta_session_id = request.session_id.clone();

        self.provider
            .stream_translate(&request, &mut |chunk| {
                if let Ok(mut text) = delta_text.lock() {
                    text.push_str(&chunk);
                }
                emit(TranslationEvent::Delta {
                    session_id: delta_session_id.clone(),
                    text: chunk,
                });
            })
            .await?;

        let full_text = full_text.lock().map(|text| text.clone()).unwrap_or_default();

        emit(TranslationEvent::Finished {
            session_id: request.session_id,
            full_text,
        });

        Ok(())
    }
}
