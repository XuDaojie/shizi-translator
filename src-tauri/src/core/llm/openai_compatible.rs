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

/// 与 Claude provider 对齐的默认输出上限（OpenAI 兼容协议可选字段）。
/// 说明：智谱等端对敏感内容的强制截断走 `finish_reason=sensitive` / `content_filter`，
/// 与 max_tokens 无关；此处仅保证未设上限时不会过早因 length 截断。
const DEFAULT_MAX_TOKENS: u32 = 4096;

/// OpenAI Chat Completions 官方 JSON 为 snake_case（stream_options / max_tokens / include_usage）。
#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    stream: bool,
    stream_options: StreamOptions,
    max_tokens: u32,
    messages: Vec<ChatMessage>,
}

#[derive(Serialize)]
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
    /// 智谱非流式/流式敏感审核：`contentFilter: [{ level, role }]`
    #[serde(default, alias = "contentFilter")]
    content_filter: Option<Vec<ContentFilterItem>>,
}

#[derive(Deserialize)]
struct ChatChoice {
    delta: Option<ChatDelta>,
    /// `stop` | `length` | `sensitive`(智谱) | `content_filter`(OpenAI 系) | …
    finish_reason: Option<String>,
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

/// 智谱等返回的审核条目（level/role 仅用于识别存在，不参与展示）。
#[derive(Deserialize)]
struct ContentFilterItem {
    #[serde(default)]
    #[allow(dead_code)]
    level: Option<i64>,
    #[serde(default)]
    #[allow(dead_code)]
    role: Option<String>,
}

#[derive(Deserialize)]
struct ApiErrorEnvelope {
    error: ApiError,
    #[serde(default, alias = "contentFilter")]
    content_filter: Option<Vec<ContentFilterItem>>,
}

/// OpenAI / 智谱错误体。智谱敏感示例：
/// `{"contentFilter":[{"level":1,"role":"assistant"}],"error":{"code":"1301","message":"系统检测到…"}}`
#[derive(Deserialize)]
struct ApiError {
    /// 智谱为字符串 `"1301"`，部分兼容端可能给数字。
    #[serde(default)]
    code: Option<serde_json::Value>,
    #[serde(default)]
    message: String,
}

/// 智谱内容安全错误码（输入/生成敏感）。
const ZHIPU_SENSITIVE_CODE: &str = "1301";

const SENSITIVE_CONTENT_FALLBACK_MSG: &str =
    "系统检测到输入或生成内容可能包含不安全或敏感内容，请您避免输入易产生敏感内容的提示语，感谢您的配合。";

impl ApiError {
    fn code_str(&self) -> Option<String> {
        match &self.code {
            Some(serde_json::Value::String(s)) => Some(s.clone()),
            Some(serde_json::Value::Number(n)) => Some(n.to_string()),
            Some(other) => Some(other.to_string()),
            None => None,
        }
    }

    /// 优先使用服务商原文 message；空则按 code / 审核标记兜底。
    fn into_user_message(self, has_content_filter: bool) -> String {
        let msg = self.message.trim();
        if !msg.is_empty() {
            return msg.to_string();
        }
        let code = self.code_str();
        if code.as_deref() == Some(ZHIPU_SENSITIVE_CODE) || has_content_filter {
            return SENSITIVE_CONTENT_FALLBACK_MSG.to_string();
        }
        if let Some(code) = code {
            return format!("服务返回错误（code={code}）");
        }
        "服务返回错误".to_string()
    }
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
            max_tokens: DEFAULT_MAX_TOKENS,
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
        let message = Self::message_from_error_body(&body).unwrap_or_else(|| {
            format!(
                "HTTP {}: {}",
                status,
                body.chars().take(500).collect::<String>()
            )
        });

        log::warn!(
            "OpenAI 响应非 2xx: status={} retryable={} msg={}",
            status,
            retryable,
            message
        );

        if retryable {
            TranslationError::Http(message)
        } else {
            TranslationError::Api {
                message,
                retryable: false,
            }
        }
    }

