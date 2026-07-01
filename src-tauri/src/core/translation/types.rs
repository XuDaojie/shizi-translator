use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationSessionId(pub String);

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

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

    pub fn kind(&self) -> &'static str {
        match self {
            Self::ManualText(_) => "manualText",
            Self::SelectedText(_) => "selectedText",
            Self::OcrText { .. } => "ocrText",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "type")]
pub enum TranslationEvent {
    Started {
        session_id: TranslationSessionId,
        source_text: String,
        source_type: String,
    },
    Delta {
        session_id: TranslationSessionId,
        text: String,
    },
    Finished {
        session_id: TranslationSessionId,
        full_text: String,
        usage: Option<TokenUsage>,
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
    fn translation_input_kind_returns_serde_tag_literal() {
        assert_eq!(
            TranslationInput::ManualText("x".to_string()).kind(),
            "manualText"
        );
        assert_eq!(
            TranslationInput::SelectedText("x".to_string()).kind(),
            "selectedText"
        );
        assert_eq!(
            TranslationInput::OcrText {
                text: "x".to_string(),
                image_id: None,
            }
            .kind(),
            "ocrText"
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
            source_type: "ocrText".to_string(),
        };

        let payload = serde_json::to_value(event).expect("事件应可序列化");

        assert_eq!(payload["type"], "started");
        assert_eq!(payload["sessionId"], "session-1");
        assert_eq!(payload["sourceText"], "OCR 原文");
        assert_eq!(payload["sourceType"], "ocrText");
        assert!(payload.get("session_id").is_none());
        assert!(payload.get("source_text").is_none());
        assert!(payload.get("source_type").is_none());
    }

    #[test]
    fn started_event_source_type_serializes_for_each_kind() {
        for kind in ["manualText", "selectedText", "ocrText"] {
            let event = TranslationEvent::Started {
                session_id: TranslationSessionId("session-x".to_string()),
                source_text: "x".to_string(),
                source_type: kind.to_string(),
            };

            let payload = serde_json::to_value(event).expect("事件应可序列化");

            assert_eq!(payload["sourceType"], kind);
        }
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

    #[test]
    fn token_usage_serializes_camel_case() {
        let usage = TokenUsage {
            input_tokens: 27,
            output_tokens: 18,
        };
        let payload = serde_json::to_value(usage).expect("usage 应可序列化");
        assert_eq!(payload["inputTokens"], 27);
        assert_eq!(payload["outputTokens"], 18);
        assert!(payload.get("input_tokens").is_none());
    }

    #[test]
    fn finished_event_serializes_with_usage_when_present() {
        let event = TranslationEvent::Finished {
            session_id: TranslationSessionId("session-1".to_string()),
            full_text: "你好".to_string(),
            usage: Some(TokenUsage {
                input_tokens: 27,
                output_tokens: 18,
            }),
        };
        let payload = serde_json::to_value(event).expect("事件应可序列化");
        assert_eq!(payload["type"], "finished");
        assert_eq!(payload["fullText"], "你好");
        assert_eq!(payload["usage"]["inputTokens"], 27);
        assert_eq!(payload["usage"]["outputTokens"], 18);
    }

    #[test]
    fn finished_event_serializes_usage_null_when_absent() {
        let event = TranslationEvent::Finished {
            session_id: TranslationSessionId("session-1".to_string()),
            full_text: "你好".to_string(),
            usage: None,
        };
        let payload = serde_json::to_value(event).expect("事件应可序列化");
        assert!(payload["usage"].is_null());
    }
}
