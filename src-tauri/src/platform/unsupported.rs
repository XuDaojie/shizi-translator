use crate::core::{
    capture::{CaptureError, CapturedImage},
    ocr::{meta::RecognizeImageFullResult, OcrError, OcrHints},
    ocr_translation::OcrTranslationError,
    translation::TranslationInput,
};

/// PDF 首页渲染结果（非 Windows 平台占位类型，与 windows/pdf 对齐）。
#[derive(Debug, Clone)]
pub struct PdfFirstPage {
    pub image: CapturedImage,
    pub page_count: u32,
}

/// 非 Windows 平台暂不支持 PDF 识别。
pub async fn render_pdf_first_page(_bytes: &[u8]) -> Result<PdfFirstPage, OcrError> {
    Err(OcrError::PdfOpenFailed(
        "当前平台暂不支持 PDF 识别".into(),
    ))
}

pub async fn capture_screen() -> Result<CapturedImage, CaptureError> {
    Err(CaptureError::UnsupportedPlatform)
}

pub async fn recognize_region(
    _frame: &CapturedImage,
    _region: (u32, u32, u32, u32),
    _hints: OcrHints,
    _ocr_services: &[crate::core::config::types::OcrServiceInstanceConfig],
) -> Result<Option<TranslationInput>, OcrTranslationError> {
    Err(OcrTranslationError::Capture(CaptureError::UnsupportedPlatform))
}

/// `service_id`：OCR 窗临时渠道；`None` 时仅用配置中 enabled 引擎。
pub async fn recognize_image_full(
    _image: CapturedImage,
    _hints: OcrHints,
    _ocr_services: &[crate::core::config::types::OcrServiceInstanceConfig],
    _service_id: Option<String>,
) -> Result<RecognizeImageFullResult, OcrError> {
    Err(OcrError::UnsupportedPlatform)
}

/// `service_id`：OCR 窗临时渠道；`None` 时仅用配置中 enabled 引擎。
pub async fn recognize_cropped_full(
    _frame: &CapturedImage,
    _region: (u32, u32, u32, u32),
    _hints: OcrHints,
    _ocr_services: &[crate::core::config::types::OcrServiceInstanceConfig],
    _service_id: Option<String>,
) -> Result<RecognizeImageFullResult, OcrTranslationError> {
    Err(OcrTranslationError::Ocr(OcrError::UnsupportedPlatform))
}

/// 非 Windows 平台无法获取光标上下文，返回 `None`，调用方退化为不定位。
pub fn cursor_logical_context(_scale: f64) -> Option<(f64, f64, f64, f64, f64, f64)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        capture::{CapturedImage, CapturedImageFormat},
        ocr::OcrHints,
    };

    #[tokio::test]
    async fn capture_screen_unsupported_on_non_windows() {
        assert!(matches!(
            capture_screen().await,
            Err(CaptureError::UnsupportedPlatform)
        ));
    }

    #[tokio::test]
    async fn recognize_region_unsupported_on_non_windows() {
        let frame = CapturedImage {
            bytes: vec![0; 4],
            width: 1,
            height: 1,
            format: CapturedImageFormat::Bgra8,
        };
        let error = recognize_region(&frame, (0, 0, 1, 1), OcrHints::default(), &[])
            .await
            .expect_err("非 windows 平台应返回错误");
        assert!(matches!(
            error,
            OcrTranslationError::Capture(CaptureError::UnsupportedPlatform)
        ));
    }

    #[tokio::test]
    async fn recognize_image_full_unsupported_on_non_windows() {
        let image = CapturedImage {
            bytes: vec![0; 4],
            width: 1,
            height: 1,
            format: CapturedImageFormat::Bgra8,
        };
        let error = recognize_image_full(image, OcrHints::default(), &[], None)
            .await
            .expect_err("非 windows 平台应返回错误");
        assert_eq!(error, OcrError::UnsupportedPlatform);
    }

    #[tokio::test]
    async fn recognize_cropped_full_unsupported_on_non_windows() {
        let frame = CapturedImage {
            bytes: vec![0; 4],
            width: 1,
            height: 1,
            format: CapturedImageFormat::Bgra8,
        };
        let error = recognize_cropped_full(&frame, (0, 0, 1, 1), OcrHints::default(), &[], None)
            .await
            .expect_err("非 windows 平台应返回错误");
        assert!(matches!(
            error,
            OcrTranslationError::Ocr(OcrError::UnsupportedPlatform)
        ));
    }
}
