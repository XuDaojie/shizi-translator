use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationSessionId(pub String);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationRequest {
    pub session_id: TranslationSessionId,
    pub input: TranslationInput,
    pub target_lang: String,
}

impl TranslationRequest {
    pub fn source_text(&self) -> &str {
        self.input.text()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum TranslationInput {
    ManualText(String),
    SelectedText(String),
    OcrText {
        text: String,
        image_id: Option<String>,
    },
}

impl TranslationInput {
    pub fn text(&self) -> &str {
        match self {
            Self::ManualText(text) | Self::SelectedText(text) => text,
            Self::OcrText { text, .. } => text,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "type")]
pub enum TranslationEvent {
    Started {
        session_id: TranslationSessionId,
        source_text: String,
    },
    Delta {
        session_id: TranslationSessionId,
        text: String,
    },
    Finished {
        session_id: TranslationSessionId,
        full_text: String,
    },
    Failed {
        session_id: TranslationSessionId,
        message: String,
        retryable: bool,
    },
    Cancelled {
        session_id: TranslationSessionId,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translation_input_text_returns_inner_text() {
        assert_eq!(
            TranslationInput::ManualText("manual".to_string()).text(),
            "manual"
        );
        assert_eq!(
            TranslationInput::SelectedText("selected".to_string()).text(),
            "selected"
        );
        assert_eq!(
            TranslationInput::OcrText {
                text: "ocr".to_string(),
                image_id: Some("image-1".to_string()),
            }
            .text(),
            "ocr"
        );
    }

    #[test]
    fn translation_request_source_text_reads_input_text() {
        let request = TranslationRequest {
            session_id: TranslationSessionId("session-1".to_string()),
            input: TranslationInput::SelectedText("hello".to_string()),
            target_lang: "中文".to_string(),
        };

        assert_eq!(request.source_text(), "hello");
    }

    #[test]
    fn started_event_serializes_with_frontend_field_names() {
        let event = TranslationEvent::Started {
            session_id: TranslationSessionId("session-1".to_string()),
            source_text: "OCR 原文".to_string(),
        };

        let payload = serde_json::to_value(event).expect("事件应可序列化");

        assert_eq!(payload["type"], "started");
        assert_eq!(payload["sessionId"], "session-1");
        assert_eq!(payload["sourceText"], "OCR 原文");
        assert!(payload.get("session_id").is_none());
        assert!(payload.get("source_text").is_none());
    }

    #[test]
    fn cancelled_event_serializes_with_frontend_field_names() {
        let event = TranslationEvent::Cancelled {
            session_id: TranslationSessionId("session-cancel-1".to_string()),
        };

        let payload = serde_json::to_value(event).expect("事件应可序列化");

        assert_eq!(payload["type"], "cancelled");
        assert_eq!(payload["sessionId"], "session-cancel-1");
        assert!(payload.get("session_id").is_none());
    }
}
