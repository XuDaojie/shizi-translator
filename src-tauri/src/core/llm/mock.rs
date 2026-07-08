use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::core::{
    llm::{LlmError, LlmProvider, LlmStreamEvent},
    translation::{TokenUsage, TranslationRequest},
};

pub struct MockLlmProvider;

#[async_trait::async_trait]
impl LlmProvider for MockLlmProvider {
    async fn stream_translate(
        &self,
        request: &TranslationRequest,
        on_event: &mut (dyn FnMut(LlmStreamEvent) + Send),
        cancel: &CancellationToken,
    ) -> Result<(), LlmError> {
        let is_auto = request.prompts.source_lang == "auto";
        let mut chunks: Vec<String> = Vec::new();
        if is_auto {
            chunks.push("【源语言：英语】\n".to_string());
        }
        chunks.push("[Mock 翻译] ".to_string());
        chunks.push(request.source_text().to_string());
        chunks.push(" -> ".to_string());
        chunks.push(request.target_lang.clone());

        for chunk in chunks {
            on_event(LlmStreamEvent::Delta(chunk));
            tokio::select! {
                _ = cancel.cancelled() => return Ok(()),
                _ = tokio::time::sleep(Duration::from_millis(180)) => {}
            }
        }

        // 固定假 usage，供单测覆盖 usage 全链路
        on_event(LlmStreamEvent::Usage(TokenUsage {
            input_tokens: 2,
            output_tokens: 2,
        }));

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
            .stream_translate(
                &request(),
                &mut |ev: LlmStreamEvent| events.push(ev),
                &cancel,
            )
            .await
            .expect("mock 应成功");

        let usage = events.iter().find_map(|ev| match ev {
            LlmStreamEvent::Usage(u) => Some(u.clone()),
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
            .stream_translate(
                &request(),
                &mut |ev: LlmStreamEvent| events.push(ev),
                &cancel,
            )
            .await
            .expect("mock 应成功");

        matches!(events.last(), Some(LlmStreamEvent::Usage(_)));
        assert!(events
            .iter()
            .any(|ev| matches!(ev, LlmStreamEvent::Delta(_))));
    }

    #[tokio::test]
    async fn mock_emits_detection_header_when_auto() {
        let provider = MockLlmProvider;
        let cancel = CancellationToken::new();
        let mut events = Vec::new();
        let mut req = request();
        req.prompts.source_lang = "auto".to_string();
        provider
            .stream_translate(&req, &mut |ev: LlmStreamEvent| events.push(ev), &cancel)
            .await
            .expect("mock 应成功");
        let text: String = events
            .iter()
            .filter_map(|ev| match ev {
                LlmStreamEvent::Delta(t) => Some(t.clone()),
                _ => None,
            })
            .collect();
        assert!(
            text.starts_with("【源语言：英语】\n"),
            "auto 时应首行输出标记: {}",
            text
        );
    }
}