    /// 从智谱/OpenAI 错误 JSON 提取用户可见文案（含 contentFilter + code 1301）。
    fn message_from_error_body(body: &str) -> Option<String> {
        let env: ApiErrorEnvelope = serde_json::from_str(body).ok()?;
        let has_cf = env
            .content_filter
            .as_ref()
            .map(|v| !v.is_empty())
            .unwrap_or(false);
        Some(env.error.into_user_message(has_cf))
    }

    fn content_safety_error(message: String) -> TranslationError {
        TranslationError::Api {
            message,
            retryable: false,
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

            let has_content_filter = chunk
                .content_filter
                .as_ref()
                .map(|v| !v.is_empty())
                .unwrap_or(false);

            if let Some(error) = chunk.error {
                let code = error.code_str();
                let message = error.into_user_message(has_content_filter);
                log::warn!(
                    "OpenAI 兼容流内 error: code={:?} content_filter={} msg={}",
                    code,
                    has_content_filter,
                    message
                );
                return Err(Self::content_safety_error(message));
            }

            // 仅有 contentFilter、无 error 字段时也按内容安全失败处理。
            if has_content_filter {
                log::warn!("OpenAI 兼容流内 contentFilter 触发，无 error 字段");
                return Err(Self::content_safety_error(
                    SENSITIVE_CONTENT_FALLBACK_MSG.to_string(),
                ));
            }

            if let Some(usage) = chunk.usage {
                on_event(TranslationStreamEvent::Usage(TokenUsage {
                    input_tokens: usage.prompt_tokens,
                    output_tokens: usage.completion_tokens,
                }));
            }

            if let Some(choices) = chunk.choices {
                for choice in choices {
                    // 先吐出本 chunk 正文，再处理结束原因（敏感截断时常已有部分译文）。
                    if let Some(content) = choice.delta.and_then(|delta| delta.content) {
                        if !content.is_empty() {
                            on_event(TranslationStreamEvent::Delta(content));
                        }
                    }
                    if let Some(reason) = choice.finish_reason.as_deref() {
                        match reason {
                            // 智谱：sensitive；OpenAI 系：content_filter（流内可能无完整 error 体）
                            "sensitive" | "content_filter" => {
                                log::warn!(
                                    "OpenAI 兼容流式输出被内容安全策略截断 (finish_reason={reason})"
                                );
                                return Err(Self::content_safety_error(
                                    SENSITIVE_CONTENT_FALLBACK_MSG.to_string(),
                                ));
                            }
                            "length" => {
                                log::warn!(
                                    "OpenAI 流式输出因 max_tokens 截断 (finish_reason=length, max_tokens={})",
                                    DEFAULT_MAX_TOKENS
                                );
                            }
                            _ => {}
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
    /// `source_text` 仅在 is_auto 时用于剥离模型回显原文。
    async fn process_stream<S, B, E>(
        stream: S,
        is_auto: bool,
        source_text: &str,
        on_event: &mut (dyn FnMut(TranslationStreamEvent) + Send),
        cancel: &CancellationToken,
    ) -> Result<(), TranslationError>
    where
        S: futures_util::Stream<Item = Result<B, E>> + Unpin,
        B: AsRef<[u8]>,
        E: std::fmt::Display,
    {
        let mut parser = if is_auto {
            AutoLangHeaderParser::with_source(source_text)
        } else {
            AutoLangHeaderParser::new()
        };

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
        Self::process_stream(
            response.bytes_stream(),
            is_auto,
            request.source_text(),
            on_event,
            cancel,
        )
        .await
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
            model_name: "mock-model".to_string(),
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
        // OpenAI 兼容协议要求 snake_case 字段名
        assert_eq!(json["stream_options"]["include_usage"], true);
        assert_eq!(json["max_tokens"], DEFAULT_MAX_TOKENS);
    }

    #[test]
    fn consume_sse_event_accepts_finish_reason_length() {
        let event = r#"data: {"choices":[{"delta":{"content":"半句"},"finish_reason":"length"}]}"#;
        let mut events: Vec<TranslationStreamEvent> = Vec::new();
        let done = OpenAiCompatibleProvider::consume_sse_event(event, &mut |ev| {
            events.push(ev);
        })
        .unwrap();
        assert!(!done);
        let text: String = events
            .iter()
            .filter_map(|ev| match ev {
                TranslationStreamEvent::Delta(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(text, "半句");
    }

    #[test]
    fn consume_sse_event_errors_on_finish_reason_sensitive() {
        // 智谱等端：敏感词审核会以 finish_reason=sensitive 结束流，此前可能已有部分译文。
        let event =
            r#"data: {"choices":[{"delta":{"content":"功能"},"finish_reason":"sensitive"}]}"#;
        let mut events: Vec<TranslationStreamEvent> = Vec::new();
        let err = OpenAiCompatibleProvider::consume_sse_event(event, &mut |ev| {
            events.push(ev);
        })
        .expect_err("sensitive 应返回 Api 错误");
        assert!(
            matches!(err, TranslationError::Api { .. }),
            "应为 Api 错误: {err:?}"
        );
        let msg = err.to_string();
        assert!(
            msg.contains("敏感") || msg.contains("不安全"),
            "错误信息应说明敏感内容: {msg}"
        );
        // 已产出的部分 delta 仍会发出（前端 Failed 会覆盖展示文案）。
        let text: String = events
            .iter()
            .filter_map(|ev| match ev {
                TranslationStreamEvent::Delta(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(text, "功能");
    }

    #[test]
    fn consume_sse_event_errors_on_finish_reason_content_filter() {
        let event =
            r#"data: {"choices":[{"delta":{},"finish_reason":"content_filter"}]}"#;
        let err = OpenAiCompatibleProvider::consume_sse_event(event, &mut |_| {})
            .expect_err("content_filter 应返回 Api 错误");
        assert!(
            err.to_string().contains("敏感") || err.to_string().contains("不安全"),
            "{}",
            err
        );
    }

    #[test]
    fn message_from_error_body_parses_zhipu_1301_content_filter() {
        // 用户提供的智谱非流式实际错误体
        let body = r#"{
            "contentFilter": [
                { "level": 1, "role": "assistant" }
            ],
            "error": {
                "code": "1301",
                "message": "系统检测到输入或生成内容可能包含不安全或敏感内容，请您避免输入易产生敏感内容的提示语，感谢您的配合。"
            }
        }"#;
        let msg = OpenAiCompatibleProvider::message_from_error_body(body)
            .expect("应解析智谱 1301 错误体");
        assert!(msg.contains("不安全或敏感内容"), "应透传官方 message: {msg}");
        assert!(!msg.contains("1301"), "默认展示不拼 code，保持官方原文: {msg}");
    }

    #[test]
    fn consume_sse_event_errors_on_zhipu_error_object_in_stream() {
        let event = r#"data: {"contentFilter":[{"level":1,"role":"assistant"}],"error":{"code":"1301","message":"系统检测到输入或生成内容可能包含不安全或敏感内容，请您避免输入易产生敏感内容的提示语，感谢您的配合。"}}"#;
        let err = OpenAiCompatibleProvider::consume_sse_event(event, &mut |_| {})
            .expect_err("流内智谱 error 应失败");
        let msg = err.to_string();
        assert!(
            msg.contains("不安全或敏感内容"),
            "应透传官方 message: {msg}"
        );
    }

    #[test]
    fn api_error_code_accepts_numeric_1301() {
        let raw = r#"{"error":{"code":1301,"message":""},"contentFilter":[{"level":1,"role":"assistant"}]}"#;
        let msg = OpenAiCompatibleProvider::message_from_error_body(raw)
            .expect("数字 code 也应可解析");
        assert!(msg.contains("敏感") || msg.contains("不安全"), "{msg}");
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
        OpenAiCompatibleProvider::process_stream(stream, true, "hello", &mut on_event, &cancel)
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
        OpenAiCompatibleProvider::process_stream(stream, true, "hello", &mut on_event, &cancel)
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
        OpenAiCompatibleProvider::process_stream(stream, false, "", &mut on_event, &cancel)
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
