pub mod capture;
pub mod ocr;
pub mod cursor;

use crate::core::{
    capture::{CaptureError, CapturedImage},
    ocr::{
        resolve::{resolve_ocr_engine, ResolvedOcrEngine},
        vision_openai::VisionOcrEngine,
        OcrHints,
    },
    ocr_translation::{recognize_cropped_for_translation, OcrTranslationError},
    translation::TranslationInput,
};
use ocr::WindowsOcrEngine;

/// 抓光标所在显示器整屏帧 + 该显示器 scale_factor。
pub async fn capture_screen() -> Result<CapturedImage, CaptureError> {
    capture::WindowsScreenCapture::new().capture_monitor().await
}

/// 返回光标所在显示器工作区逻辑像素。
pub use cursor::cursor_logical_context;

/// 对已抓帧按物理像素矩形裁剪并 OCR。
/// 按 `ocr_services` 解析引擎；视觉失败不回退 Windows。
pub async fn recognize_region(
    frame: &CapturedImage,
    region: (u32, u32, u32, u32),
    hints: OcrHints,
    ocr_services: &[crate::core::config::types::OcrServiceInstanceConfig],
) -> Result<Option<TranslationInput>, OcrTranslationError> {
    let resolved = resolve_ocr_engine(ocr_services)?;
    match resolved {
        ResolvedOcrEngine::WindowsMedia => {
            recognize_cropped_for_translation(frame, region, &WindowsOcrEngine, hints).await
        }
        ResolvedOcrEngine::VisionOpenAiCompatible(cfg) => {
            let engine = VisionOcrEngine::new(cfg).map_err(OcrTranslationError::from)?;
            recognize_cropped_for_translation(frame, region, &engine, hints).await
        }
    }
}
