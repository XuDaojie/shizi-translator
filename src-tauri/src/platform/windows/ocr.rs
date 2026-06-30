use crate::core::{
    capture::{CapturedImage, CapturedImageFormat},
    ocr::{OcrBoundingBox, OcrEngine, OcrError, OcrHints, OcrLine, OcrResult, OcrWord},
};
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Storage::Streams::DataWriter;

pub struct WindowsOcrEngine;

impl WindowsOcrEngine {
    pub fn is_available() -> bool {
        windows::Media::Ocr::OcrEngine::TryCreateFromUserProfileLanguages().is_ok()
    }
}

fn validate_image_dimensions(width: u32, height: u32, max_dimension: u32) -> Result<(), OcrError> {
    if width > max_dimension || height > max_dimension {
        return Err(OcrError::ImageTooLarge);
    }
    Ok(())
}

pub fn validate_raw_image(image: &CapturedImage) -> Result<(), OcrError> {
    match image.format {
        CapturedImageFormat::Png => Err(OcrError::ImageConversionFailed(
            "暂不支持 PNG OCR 输入".to_string(),
        )),
        CapturedImageFormat::Rgba8 | CapturedImageFormat::Bgra8 => {
            let expected_len = image
                .width
                .checked_mul(image.height)
                .and_then(|pixels| pixels.checked_mul(4))
                .map(|bytes| bytes as usize)
                .ok_or_else(|| OcrError::ImageConversionFailed("图片尺寸溢出".to_string()))?;

            if image.bytes.len() != expected_len {
                return Err(OcrError::ImageConversionFailed(format!(
                    "图片字节长度不匹配：期望 {expected_len}，实际 {}",
                    image.bytes.len()
                )));
            }

            Ok(())
        }
    }
}

#[async_trait::async_trait]
impl OcrEngine for WindowsOcrEngine {
    async fn recognize(
        &self,
        image: CapturedImage,
        hints: OcrHints,
    ) -> Result<OcrResult, OcrError> {
        validate_raw_image(&image)?;
        validate_image_dimensions(
            image.width,
            image.height,
            windows::Media::Ocr::OcrEngine::MaxImageDimension()
                .map_err(|_| OcrError::EngineUnavailable)?,
        )?;

        let engine = create_engine(hints)?;
        let bitmap = captured_image_to_software_bitmap(image)?;
        let result = engine
            .RecognizeAsync(&bitmap)
            .map_err(|error| OcrError::ImageConversionFailed(error.to_string()))?
            .get()
            .map_err(|error| OcrError::ImageConversionFailed(error.to_string()))?;

        convert_result(result)
    }
}

fn create_engine(hints: OcrHints) -> Result<windows::Media::Ocr::OcrEngine, OcrError> {
    if hints.preferred_languages.is_empty() {
        return windows::Media::Ocr::OcrEngine::TryCreateFromUserProfileLanguages()
            .map_err(|_| OcrError::EngineUnavailable);
    }

    let mut last_unavailable = None;
    for language in hints.preferred_languages {
        match windows::Globalization::Language::CreateLanguage(&language.clone().into()) {
            Ok(language_obj) => {
                match windows::Media::Ocr::OcrEngine::IsLanguageSupported(&language_obj) {
                    Ok(true) => {
                        return windows::Media::Ocr::OcrEngine::TryCreateFromLanguage(
                            &language_obj,
                        )
                        .map_err(|_| OcrError::EngineUnavailable);
                    }
                    _ => {
                        last_unavailable = Some(
                            language_obj
                                .LanguageTag()
                                .map(|s| s.to_string())
                                .unwrap_or_default(),
                        )
                    }
                }
            }
            Err(_) => last_unavailable = Some(language),
        }
    }

    Err(OcrError::LanguageUnavailable(
        last_unavailable.unwrap_or_default(),
    ))
}

