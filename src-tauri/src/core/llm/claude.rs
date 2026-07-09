use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::core::translation::{
    auto_lang::AutoLangHeaderParser,
    provider::{TranslationError, TranslationProvider, TranslationStreamEvent},
    TokenUsage, TranslationRequest,
};

#[derive(Debug, Clone)]
pub struct ClaudeConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub timeout_seconds: u64,
}

impl ClaudeConfig {
    pub fn new() -> Self {
        Self {
            api_key: None,
            base_url: "https://api.anthropic.com".to_string(),
            model: "claude-haiku-4-5".to_string(),
            timeout_seconds: 60,
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
        format!("{}/v1/messages", self.config.base_url.trim_end_matches('/'))
    }

    /// Models that support adaptive (effort-based) thinking.
    fn is_adaptive_model(&self) -> bool {
        let model = self.config.model.to_lowercase();
        model.contains("sonnet-5")
            || model.contains("opus-4-8")
            || model.contains("opus-4-7")
            || model.contains("opus-4-6")
            || model.contains("sonnet-4-6")
            || model.contains("fable-5")
            || model.contains("mythos-5")
            || model.contains("mythos-preview")
    }

    fn thinking_config(level: &str, adaptive: bool) -> Option<ClaudeThinkingConfig> {
        if adaptive {
            Some(ClaudeThinkingConfig {
                thinking_type: "adaptive".to_string(),
                budget_tokens: None,
            })
        } else {
            let budget = match level {
                "short" => 1024,
                "long" => 3072,
                _ => 2048,
            };
            Some(ClaudeThinkingConfig {
                thinking_type: "enabled".to_string(),
                budget_tokens: Some(budget),
            })
        }
    }

    fn output_config(level: &str, adaptive: bool) -> Option<ClaudeOutputConfig> {
        if level == "off" || !adaptive {
            return None;
        }
        let effort = match level {
            "short" => "low".to_string(),
            "long" => "high".to_string(),
            _ => "medium".to_string(),
        };
        Some(ClaudeOutputConfig { effort })
    }

    fn request_body(&self, request: &TranslationRequest) -> ClaudeMessagesRequest {
        let adaptive = self.is_adaptive_model();
        let level = request.prompts.chain_of_thought.trim();
        let enabled = request.thinking_enabled();
        ClaudeMessagesRequest {
            model: self.config.model.clone(),
            max_tokens: 4096,
            stream: true,
            thinking: if enabled { Self::thinking_config(level, adaptive) } else { None },
            output_config: Self::output_config(level, adaptive),
            system: request.system_prompt(),
            messages: vec![ClaudeMessage {
                role: "user".to_string(),
                content: request.user_prompt(),
            }],
        }
    }

    /// Parse an SSE event text and emit stream events.
    ///
    /// `input_tokens` carries the input token count parsed from `message_start`
    /// across events, so that the subsequent `message_delta` event can assemble
    /// a complete `TokenUsage`. Claude API returns input and output usage in
    /// two separate events, unlike OpenAI which combines them in the final chunk.
    pub fn consume_sse_event(
        event: &str,
        input_tokens: &mut Option<u64>,
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

            let parsed: ClaudeSseEvent =
                serde_json::from_str(data).map_err(|e| TranslationError::Parse(e.to_string()))?;

            let event_type = event
                .lines()
                .find_map(|l| l.strip_prefix("event:").map(|s| s.trim().to_string()))
                .unwrap_or_default();

            if event_type == "error" {
                if let Some(err) = parsed.error {
                    return Err(TranslationError::Api {
                        message: err.message,
                        retryable: false,
                    });
                }
            }

            if event_type == "message_stop" {
                return Ok(true);
            }

            if event_type == "message_start" {
                if let Some(input) = parsed
                    .message
                    .and_then(|m| m.usage)
                    .and_then(|u| u.input_tokens)
                {
                    *input_tokens = Some(input);
                    on_event(TranslationStreamEvent::Usage(TokenUsage {
                        input_tokens: input,
                        output_tokens: 0,
                    }));
                }
            }

            if event_type == "message_delta" {
                if let Some(output) = parsed.usage.and_then(|u| u.output_tokens) {
                    let input = input_tokens.unwrap_or(0);
                    on_event(TranslationStreamEvent::Usage(TokenUsage {
                        input_tokens: input,
                        output_tokens: output,
                    }));
                }
            }

            if event_type == "content_block_delta" {
                if let Some(delta) = &parsed.delta {
                    if delta.delta_type.as_deref() == Some("text_delta") {
                        if let Some(text) = &delta.text {
                            if !text.is_empty() {
                                on_event(TranslationStreamEvent::Delta(text.clone()));
                            }
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    async fn parse_error_response(response: reqwest::Response) -> TranslationError {
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
            TranslationError::Http(message)
        } else {
            TranslationError::Api {
                message,
                retryable: false,
            }
        }
    }

    /// 消费 SSE 字节流，解析事件并经 forward 闭包（注入 AutoLangHeaderParser）转发。
    /// `message_stop` 或流自然结束时执行 finish：补发 pending 译文与 DetectedSourceLang。
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
        let mut input_tokens: Option<u64> = None;

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

                        if Self::consume_sse_event(&event, &mut input_tokens, &mut forward)? {
                            break 'sse;
                        }
                    }
                }
            }
        }

        if !buffer.trim().is_empty() {
            Self::consume_sse_event(&buffer, &mut input_tokens, &mut forward)?;
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

#[derive(Serialize)]
struct ClaudeMessagesRequest {
    model: String,
    max_tokens: u32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<ClaudeThinkingConfig>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "output_config")]
    output_config: Option<ClaudeOutputConfig>,
    system: String,
    messages: Vec<ClaudeMessage>,
}

#[derive(Serialize)]
struct ClaudeThinkingConfig {
    #[serde(rename = "type")]
    thinking_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    budget_tokens: Option<u32>,
}

#[derive(Serialize)]
struct ClaudeOutputConfig {
    effort: String,
}

#[derive(Serialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct ClaudeSseEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<EventDelta>,
    error: Option<ClaudeEventError>,
    message: Option<ClaudeSseMessageData>,
    usage: Option<ClaudeUsage>,
}

