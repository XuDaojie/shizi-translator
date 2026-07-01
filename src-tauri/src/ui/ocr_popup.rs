use crate::{
    app::state::AppState,
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

    // capture 独立锁：挡住 OCR/recognize 期间二次 Alt+O 覆盖 pending_capture。
    // 持锁到 submit_capture_region / cancel_capture 释放；本函数每条失败路径都须 finish_capture。
    if let Err(message) = state.try_begin_capture() {
        show_translation_error(&app, message);
        return;
    }

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

pub fn friendly_ocr_error(error: OcrTranslationError) -> String {
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
