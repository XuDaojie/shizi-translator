use serde::Deserialize;

use crate::core::llm::LlmError;

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

pub struct ClaudeProvider;

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

impl ClaudeProvider {
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
}
