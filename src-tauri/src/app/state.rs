use crate::core::translation::TranslationService;

#[derive(Clone)]
pub struct AppState {
    pub translation_service: TranslationService,
}

impl AppState {
    pub fn new(translation_service: TranslationService) -> Self {
        Self { translation_service }
    }
}
