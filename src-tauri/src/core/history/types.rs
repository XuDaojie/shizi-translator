use crate::core::translation::TranslationInput;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryTrigger {
    Manual,
    Selection,
    Screenshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryResultStatus {
    Pending,
    Success,
    Error,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct NewHistorySession {
    pub id: String,
    pub batch_id: String,
    pub trigger: HistoryTrigger,
    pub source_lang: String,
    pub target_lang: String,
    pub source_text: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct NewHistoryResult {
    pub session_id: String,
    pub service_instance_id: String,
    pub service_name: String,
    pub service_type: String,
    pub protocol: String,
    pub model_name: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryResultDto {
    pub service_instance_id: String,
    pub service_name: String,
    pub service_type: String,
    pub protocol: String,
    pub model_name: String,
    pub translation: String,
    pub error_message: String,
    pub status: HistoryResultStatus,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySessionDto {
    pub id: String,
    pub timestamp: String,
    pub trigger: HistoryTrigger,
    pub source_lang: String,
    pub target_lang: String,
    pub source: String,
    pub results: Vec<HistoryResultDto>,
}

pub fn history_trigger_for_input(input: &TranslationInput) -> HistoryTrigger {
    match input {
        TranslationInput::ManualText(_) => HistoryTrigger::Manual,
        TranslationInput::SelectedText(_) => HistoryTrigger::Selection,
        TranslationInput::OcrText { .. } => HistoryTrigger::Screenshot,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_translation_input_to_history_trigger() {
        assert_eq!(
            history_trigger_for_input(&TranslationInput::ManualText("x".to_string())),
            HistoryTrigger::Manual
        );
        assert_eq!(
            history_trigger_for_input(&TranslationInput::SelectedText("x".to_string())),
            HistoryTrigger::Selection
        );
        assert_eq!(
            history_trigger_for_input(&TranslationInput::OcrText {
                text: "x".to_string(),
                image_id: None
            }),
            HistoryTrigger::Screenshot
        );
    }
}
