use crate::{
    app::state::AppState,
    core::{capture::CaptureError, ocr::OcrError, ocr_translation::OcrTranslationError},
    platform::capture_and_recognize,
    ui::web_popup::{show_translation_error, start_translation_from_input},
};

use crate::{app::window::show_window, core::ocr::OcrHints};
use tauri::Manager;

pub async fn start_translation_from_ocr(app: tauri::AppHandle, state: AppState) {
    // ponytail: OCR 阶段不持有 translation_busy；picker 模态天然串行，
    // 翻译阶段仍由 start_translation_from_input 内部 try_begin_translation 保护。
    // busy peek 与 OCR→翻译间存在微小竞态窗口，MVP 可接受；后续可让 OCR 入口占住 busy。
    if state.is_translation_busy() {
        show_translation_error(&app, "正在翻译中，请稍后再试");
        return;
    }

    // GraphicsCapturePicker 在桌面应用中需要可见 owner window handle，否则 picker 可能不显示。
    show_window(&app);

    // GraphicsCapturePicker 在桌面应用中需要 owner window handle，否则 PickSingleItemAsync 失败。
    let owner_hwnd = app
        .get_webview_window("main")
        .and_then(|window| window.hwnd().ok())
        .map(|hwnd| hwnd.0 as isize)
        .unwrap_or(0);

    match capture_and_recognize(OcrHints::default(), owner_hwnd).await {
        Ok(None) => {} // 用户取消截图，静默
        Ok(Some(input)) => {
            if let Err(error) = start_translation_from_input(input, app.clone(), &state) {
                show_translation_error(&app, error);
            }
        }
        Err(error) => show_translation_error(&app, friendly_ocr_error(error)),
    }
}

fn friendly_ocr_error(error: OcrTranslationError) -> String {
    match error {
        OcrTranslationError::Capture(CaptureError::UnsupportedPlatform) => {
            "当前平台暂不支持截图 OCR".to_string()
        }
        OcrTranslationError::Capture(CaptureError::NoCaptureTarget) => {
            "未选择截图区域或窗口".to_string()
        }
        OcrTranslationError::Capture(CaptureError::PermissionDenied) => {
            "无法访问屏幕捕获权限".to_string()
        }
        OcrTranslationError::Capture(CaptureError::BackendUnavailable(detail)) => {
            format!("截图失败，请稍后重试（{detail}）")
        }
        OcrTranslationError::Capture(CaptureError::ImageConversionFailed(detail)) => {
            format!("截图图像转换失败（{detail}）")
        }
        OcrTranslationError::Ocr(OcrError::EngineUnavailable) => "系统 OCR 能力不可用".to_string(),
        OcrTranslationError::Ocr(OcrError::LanguageUnavailable(_)) => "缺少 OCR 语言包".to_string(),
        OcrTranslationError::Ocr(OcrError::ImageTooLarge) => "截图区域过大，请缩小区域".to_string(),
        OcrTranslationError::Ocr(OcrError::EmptyResult) => "未识别到文本".to_string(),
        OcrTranslationError::Ocr(OcrError::ImageConversionFailed(_)) => {
            "OCR 图像转换失败".to_string()
        }
        OcrTranslationError::Ocr(OcrError::UnsupportedPlatform) => {
            "当前平台暂不支持截图 OCR".to_string()
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
            "未识别到文本"
        );
    }

    #[test]
    fn friendly_error_maps_unsupported_platform() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Capture(
                CaptureError::UnsupportedPlatform
            )),
            "当前平台暂不支持截图 OCR"
        );
    }

    #[test]
    fn friendly_error_maps_language_unavailable() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Ocr(OcrError::LanguageUnavailable(
                "zh-Hans-CN".to_string()
            ))),
            "缺少 OCR 语言包"
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
}
