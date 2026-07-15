use crate::core::{
    capture::{CaptureError, CapturedImage},
    ocr::OcrHints,
    ocr_translation::OcrTranslationError,
    translation::TranslationInput,
};

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
}
