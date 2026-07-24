//! Bridge JSON 协议信封与 payload 类型。
//!
//! 请求 type：
//! `start_translation` | `cancel_translation` | `retry_translation` |
//! `set_session_languages` | `open_settings` | `trigger_ocr_translation` |
//! `save_edge_translate_env` | `take_pending_source_text` | `get_app_config` |
//! `report_content_size` | `ready`
//!
//! 推送 type：
//! `translation_event` | `app_config_changed` | `interface_language_changed` |
//! `show_context` | `response`
//!
//! 宿主接线前类型可能暂无外部调用方。

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// 宿主 ← WebView 请求信封（反序列化）。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeEnvelope {
    pub bridge_version: u32,
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
}

/// 宿主 → WebView 推送信封（序列化）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgePush {
    pub bridge_version: u32,
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartTranslationPayload {
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSessionLanguagesPayload {
    pub source_lang: String,
    pub target_lang: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportContentSizePayload {
    pub width: f64,
    pub height: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_request_roundtrip_start_translation() {
        let raw = r#"{"bridgeVersion":1,"type":"start_translation","requestId":"r1","payload":{"text":"hello"}}"#;
        let env: BridgeEnvelope = serde_json::from_str(raw).unwrap();
        assert_eq!(env.bridge_version, 1);
        assert_eq!(env.type_name, "start_translation");
        let p: StartTranslationPayload = serde_json::from_value(env.payload.unwrap()).unwrap();
        assert_eq!(p.text, "hello");
    }

    #[test]
    fn translation_event_push_matches_camel_case_fields() {
        let push = BridgePush {
            bridge_version: 1,
            type_name: "translation_event".into(),
            payload: Some(serde_json::json!({
                "type": "delta",
                "sessionId": "b1:svc",
                "serviceInstanceId": "svc",
                "text": "你好"
            })),
        };
        let s = serde_json::to_string(&push).unwrap();
        assert!(s.contains("bridgeVersion"));
        assert!(s.contains("translation_event"));
        assert!(s.contains("serviceInstanceId"));
    }

    #[test]
    fn unknown_type_is_parseable_for_ignore() {
        let raw = r#"{"bridgeVersion":1,"type":"future_thing"}"#;
        let env: BridgeEnvelope = serde_json::from_str(raw).unwrap();
        assert_eq!(env.type_name, "future_thing");
    }
}
