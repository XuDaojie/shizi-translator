use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

use tokio_util::sync::CancellationToken;

use crate::core::{
    translation::{
        auto_lang::AutoLangHeaderParser,
        provider::{TranslationError, TranslationProvider, TranslationStreamEvent},
        TokenUsage, TranslationRequest,
    },
};

pub struct OpenAiCompatibleProvider {
    client: reqwest::Client,
    config: OpenAiCompatibleConfig,
}

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub timeout_seconds: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatCompletionRequest {
    model: String,
    stream: bool,
    stream_options: StreamOptions,
    messages: Vec<ChatMessage>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StreamOptions {
    include_usage: bool,
}

#[derive(Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Deserialize)]
struct ChatCompletionChunk {
    choices: Option<Vec<ChatChoice>>,
    usage: Option<ChatUsage>,
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
struct ChatUsage {
    prompt_tokens: u64,
    completion_tokens: u64,
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
    pub fn new(config: OpenAiCompatibleConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .expect("创建 HTTP client 失败");

        Self { client, config }
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
            stream_options: StreamOptions {
                include_usage: true,
            },
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: request.system_prompt(),
                },
                ChatMessage {
                    role: "user",
                    content: request.user_prompt(),
                },
            ],
        }
    }

    async fn parse_error_response(response: reqwest::Response) -> TranslationError {
        let status = response.status();
        let retryable = status.as_u16() == 429 || status.is_server_error();
        let body = response.text().await.unwrap_or_default();
        let message = serde_json::from_str::<ApiErrorEnvelope>(&body)
            .map(|error| error.error.message)
            .unwrap_or_else(|_| {
                format!(
                    "HTTP {}: {}",
                    status,
                    body.chars().take(500).collect::<String>()
                )
            });

        log::warn!("OpenAI 响应非 2xx: status={} retryable={}", status, retryable);

        if retryable {
            TranslationError::Http(message)
        } else {
            TranslationError::Api {
                message,
                retryable: false,
            }
        }
    }

    fn consume_sse_event(
        event: &str,
        on_event: &mut (dyn FnMut(TranslationStreamEvent) + Send),
    ) -> Result<bool, TranslationError> {
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
                .map_err(|error| TranslationError::Parse(error.to_string()))?;

            if let Some(error) = chunk.error {
                return Err(TranslationError::Api {
                    message: error.message,
                    retryable: false,
                });
            }

            if let Some(usage) = chunk.usage {
                on_event(TranslationStreamEvent::Usage(TokenUsage {
                    input_tokens: usage.prompt_tokens,
                    output_tokens: usage.completion_tokens,
                }));
            }

            if let Some(choices) = chunk.choices {
                for choice in choices {
                    if let Some(content) = choice.delta.and_then(|delta| delta.content) {
                        if !content.is_empty() {
                            on_event(TranslationStreamEvent::Delta(content));
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    /// 消费 SSE 字节流，解析事件并经 forward 闭包（注入 AutoLangHeaderParser）转发。
    /// `[DONE]` 或流自然结束时执行 finish：补发 pending 译文与 DetectedSourceLang。
    /// cancel 时直接返回，不执行 finish（取消不应补发）。
    async fn process_stream<S, B, E>(
        stream: S,
        is_auto: bool,
        on_event: &mut (dyn FnMut(TranslationStreamEvent) + Send),
        cancel: &CancellationToken,
    ) -> Result<(), TranslationError>
    where
        S: futures_util::Stream<Item = Result<B, E>> + Unpin,
        B: AsRef<[u8]>,
        E: std::fmt::Display,
    {
        let mut parser = AutoLangHeaderParser::new();

        let mut forward = |ev: TranslationStreamEvent| {
            if let TranslationStreamEvent::Delta(text) = ev {
                if is_auto {
                    for piece in parser.feed(&text) {
                        on_event(TranslationStreamEvent::Delta(piece));
                    }
                } else {
                    on_event(TranslationStreamEvent::Delta(text));
                }
            } else {
                on_event(ev);
            }
        };

        let mut stream = stream;
        let mut buffer = String::new();

        'sse: loop {
            tokio::select! {
                _ = cancel.cancelled() => return Ok(()),
                bytes = stream.next() => {
                    let Some(bytes) = bytes else { break };
                    let bytes = bytes.map_err(|e| TranslationError::Http(e.to_string()))?;
                    buffer.push_str(&String::from_utf8_lossy(bytes.as_ref()));
                    buffer = buffer.replace("\r\n", "\n");

                    while let Some(index) = buffer.find("\n\n") {
                        let event = buffer[..index].to_string();
                        buffer = buffer[index + 2..].to_string();

                        if Self::consume_sse_event(&event, &mut forward)? {
                            break 'sse;
                        }
                    }
                }
            }
        }

        if !buffer.trim().is_empty() {
            Self::consume_sse_event(&buffer, &mut forward)?;
        }

        // 释放 forward 持有的 &mut parser 与 &mut on_event 借用，供后续 finish/on_event 使用。
        drop(forward);

        if is_auto {
            let (pieces, lang) = parser.finish();
            for piece in pieces {
                on_event(TranslationStreamEvent::Delta(piece));
            }
            if let Some(lang) = lang {
                on_event(TranslationStreamEvent::DetectedSourceLang(lang));
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl TranslationProvider for OpenAiCompatibleProvider {
    async fn translate(
        &self,
        request: &TranslationRequest,
        on_event: &mut (dyn FnMut(TranslationStreamEvent) + Send),
        cancel: &CancellationToken,
    ) -> Result<(), TranslationError> {
        let api_key = self
            .config
            .api_key
            .as_deref()
            .ok_or(TranslationError::MissingConfig("OpenAI API Key"))?;

        log::info!(
            "OpenAI 请求: endpoint={} model={} key={}",
            self.endpoint(),
            self.config.model,
            crate::core::logging::redact_api_key(api_key)
        );

        let response = self
            .client
            .post(self.endpoint())
            .bearer_auth(api_key)
            .json(&self.request_body(request))
            .send()
            .await
            .map_err(|error| TranslationError::Http(error.to_string()))?;

        if !response.status().is_success() {
            return Err(Self::parse_error_response(response).await);
        }

        let is_auto = request.source_lang == "auto";
        Self::process_stream(response.bytes_stream(), is_auto, on_event, cancel).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::translation::{
        TranslationInput, TranslationPromptConfig, TranslationSessionId,
    };

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
            input: TranslationInput::ManualText("hi".to_string()),
            source_lang: String::new(),
            target_lang: "中文".to_string(),
            service: fake_service(),
            prompts: TranslationPromptConfig::default(),
        }
    }

    #[test]
    fn consume_sse_event_extracts_usage_from_final_chunk() {
        let event =
            "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":27,\"completion_tokens\":18}}";
        let mut events: Vec<TranslationStreamEvent> = Vec::new();
        let done = OpenAiCompatibleProvider::consume_sse_event(event, &mut |ev| {
            events.push(ev);
        })
        .unwrap();
        assert!(!done);
        let usage = events.iter().find_map(|ev| match ev {
            TranslationStreamEvent::Usage(u) => Some(u.clone()),
            _ => None,
        });
        assert_eq!(
            usage,
            Some(TokenUsage {
                input_tokens: 27,
                output_tokens: 18
            })
        );
    }

    #[test]
    fn request_body_includes_stream_options_include_usage() {
        let config = OpenAiCompatibleConfig {
            api_key: Some("sk-x".to_string()),
            base_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-4o-mini".to_string(),
            timeout_seconds: 60,
        };
        let provider = OpenAiCompatibleProvider::new(config);
        let body = provider.request_body(&request());
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["streamOptions"]["includeUsage"], true);
    }

    #[test]
    fn request_body_uses_request_prompts() {
        let config = OpenAiCompatibleConfig {
            api_key: Some("sk-x".to_string()),
            base_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-4o-mini".to_string(),
            timeout_seconds: 60,
        };
        let provider = OpenAiCompatibleProvider::new(config);
        let mut request = request();
        request.source_lang = "English".to_string();
        request.prompts = TranslationPromptConfig {
            system_prompt: "sys".to_string(),
            translation_prompt: "{source_lang}->{target_lang}:{text}".to_string(),
            chain_of_thought: "off".to_string(),
        };

        let json = serde_json::to_value(provider.request_body(&request)).unwrap();

        assert_eq!(json["messages"][0]["content"], "sys");
        assert_eq!(json["messages"][1]["content"], "English->中文:hi");
    }

    #[tokio::test]
    async fn process_stream_done_emits_detected_source_lang_and_flushes_pending() {
        // 端到端验证 [DONE] -> break 'sse -> finish 逻辑执行：
        // auto 模式发 DetectedSourceLang，译文不被标记行污染。
        let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"【源语言：英语】\\n你好\"}}]}\n\ndata: [DONE]\n\n";
        let stream = futures_util::stream::iter(vec![Ok::<&[u8], String>(sse.as_bytes())]);
        let cancel = tokio_util::sync::CancellationToken::new();
        let mut events: Vec<TranslationStreamEvent> = Vec::new();
        let mut on_event = |ev: TranslationStreamEvent| events.push(ev);
        OpenAiCompatibleProvider::process_stream(stream, true, &mut on_event, &cancel)
            .await
            .expect("process_stream 应成功");

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
        assert_eq!(text, "你好");
    }

    #[tokio::test]
    async fn process_stream_done_flushes_pending_when_no_newline() {
        // 短译文无 \n 时滞留 parser.pending，[DONE] 后 finish 应补发。
        let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"你好\"}}]}\n\ndata: [DONE]\n\n";
        let stream = futures_util::stream::iter(vec![Ok::<&[u8], String>(sse.as_bytes())]);
        let cancel = tokio_util::sync::CancellationToken::new();
        let mut events: Vec<TranslationStreamEvent> = Vec::new();
        let mut on_event = |ev: TranslationStreamEvent| events.push(ev);
        OpenAiCompatibleProvider::process_stream(stream, true, &mut on_event, &cancel)
            .await
            .expect("process_stream 应成功");

        let text: String = events
            .iter()
            .filter_map(|ev| match ev {
                TranslationStreamEvent::Delta(t) => Some(t.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(text, "你好");
        assert!(events
            .iter()
            .all(|ev| !matches!(ev, TranslationStreamEvent::DetectedSourceLang(_))));
    }

    #[tokio::test]
    async fn process_stream_non_auto_done_passes_delta_directly() {
        // 非 auto 模式：Delta 直通，不发 DetectedSourceLang。
        let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"你好\"}}]}\n\ndata: [DONE]\n\n";
        let stream = futures_util::stream::iter(vec![Ok::<&[u8], String>(sse.as_bytes())]);
        let cancel = tokio_util::sync::CancellationToken::new();
        let mut events: Vec<TranslationStreamEvent> = Vec::new();
        let mut on_event = |ev: TranslationStreamEvent| events.push(ev);
        OpenAiCompatibleProvider::process_stream(stream, false, &mut on_event, &cancel)
            .await
            .expect("process_stream 应成功");

        let text: String = events
            .iter()
            .filter_map(|ev| match ev {
                TranslationStreamEvent::Delta(t) => Some(t.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(text, "你好");
        assert!(events
            .iter()
            .all(|ev| !matches!(ev, TranslationStreamEvent::DetectedSourceLang(_))));
    }
}
