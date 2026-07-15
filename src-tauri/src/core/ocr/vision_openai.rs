use std::time::Duration;

use serde::Deserialize;

use crate::core::capture::CapturedImage;

use super::image_encode::{encode_captured_image_png_info, png_to_data_url};
use super::resolve::VisionOcrConfig;
use super::{OcrEngine, OcrError, OcrHints, OcrResult};

/// 与 frontend/src/settings/tokens.ts DEFAULT_OCR_PROMPT 对齐
pub const DEFAULT_OCR_PROMPT: &str = "提取图中全部文字，保持阅读顺序";
pub const VISION_OCR_TIMEOUT_SECS: u64 = 60;
pub const VISION_OCR_MAX_TOKENS: u32 = 2048;
const USER_HINT: &str = "请识别图中全部文字。";

pub struct VisionOcrEngine {
    config: VisionOcrConfig,
    client: reqwest::Client,
}

impl VisionOcrEngine {
    pub fn new(config: VisionOcrConfig) -> Result<Self, OcrError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(VISION_OCR_TIMEOUT_SECS))
            .build()
            .map_err(|e| OcrError::Http(e.to_string()))?;
        Ok(Self { config, client })
    }

    fn endpoint(&self) -> String {
        format!(
            "{}/chat/completions",
            self.config.endpoint.trim_end_matches('/')
        )
    }

    /// 纯函数：组 OpenAI Chat Completions 多模态非流式请求体
    pub(crate) fn build_request_body(model: &str, system: &str, data_url: &str) -> serde_json::Value {
        serde_json::json!({
            "model": model,
            "stream": false,
            "max_tokens": VISION_OCR_MAX_TOKENS,
            "messages": [
                {
                    "role": "system",
                    "content": system
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": USER_HINT
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": data_url,
                                "detail": "high"
                            }
                        }
                    ]
                }
            ]
        })
    }

    /// 纯函数：解析 2xx JSON → 文本
    pub(crate) fn parse_success_content(body: &str) -> Result<String, OcrError> {
        let parsed: ChatCompletionResponse = serde_json::from_str(body).map_err(|e| {
            OcrError::Http(format!("OCR 响应解析失败：{}", e))
        })?;

        let content = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message)
            .and_then(|m| m.content)
            .ok_or(OcrError::EmptyResult)?;

        let text = match content {
            MessageContent::String(s) => s,
            MessageContent::Array(parts) => {
                let mut out = String::new();
                for part in parts {
                    if part.part_type.as_deref() == Some("text") {
                        if let Some(t) = part.text {
                            out.push_str(&t);
                        }
                    }
                }
                out
            }
        };

        let text = text.trim().to_string();
        if text.is_empty() {
            return Err(OcrError::EmptyResult);
        }
        Ok(text)
    }

    /// 纯函数：HTTP 状态 + body → OcrError
    pub(crate) fn map_http_error(status: u16, body: &str) -> OcrError {
        let message = extract_error_message(body).unwrap_or_else(|| {
            format!(
                "HTTP {}: {}",
                status,
                body.chars().take(500).collect::<String>()
            )
        });

        if status == 401 || status == 403 {
            return OcrError::Auth(message);
        }

        let retryable = status == 429 || (500..600).contains(&status);
        if retryable {
            OcrError::Http(message)
        } else {
            OcrError::Api {
                message,
                retryable: false,
            }
        }
    }
}

