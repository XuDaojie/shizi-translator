use std::{env, time::Duration};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

use crate::core::{
    llm::{LlmError, LlmProvider},
    translation::TranslationRequest,
};

pub struct OpenAiCompatibleProvider {
    client: reqwest::Client,
    config: OpenAiCompatibleConfig,
}

struct OpenAiCompatibleConfig {
    api_key: Option<String>,
    base_url: String,
    model: String,
}

#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    stream: bool,
    messages: Vec<ChatMessage>,
}

#[derive(Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Deserialize)]
struct ChatCompletionChunk {
    choices: Option<Vec<ChatChoice>>,
    error: Option<ApiError>,
}

#[derive(Deserialize)]
struct ChatChoice {
    delta: Option<ChatDelta>,
}

#[derive(Deserialize)]
struct ChatDelta {
    content: Option<String>,
}

#[derive(Deserialize)]
struct ApiErrorEnvelope {
    error: ApiError,
}

#[derive(Deserialize)]
struct ApiError {
    message: String,
}

impl OpenAiCompatibleProvider {
    pub fn from_env() -> Self {
        let timeout_secs = env::var("SHIZI_OPENAI_TIMEOUT_SECS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(60);

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .expect("创建 HTTP client 失败");

        Self {
            client,
            config: OpenAiCompatibleConfig {
                api_key: env::var("SHIZI_OPENAI_API_KEY").ok(),
                base_url: env::var("SHIZI_OPENAI_BASE_URL")
                    .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
                model: env::var("SHIZI_OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string()),
            },
        }
    }

    fn endpoint(&self) -> String {
        format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        )
    }

    fn request_body(&self, request: &TranslationRequest) -> ChatCompletionRequest {
        ChatCompletionRequest {
            model: self.config.model.clone(),
            stream: true,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: "你是一个专业翻译引擎。只输出译文，不要解释。".to_string(),
                },
                ChatMessage {
                    role: "user",
                    content: format!(
                        "请将以下文本翻译为{}：\n\n{}",
                        request.target_lang, request.source_text
                    ),
                },
            ],
        }
    }

    async fn parse_error_response(response: reqwest::Response) -> LlmError {
        let status = response.status();
        let retryable = status.as_u16() == 429 || status.is_server_error();
        let body = response.text().await.unwrap_or_default();
        let message = serde_json::from_str::<ApiErrorEnvelope>(&body)
            .map(|error| error.error.message)
            .unwrap_or_else(|_| format!("HTTP {}: {}", status, body.chars().take(500).collect::<String>()));

        if retryable {
            LlmError::Http(message)
        } else {
            LlmError::Api { message, retryable: false }
        }
    }

    fn consume_sse_event(
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
            if data == "[DONE]" {
                return Ok(true);
            }

            let chunk = serde_json::from_str::<ChatCompletionChunk>(data)
                .map_err(|error| LlmError::Parse(error.to_string()))?;

            if let Some(error) = chunk.error {
                return Err(LlmError::Api {
                    message: error.message,
                    retryable: false,
                });
            }

            if let Some(choices) = chunk.choices {
                for choice in choices {
                    if let Some(content) = choice.delta.and_then(|delta| delta.content) {
                        if !content.is_empty() {
                            on_delta(content);
                        }
                    }
                }
            }
        }

        Ok(false)
    }
}

#[async_trait::async_trait]
impl LlmProvider for OpenAiCompatibleProvider {
    async fn stream_translate(
        &self,
        request: &TranslationRequest,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<(), LlmError> {
        let api_key = self
            .config
            .api_key
            .as_deref()
            .ok_or(LlmError::MissingEnv("SHIZI_OPENAI_API_KEY"))?;

        let response = self
            .client
            .post(self.endpoint())
            .bearer_auth(api_key)
            .json(&self.request_body(request))
            .send()
            .await
            .map_err(|error| LlmError::Http(error.to_string()))?;

        if !response.status().is_success() {
            return Err(Self::parse_error_response(response).await);
        }

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        while let Some(bytes) = stream.next().await {
            let bytes = bytes.map_err(|error| LlmError::Http(error.to_string()))?;
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

        if !buffer.trim().is_empty() {
            Self::consume_sse_event(&buffer, on_delta)?;
        }

        Ok(())
    }
}
