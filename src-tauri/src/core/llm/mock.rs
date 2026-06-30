use std::{thread, time::Duration};

use crate::core::{
    llm::{LlmError, LlmProvider},
    translation::TranslationRequest,
};

pub struct MockLlmProvider;

#[async_trait::async_trait]
impl LlmProvider for MockLlmProvider {
    async fn stream_translate(
        &self,
        request: &TranslationRequest,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<(), LlmError> {
        let chunks = [
            "[Mock 翻译] ".to_string(),
            request.source_text().to_string(),
            " -> ".to_string(),
            request.target_lang.clone(),
        ];

        for chunk in chunks {
            on_delta(chunk);
            thread::sleep(Duration::from_millis(180));
        }

        Ok(())
    }
}
