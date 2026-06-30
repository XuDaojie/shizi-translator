use crate::core::{
    capture::CaptureError,
    ocr::OcrHints,
    ocr_translation::OcrTranslationError,
    translation::TranslationInput,
};

pub struct GraphicsCaptureProbe;

impl GraphicsCaptureProbe {
    pub fn is_supported() -> bool {
        false
    }
}

pub async fn capture_and_recognize(
    _hints: OcrHints,
) -> Result<Option<TranslationInput>, OcrTranslationError> {
    Err(OcrTranslationError::Capture(CaptureError::UnsupportedPlatform))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        capture::CaptureError,
        ocr::OcrHints,
        ocr_translation::OcrTranslationError,
    };

    #[tokio::test]
    async fn capture_and_recognize_unsupported_on_non_windows() {
        let error = capture_and_recognize(OcrHints::default())
            .await
            .expect_err("非 windows 平台应返回错误");

        assert!(matches!(
            error,
            OcrTranslationError::Capture(CaptureError::UnsupportedPlatform)
        ));
    }
}
