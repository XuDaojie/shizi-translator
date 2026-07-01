use crate::core::{
    capture::{CaptureError, CapturedImage},
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

/// overlay 路径：对已抓到的整屏帧按物理像素矩形裁剪后 OCR，转成翻译输入。
/// overlay 路径：对已抓到的整屏帧按物理像素矩形裁剪后 OCR，转成翻译输入。
///
/// `region` 单位为物理像素，调用方需先通过 [`crate::core::capture::css_rect_to_physical`]
/// 把 overlay 前端回传的 CSS 逻辑像素矩形按 `scale_factor` 换算后再传入。
/// 签名上返回 `Option` 以对齐取消语义，但本函数永不返回 `Ok(None)`
/// ——空文本走 `Err(OcrError::EmptyResult)`，非空走 `Ok(Some(_))`。
pub async fn recognize_cropped_for_translation<O>(
    frame: &CapturedImage,
    region: (u32, u32, u32, u32),
    ocr: &O,
    hints: OcrHints,
) -> Result<Option<TranslationInput>, OcrTranslationError>
where
    O: OcrEngine,
{
    let (x, y, w, h) = region;
    let cropped = frame.crop(x, y, w, h)?;
    let result = ocr.recognize(cropped, hints).await?;
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
        capture::{CapturedImage, CapturedImageFormat},
        ocr::{OcrEngine, OcrHints, OcrResult},
    };

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

    fn bgra_4x4() -> CapturedImage {
        CapturedImage {
            bytes: vec![128; 4 * 4 * 4],
            width: 4,
            height: 4,
            format: CapturedImageFormat::Bgra8,
        }
    }

    #[tokio::test]
    async fn cropped_workflow_returns_ocr_input() {
        let frame = bgra_4x4();
        let input = recognize_cropped_for_translation(
            &frame,
            (1, 1, 2, 2),
            &FakeOcr {
                text: " Hi ".to_string(),
            },
            OcrHints::default(),
        )
        .await
        .expect("裁剪 OCR workflow 应成功")
        .expect("应返回 OCR 输入");

        assert_eq!(input.text(), "Hi");
    }

    #[tokio::test]
    async fn cropped_workflow_rejects_empty_text() {
        let frame = bgra_4x4();
        let error = recognize_cropped_for_translation(
            &frame,
            (0, 0, 2, 2),
            &FakeOcr {
                text: "   ".to_string(),
            },
            OcrHints::default(),
        )
        .await
        .expect_err("空文本应报错");

        assert!(matches!(
            error,
            OcrTranslationError::Ocr(crate::core::ocr::OcrError::EmptyResult)
        ));
    }

    #[tokio::test]
    async fn cropped_workflow_propagates_crop_error() {
        let frame = bgra_4x4();
        let error = recognize_cropped_for_translation(
            &frame,
            (3, 3, 5, 5),
            &FakeOcr {
                text: "x".to_string(),
            },
            OcrHints::default(),
        )
        .await
        .expect_err("越界裁剪应报错");

        assert!(matches!(
            error,
            OcrTranslationError::Capture(crate::core::capture::CaptureError::ImageConversionFailed(_))
        ));
    }
}
