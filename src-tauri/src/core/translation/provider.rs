use tokio_util::sync::CancellationToken;

use crate::core::translation::{TokenUsage, TranslationRequest};

#[derive(Debug, thiserror::Error)]
pub enum TranslationError {
    #[error("缺少配置 {0}")]
    MissingConfig(&'static str),
    #[error("HTTP 请求失败：{0}")]
    Http(String),
    #[error("服务返回错误：{message}")]
    Api { message: String, retryable: bool },
    #[error("响应解析失败：{0}")]
    Parse(String),
}

impl TranslationError {
    pub fn retryable(&self) -> bool {
        match self {
            Self::MissingConfig(_) | Self::Parse(_) => false,
            Self::Http(_) => true,
            Self::Api { retryable, .. } => *retryable,
        }
    }
}

/// provider 向 service 输出的流事件。
/// - Delta：译文增量（流式逐 chunk，或非流式一次性）
/// - Usage：token 用量（仅 LLM 发，ML 恒不发）
/// - DetectedSourceLang：auto 检测到的源语言（LLM 首行解析后发，ML 从响应填）
#[derive(Debug)]
pub enum TranslationStreamEvent {
    Delta(String),
    Usage(TokenUsage),
    DetectedSourceLang(String),
}

#[async_trait::async_trait]
pub trait TranslationProvider: Send + Sync {
    async fn translate(
        &self,
        request: &TranslationRequest,
        on_event: &mut (dyn FnMut(TranslationStreamEvent) + Send),
        cancel: &CancellationToken,
    ) -> Result<(), TranslationError>;
}

/// 非流式 provider 的一次性翻译结果。
#[derive(Debug, Default, Clone)]
pub struct TranslationResult {
    pub text: String,
    pub usage: Option<TokenUsage>,
    pub detected_source_lang: Option<String>,
}

/// 非流式翻译 provider（机器翻译、未来非流式 LLM）。只返回完整结果，不接触 on_event。
#[async_trait::async_trait]
pub trait BatchTranslateProvider: Send + Sync {
    async fn translate_once(
        &self,
        request: &TranslationRequest,
        cancel: &CancellationToken,
    ) -> Result<TranslationResult, TranslationError>;
}

/// 把 BatchTranslateProvider 适配为 TranslationProvider。
/// 事件顺序：取消检查 -> DetectedSourceLang -> Delta -> Usage。
pub struct StreamingAdapter<T>(pub T);

#[async_trait::async_trait]
impl<T: BatchTranslateProvider> TranslationProvider for StreamingAdapter<T> {
    async fn translate(
        &self,
        request: &TranslationRequest,
        on_event: &mut (dyn FnMut(TranslationStreamEvent) + Send),
        cancel: &CancellationToken,
    ) -> Result<(), TranslationError> {
        if cancel.is_cancelled() {
            return Ok(());
        }
        let result = self.0.translate_once(request, cancel).await?;
        if let Some(lang) = result.detected_source_lang {
            on_event(TranslationStreamEvent::DetectedSourceLang(lang));
        }
        on_event(TranslationStreamEvent::Delta(result.text));
        if let Some(usage) = result.usage {
            on_event(TranslationStreamEvent::Usage(usage));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::translation::{
        TranslationInput, TranslationPromptConfig, TranslationServiceMeta, TranslationSessionId,
    };

    fn request() -> TranslationRequest {
        TranslationRequest {
            session_id: TranslationSessionId("test".to_string()),
            input: TranslationInput::ManualText("hi".to_string()),
            source_lang: "auto".to_string(),
            target_lang: "中文".to_string(),
            service: TranslationServiceMeta::default(),
            prompts: TranslationPromptConfig::default(),
        }
    }

    struct BatchFake {
        text: String,
        usage: Option<TokenUsage>,
        detected: Option<String>,
    }

    #[async_trait::async_trait]
    impl BatchTranslateProvider for BatchFake {
        async fn translate_once(
            &self,
            _request: &TranslationRequest,
            _cancel: &CancellationToken,
        ) -> Result<TranslationResult, TranslationError> {
            Ok(TranslationResult {
                text: self.text.clone(),
                usage: self.usage.clone(),
                detected_source_lang: self.detected.clone(),
            })
        }
    }

    #[tokio::test]
    async fn streaming_adapter_emits_events_in_order() {
        let provider = StreamingAdapter(BatchFake {
            text: "译文".to_string(),
            usage: Some(TokenUsage { input_tokens: 1, output_tokens: 2 }),
            detected: Some("英语".to_string()),
        });
        let cancel = CancellationToken::new();
        let mut events = Vec::new();
        provider
            .translate(&request(), &mut |ev| events.push(ev), &cancel)
            .await
            .unwrap();
        let kinds: Vec<&str> = events
            .iter()
            .map(|e| match e {
                TranslationStreamEvent::DetectedSourceLang(_) => "detected",
                TranslationStreamEvent::Delta(_) => "delta",
                TranslationStreamEvent::Usage(_) => "usage",
            })
            .collect();
        assert_eq!(kinds, vec!["detected", "delta", "usage"]);
    }

    #[tokio::test]
    async fn streaming_adapter_skips_none_events() {
        let provider = StreamingAdapter(BatchFake {
            text: "译文".to_string(),
            usage: None,
            detected: None,
        });
        let cancel = CancellationToken::new();
        let mut events = Vec::new();
        provider
            .translate(&request(), &mut |ev| events.push(ev), &cancel)
            .await
            .unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], TranslationStreamEvent::Delta(_)));
    }

    #[tokio::test]
    async fn streaming_adapter_early_returns_when_cancelled() {
        let provider = StreamingAdapter(BatchFake {
            text: "译文".to_string(),
            usage: None,
            detected: None,
        });
        let cancel = CancellationToken::new();
        cancel.cancel();
        let mut events = Vec::new();
        provider
            .translate(&request(), &mut |ev| events.push(ev), &cancel)
            .await
            .unwrap();
        assert!(events.is_empty(), "取消时应早退不发任何事件");
    }
}