#[derive(Deserialize)]
struct EventDelta {
    #[serde(rename = "type")]
    delta_type: Option<String>,
    text: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
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

#[derive(Deserialize)]
struct ClaudeSseMessageData {
    usage: Option<ClaudeUsage>,
}

#[derive(Deserialize)]
struct ClaudeUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
}

#[async_trait::async_trait]
impl TranslationProvider for ClaudeProvider {
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
            .ok_or(TranslationError::MissingConfig("Claude API Key"))?;

        log::info!(
            "Claude 请求: endpoint={} model={} key={}",
            self.endpoint(),
            self.config.model,
            crate::core::logging::redact_api_key(api_key)
        );

        let body = self.request_body(request);

        let response = self
            .client
            .post(self.endpoint())
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| TranslationError::Http(e.to_string()))?;

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
    use crate::core::translation::provider::TranslationStreamEvent;
    use crate::core::translation::TranslationPromptConfig;

    #[test]
    fn consume_sse_event_extracts_text_delta() {
        let event = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"你好\"}}";
        let mut texts = Vec::new();
        let mut input_tokens: Option<u64> = None;
        let done = ClaudeProvider::consume_sse_event(event, &mut input_tokens, &mut |ev| {
            if let TranslationStreamEvent::Delta(t) = ev {
                texts.push(t);
            }
        })
        .unwrap();
        assert!(!done);
        assert_eq!(texts, vec!["你好"]);
    }

    #[test]
    fn consume_sse_event_message_stop_returns_done() {
        let event = "event: message_stop\ndata: {\"type\":\"message_stop\"}";
        let mut texts = Vec::new();
        let mut input_tokens: Option<u64> = None;
        let done = ClaudeProvider::consume_sse_event(event, &mut input_tokens, &mut |ev| {
            if let TranslationStreamEvent::Delta(t) = ev {
                texts.push(t);
            }
        })
        .unwrap();
        assert!(done);
        assert!(texts.is_empty());
    }

    #[test]
    fn consume_sse_event_ignores_ping() {
        let event = "event: ping\ndata: {\"type\":\"ping\"}";
        let mut texts = Vec::new();
        let mut input_tokens: Option<u64> = None;
        let done = ClaudeProvider::consume_sse_event(event, &mut input_tokens, &mut |ev| {
            if let TranslationStreamEvent::Delta(t) = ev {
                texts.push(t);
            }
        })
        .unwrap();
        assert!(!done);
        assert!(texts.is_empty());
    }

    #[test]
    fn consume_sse_event_error_returns_api_error() {
        let event = "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"invalid_request_error\",\"message\":\"bad key\"}}";
        let mut input_tokens: Option<u64> = None;
        let result = ClaudeProvider::consume_sse_event(event, &mut input_tokens, &mut |_| {});
        match result {
            Err(TranslationError::Api {
                retryable: false, ..
            }) => {}
            other => panic!("预期 Api(retryable=false)，得到：{other:?}"),
        }
    }

    #[test]
    fn consume_sse_event_multiple_events_mixed() {
        let event = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" World\"}}";
        let mut texts = Vec::new();
        let mut input_tokens: Option<u64> = None;
        let done = ClaudeProvider::consume_sse_event(event, &mut input_tokens, &mut |ev| {
            if let TranslationStreamEvent::Delta(t) = ev {
                texts.push(t);
            }
        })
        .unwrap();
        assert!(!done);
        assert_eq!(texts, vec!["Hello", " World"]);
    }

    #[tokio::test]
    async fn translate_requires_api_key() {
        let provider = ClaudeProvider::new(ClaudeConfig::new());
        let request = crate::core::translation::TranslationRequest {
            session_id: crate::core::translation::TranslationSessionId("test".to_string()),
            input: crate::core::translation::TranslationInput::ManualText("hello".to_string()),
            source_lang: String::new(),
            target_lang: "中文".to_string(),
            service: crate::core::translation::TranslationServiceMeta::default(),
            prompts: TranslationPromptConfig::default(),
        };
        let cancel = tokio_util::sync::CancellationToken::new();
        let result = provider
            .translate(&request, &mut |_| {}, &cancel)
            .await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TranslationError::MissingConfig(_)));
    }

    #[test]
    fn request_body_uses_request_prompts_and_manual_thinking_for_haiku() {
        let provider = ClaudeProvider::new(ClaudeConfig {
            api_key: Some("sk-x".to_string()),
            base_url: "https://api.anthropic.com".to_string(),
            model: "claude-haiku-4-5".to_string(),
            timeout_seconds: 60,
        });
        let request = crate::core::translation::TranslationRequest {
            session_id: crate::core::translation::TranslationSessionId("test".to_string()),
            input: crate::core::translation::TranslationInput::ManualText("hi".to_string()),
            source_lang: "English".to_string(),
            target_lang: "中文".to_string(),
            service: crate::core::translation::TranslationServiceMeta::default(),
            prompts: TranslationPromptConfig {
                system_prompt: "sys".to_string(),
                translation_prompt: "{source_lang}->{target_lang}:{text}".to_string(),
                chain_of_thought: "medium".to_string(),
            },
        };

        let json = serde_json::to_value(provider.request_body(&request)).unwrap();

        assert_eq!(json["system"], "sys");
        assert_eq!(json["messages"][0]["content"], "English->中文:hi");
        assert_eq!(json["thinking"]["type"], "enabled");
        assert_eq!(json["thinking"]["budget_tokens"], 2048);
        assert!(json.get("output_config").is_none());
    }

    #[test]
    fn request_body_uses_adaptive_thinking_and_effort_for_supported_models() {
        let provider = ClaudeProvider::new(ClaudeConfig {
            api_key: Some("sk-x".to_string()),
            base_url: "https://api.anthropic.com".to_string(),
            model: "claude-sonnet-4-6".to_string(),
            timeout_seconds: 60,
        });
        let request = crate::core::translation::TranslationRequest {
            session_id: crate::core::translation::TranslationSessionId("test".to_string()),
            input: crate::core::translation::TranslationInput::ManualText("hi".to_string()),
            source_lang: "English".to_string(),
            target_lang: "中文".to_string(),
            service: crate::core::translation::TranslationServiceMeta::default(),
            prompts: TranslationPromptConfig {
                system_prompt: "sys".to_string(),
                translation_prompt: "{source_lang}->{target_lang}:{text}".to_string(),
                chain_of_thought: "medium".to_string(),
            },
        };

        let json = serde_json::to_value(provider.request_body(&request)).unwrap();

        assert_eq!(json["thinking"]["type"], "adaptive");
        assert_eq!(json["output_config"]["effort"], "medium");
    }

    #[test]
    fn request_body_maps_thinking_levels_to_distinct_payload_values() {
        let provider = ClaudeProvider::new(ClaudeConfig {
            api_key: Some("sk-x".to_string()),
            base_url: "https://api.anthropic.com".to_string(),
            model: "claude-haiku-4-5".to_string(),
            timeout_seconds: 60,
        });
        let mut request = crate::core::translation::TranslationRequest {
            session_id: crate::core::translation::TranslationSessionId("test".to_string()),
            input: crate::core::translation::TranslationInput::ManualText("hi".to_string()),
            source_lang: "English".to_string(),
            target_lang: "中文".to_string(),
            service: crate::core::translation::TranslationServiceMeta::default(),
            prompts: TranslationPromptConfig {
                system_prompt: "sys".to_string(),
                translation_prompt: "{source_lang}->{target_lang}:{text}".to_string(),
                chain_of_thought: "short".to_string(),
            },
        };

        let short_json = serde_json::to_value(provider.request_body(&request)).unwrap();
        request.prompts.chain_of_thought = "long".to_string();
        let long_json = serde_json::to_value(provider.request_body(&request)).unwrap();

        assert_ne!(
            short_json["thinking"]["budget_tokens"],
            long_json["thinking"]["budget_tokens"]
        );

        let adaptive_provider = ClaudeProvider::new(ClaudeConfig {
            api_key: Some("sk-x".to_string()),
            base_url: "https://api.anthropic.com".to_string(),
            model: "claude-opus-4-7".to_string(),
            timeout_seconds: 60,
        });
        request.prompts.chain_of_thought = "short".to_string();
        let short_json = serde_json::to_value(adaptive_provider.request_body(&request)).unwrap();
        request.prompts.chain_of_thought = "long".to_string();
        let long_json = serde_json::to_value(adaptive_provider.request_body(&request)).unwrap();

        assert_ne!(
            short_json["output_config"]["effort"],
            long_json["output_config"]["effort"]
        );
    }

    #[tokio::test]
    async fn process_stream_message_stop_emits_detected_source_lang_and_flushes_pending() {
        // 端到端验证 message_stop -> break 'sse -> finish 逻辑执行：
        // auto 模式发 DetectedSourceLang，译文不被标记行污染。
        let sse = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"【源语言：英语】\\n你好\"}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
        let stream = futures_util::stream::iter(vec![Ok::<&[u8], String>(sse.as_bytes())]);
        let cancel = tokio_util::sync::CancellationToken::new();
        let mut events: Vec<TranslationStreamEvent> = Vec::new();
        let mut on_event = |ev: TranslationStreamEvent| events.push(ev);
        ClaudeProvider::process_stream(stream, true, &mut on_event, &cancel)
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
    async fn process_stream_message_stop_flushes_pending_when_no_newline() {
        // 短译文无 \n 时滞留 parser.pending，message_stop 后 finish 应补发。
        let sse = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"你好\"}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
        let stream = futures_util::stream::iter(vec![Ok::<&[u8], String>(sse.as_bytes())]);
        let cancel = tokio_util::sync::CancellationToken::new();
        let mut events: Vec<TranslationStreamEvent> = Vec::new();
        let mut on_event = |ev: TranslationStreamEvent| events.push(ev);
        ClaudeProvider::process_stream(stream, true, &mut on_event, &cancel)
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
