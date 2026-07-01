use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::core::{
    llm::{LlmError, LlmProvider},
    translation::TranslationRequest,
};

#[derive(Debug, Clone)]
pub struct ClaudeConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub timeout_seconds: u64,
    pub enable_thinking: bool,
}

impl ClaudeConfig {
    pub fn new() -> Self {
        Self {
            api_key: None,
            base_url: "https://api.anthropic.com".to_string(),
            model: "claude-haiku-4-5".to_string(),
            timeout_seconds: 60,
            enable_thinking: false,
        }
    }
}

impl Default for ClaudeConfig {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ClaudeProvider {
    client: reqwest::Client,
    config: ClaudeConfig,
}

impl ClaudeProvider {
    pub fn new(config: ClaudeConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .expect("创建 HTTP client 失败");
        Self { client, config }
    }

    fn endpoint(&self) -> String {
        format!(
            "{}/v1/messages",
            self.config.base_url.trim_end_matches('/')
        )
    }

    pub fn consume_sse_event(
        event: &str,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<bool, LlmError> {
        for line in event.lines() {
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data.is_empty() {
                continue;
            }

            let parsed: ClaudeSseEvent = serde_json::from_str(data)
                .map_err(|e| LlmError::Parse(e.to_string()))?;

            let event_type = event.lines().find_map(|l| {
                l.strip_prefix("event:").map(|s| s.trim().to_string())
            }).unwrap_or_default();

            if event_type == "error" {
                if let Some(err) = parsed.error {
                    return Err(LlmError::Api {
                        message: err.message,
                        retryable: false,
                    });
                }
            }

            if event_type == "message_stop" {
                return Ok(true);
            }

            if event_type == "content_block_delta" {
                if let Some(delta) = &parsed.delta {
                    if delta.delta_type == "text_delta" {
                        if let Some(text) = &delta.text {
                            if !text.is_empty() {
                                on_delta(text.clone());
                            }
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    async fn parse_error_response(response: reqwest::Response) -> LlmError {
        let status = response.status();
        let retryable = status.as_u16() == 429 || status.is_server_error();
        let body = response.text().await.unwrap_or_default();
        let message = serde_json::from_str::<ClaudeErrorEnvelope>(&body)
            .map(|e| e.error.message)
            .unwrap_or_else(|_| {
                format!(
                    "HTTP {}: {}",
                    status,
                    body.chars().take(500).collect::<String>()
                )
            });
        if retryable {
            LlmError::Http(message)
        } else {
            LlmError::Api { message, retryable: false }
        }
    }
}

#[derive(Serialize)]
struct ClaudeMessagesRequest {
    model: String,
    max_tokens: u32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<ClaudeThinkingConfig>,
    system: String,
    messages: Vec<ClaudeMessage>,
}

#[derive(Serialize)]
struct ClaudeThinkingConfig {
    #[serde(rename = "type")]
    thinking_type: String,
}

#[derive(Serialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ClaudeSseEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<EventDelta>,
    error: Option<ClaudeEventError>,
}

#[derive(Deserialize)]
struct EventDelta {
    #[serde(rename = "type")]
    delta_type: String,
    text: Option<String>,
}

#[derive(Deserialize)]
struct ClaudeEventError {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}

#[derive(Deserialize)]
struct ClaudeErrorEnvelope {
    error: ClaudeApiErrorDetail,
}

#[derive(Deserialize)]
struct ClaudeApiErrorDetail {
    message: String,
}

#[async_trait::async_trait]
impl LlmProvider for ClaudeProvider {
    async fn stream_translate(
        &self,
        request: &TranslationRequest,
        on_delta: &mut (dyn FnMut(String) + Send),
        cancel: &CancellationToken,
    ) -> Result<(), LlmError> {
        let api_key = self
            .config
            .api_key
            .as_deref()
            .ok_or(LlmError::MissingConfig("Claude API Key"))?;

        let body = ClaudeMessagesRequest {
            model: self.config.model.clone(),
            max_tokens: 4096,
            stream: true,
            thinking: if self.config.enable_thinking {
                Some(ClaudeThinkingConfig {
                    thinking_type: "adaptive".to_string(),
                })
            } else {
                None
            },
            system: "你是一个专业翻译引擎。只输出译文，不要解释。".to_string(),
            messages: vec![ClaudeMessage {
                role: "user".to_string(),
                content: format!(
                    "请将以下文本翻译为{}：\n\n{}",
                    request.target_lang,
                    request.source_text()
                ),
            }],
        };

        let response = self
            .client
            .post(self.endpoint())
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::Http(e.to_string()))?;

        if !response.status().is_success() {
            return Err(Self::parse_error_response(response).await);
        }

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        loop {
            tokio::select! {
                _ = cancel.cancelled() => return Ok(()),
                bytes = stream.next() => {
                    let Some(bytes) = bytes else { break };
                    let bytes = bytes.map_err(|e| LlmError::Http(e.to_string()))?;
                    buffer.push_str(&String::from_utf8_lossy(&bytes));
                    buffer = buffer.replace("\r\n", "\n");

                    while let Some(index) = buffer.find("\n\n") {
                        let event = buffer[..index].to_string();
                        buffer = buffer[index + 2..].to_string();

                        if Self::consume_sse_event(&event, on_delta)? {
                            return Ok(());
                        }
                    }
                }
            }
        }

        if !buffer.trim().is_empty() {
            Self::consume_sse_event(&buffer, on_delta)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consume_sse_event_extracts_text_delta() {
        let event = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"你好\"}}";
        let mut texts = Vec::new();
        let done = ClaudeProvider::consume_sse_event(event, &mut |t| texts.push(t)).unwrap();
        assert!(!done);
        assert_eq!(texts, vec!["你好"]);
    }

    #[test]
    fn consume_sse_event_message_stop_returns_done() {
        let event = "event: message_stop\ndata: {\"type\":\"message_stop\"}";
        let mut texts = Vec::new();
        let done = ClaudeProvider::consume_sse_event(event, &mut |t| texts.push(t)).unwrap();
        assert!(done);
        assert!(texts.is_empty());
    }

    #[test]
    fn consume_sse_event_ignores_ping() {
        let event = "event: ping\ndata: {\"type\":\"ping\"}";
        let mut texts = Vec::new();
        let done = ClaudeProvider::consume_sse_event(event, &mut |t| texts.push(t)).unwrap();
        assert!(!done);
        assert!(texts.is_empty());
    }

    #[test]
    fn consume_sse_event_error_returns_api_error() {
        let event = "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"invalid_request_error\",\"message\":\"bad key\"}}";
        let mut texts = Vec::new();
        let result = ClaudeProvider::consume_sse_event(event, &mut |t| texts.push(t));
        match result {
            Err(LlmError::Api { retryable: false, .. }) => {}
            other => panic!("预期 Api(retryable=false)，得到：{other:?}"),
        }
    }

    #[test]
    fn consume_sse_event_multiple_events_mixed() {
        let event = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" World\"}}";
        let mut texts = Vec::new();
        let done = ClaudeProvider::consume_sse_event(event, &mut |t| texts.push(t)).unwrap();
        assert!(!done);
        assert_eq!(texts, vec!["Hello", " World"]);
    }

    #[tokio::test]
    async fn stream_translate_requires_api_key() {
        let provider = ClaudeProvider::new(ClaudeConfig::new());
        let request = crate::core::translation::TranslationRequest {
            session_id: crate::core::translation::TranslationSessionId("test".to_string()),
            input: crate::core::translation::TranslationInput::ManualText("hello".to_string()),
            target_lang: "中文".to_string(),
        };
        let cancel = tokio_util::sync::CancellationToken::new();
        let result = provider
            .stream_translate(&request, &mut |_| {}, &cancel)
            .await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LlmError::MissingConfig(_)));
    }
}
