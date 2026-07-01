pub mod capture;
pub mod ocr;

use crate::core::{
    capture::{CaptureError, CapturedImage},
    ocr::OcrHints,
    ocr_translation::{recognize_cropped_for_translation, OcrTranslationError},
    translation::TranslationInput,
};
use ocr::WindowsOcrEngine;

/// 抓光标所在显示器整屏帧 + 该显示器 scale_factor。
pub async fn capture_screen() -> Result<CapturedImage, CaptureError> {
    capture::WindowsScreenCapture::new().capture_monitor().await
}

/// 对已抓帧按物理像素矩形裁剪并 OCR。
pub async fn recognize_region(
    frame: &CapturedImage,
    region: (u32, u32, u32, u32),
    hints: OcrHints,
) -> Result<Option<TranslationInput>, OcrTranslationError> {
    recognize_cropped_for_translation(frame, region, &WindowsOcrEngine, hints).await
}