fn captured_image_to_software_bitmap(image: CapturedImage) -> Result<SoftwareBitmap, OcrError> {
    let bgra_bytes = match image.format {
        CapturedImageFormat::Bgra8 => image.bytes,
        CapturedImageFormat::Rgba8 => image
            .bytes
            .chunks_exact(4)
            .flat_map(|px| [px[2], px[1], px[0], px[3]])
            .collect(),
        CapturedImageFormat::Png => {
            return Err(OcrError::ImageConversionFailed(
                "暂不支持 PNG OCR 输入".to_string(),
            ))
        }
    };

    let writer =
        DataWriter::new().map_err(|error| OcrError::ImageConversionFailed(error.to_string()))?;
    writer
        .WriteBytes(&bgra_bytes)
        .map_err(|error| OcrError::ImageConversionFailed(error.to_string()))?;
    let buffer = writer
        .DetachBuffer()
        .map_err(|error| OcrError::ImageConversionFailed(error.to_string()))?;

    SoftwareBitmap::CreateCopyFromBuffer(
        &buffer,
        BitmapPixelFormat::Bgra8,
        image.width as i32,
        image.height as i32,
    )
    .map_err(|error| OcrError::ImageConversionFailed(error.to_string()))
}

fn convert_result(result: windows::Media::Ocr::OcrResult) -> Result<OcrResult, OcrError> {
    let text = result
        .Text()
        .map_err(|error| OcrError::ImageConversionFailed(error.to_string()))?
        .to_string();

    if text.trim().is_empty() {
        return Err(OcrError::EmptyResult);
    }

    let mut lines = Vec::new();
    for line in result
        .Lines()
        .map_err(|error| OcrError::ImageConversionFailed(error.to_string()))?
    {
        let line_text = line
            .Text()
            .map_err(|error| OcrError::ImageConversionFailed(error.to_string()))?
            .to_string();
        let mut words = Vec::new();

        for word in line
            .Words()
            .map_err(|error| OcrError::ImageConversionFailed(error.to_string()))?
        {
            let rect = word
                .BoundingRect()
                .map_err(|error| OcrError::ImageConversionFailed(error.to_string()))?;
            words.push(OcrWord {
                text: word
                    .Text()
                    .map_err(|error| OcrError::ImageConversionFailed(error.to_string()))?
                    .to_string(),
                bounding_box: OcrBoundingBox {
                    x: rect.X,
                    y: rect.Y,
                    width: rect.Width,
                    height: rect.Height,
                },
            });
        }

        lines.push(OcrLine {
            text: line_text,
            words,
        });
    }

    Ok(OcrResult {
        text: text.trim().to_string(),
        lines,
        engine: "windows-media-ocr".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        capture::{CapturedImage, CapturedImageFormat},
        ocr::OcrError,
    };

    fn image(
        format: CapturedImageFormat,
        bytes: Vec<u8>,
        width: u32,
        height: u32,
    ) -> CapturedImage {
        CapturedImage {
            bytes,
            width,
            height,
            format,
        }
    }

    #[test]
    fn validate_raw_image_rejects_png_input() {
        let error = validate_raw_image(&image(CapturedImageFormat::Png, vec![], 1, 1))
            .expect_err("PNG 在本切片中不支持");

        assert!(matches!(error, OcrError::ImageConversionFailed(_)));
    }

    #[test]
    fn validate_raw_image_rejects_mismatched_rgba_buffer_len() {
        let error = validate_raw_image(&image(CapturedImageFormat::Rgba8, vec![0, 1, 2], 1, 1))
            .expect_err("RGBA 字节长度必须匹配 width * height * 4");

        assert!(matches!(error, OcrError::ImageConversionFailed(_)));
    }

    #[test]
    fn validate_raw_image_accepts_matching_rgba_buffer_len() {
        validate_raw_image(&image(CapturedImageFormat::Rgba8, vec![0, 1, 2, 3], 1, 1))
            .expect("RGBA 字节长度匹配时应通过校验");
    }
    #[test]
    fn rejects_image_larger_than_max_dimension() {
        let max = 10;
        let error = validate_image_dimensions(11, 1, max).expect_err("超过最大边长应失败");

        assert_eq!(error, OcrError::ImageTooLarge);
    }

    #[test]
    fn accepts_image_within_max_dimension() {
        validate_image_dimensions(10, 10, 10).expect("边长不超过限制应通过");
    }

    #[tokio::test]
    #[ignore]
    async fn windows_ocr_engine_can_be_called_with_generated_bitmap() {
        if !WindowsOcrEngine::is_available() {
            return;
        }

        let image = image(CapturedImageFormat::Bgra8, vec![255; 32 * 32 * 4], 32, 32);
        let result = WindowsOcrEngine.recognize(image, OcrHints::default()).await;

        assert!(matches!(result, Ok(_) | Err(OcrError::EmptyResult)));
    }
}
