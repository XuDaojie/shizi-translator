//! UI → Rust Bridge 请求分发：type 映射到现有翻译/配置/OCR 入口。
//!
//! 不实现 push hub / hostfxr；仅同步分发并返回 [`BridgeResponse`]。
//! 宿主接线前 API 可能暂无外部调用方。

#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::Manager;

use super::protocol::{
    BridgeEnvelope, ReportContentSizePayload, SetSessionLanguagesPayload, StartTranslationPayload,
};
use crate::{
    app::{
        popup_ui::{PopupUi, PopupUiKind, WebviewPopupUi},
        shortcuts::trigger_ocr_translate,
        state::AppState,
        window::request_show_settings_window,
    },
    core::config::AppConfig,
    ui::web_popup::{start_translation_from_input, start_translation_from_text},
};

#[cfg(windows)]
use crate::app::popup_ui::WinUiPopupUi;

/// 按当前 session active 调整弹窗尺寸。
fn report_content_size(app: &tauri::AppHandle, width: f64, height: f64) -> Result<(), String> {
    let active = app
        .try_state::<AppState>()
        .and_then(|s| s.popup_ui_session.lock().ok().map(|g| g.active()))
        .unwrap_or(PopupUiKind::Webview);

    match active {
        PopupUiKind::WinUi => {
            #[cfg(windows)]
            {
                WinUiPopupUi::new().set_size(app, width, height)
            }
            #[cfg(not(windows))]
            {
                WebviewPopupUi::new().set_size(app, width, height)
            }
        }
        PopupUiKind::Webview => WebviewPopupUi::new().set_size(app, width, height),
    }
}

/// 与协议信封一致的 bridgeVersion。
pub const BRIDGE_VERSION: u32 = 1;

/// WinUI / Bridge UI 首帧 ready 标志。
///
/// 对齐现 WebView 弹窗约 2s 的 readyGate：宿主应在收到 `ready` 后解除 show 门闩；
/// 超时仍 show 的门闩逻辑在宿主侧（本模块仅记录 flag，便于后续挂接）。
static BRIDGE_UI_READY: AtomicBool = AtomicBool::new(false);

/// 可单测的请求 type 分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeOp {
    StartTranslation,
    CancelTranslation,
    RetryTranslation,
    SetSessionLanguages,
    OpenSettings,
    TriggerOcrTranslation,
    SaveEdgeTranslateEnv,
    TakePendingSourceText,
    GetAppConfig,
    ReportContentSize,
    Ready,
    Unknown,
}

/// 将 Bridge 请求 `type` 映射为操作枚举。
pub fn classify(type_name: &str) -> BridgeOp {
    match type_name {
        "start_translation" => BridgeOp::StartTranslation,
        "cancel_translation" => BridgeOp::CancelTranslation,
        "retry_translation" => BridgeOp::RetryTranslation,
        "set_session_languages" => BridgeOp::SetSessionLanguages,
        "open_settings" => BridgeOp::OpenSettings,
        "trigger_ocr_translation" => BridgeOp::TriggerOcrTranslation,
        "save_edge_translate_env" => BridgeOp::SaveEdgeTranslateEnv,
        "take_pending_source_text" => BridgeOp::TakePendingSourceText,
        "get_app_config" => BridgeOp::GetAppConfig,
        "report_content_size" => BridgeOp::ReportContentSize,
        "ready" => BridgeOp::Ready,
        _ => BridgeOp::Unknown,
    }
}

/// 宿主 → UI 的请求响应信封。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeResponse {
    pub bridge_version: u32,
    #[serde(rename = "type")]
    pub type_name: &'static str,
    pub request_id: Option<String>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl BridgeResponse {
    pub fn ok(request_id: Option<String>, body: Option<serde_json::Value>) -> Self {
        Self {
            bridge_version: BRIDGE_VERSION,
            type_name: "response",
            request_id,
            ok: true,
            body,
            error: None,
        }
    }

    pub fn err(request_id: Option<String>, error: impl Into<String>) -> Self {
        Self {
            bridge_version: BRIDGE_VERSION,
            type_name: "response",
            request_id,
            ok: false,
            body: None,
            error: Some(error.into()),
        }
    }
}

/// Bridge UI 是否已上报 ready。
pub fn is_bridge_ui_ready() -> bool {
    BRIDGE_UI_READY.load(Ordering::SeqCst)
}

/// 标记 Bridge UI ready（`ready` 请求与测试用）。
pub fn mark_bridge_ui_ready() {
    BRIDGE_UI_READY.store(true, Ordering::SeqCst);
}

#[cfg(test)]
fn reset_bridge_ui_ready_for_test() {
    BRIDGE_UI_READY.store(false, Ordering::SeqCst);
}

