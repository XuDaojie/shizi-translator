use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{imageops::FilterType, ImageBuffer, ImageFormat, RgbaImage};

use crate::core::capture::{CapturedImage, CapturedImageFormat};

use super::OcrError;

/// 视觉模型输入图像最长边上限（像素）。
pub const VISION_MAX_LONG_EDGE: u32 = 2048;

/// 将截图编码为 PNG 字节；最长边 > 2048 时等比缩小。
pub fn encode_captured_image_png(image: &CapturedImage) -> Result<Vec<u8>, OcrError> {
    let rgba = captured_to_rgba(image)?;
    let (src_w, src_h) = rgba.dimensions();
    let rgba = maybe_scale(rgba);
    let (out_w, out_h) = rgba.dimensions();

    let mut png = Vec::new();
    {
        let mut cursor = std::io::Cursor::new(&mut png);
        rgba.write_to(&mut cursor, ImageFormat::Png)
            .map_err(|e| OcrError::ImageConversionFailed(e.to_string()))?;
    }

    log::debug!(
        "encode_captured_image_png: {}x{} format={:?} -> {}x{} png_len={}",
        src_w,
        src_h,
        image.format,
        out_w,
        out_h,
        png.len()
    );

    Ok(png)
}

/// `data:image/png;base64,...`
pub fn png_to_data_url(png: &[u8]) -> String {
    format!("data:image/png;base64,{}", STANDARD.encode(png))
}

fn captured_to_rgba(image: &CapturedImage) -> Result<RgbaImage, OcrError> {
    match image.format {
        CapturedImageFormat::Bgra8 => {
            validate_raw_len(image)?;
            let rgba_bytes = bgra_to_rgba(&image.bytes);
            ImageBuffer::from_raw(image.width, image.height, rgba_bytes).ok_or_else(|| {
                OcrError::ImageConversionFailed("无法从 BGRA 构建 RgbaImage".to_string())
            })
        }
        CapturedImageFormat::Rgba8 => {
            validate_raw_len(image)?;
            ImageBuffer::from_raw(image.width, image.height, image.bytes.clone()).ok_or_else(
                || OcrError::ImageConversionFailed("无法从 RGBA 构建 RgbaImage".to_string()),
            )
        }
        CapturedImageFormat::Png => {
            let dyn_img = image::load_from_memory(&image.bytes)
                .map_err(|e| OcrError::ImageConversionFailed(e.to_string()))?;
            Ok(dyn_img.to_rgba8())
        }
    }
}

fn validate_raw_len(image: &CapturedImage) -> Result<(), OcrError> {
    let expected = (image.width as usize)
        .checked_mul(image.height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| OcrError::ImageConversionFailed("图片尺寸溢出".to_string()))?;
    if image.bytes.len() < expected {
        return Err(OcrError::ImageConversionFailed(format!(
            "缓冲区与声明尺寸不匹配：期望至少 {} 字节，实际 {}",
            expected,
            image.bytes.len()
        )));
    }
    Ok(())
}

fn bgra_to_rgba(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for chunk in bytes.chunks_exact(4) {
        out.push(chunk[2]); // R
        out.push(chunk[1]); // G
        out.push(chunk[0]); // B
        out.push(chunk[3]); // A
    }
    out
}

fn maybe_scale(rgba: RgbaImage) -> RgbaImage {
    let (w, h) = rgba.dimensions();
    let long = w.max(h);
    if long <= VISION_MAX_LONG_EDGE {
        return rgba;
    }
    let new_w = ((w as f64) * (VISION_MAX_LONG_EDGE as f64) / (long as f64)).round() as u32;
    let new_h = ((h as f64) * (VISION_MAX_LONG_EDGE as f64) / (long as f64)).round() as u32;
    let new_w = new_w.max(1);
    let new_h = new_h.max(1);
    image::imageops::resize(&rgba, new_w, new_h, FilterType::Triangle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::capture::{CapturedImage, CapturedImageFormat};

    #[test]
    fn encodes_bgra_1x1_to_valid_png() {
        let img = CapturedImage {
            bytes: vec![0, 0, 255, 255], // B,G,R,A → 红
            width: 1,
            height: 1,
            format: CapturedImageFormat::Bgra8,
        };
        let png = encode_captured_image_png(&img).expect("png");
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
        let url = png_to_data_url(&png);
        assert!(url.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn scales_down_when_long_edge_exceeds_2048() {
        let w = 3000u32;
        let h = 10u32;
        let img = CapturedImage {
            bytes: vec![0u8; (w * h * 4) as usize],
            width: w,
            height: h,
            format: CapturedImageFormat::Rgba8,
        };
        let png = encode_captured_image_png(&img).unwrap();
        let decoded = image::load_from_memory(&png).unwrap();
        assert!(decoded.width() <= VISION_MAX_LONG_EDGE);
        assert!(decoded.height() <= VISION_MAX_LONG_EDGE);
    }
}
