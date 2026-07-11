use crate::core::{
    config::ServiceInstanceConfig,
    translation::{TranslationInput, TranslationRequest},
};

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

impl NewHistorySession {
    pub fn from_translation(
        batch_id: &str,
        input: &TranslationInput,
        source_lang: String,
        target_lang: String,
        created_at: String,
    ) -> Self {
        Self {
            id: batch_id.to_string(),
            batch_id: batch_id.to_string(),
            trigger: history_trigger_for_input(input),
            source_lang,
            target_lang,
            source_text: input.text().to_string(),
            created_at,
        }
    }
}

impl NewHistoryResult {
    pub fn from_request(
        request: &TranslationRequest,
        service: &ServiceInstanceConfig,
        session_id: &str,
    ) -> Self {
        Self {
            session_id: session_id.to_string(),
            service_instance_id: request.service.service_instance_id.clone(),
            service_name: request.service.service_name.clone(),
            service_type: request.service.service_type.clone(),
            protocol: request.service.protocol.clone(),
            // 与事件 meta 一致：MT 不写入模型名
            model_name: if service.protocol == "microsoft_edge" || request.service.protocol == "microsoft_edge"
            {
                String::new()
            } else {
                service.model.clone()
            },
        }
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

    #[test]
    fn new_history_session_uses_batch_id_as_session_id() {
        let item = NewHistorySession::from_translation(
            "batch-1",
            &TranslationInput::ManualText("hello".to_string()),
            "auto".to_string(),
            "zh-CN".to_string(),
            "2026-07-11T00:00:00Z".to_string(),
        );

        assert_eq!(item.id, "batch-1");
        assert_eq!(item.batch_id, "batch-1");
        assert_eq!(item.trigger, HistoryTrigger::Manual);
        assert_eq!(item.source_text, "hello");
    }
}
