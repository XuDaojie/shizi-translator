use std::{sync::Arc, time::Duration};

use crate::core::llm::LlmProvider;

use super::{TranslationEvent, TranslationRequest};

#[derive(Debug)]
pub struct TranslationError;

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
        emit(TranslationEvent::Started {
            session_id: request.session_id.clone(),
            source_text: request.source_text.clone(),
        });

        let chunks = self.provider.stream_translate(&request);
        let mut full_text = String::new();

        for chunk in chunks {
            full_text.push_str(&chunk);
            emit(TranslationEvent::Delta {
                session_id: request.session_id.clone(),
                text: chunk,
            });
            std::thread::sleep(Duration::from_millis(180));
        }

        emit(TranslationEvent::Finished {
            session_id: request.session_id,
            full_text,
        });

        Ok(())
    }
}
