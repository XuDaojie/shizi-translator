use tokio_util::sync::CancellationToken;

use crate::core::translation::{TokenUsage, TranslationRequest};

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("缺少配置 {0}")]
    MissingConfig(&'static str),
    #[error("HTTP 请求失败：{0}")]
    Http(String),
    #[error("服务返回错误：{message}")]
    Api { message: String, retryable: bool },
    #[error("流式响应解析失败：{0}")]
    Parse(String),
}

impl LlmError {
    pub fn retryable(&self) -> bool {
        match self {
            Self::MissingConfig(_) | Self::Parse(_) => false,
            Self::Http(_) => true,
            Self::Api { retryable, .. } => *retryable,
        }
    }
}

/// provider 向 service 输出的流事件。Delta 为文本增量，Usage 为 token 用量。
#[derive(Debug)]
pub enum LlmStreamEvent {
    Delta(String),
    Usage(TokenUsage),
}

#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    async fn stream_translate(
        &self,
        request: &TranslationRequest,
        on_event: &mut (dyn FnMut(LlmStreamEvent) + Send),
        cancel: &CancellationToken,
    ) -> Result<(), LlmError>;
}