/// 将 `AppConfig` 序列化为 Bridge body，并清除 `services[].apiKey` 与 `ocrServices[].apiKey`。
pub fn redact_config_for_bridge(mut config: AppConfig) -> serde_json::Value {
    for service in &mut config.services {
        service.api_key = None;
    }
    for service in &mut config.ocr_services {
        service.api_key = None;
    }
    serde_json::to_value(config).unwrap_or(serde_json::Value::Null)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveEdgeTranslateEnvPayload {
    user_agent: String,
    accept_language: String,
}

fn payload_value(envelope: &BridgeEnvelope) -> Result<&serde_json::Value, String> {
    envelope
        .payload
        .as_ref()
        .ok_or_else(|| "缺少 payload".to_string())
}

fn parse_payload<T: for<'de> Deserialize<'de>>(envelope: &BridgeEnvelope) -> Result<T, String> {
    let value = payload_value(envelope)?;
    serde_json::from_value(value.clone()).map_err(|e| format!("payload 解析失败: {e}"))
}

/// 处理一条 UI→Rust Bridge 请求，映射到现有内部入口（非 tauri command 包装）。
pub fn handle_bridge_request(
    app: &tauri::AppHandle,
    state: &AppState,
    envelope: &BridgeEnvelope,
) -> BridgeResponse {
    let request_id = envelope.request_id.clone();

    match classify(&envelope.type_name) {
        BridgeOp::StartTranslation => match parse_payload::<StartTranslationPayload>(envelope) {
            Ok(p) => match start_translation_from_text(p.text, app.clone(), state) {
                Ok(batch_id) => BridgeResponse::ok(request_id, Some(json!(batch_id))),
                Err(e) => BridgeResponse::err(request_id, e),
            },
            Err(e) => BridgeResponse::err(request_id, e),
        },

        BridgeOp::CancelTranslation => match state.cancel_current_translation() {
            Ok(()) => BridgeResponse::ok(request_id, None),
            Err(e) => BridgeResponse::err(request_id, e),
        },

        BridgeOp::RetryTranslation => {
            match state.take_last_translation_input() {
                Ok(Some(input)) => match start_translation_from_input(input, app.clone(), state) {
                    Ok(batch_id) => BridgeResponse::ok(request_id, Some(json!(batch_id))),
                    Err(e) => BridgeResponse::err(request_id, e),
                },
                Ok(None) => BridgeResponse::err(request_id, "没有可重试的翻译"),
                Err(e) => BridgeResponse::err(request_id, e),
            }
        }

        BridgeOp::SetSessionLanguages => {
            match parse_payload::<SetSessionLanguagesPayload>(envelope) {
                Ok(p) => match state.set_session_languages(p.source_lang, p.target_lang) {
                    Ok(()) => BridgeResponse::ok(request_id, None),
                    Err(e) => BridgeResponse::err(request_id, e),
                },
                Err(e) => BridgeResponse::err(request_id, e),
            }
        }

        BridgeOp::OpenSettings => {
            // 独立线程打开，避免 Windows 上首次建窗在回调栈死锁（同 tray 路径）。
            request_show_settings_window(app);
            BridgeResponse::ok(request_id, None)
        }

        BridgeOp::TriggerOcrTranslation => {
            // `start_translation_from_ocr` 内部会先 hide 弹窗再抓帧。
            trigger_ocr_translate(app);
            BridgeResponse::ok(request_id, None)
        }

        BridgeOp::SaveEdgeTranslateEnv => {
            match parse_payload::<SaveEdgeTranslateEnvPayload>(envelope) {
                Ok(p) => match state.set_edge_translate_env(crate::core::mt::EdgeTranslateEnv {
                    user_agent: p.user_agent,
                    accept_language: p.accept_language,
                }) {
                    Ok(()) => BridgeResponse::ok(request_id, None),
                    Err(e) => BridgeResponse::err(request_id, e),
                },
                Err(e) => BridgeResponse::err(request_id, e),
            }
        }

        BridgeOp::TakePendingSourceText => match state.take_pending_source_text() {
            Ok(text) => BridgeResponse::ok(request_id, Some(json!(text))),
            Err(e) => BridgeResponse::err(request_id, e),
        },

        BridgeOp::GetAppConfig => match state.config_store.get() {
            Ok(config) => {
                let body = redact_config_for_bridge(config);
                BridgeResponse::ok(request_id, Some(body))
            }
            Err(e) => BridgeResponse::err(request_id, e.to_string()),
        },

        BridgeOp::ReportContentSize => match parse_payload::<ReportContentSizePayload>(envelope) {
            Ok(p) => {
                // 优先 WinUI（若会话 active 为 winui 且宿主在跑）；否则 webview。
                let result = report_content_size(app, p.width, p.height);
                match result {
                    Ok(()) => BridgeResponse::ok(request_id, None),
                    Err(e) => BridgeResponse::err(request_id, e),
                }
            }
            Err(e) => BridgeResponse::err(request_id, e),
        },

        BridgeOp::Ready => {
            // stub：记 ready 标志；2s 超时 show 门闩由宿主侧实现（对齐 mainWindowReady）。
            mark_bridge_ui_ready();
            log::debug!("popup_bridge: UI ready");
            BridgeResponse::ok(request_id, None)
        }

        BridgeOp::Unknown => {
            log::warn!(
                "popup_bridge: 未知请求 type={} requestId={:?}",
                envelope.type_name,
                request_id
            );
            BridgeResponse::err(
                request_id,
                format!("未知 bridge 请求类型: {}", envelope.type_name),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_types() {
        assert!(matches!(
            classify("start_translation"),
            BridgeOp::StartTranslation
        ));
        assert!(matches!(
            classify("cancel_translation"),
            BridgeOp::CancelTranslation
        ));
        assert!(matches!(
            classify("retry_translation"),
            BridgeOp::RetryTranslation
        ));
        assert!(matches!(
            classify("set_session_languages"),
            BridgeOp::SetSessionLanguages
        ));
        assert!(matches!(classify("open_settings"), BridgeOp::OpenSettings));
        assert!(matches!(
            classify("trigger_ocr_translation"),
            BridgeOp::TriggerOcrTranslation
        ));
        assert!(matches!(
            classify("save_edge_translate_env"),
            BridgeOp::SaveEdgeTranslateEnv
        ));
        assert!(matches!(
            classify("take_pending_source_text"),
            BridgeOp::TakePendingSourceText
        ));
        assert!(matches!(classify("get_app_config"), BridgeOp::GetAppConfig));
        assert!(matches!(
            classify("report_content_size"),
            BridgeOp::ReportContentSize
        ));
        assert!(matches!(classify("ready"), BridgeOp::Ready));
        assert!(matches!(classify("nope"), BridgeOp::Unknown));
    }

    #[test]
    fn redact_config_clears_api_keys() {
        let mut config = AppConfig::default();
        if let Some(svc) = config.services.first_mut() {
            svc.api_key = Some("sk-secret-service".into());
        }
        if config.ocr_services.is_empty() {
            config.ocr_services.push(
                crate::core::config::types::OcrServiceInstanceConfig {
                    id: "ocr-1".into(),
                    service_type: "vision".into(),
                    name: "Vision".into(),
                    enabled: true,
                    api_key: Some("sk-secret-ocr".into()),
                    endpoint: String::new(),
                    model: String::new(),
                    preferred_lang: String::new(),
                    ocr_prompt: String::new(),
                },
            );
        } else if let Some(ocr) = config.ocr_services.first_mut() {
            ocr.api_key = Some("sk-secret-ocr".into());
        }

        let value = redact_config_for_bridge(config);
        let s = value.to_string();
        assert!(
            !s.contains("sk-secret-service"),
            "services apiKey 应脱敏: {s}"
        );
        assert!(!s.contains("sk-secret-ocr"), "ocrServices apiKey 应脱敏: {s}");

        let services = value
            .get("services")
            .and_then(|v| v.as_array())
            .expect("services");
        if let Some(first) = services.first() {
            let key = first.get("apiKey");
            assert!(
                key.is_none() || key == Some(&serde_json::Value::Null),
                "apiKey 应为 null/缺省: {key:?}"
            );
        }
        let ocr_services = value
            .get("ocrServices")
            .and_then(|v| v.as_array())
            .expect("ocrServices");
        if let Some(first) = ocr_services.first() {
            let key = first.get("apiKey");
            assert!(
                key.is_none() || key == Some(&serde_json::Value::Null),
                "ocr apiKey 应为 null/缺省: {key:?}"
            );
        }
    }

    #[test]
    fn bridge_response_serializes_camel_case() {
        let resp = BridgeResponse::ok(Some("r1".into()), Some(json!({"x": 1})));
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("\"bridgeVersion\":1"));
        assert!(s.contains("\"type\":\"response\""));
        assert!(s.contains("\"requestId\":\"r1\""));
        assert!(s.contains("\"ok\":true"));
        assert!(!s.contains("\"error\""));
    }

    #[test]
    fn bridge_response_err_skips_body() {
        let resp = BridgeResponse::err(None, "boom");
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("\"ok\":false"));
        assert!(s.contains("\"error\":\"boom\""));
        assert!(!s.contains("\"body\""));
    }

    #[test]
    fn ready_flag_marks_and_reads() {
        reset_bridge_ui_ready_for_test();
        assert!(!is_bridge_ui_ready());
        mark_bridge_ui_ready();
        assert!(is_bridge_ui_ready());
        reset_bridge_ui_ready_for_test();
    }
}
