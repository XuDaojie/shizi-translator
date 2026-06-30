use crate::core::translation::TranslationRequest;

pub trait LlmProvider: Send + Sync {
    fn stream_translate(&self, request: &TranslationRequest) -> Vec<String>;
}
