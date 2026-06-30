use crate::core::{llm::LlmProvider, translation::TranslationRequest};

pub struct MockLlmProvider;

impl LlmProvider for MockLlmProvider {
    fn stream_translate(&self, request: &TranslationRequest) -> Vec<String> {
        vec![
            "[Mock 翻译] ".to_string(),
            request.source_text.clone(),
            " -> ".to_string(),
            request.target_lang.clone(),
        ]
    }
}
