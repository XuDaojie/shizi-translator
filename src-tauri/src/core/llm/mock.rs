use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::core::translation::provider::{
    TranslationError, TranslationProvider, TranslationStreamEvent,
};
use crate::core::translation::{TokenUsage, TranslationRequest};

pub struct MockLlmProvider;

#[async_trait::async_trait]
impl TranslationProvider for MockLlmProvider {
    async fn translate(
        &self,
        request: &TranslationRequest,
        on_event: &mut (dyn FnMut(TranslationStreamEvent) + Send),
        cancel: &CancellationToken,
    ) -> Result<(), TranslationError> {
        let is_auto = request.source_lang == "auto";
        if is_auto {
            on_event(TranslationStreamEvent::DetectedSourceLang("英语".to_string()));
        }
        let chunks: Vec<String> = vec![
            "[Mock 翻译] ".to_string(),
            request.source_text().to_string(),
            " -> ".to_string(),
            request.target_lang.clone(),
        ];
        for chunk in chunks {
            on_event(TranslationStreamEvent::Delta(chunk));
            tokio::select! {
                _ = cancel.cancelled() => return Ok(()),
                _ = tokio::time::sleep(Duration::from_millis(180)) => {}
            }
        }
        on_event(TranslationStreamEvent::Usage(TokenUsage { input_tokens: 2, output_tokens: 2 }));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::translation::{TranslationInput, TranslationSessionId};

    fn fake_service() -> crate::core::translation::TranslationServiceMeta {
        crate::core::translation::TranslationServiceMeta {
            service_instance_id: "test".to_string(),
            service_name: "test".to_string(),
            service_type: "llm".to_string(),
            protocol: "mock".to_string(),
        }
    }

    fn request() -> TranslationRequest {
        TranslationRequest {
            session_id: TranslationSessionId("test".to_string()),
            input: TranslationInput::ManualText("hello world".to_string()),
            source_lang: String::new(),
            target_lang: "中文".to_string(),
            service: fake_service(),
            prompts: Default::default(),
        }
    }

    #[tokio::test]
    async fn mock_emits_usage_at_end() {
        let provider = MockLlmProvider;
        let cancel = CancellationToken::new();
        let mut events = Vec::new();
        provider
            .translate(
                &request(),
                &mut |ev: TranslationStreamEvent| events.push(ev),
                &cancel,
            )
            .await
            .expect("mock 应成功");

        let usage = events.iter().find_map(|ev| match ev {
            TranslationStreamEvent::Usage(u) => Some(u.clone()),
            _ => None,
        });
        assert!(usage.is_some(), "mock 应在流末发 Usage 事件");
        let usage = usage.unwrap();
        assert_eq!(
            usage,
            TokenUsage {
                input_tokens: 2,
                output_tokens: 2
            }
        );
    }

    #[tokio::test]
    async fn mock_emits_delta_before_usage() {
        let provider = MockLlmProvider;
        let cancel = CancellationToken::new();
        let mut events = Vec::new();
        provider
            .translate(
                &request(),
                &mut |ev: TranslationStreamEvent| events.push(ev),
                &cancel,
            )
            .await
            .expect("mock 应成功");

        assert!(matches!(events.last(), Some(TranslationStreamEvent::Usage(_))));
        assert!(events
            .iter()
            .any(|ev| matches!(ev, TranslationStreamEvent::Delta(_))));
    }

    #[tokio::test]
    async fn mock_emits_detected_source_lang_when_auto() {
        let provider = MockLlmProvider;
        let cancel = CancellationToken::new();
        let mut events = Vec::new();
        let mut req = request();
        req.source_lang = "auto".to_string();
        provider
            .translate(&req, &mut |ev: TranslationStreamEvent| events.push(ev), &cancel)
            .await
            .expect("mock 应成功");
        let detected = events.iter().find_map(|ev| match ev {
            TranslationStreamEvent::DetectedSourceLang(l) => Some(l.clone()),
            _ => None,
        });
        assert_eq!(detected, Some("英语".to_string()));
        let text: String = events
            .iter()
            .filter_map(|ev| match ev {
                TranslationStreamEvent::Delta(t) => Some(t.clone()),
                _ => None,
            })
            .collect();
        assert!(
            !text.contains("【源语言："),
            "auto 时不应输出标记文本: {}",
            text
        );
    }
}
