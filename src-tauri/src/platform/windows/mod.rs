pub mod capture;
pub mod ocr;
pub mod cursor;

use std::time::Instant;

use base64::{engine::general_purpose::STANDARD, Engine as _};

use crate::core::{
    capture::{CaptureError, CapturedImage},
    ocr::{
        image_encode::{encode_captured_image_png_info, encode_png_unscaled},
        meta::{OcrRunMeta, RecognizeImageFullResult, RecognizeImageResponse},
        resolve::{resolve_ocr_engine, ResolvedOcrEngine},
        vision_openai::VisionOcrEngine,
        OcrEngine, OcrError, OcrHints,
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
            log::info!("OCR 截图翻译引擎: windows-media-ocr");
            recognize_cropped_for_translation(frame, region, &WindowsOcrEngine, hints).await
        }
        ResolvedOcrEngine::VisionOpenAiCompatible(cfg) => {
            log::info!(
                "OCR 截图翻译引擎: {} model={}",
                cfg.service_type,
                cfg.model
            );
            let engine = VisionOcrEngine::new(cfg).map_err(OcrTranslationError::from)?;
            recognize_cropped_for_translation(frame, region, &engine, hints).await
        }
    }
}

/// 纯识别编排：OCR 正文 + 运行元信息 + 预览 PNG base64（不进入翻译链路）。
/// `model_hint` 保留签名兼容，实际 model 从 resolve 结果取。
/// 成功时携带源图拷贝，供 ui 层写 last_ocr_image（本函数不碰 AppState）。
pub async fn recognize_image_full(
    image: CapturedImage,
    hints: OcrHints,
    ocr_services: &[crate::core::config::types::OcrServiceInstanceConfig],
    _model_hint: Option<String>,
) -> Result<RecognizeImageFullResult, OcrError> {
    let start = Instant::now();
    let source_image = image.clone(); // 入口缓存用
    let source_width = image.width;
    let source_height = image.height;
    let preview_png = encode_png_unscaled(&image)?;
    let preview_b64 = STANDARD.encode(&preview_png);

    let resolved = resolve_ocr_engine(ocr_services)?;
    let (result, model, vision_encode) = match resolved {
        ResolvedOcrEngine::WindowsMedia => {
            log::info!("OCR 纯识别引擎: windows-media-ocr");
            let r = WindowsOcrEngine.recognize(image, hints).await?;
            (r, None, None)
        }
        ResolvedOcrEngine::VisionOpenAiCompatible(cfg) => {
            log::info!(
                "OCR 纯识别引擎: {} model={}",
                cfg.service_type,
                cfg.model
            );
            let model = Some(cfg.model.clone());
            // 与 vision 内部双 encode，可接受（plan 锁定）
            let enc = encode_captured_image_png_info(&image)?;
            let engine = VisionOcrEngine::new(cfg)?;
            let r = engine.recognize(image, hints).await?;
            (r, model, Some(enc))
        }
    };

    let text = result.text.trim().to_string();
    if text.is_empty() {
        return Err(OcrError::EmptyResult);
    }

    let (sent_w, sent_h, png_bytes, scaled) = if let Some(enc) = vision_encode.as_ref() {
        (
            enc.sent_width,
            enc.sent_height,
            Some(enc.png.len() as u64),
            enc.scaled,
        )
    } else {
        (source_width, source_height, None, false)
    };

    let meta = OcrRunMeta {
        engine: result.engine.clone(),
        model,
        source_width,
        source_height,
        sent_width: sent_w,
        sent_height: sent_h,
        png_bytes,
        latency_ms: start.elapsed().as_millis() as u64,
        http_status: if vision_encode.is_some() {
            Some(200)
        } else {
            None
        },
        scaled,
    };

    log::info!("OCR 纯识别: {}", meta.info_summary());
    log::info!(
        "OCR 纯识别文本: {}",
        crate::core::logging::redact_text(
            &text,
            crate::core::logging::effective_redact_level()
        )
    );
    // 禁止 log preview_b64 / API Key / source_image

    Ok(RecognizeImageFullResult {
        response: RecognizeImageResponse {
            text,
            meta,
            preview_png_base64: preview_b64,
        },
        source_image,
    })
}

/// 裁剪后纯识别（与 recognize_region 对称，不进入翻译）。
pub async fn recognize_cropped_full(
    frame: &CapturedImage,
    region: (u32, u32, u32, u32),
    hints: OcrHints,
    ocr_services: &[crate::core::config::types::OcrServiceInstanceConfig],
) -> Result<RecognizeImageFullResult, OcrTranslationError> {
    let cropped = frame.crop(region.0, region.1, region.2, region.3)?;
    log::debug!(
        "OCR 裁剪物理矩形: x={} y={} w={} h={}",
        region.0,
        region.1,
        region.2,
        region.3
    );
    recognize_image_full(cropped, hints, ocr_services, None)
        .await
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::capture::{CapturedImage, CapturedImageFormat};

    fn tiny_frame() -> CapturedImage {
        CapturedImage {
            bytes: vec![0; 4],
            width: 1,
            height: 1,
            format: CapturedImageFormat::Bgra8,
        }
    }

    #[tokio::test]
    async fn recognize_image_full_no_engine_configured() {
        let err = recognize_image_full(tiny_frame(), OcrHints::default(), &[], None)
            .await
            .expect_err("无 OCR 服务应失败");
        assert_eq!(err, OcrError::NoEngineConfigured);
    }

    #[tokio::test]
    async fn recognize_cropped_full_no_engine_configured() {
        let frame = tiny_frame();
        let err = recognize_cropped_full(&frame, (0, 0, 1, 1), OcrHints::default(), &[])
            .await
            .expect_err("无 OCR 服务应失败");
        assert!(matches!(
            err,
            OcrTranslationError::Ocr(OcrError::NoEngineConfigured)
        ));
    }
}
