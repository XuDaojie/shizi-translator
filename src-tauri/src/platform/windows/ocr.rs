use crate::core::{
    capture::{CapturedImage, CapturedImageFormat},
    ocr::OcrError,
};

pub struct WindowsOcrEngine;

impl WindowsOcrEngine {
    pub fn is_available() -> bool {
        windows::Media::Ocr::OcrEngine::TryCreateFromUserProfileLanguages().is_ok()
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        capture::{CapturedImage, CapturedImageFormat},
        ocr::OcrError,
    };

    fn image(format: CapturedImageFormat, bytes: Vec<u8>, width: u32, height: u32) -> CapturedImage {
        CapturedImage { bytes, width, height, format }
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
}
