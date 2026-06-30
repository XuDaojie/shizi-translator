use crate::core::{
    capture::{CaptureError, ScreenCapture},
    ocr::{OcrEngine, OcrError, OcrHints},
    translation::TranslationInput,
};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum OcrTranslationError {
    #[error(transparent)]
    Capture(#[from] CaptureError),
    #[error(transparent)]
    Ocr(#[from] OcrError),
}

pub async fn recognize_capture_for_translation<C, O>(
    capture: &C,
    ocr: &O,
    hints: OcrHints,
) -> Result<Option<TranslationInput>, OcrTranslationError>
where
    C: ScreenCapture,
    O: OcrEngine,
{
    let Some(image) = capture.capture_interactive().await? else {
        return Ok(None);
    };

    let result = ocr.recognize(image, hints).await?;
    let text = result.text.trim().to_string();
    if text.is_empty() {
        return Err(OcrError::EmptyResult.into());
    }

    Ok(Some(TranslationInput::OcrText {
        text,
        image_id: None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        capture::{CaptureError, CapturedImage, CapturedImageFormat, CaptureRegion, ScreenCapture},
        ocr::{OcrEngine, OcrHints, OcrResult},
    };

    struct FakeCapture {
        image: Option<CapturedImage>,
    }

    #[async_trait::async_trait]
    impl ScreenCapture for FakeCapture {
        async fn capture_region(&self, _region: CaptureRegion) -> Result<CapturedImage, CaptureError> {
            self.image
                .clone()
                .ok_or(CaptureError::NoCaptureTarget)
        }

        async fn capture_interactive(&self) -> Result<Option<CapturedImage>, CaptureError> {
            Ok(self.image.clone())
        }
    }

    struct FakeOcr {
        text: String,
    }

    #[async_trait::async_trait]
    impl OcrEngine for FakeOcr {
        async fn recognize(
            &self,
            _image: CapturedImage,
            _hints: OcrHints,
        ) -> Result<OcrResult, crate::core::ocr::OcrError> {
            Ok(OcrResult {
                text: self.text.clone(),
                lines: vec![],
                engine: "fake".to_string(),
            })
        }
    }

    fn image() -> CapturedImage {
        CapturedImage {
            bytes: vec![0, 1, 2, 3],
            width: 1,
            height: 1,
            format: CapturedImageFormat::Rgba8,
        }
    }

    #[tokio::test]
    async fn workflow_returns_ocr_translation_input() {
        let input = recognize_capture_for_translation(
            &FakeCapture { image: Some(image()) },
            &FakeOcr { text: " Hello ".to_string() },
            OcrHints::default(),
        )
        .await
        .expect("OCR workflow 应成功")
        .expect("应返回 OCR 输入");

        assert_eq!(input.text(), "Hello");
    }

    #[tokio::test]
    async fn workflow_returns_none_when_user_cancels_capture() {
        let input = recognize_capture_for_translation(
            &FakeCapture { image: None },
            &FakeOcr { text: "Hello".to_string() },
            OcrHints::default(),
        )
        .await
        .expect("用户取消不是错误");

        assert!(input.is_none());
    }

    #[tokio::test]
    async fn workflow_rejects_empty_ocr_text() {
        let error = recognize_capture_for_translation(
            &FakeCapture { image: Some(image()) },
            &FakeOcr { text: "  ".to_string() },
            OcrHints::default(),
        )
        .await
        .expect_err("空 OCR 文本应返回错误");

        assert!(matches!(error, OcrTranslationError::Ocr(crate::core::ocr::OcrError::EmptyResult)));
    }
}
