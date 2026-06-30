pub mod capture;
pub mod ocr;

use crate::core::{
    ocr::OcrHints,
    ocr_translation::{recognize_capture_for_translation, OcrTranslationError},
    translation::TranslationInput,
};
use capture::WindowsScreenCapture;
use ocr::WindowsOcrEngine;

pub async fn capture_and_recognize(
    hints: OcrHints,
) -> Result<Option<TranslationInput>, OcrTranslationError> {
    recognize_capture_for_translation(&WindowsScreenCapture, &WindowsOcrEngine, hints).await
}
