use crate::{
    app::{
        popup_window,
        state::{AppState, CapturePurpose},
    },
    core::{capture::CaptureError, ocr::OcrError, ocr_translation::OcrTranslationError},
    platform::capture_screen,
    ui::{overlay, web_popup::show_translation_error},
};

use tauri::Manager;

pub async fn start_translation_from_ocr(app: tauri::AppHandle, state: AppState) {
    if state.is_translation_busy() {
        show_translation_error(&app, "正在翻译中，请稍后再试");
        return;
    }

    // capture 独立锁：挡住 OCR/recognize 期间二次截图快捷键覆盖 pending_capture。
    // 持锁到 submit_capture_region / cancel_capture 释放；本函数每条失败路径都须 finish_capture。
    if let Err(message) = state.try_begin_capture() {
        show_translation_error(&app, message);
        return;
    }
    // Alt+S / 弹窗截图翻译入口：提交后走翻译链路，禁止纯识别分叉。
    let _ = state.set_capture_purpose(CapturePurpose::Translate);

    // 先抓整屏帧（overlay 显示前拍完，避免把 overlay 截进图里）
    let frame = match capture_screen().await {
        Ok(frame) => frame,
        Err(error) => {
            let _ = state.finish_capture();
            show_translation_error(&app, friendly_ocr_error(OcrTranslationError::Capture(error)));
            return;
        }
    };

    // scale_factor 取主窗口缩放（MVP 简化；多屏精确缩放留后续）
    let scale = app
        .get_webview_window("main")
        .and_then(|w| w.scale_factor().ok())
        .unwrap_or(1.0);

    if let Err(error) = state.set_pending_capture(frame, scale) {
        let _ = state.finish_capture();
        show_translation_error(&app, error);
        return;
    }

    // 读取配置以决定 overlay 创建策略
    let config = match state.config_store.get() {
        Ok(c) => c,
        Err(e) => {
            let _ = state.take_pending_capture();
            let _ = state.finish_capture();
            show_translation_error(&app, format!("读取配置失败: {e}"));
            return;
        }
    };

    // overlay 自身承载交互，不需要主窗口可见。成功打开后保留 capture 锁，等 submit/cancel 释放。
    if let Err(error) = overlay::open_overlay(&app, &config) {
        let _ = state.take_pending_capture();
        let _ = state.finish_capture();
        show_translation_error(&app, format!("无法打开截图窗口：{error}"));
    }
}

/// 翻译弹窗「截图翻译」按钮入口：先隐藏弹窗避免被抓进截图帧，再复用截图翻译链路。
/// 框选完成后 submit_capture_region 内部 show_translation_popup 会重新 show 并定位弹窗。
#[tauri::command]
pub async fn trigger_ocr_translation(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    if let Some(popup) = app.get_webview_window(popup_window::POPUP_LABEL) {
        let _ = popup.hide();
    }
    start_translation_from_ocr(app, state.inner().clone()).await;
    Ok(())
}

