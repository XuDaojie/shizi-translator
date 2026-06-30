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

#[derive(Debug, Clone, Serialize)]
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
#[serde(rename_all = "camelCase", tag = "type")]
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
}