#[async_trait::async_trait]
impl OcrEngine for VisionOcrEngine {
    async fn recognize(
        &self,
        image: CapturedImage,
        _hints: OcrHints,
    ) -> Result<OcrResult, OcrError> {
        let start = std::time::Instant::now();
        let encoded = encode_captured_image_png_info(&image)?;
        log::debug!(
            "Vision OCR 编码: src={}x{} sent={}x{} scaled={} png_bytes={}",
            encoded.source_width,
            encoded.source_height,
            encoded.sent_width,
            encoded.sent_height,
            encoded.scaled,
            encoded.png.len()
        );
        let data_url = png_to_data_url(&encoded.png);
        let system = if self.config.ocr_prompt.trim().is_empty() {
            DEFAULT_OCR_PROMPT
        } else {
            self.config.ocr_prompt.as_str()
        };
        let endpoint = self.endpoint();
        log::debug!(
            "Vision OCR 请求: endpoint={} model={} prompt_len={}",
            endpoint,
            self.config.model,
            system.chars().count()
        );
        log::debug!("Vision OCR system prompt: {system}");
        let body = Self::build_request_body(&self.config.model, system, &data_url);
        let resp = self
            .client
            .post(endpoint)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| OcrError::Http(e.to_string()))?;
        let status = resp.status().as_u16();
        let text = resp
            .text()
            .await
            .map_err(|e| OcrError::Http(e.to_string()))?;
        if !(200..300).contains(&status) {
            return Err(Self::map_http_error(status, &text));
        }
        let content = Self::parse_success_content(&text)?;
        log::info!(
            "Vision OCR 完成: status={} latency_ms={} text={}",
            status,
            start.elapsed().as_millis(),
            crate::core::logging::redact_text(
                &content,
                crate::core::logging::effective_redact_level()
            )
        );
        Ok(OcrResult {
            text: content,
            lines: vec![],
            engine: self.config.service_type.clone(),
        })
    }
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    #[serde(default)]
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    #[serde(default)]
    message: Option<ChatMessage>,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    #[serde(default)]
    content: Option<MessageContent>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MessageContent {
    String(String),
    Array(Vec<ContentPart>),
}

#[derive(Debug, Deserialize)]
struct ContentPart {
    #[serde(rename = "type")]
    part_type: Option<String>,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiErrorEnvelope {
    error: ApiErrorBody,
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    #[serde(default)]
    message: String,
}

/// 最小错误解析：优先 `error.message`
fn extract_error_message(body: &str) -> Option<String> {
    let env: ApiErrorEnvelope = serde_json::from_str(body).ok()?;
    let msg = env.error.message.trim();
    if msg.is_empty() {
        None
    } else {
        Some(msg.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_body_is_non_streaming_with_image_url() {
        let body = VisionOcrEngine::build_request_body(
            "gpt-4o",
            "提取图中全部文字，保持阅读顺序",
            "data:image/png;base64,AAA",
        );
        assert_eq!(body["stream"], false);
        assert_eq!(body["max_tokens"], 2048);
        assert_eq!(body["model"], "gpt-4o");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(
            body["messages"][0]["content"],
            "提取图中全部文字，保持阅读顺序"
        );
        let user_content = &body["messages"][1]["content"];
        assert_eq!(user_content[0]["type"], "text");
        assert_eq!(user_content[0]["text"], USER_HINT);
        assert_eq!(user_content[1]["type"], "image_url");
        assert_eq!(
            user_content[1]["image_url"]["url"],
            "data:image/png;base64,AAA"
        );
        assert_eq!(user_content[1]["image_url"]["detail"], "high");
    }

    #[test]
    fn request_body_sets_image_url_detail_high() {
        let body = VisionOcrEngine::build_request_body(
            "gpt-4o",
            "sys",
            "data:image/png;base64,AAA",
        );
        assert_eq!(
            body["messages"][1]["content"][1]["image_url"]["detail"],
            "high"
        );
    }

    #[test]
    fn parse_success_string_content() {
        let raw = r#"{"choices":[{"message":{"content":"  Hello  "}}]}"#;
        assert_eq!(
            VisionOcrEngine::parse_success_content(raw).unwrap(),
            "Hello"
        );
    }

    #[test]
    fn parse_success_array_content() {
        let raw = r#"{"choices":[{"message":{"content":[
      {"type":"text","text":"A"},
      {"type":"text","text":"B"}
    ]}}]}"#;
        assert_eq!(VisionOcrEngine::parse_success_content(raw).unwrap(), "AB");
    }

    #[test]
    fn parse_empty_content_is_empty_result() {
        let raw = r#"{"choices":[{"message":{"content":"   "}}]}"#;
        assert!(matches!(
            VisionOcrEngine::parse_success_content(raw),
            Err(OcrError::EmptyResult)
        ));
    }

    #[test]
    fn map_401_to_auth() {
        let err = VisionOcrEngine::map_http_error(401, r#"{"error":{"message":"bad key"}}"#);
        assert!(matches!(err, OcrError::Auth(_)));
        if let OcrError::Auth(msg) = err {
            assert!(msg.contains("bad key"));
        }
    }
}