pub fn friendly_ocr_error(error: OcrTranslationError) -> String {
    match error {
        OcrTranslationError::Capture(CaptureError::UnsupportedPlatform) => {
            "截图失败：当前平台暂不支持截图 OCR。".to_string()
        }
        OcrTranslationError::Capture(CaptureError::NoCaptureTarget) => {
            "截图失败：未选择截图区域或窗口。".to_string()
        }
        OcrTranslationError::Capture(CaptureError::PermissionDenied) => {
            "截图失败：无法访问屏幕捕获权限。".to_string()
        }
        OcrTranslationError::Capture(CaptureError::BackendUnavailable(detail)) => {
            format!("截图失败，请稍后重试（{detail}）")
        }
        OcrTranslationError::Capture(CaptureError::ImageConversionFailed(detail)) => {
            format!("截图失败：图像转换失败（{detail}）")
        }
        OcrTranslationError::Ocr(OcrError::EngineUnavailable) => {
            "OCR 识别失败：系统 OCR 能力不可用。".to_string()
        }
        OcrTranslationError::Ocr(OcrError::LanguageUnavailable(_)) => {
            "OCR 识别失败：缺少 OCR 语言包。请在「Windows 设置 > 时间和语言 > 语言」安装对应 OCR 语言包后重试。".to_string()
        }
        OcrTranslationError::Ocr(OcrError::ImageTooLarge) => {
            "OCR 识别失败：截图区域过大，请缩小区域后重新按 Alt+S 截图。".to_string()
        }
        OcrTranslationError::Ocr(OcrError::EmptyResult) => {
            "OCR 识别失败：未识别到文本。请重新按 Alt+S 框选更清晰的区域。".to_string()
        }
        OcrTranslationError::Ocr(OcrError::ImageConversionFailed(_)) => {
            "OCR 识别失败：图像转换失败，请重新截图。".to_string()
        }
        OcrTranslationError::Ocr(OcrError::UnsupportedPlatform) => {
            "OCR 识别失败：当前平台暂不支持截图 OCR。".to_string()
        }
        OcrTranslationError::Ocr(OcrError::NoEngineConfigured) => {
            "OCR 识别失败：没有可用的文字识别服务。请在「设置 → 服务 → 文字识别」启用一项。".to_string()
        }
        OcrTranslationError::Ocr(OcrError::UnsupportedProtocol(ref p)) => {
            format!("OCR 识别失败：当前版本不支持该识别协议（{p}）。请改用 Windows 媒体 OCR 或 OpenAI 兼容视觉。")
        }
        OcrTranslationError::Ocr(OcrError::Auth(ref d)) => {
            format!("OCR 识别失败：认证失败（{d}）。请在「设置 → 文字识别」检查 API Key。")
        }
        OcrTranslationError::Ocr(OcrError::Api { ref message, .. }) => {
            format!(
                "OCR 识别失败：{message}。请确认「设置 → 服务 → 文字识别」当前启用的引擎与模型可用。"
            )
        }
        OcrTranslationError::Ocr(OcrError::Http(ref d)) => {
            format!("OCR 识别失败：网络错误（{d}）。请确认当前启用的文字识别服务配置正确。")
        }
        OcrTranslationError::Ocr(OcrError::UnknownService(_)) => {
            "OCR 识别失败：渠道已不存在，请重新选择。".to_string()
        }
        OcrTranslationError::Ocr(OcrError::PdfOpenFailed(_)) => {
            "OCR 识别失败：无法打开 PDF 文件。".to_string()
        }
        OcrTranslationError::Ocr(OcrError::PdfEmptyDocument) => {
            "OCR 识别失败：PDF 中没有可识别的页面。".to_string()
        }
        OcrTranslationError::Ocr(OcrError::PdfRenderFailed(_)) => {
            "OCR 识别失败：PDF 页面渲染失败。".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{capture::CaptureError, ocr::OcrError, ocr_translation::OcrTranslationError};

    #[test]
    fn friendly_error_maps_empty_result() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Ocr(OcrError::EmptyResult)),
            "OCR 识别失败：未识别到文本。请重新按 Alt+S 框选更清晰的区域。"
        );
    }

    #[test]
    fn friendly_error_maps_language_unavailable() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Ocr(OcrError::LanguageUnavailable(
                "zh-Hans-CN".to_string()
            ))),
            "OCR 识别失败：缺少 OCR 语言包。请在「Windows 设置 > 时间和语言 > 语言」安装对应 OCR 语言包后重试。"
        );
    }

    #[test]
    fn friendly_error_maps_image_too_large() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Ocr(OcrError::ImageTooLarge)),
            "OCR 识别失败：截图区域过大，请缩小区域后重新按 Alt+S 截图。"
        );
    }

    #[test]
    fn friendly_error_maps_engine_unavailable() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Ocr(OcrError::EngineUnavailable)),
            "OCR 识别失败：系统 OCR 能力不可用。"
        );
    }

    #[test]
    fn friendly_error_maps_ocr_image_conversion_failed() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Ocr(OcrError::ImageConversionFailed(
                "boom".to_string()
            ))),
            "OCR 识别失败：图像转换失败，请重新截图。"
        );
    }

    #[test]
    fn friendly_error_maps_ocr_unsupported_platform() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Ocr(OcrError::UnsupportedPlatform)),
            "OCR 识别失败：当前平台暂不支持截图 OCR。"
        );
    }

    #[test]
    fn friendly_error_maps_no_engine_configured() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Ocr(OcrError::NoEngineConfigured)),
            "OCR 识别失败：没有可用的文字识别服务。请在「设置 → 服务 → 文字识别」启用一项。"
        );
    }

    #[test]
    fn friendly_error_maps_unsupported_protocol() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Ocr(OcrError::UnsupportedProtocol(
                "claude-vision".to_string()
            ))),
            "OCR 识别失败：当前版本不支持该识别协议（claude-vision）。请改用 Windows 媒体 OCR 或 OpenAI 兼容视觉。"
        );
    }

    #[test]
    fn friendly_error_maps_auth() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Ocr(OcrError::Auth(
                "missing key".to_string()
            ))),
            "OCR 识别失败：认证失败（missing key）。请在「设置 → 文字识别」检查 API Key。"
        );
    }

    #[test]
    fn friendly_error_maps_api() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Ocr(OcrError::Api {
                message: "rate limit".to_string(),
                retryable: true,
            })),
            "OCR 识别失败：rate limit。请确认「设置 → 服务 → 文字识别」当前启用的引擎与模型可用。"
        );
    }

    #[test]
    fn friendly_error_maps_http() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Ocr(OcrError::Http(
                "timeout".to_string()
            ))),
            "OCR 识别失败：网络错误（timeout）。请确认当前启用的文字识别服务配置正确。"
        );
    }

    #[test]
    fn friendly_error_maps_capture_unsupported_platform() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Capture(
                CaptureError::UnsupportedPlatform
            )),
            "截图失败：当前平台暂不支持截图 OCR。"
        );
    }

    #[test]
    fn friendly_error_maps_capture_no_target() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Capture(CaptureError::NoCaptureTarget)),
            "截图失败：未选择截图区域或窗口。"
        );
    }

    #[test]
    fn friendly_error_maps_capture_permission_denied() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Capture(
                CaptureError::PermissionDenied
            )),
            "截图失败：无法访问屏幕捕获权限。"
        );
    }

    #[test]
    fn friendly_error_maps_backend_unavailable() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Capture(
                CaptureError::BackendUnavailable("boom".to_string())
            )),
            "截图失败，请稍后重试（boom）"
        );
    }

    #[test]
    fn friendly_error_maps_capture_image_conversion_failed() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Capture(
                CaptureError::ImageConversionFailed("boom".to_string())
            )),
            "截图失败：图像转换失败（boom）"
        );
    }

    #[test]
    fn friendly_unknown_service_and_pdf_errors() {
        assert!(
            friendly_ocr_error(OcrTranslationError::Ocr(OcrError::UnknownService(
                "abc".into()
            )))
            .contains("渠道")
        );
        assert!(
            friendly_ocr_error(OcrTranslationError::Ocr(OcrError::PdfOpenFailed(
                "x".into()
            )))
            .contains("PDF")
        );
        assert!(
            friendly_ocr_error(OcrTranslationError::Ocr(OcrError::PdfEmptyDocument))
                .contains("页")
        );
        assert!(
            friendly_ocr_error(OcrTranslationError::Ocr(OcrError::PdfRenderFailed(
                "x".into()
            )))
            .contains("渲染")
        );
    }
}
