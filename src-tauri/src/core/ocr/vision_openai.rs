use std::time::Duration;

use serde::Deserialize;

use crate::core::capture::CapturedImage;

use super::image_encode::{encode_captured_image_png_info, png_to_data_url};
use super::resolve::VisionOcrConfig;
use super::{OcrEngine, OcrError, OcrHints, OcrResult};

/// 通用视觉模型默认 OCR 提示（与 frontend `DEFAULT_OCR_PROMPT` 对齐）。
pub const DEFAULT_OCR_PROMPT: &str =
    "请识别图中全部文字，按阅读顺序完整输出。只输出文字，不要解释。";
/// DeepSeek-OCR 官方推荐默认任务句（与 frontend `DEFAULT_DEEPSEEK_OCR_PROMPT` 对齐）。
pub const DEFAULT_DEEPSEEK_OCR_PROMPT: &str = "Free OCR.";
pub const VISION_OCR_TIMEOUT_SECS: u64 = 60;
pub const VISION_OCR_MAX_TOKENS: u32 = 2048;
/// OCR 固定 temperature=0，降低采样随机导致的结果漂移与胡话。
pub const VISION_OCR_TEMPERATURE: f32 = 0.0;

/// **临时**开关：`true` 时 debug 输出完整请求体/响应体（含 base64 图）。
/// 仅用于排查 OCR 问题；修完后务必改回 `false`，恢复 `sanitize_request_body_for_log`。
/// Authorization 始终走 `format_auth_header_for_log`，不写 Key 明文。
const VISION_OCR_TEMP_LOG_FULL_BODY: bool = true;

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

    /// 纯函数：组 OpenAI Chat Completions 多模态非流式请求体。
    ///
    /// - 通用视觉模型：user content = [text 完整 prompt, image_url + detail=high]
    /// - DeepSeek-OCR（硅基流动等）：**不传 detail**（文档写明不支持，固定 1024 Base），
    ///   content = [image_url, text]（图在前）。此前对 DeepSeek-OCR 传 `detail=high`
    ///   且 text-before-image 时，大图易返回 `}}]}}]` 退化串。
    /// - 不用 system 放主指令；`temperature=0` 压采样随机。
    pub(crate) fn build_request_body(model: &str, prompt: &str, data_url: &str) -> serde_json::Value {
        let deepseek_ocr = is_deepseek_ocr_model(model);
        let image_part = if deepseek_ocr {
            serde_json::json!({
                "type": "image_url",
                "image_url": { "url": data_url }
            })
        } else {
            serde_json::json!({
                "type": "image_url",
                "image_url": {
                    "url": data_url,
                    "detail": "high"
                }
            })
        };
        let text_part = serde_json::json!({
            "type": "text",
            "text": prompt
        });
        // DeepSeek-OCR：图 → 提示；其它 VLM：提示 → 图
        let content = if deepseek_ocr {
            serde_json::json!([image_part, text_part])
        } else {
            serde_json::json!([text_part, image_part])
        };
        serde_json::json!({
            "model": model,
            "stream": false,
            "temperature": VISION_OCR_TEMPERATURE,
            "max_tokens": VISION_OCR_MAX_TOKENS,
            "messages": [
                {
                    "role": "user",
                    "content": content
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
        // 模型崩溃时会吐出 `}}]}}]` / `} } } }` 等 JSON 碎片（DeepSeek-OCR 常见）
        if is_degenerate_ocr_output(&text) {
            return Err(OcrError::Api {
                message: "OCR 模型返回了无效内容（疑似格式不兼容或生成退化）。\
建议：换用通用视觉模型（如 gpt-4o / Qwen-VL），或检查 DeepSeek-OCR 提示词（可用 Free OCR.）。"
                    .into(),
                retryable: true,
            });
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
        let system = effective_ocr_prompt(&self.config.model, &self.config.ocr_prompt);
        let endpoint = self.endpoint();
        let body = Self::build_request_body(&self.config.model, system, &data_url);
        log::debug!("Vision OCR 请求诊断: POST {endpoint}");
        log::debug!(
            "Vision OCR 请求头: Authorization={}, Content-Type=application/json",
            format_auth_header_for_log(&self.config.api_key)
        );
        // TEMP: 排查 OCR 问题时临时输出完整请求体（含 base64）。修好后改 false，恢复脱敏。
        // Authorization 始终脱敏，永不写 Key 明文。
        if VISION_OCR_TEMP_LOG_FULL_BODY {
            log::debug!("Vision OCR 请求体(TEMP完整): {body}");
        } else {
            log::debug!(
                "Vision OCR 请求体: {}",
                sanitize_request_body_for_log(&body)
            );
        }
        // 完整 OCR 提示词（配置非 secret；现放在 user content[0] text）
        log::debug!("Vision OCR prompt (user text first): {system}");
        let resp = self
            .client
            .post(&endpoint)
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
            if VISION_OCR_TEMP_LOG_FULL_BODY {
                log::debug!(
                    "Vision OCR 错误响应(TEMP完整): status={status} body={text}"
                );
            } else {
                log::debug!(
                    "Vision OCR 错误响应: status={status} body_len={}",
                    text.len()
                );
            }
            return Err(Self::map_http_error(status, &text));
        }
        let body_len = text.len();
        if VISION_OCR_TEMP_LOG_FULL_BODY {
            log::debug!("Vision OCR 响应(TEMP完整): status={status} body={text}");
        } else {
            log::debug!("Vision OCR 响应: status={status} body_len={body_len}");
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(usage) = v.get("usage") {
                    log::debug!("Vision OCR usage: {usage}");
                }
            }
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

/// SiliconFlow 等对 `deepseek-ai/DeepSeek-OCR` 有专用约定（无 detail、固定分辨率）。
pub(crate) fn is_deepseek_ocr_model(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    m.contains("deepseek-ocr") || m.contains("deepseek_ocr")
}

/// 配置为空时按模型选默认提示；非空则用用户配置（trim 后）。
pub(crate) fn effective_ocr_prompt<'a>(model: &str, configured: &'a str) -> &'a str {
    let t = configured.trim();
    if !t.is_empty() {
        return t;
    }
    if is_deepseek_ocr_model(model) {
        DEFAULT_DEEPSEEK_OCR_PROMPT
    } else {
        DEFAULT_OCR_PROMPT
    }
}

/// 是否为「只含括号/空白」的退化输出（如 `}}]}}]` 循环）。
pub(crate) fn is_degenerate_ocr_output(text: &str) -> bool {
    let t = text.trim();
    if t.chars().count() < 16 {
        return false;
    }
    let total = t.chars().count() as f64;
    let junk = t
        .chars()
        .filter(|c| matches!(c, '}' | '{' | ']' | '[' | ')' | '(' | ',' | ':' | ' ' | '\t' | '\n' | '\r'))
        .count() as f64;
    junk / total >= 0.90
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

/// 将请求体中的 image_url 替换为长度占位，供 debug 日志使用。禁止 dump 原始 base64。
pub(crate) fn sanitize_request_body_for_log(body: &serde_json::Value) -> serde_json::Value {
    let mut out = body.clone();
    if let Some(messages) = out.get_mut("messages").and_then(|m| m.as_array_mut()) {
        for msg in messages.iter_mut() {
            if let Some(content) = msg.get_mut("content").and_then(|c| c.as_array_mut()) {
                for part in content.iter_mut() {
                    if part.get("type").and_then(|t| t.as_str()) == Some("image_url") {
                        if let Some(url_val) = part
                            .pointer_mut("/image_url/url")
                            .filter(|v| v.is_string())
                        {
                            let original = url_val.as_str().unwrap_or("").to_string();
                            *url_val =
                                serde_json::Value::String(sanitize_image_url_for_log(&original));
                        }
                    }
                }
            }
        }
    }
    out
}

fn sanitize_image_url_for_log(url: &str) -> String {
    const PREFIX: &str = "data:image/png;base64,";
    if let Some(rest) = url.strip_prefix(PREFIX) {
        return format!("{PREFIX}[len={}]", rest.len());
    }
    if let Some(rest) = url.strip_prefix("data:") {
        // 其它 data URL：保留 media type 前缀到第一个逗号后的 len
        if let Some((meta, payload)) = rest.split_once(',') {
            return format!("data:{meta},[len={}]", payload.len());
        }
        return format!("data:[len={}]", rest.len());
    }
    // 非 data URL：仅 scheme + 总长度，避免 query 明文
    let scheme = url.split(':').next().unwrap_or("unknown");
    format!("{scheme}:[len={}]", url.len())
}

/// Authorization 头日志：`Bearer {redact_api_key}`。
pub(crate) fn format_auth_header_for_log(api_key: &str) -> String {
    format!(
        "Bearer {}",
        crate::core::logging::redact_api_key(api_key)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_body_is_non_streaming_with_image_url() {
        let body = VisionOcrEngine::build_request_body(
            "gpt-4o",
            DEFAULT_OCR_PROMPT,
            "data:image/png;base64,AAA",
        );
        assert_eq!(body["stream"], false);
        assert_eq!(body["temperature"], 0.0);
        assert_eq!(body["max_tokens"], 2048);
        assert_eq!(body["model"], "gpt-4o");
        // 单条 user：先完整提示词 text，再 image
        assert_eq!(body["messages"].as_array().map(|a| a.len()), Some(1));
        assert_eq!(body["messages"][0]["role"], "user");
        let user_content = &body["messages"][0]["content"];
        assert_eq!(user_content[0]["type"], "text");
        assert_eq!(user_content[0]["text"], DEFAULT_OCR_PROMPT);
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
            body["messages"][0]["content"][1]["image_url"]["detail"],
            "high"
        );
    }

    #[test]
    fn request_body_prompt_text_before_image() {
        let body = VisionOcrEngine::build_request_body(
            "m",
            "FULL_PROMPT_BEFORE_IMAGE",
            "data:image/png;base64,XYZ",
        );
        let parts = body["messages"][0]["content"].as_array().expect("parts");
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0]["type"], "text");
        assert_eq!(parts[0]["text"], "FULL_PROMPT_BEFORE_IMAGE");
        assert_eq!(parts[1]["type"], "image_url");
    }

    #[test]
    fn deepseek_ocr_image_before_text_and_no_detail() {
        let body = VisionOcrEngine::build_request_body(
            "deepseek-ai/DeepSeek-OCR",
            "Free OCR.",
            "data:image/png;base64,AAA",
        );
        let parts = body["messages"][0]["content"].as_array().expect("parts");
        assert_eq!(parts[0]["type"], "image_url");
        assert_eq!(parts[0]["image_url"]["url"], "data:image/png;base64,AAA");
        assert!(
            parts[0]["image_url"].get("detail").is_none(),
            "DeepSeek-OCR 不得传 detail"
        );
        assert_eq!(parts[1]["type"], "text");
        assert_eq!(parts[1]["text"], "Free OCR.");
    }

    #[test]
    fn parse_rejects_degenerate_brace_spam() {
        let spam = "}}]".repeat(40);
        let raw = format!(
            r#"{{"choices":[{{"message":{{"content":"{spam}"}}}}]}}"#
        );
        let err = VisionOcrEngine::parse_success_content(&raw).unwrap_err();
        assert!(
            matches!(err, OcrError::Api { .. }),
            "应拒绝退化输出: {err:?}"
        );
    }

    #[test]
    fn is_degenerate_detects_brace_loop_not_normal_text() {
        assert!(is_degenerate_ocr_output(&"}}]".repeat(30)));
        assert!(!is_degenerate_ocr_output(DEFAULT_OCR_PROMPT));
        assert!(!is_degenerate_ocr_output("Hello world from OCR"));
    }

    #[test]
    fn effective_prompt_defaults_by_model() {
        assert_eq!(
            effective_ocr_prompt("gpt-4o", ""),
            DEFAULT_OCR_PROMPT
        );
        assert_eq!(
            effective_ocr_prompt("deepseek-ai/DeepSeek-OCR", ""),
            DEFAULT_DEEPSEEK_OCR_PROMPT
        );
        assert_eq!(
            effective_ocr_prompt("deepseek-ai/DeepSeek-OCR", "  Free OCR.  "),
            "Free OCR."
        );
        assert_eq!(
            effective_ocr_prompt("gpt-4o", "自定义"),
            "自定义"
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

    #[test]
    fn sanitize_request_body_redacts_data_url_keeps_structure() {
        let long_b64 = "A".repeat(200);
        let data_url = format!("data:image/png;base64,{long_b64}");
        let body = VisionOcrEngine::build_request_body("gpt-4o", "sys-prompt-full", &data_url);
        let sanitized = sanitize_request_body_for_log(&body);
        let s = sanitized.to_string();

        assert!(s.contains("[len="), "应含长度占位: {s}");
        assert!(!s.contains(&long_b64), "不得含原始 base64 片段");
        assert_eq!(sanitized["model"], "gpt-4o");
        assert_eq!(sanitized["stream"], false);
        assert_eq!(sanitized["max_tokens"], 2048);
        assert_eq!(sanitized["messages"][0]["content"][0]["text"], "sys-prompt-full");
        assert_eq!(
            sanitized["messages"][0]["content"][1]["image_url"]["detail"],
            "high"
        );
        let url = sanitized["messages"][0]["content"][1]["image_url"]["url"]
            .as_str()
            .expect("url string");
        assert!(url.starts_with("data:image/png;base64,[len="));
        assert!(url.contains(&format!("[len={}]", long_b64.len())));
    }

    #[test]
    fn sanitize_request_body_non_data_url_records_scheme_and_len() {
        let url = "https://example.com/img.png?token=secret";
        let body = VisionOcrEngine::build_request_body("m", "s", url);
        let sanitized = sanitize_request_body_for_log(&body);
        let out = sanitized["messages"][0]["content"][1]["image_url"]["url"]
            .as_str()
            .unwrap();
        assert!(out.contains("https"), "应含 scheme: {out}");
        assert!(out.contains(&format!("len={}", url.len())), "应含 len: {out}");
        assert!(!out.contains("secret"), "不得含 query 明文 token");
    }

    #[test]
    fn format_auth_header_redacts_api_key() {
        let key = "sk-abcdef12345678";
        let header = format_auth_header_for_log(key);
        assert_eq!(header, format!("Bearer {}", crate::core::logging::redact_api_key(key)));
        assert!(!header.contains("abcdef12345678"));
        assert!(!header.contains(key));
    }
}
