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

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationServiceMeta {
    pub service_instance_id: String,
    pub service_name: String,
    pub service_type: String,
    pub protocol: String,
    /// 服务配置中的模型名（弹窗结果卡右下角展示）
    pub model_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationPromptConfig {
    pub system_prompt: String,
    pub translation_prompt: String,
    pub chain_of_thought: String,
}

impl Default for TranslationPromptConfig {
    fn default() -> Self {
        Self {
            system_prompt: String::new(),
            translation_prompt: String::new(),
            chain_of_thought: "off".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationRequest {
    pub session_id: TranslationSessionId,
    pub input: TranslationInput,
    pub source_lang: String,
    pub target_lang: String,
    pub service: TranslationServiceMeta,
    pub prompts: TranslationPromptConfig,
}

impl TranslationRequest {
    pub fn source_text(&self) -> &str {
        self.input.text()
    }

    pub fn system_prompt(&self) -> String {
        let prompt = self.prompts.system_prompt.trim();
        if prompt.is_empty() {
            "你是一个专业翻译引擎。只输出译文，不要解释。必须完整翻译全部内容，保留原文的换行、段落与列表结构，不得遗漏条目或提前结束。".to_string()
        } else {
            prompt.to_string()
        }
    }

    pub fn user_prompt(&self) -> String {
        let template = self.prompts.translation_prompt.trim();
        let base = if template.is_empty() {
            format!(
                "请将以下文本完整翻译为{}（保留所有段落、换行与列表项，勿省略任何内容）：\n\n{}",
                self.target_lang,
                self.source_text()
            )
        } else {
            let rendered = template
                .replace("{source_lang}", &self.source_lang)
                .replace("{target_lang}", &self.target_lang)
                .replace("{text}", self.source_text());
            if template.contains("{text}") {
                rendered
            } else {
                format!("{rendered}\n\n{}", self.source_text())
            }
        };

        if self.source_lang == "auto" {
            format!(
                "{base}\n\n请先在第一行用【源语言：语言名称】输出你检测到的原文语言（如：英语、日语、中文），换行后再输出完整译文。"
            )
        } else {
            base
        }
    }

    pub fn thinking_enabled(&self) -> bool {
        matches!(
            self.prompts.chain_of_thought.trim(),
            "short" | "medium" | "long"
        )
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
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "type"
)]
pub enum TranslationEvent {
    Started {
        session_id: TranslationSessionId,
        #[serde(flatten)]
        service: TranslationServiceMeta,
        source_text: String,
        source_type: String,
    },
    Delta {
        session_id: TranslationSessionId,
        #[serde(flatten)]
        service: TranslationServiceMeta,
        text: String,
    },
    Finished {
        session_id: TranslationSessionId,
        #[serde(flatten)]
        service: TranslationServiceMeta,
        full_text: String,
        usage: Option<TokenUsage>,
        detected_source_lang: Option<String>,
    },
    Failed {
        session_id: TranslationSessionId,
        #[serde(flatten)]
        service: TranslationServiceMeta,
        message: String,
        retryable: bool,
    },
    Cancelled {
        session_id: TranslationSessionId,
        #[serde(flatten)]
        service: TranslationServiceMeta,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_service() -> TranslationServiceMeta {
        TranslationServiceMeta {
            service_instance_id: "test".to_string(),
            service_name: "test".to_string(),
            service_type: "llm".to_string(),
            protocol: "mock".to_string(),
            model_name: "mock-model".to_string(),
        }
    }

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
            source_lang: String::new(),
            target_lang: "中文".to_string(),
            service: fake_service(),
            prompts: TranslationPromptConfig::default(),
        };

        assert_eq!(request.source_text(), "hello");
    }

    #[test]
    fn request_uses_custom_prompts_with_placeholders() {
        let request = TranslationRequest {
            session_id: TranslationSessionId("s1".to_string()),
            input: TranslationInput::ManualText("hello".to_string()),
            source_lang: "English".to_string(),
            target_lang: "中文".to_string(),
            service: fake_service(),
            prompts: TranslationPromptConfig {
                system_prompt: "sys".to_string(),
                translation_prompt: "from {source_lang} to {target_lang}: {text}".to_string(),
                chain_of_thought: "off".to_string(),
            },
        };

        assert_eq!(request.system_prompt(), "sys");
        assert_eq!(request.user_prompt(), "from English to 中文: hello");
    }

    #[test]
    fn prompt_without_text_placeholder_keeps_source_text() {
        let request = TranslationRequest {
            session_id: TranslationSessionId("s1".to_string()),
            input: TranslationInput::ManualText("hello".to_string()),
            source_lang: "English".to_string(),
            target_lang: "中文".to_string(),
            service: fake_service(),
            prompts: TranslationPromptConfig {
                system_prompt: "sys".to_string(),
                translation_prompt: "translate to {target_lang}".to_string(),
                chain_of_thought: "off".to_string(),
            },
        };

        assert!(request.user_prompt().contains("hello"));
    }

    #[test]
    fn thinking_enabled_only_for_supported_chain_of_thought_values() {
        let mut request = TranslationRequest {
            session_id: TranslationSessionId("s1".to_string()),
            input: TranslationInput::ManualText("hello".to_string()),
            source_lang: String::new(),
            target_lang: "中文".to_string(),
            service: fake_service(),
            prompts: TranslationPromptConfig::default(),
        };

        assert!(!request.thinking_enabled());

        for value in ["", "off", "invalid"] {
            request.prompts.chain_of_thought = value.to_string();
            assert!(!request.thinking_enabled(), "{value} 不应启用 thinking");
        }

        for value in ["short", "medium", "long"] {
            request.prompts.chain_of_thought = value.to_string();
            assert!(request.thinking_enabled(), "{value} 应启用 thinking");
        }
    }

    #[test]
    fn request_falls_back_to_default_prompts() {
        let request = TranslationRequest {
            session_id: TranslationSessionId("s1".to_string()),
            input: TranslationInput::ManualText("hello".to_string()),
            source_lang: "auto".to_string(),
            target_lang: "中文".to_string(),
            service: fake_service(),
            prompts: TranslationPromptConfig {
                system_prompt: "".to_string(),
                translation_prompt: "".to_string(),
                chain_of_thought: "off".to_string(),
            },
        };

        assert!(request.system_prompt().contains("专业翻译"));
        assert!(
            request.system_prompt().contains("完整翻译"),
            "默认 system prompt 应要求完整翻译"
        );
        assert!(request.user_prompt().contains("中文"));
        assert!(request.user_prompt().contains("hello"));
        assert!(
            request.user_prompt().contains("完整翻译"),
            "默认 user prompt 应要求完整翻译: {}",
            request.user_prompt()
        );
    }

    fn request_with_source_lang(source_lang: &str) -> TranslationRequest {
        TranslationRequest {
            session_id: TranslationSessionId("test".to_string()),
            input: TranslationInput::ManualText("hello".to_string()),
            source_lang: source_lang.to_string(),
            target_lang: "中文".to_string(),
            service: fake_service(),
            prompts: TranslationPromptConfig::default(),
        }
    }

    #[test]
    fn user_prompt_appends_detection_instruction_when_auto() {
        let request = request_with_source_lang("auto");
        let prompt = request.user_prompt();
        assert!(
            prompt.contains("【源语言：语言名称】"),
            "auto 时 user_prompt 应含检测指令: {}",
            prompt
        );
        assert!(prompt.contains("hello"), "应含原文");
    }

    #[test]
    fn user_prompt_no_append_when_specific_source() {
        let request = request_with_source_lang("en-US");
        let prompt = request.user_prompt();
        assert!(
            !prompt.contains("【源语言："),
            "具体源语言时不应追加检测指令: {}",
            prompt
        );
    }

    #[test]
    fn started_event_serializes_with_frontend_field_names() {
        let event = TranslationEvent::Started {
            session_id: TranslationSessionId("session-1".to_string()),
            service: fake_service(),
            source_text: "OCR 原文".to_string(),
            source_type: "ocrText".to_string(),
        };

        let payload = serde_json::to_value(event).expect("事件应可序列化");

        assert_eq!(payload["type"], "started");
        assert_eq!(payload["sessionId"], "session-1");
        assert_eq!(payload["sourceText"], "OCR 原文");
        assert_eq!(payload["sourceType"], "ocrText");
        assert_eq!(payload["serviceInstanceId"], "test");
        assert_eq!(payload["serviceName"], "test");
        assert_eq!(payload["serviceType"], "llm");
        assert_eq!(payload["protocol"], "mock");
        assert_eq!(payload["modelName"], "mock-model");
        assert!(payload.get("session_id").is_none());
        assert!(payload.get("source_text").is_none());
        assert!(payload.get("source_type").is_none());
        assert!(payload.get("model_name").is_none());
        assert!(
            payload.get("service").is_none(),
            "service 应打平，不作为嵌套字段"
        );
    }

    #[test]
    fn started_event_source_type_serializes_for_each_kind() {
        for kind in ["manualText", "selectedText", "ocrText"] {
            let event = TranslationEvent::Started {
                session_id: TranslationSessionId("session-x".to_string()),
                service: fake_service(),
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
            service: fake_service(),
        };

        let payload = serde_json::to_value(event).expect("事件应可序列化");

        assert_eq!(payload["type"], "cancelled");
        assert_eq!(payload["sessionId"], "session-cancel-1");
        assert_eq!(payload["serviceInstanceId"], "test");
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
            service: fake_service(),
            full_text: "你好".to_string(),
            usage: Some(TokenUsage {
                input_tokens: 27,
                output_tokens: 18,
            }),
            detected_source_lang: None,
        };
        let payload = serde_json::to_value(event).expect("事件应可序列化");
        assert_eq!(payload["type"], "finished");
        assert_eq!(payload["fullText"], "你好");
        assert_eq!(payload["usage"]["inputTokens"], 27);
        assert_eq!(payload["usage"]["outputTokens"], 18);
        assert_eq!(payload["serviceInstanceId"], "test");
    }

    #[test]
    fn finished_event_serializes_usage_null_when_absent() {
        let event = TranslationEvent::Finished {
            session_id: TranslationSessionId("session-1".to_string()),
            service: fake_service(),
            full_text: "你好".to_string(),
            usage: None,
            detected_source_lang: None,
        };
        let payload = serde_json::to_value(event).expect("事件应可序列化");
        assert!(payload["usage"].is_null());
        assert_eq!(payload["serviceInstanceId"], "test");
    }

    fn finished_event(detected: Option<&str>) -> TranslationEvent {
        TranslationEvent::Finished {
            session_id: TranslationSessionId("s1".to_string()),
            service: fake_service(),
            full_text: "译文".to_string(),
            usage: None,
            detected_source_lang: detected.map(|s| s.to_string()),
        }
    }

    #[test]
    fn finished_event_serializes_with_detected_source_lang() {
        let json = serde_json::to_string(&finished_event(Some("英语"))).expect("序列化");
        assert!(
            json.contains("\"detectedSourceLang\":\"英语\""),
            "应输出 camelCase detectedSourceLang: {}",
            json
        );
    }

    #[test]
    fn finished_event_detected_source_lang_null_when_none() {
        let json = serde_json::to_string(&finished_event(None)).expect("序列化");
        assert!(
            json.contains("\"detectedSourceLang\":null"),
            "None 时应为 null: {}",
            json
        );
    }

    #[test]
    fn service_meta_serializes_camel_case() {
        let meta = fake_service();
        let payload = serde_json::to_value(meta).expect("service meta 应可序列化");
        assert_eq!(payload["serviceInstanceId"], "test");
        assert_eq!(payload["serviceName"], "test");
        assert_eq!(payload["serviceType"], "llm");
        assert_eq!(payload["protocol"], "mock");
        assert!(payload.get("service_instance_id").is_none());
    }
}
