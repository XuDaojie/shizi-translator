#[derive(Debug, Clone, PartialEq)]
pub struct CaptureRegion {
    pub display_id: Option<String>,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CapturedImage {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub format: CapturedImageFormat,
}

impl CapturedImage {
    /// 按 BGRA 行切片裁剪。x/y/w/h 为物理像素，越界/非 BGRA/零尺寸返回 ImageConversionFailed。
    pub fn crop(&self, x: u32, y: u32, w: u32, h: u32) -> Result<CapturedImage, CaptureError> {
        if self.format != CapturedImageFormat::Bgra8 {
            return Err(CaptureError::ImageConversionFailed(
                "裁剪仅支持 BGRA8".to_string(),
            ));
        }
        if w == 0 || h == 0 {
            return Err(CaptureError::ImageConversionFailed(
                "裁剪尺寸必须为正数".to_string(),
            ));
        }
        let right = x.checked_add(w).ok_or_else(|| {
            CaptureError::ImageConversionFailed("裁剪横向范围溢出".to_string())
        })?;
        let bottom = y.checked_add(h).ok_or_else(|| {
            CaptureError::ImageConversionFailed("裁剪纵向范围溢出".to_string())
        })?;
        if right > self.width || bottom > self.height {
            return Err(CaptureError::ImageConversionFailed(
                "裁剪区域超出图像范围".to_string(),
            ));
        }

        let src_row_len = (self.width as usize) * 4;
        let dst_row_len = (w as usize) * 4;
        let mut bytes = Vec::with_capacity(dst_row_len * h as usize);
        for row in 0..h as usize {
            let src_y = y as usize + row;
            let row_start = src_y * src_row_len + (x as usize) * 4;
            bytes.extend_from_slice(&self.bytes[row_start..row_start + dst_row_len]);
        }

        Ok(CapturedImage {
            bytes,
            width: w,
            height: h,
            format: CapturedImageFormat::Bgra8,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapturedImageFormat {
    Bgra8,
    Rgba8,
    Png,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CaptureError {
    #[error("无法访问屏幕捕获权限")]
    PermissionDenied,
    #[error("未选择截图区域或窗口")]
    NoCaptureTarget,
    #[error("当前平台暂不支持截图 OCR")]
    UnsupportedPlatform,
    #[error("截图后端不可用：{0}")]
    BackendUnavailable(String),
    #[error("截图图像转换失败：{0}")]
    ImageConversionFailed(String),
}

#[async_trait::async_trait]
pub trait ScreenCapture: Send + Sync {
    async fn capture_region(&self, region: CaptureRegion) -> Result<CapturedImage, CaptureError>;

    async fn capture_interactive(&self) -> Result<Option<CapturedImage>, CaptureError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_image_reports_dimensions() {
        let image = CapturedImage {
            bytes: vec![0, 1, 2, 3],
            width: 1,
            height: 1,
            format: CapturedImageFormat::Rgba8,
        };

        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
        assert_eq!(image.bytes.len(), 4);
    }

    #[test]
    fn user_cancel_is_not_capture_error() {
        let result: Result<Option<CapturedImage>, CaptureError> = Ok(None);
        assert!(result.expect("用户取消不是错误").is_none());
    }

    fn bgra_image(width: u32, height: u32) -> CapturedImage {
        // 每像素 4 字节，值 = 行号*100 + 列号，便于断言定位
        let mut bytes = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                let v = (y * 100 + x) as u8;
                bytes.extend_from_slice(&[v, v, v, 255]);
            }
        }
        CapturedImage {
            bytes,
            width,
            height,
            format: CapturedImageFormat::Bgra8,
        }
    }

    #[test]
    fn crop_extracts_subregion_rows() {
        let image = bgra_image(4, 4);
        let cropped = image.crop(1, 1, 2, 2).expect("裁剪应成功");

        assert_eq!(cropped.width, 2);
        assert_eq!(cropped.height, 2);
        // (1,1)=101, (2,1)=102, (1,2)=201, (2,2)=202
        assert_eq!(cropped.bytes.len(), 2 * 2 * 4);
        assert_eq!(cropped.bytes[0], 101);
        assert_eq!(cropped.bytes[4], 102);
        assert_eq!(cropped.bytes[8], 201);
        assert_eq!(cropped.bytes[12], 202);
    }

    #[test]
    fn crop_rejects_out_of_bounds() {
        let image = bgra_image(4, 4);
        assert!(matches!(
            image.crop(3, 3, 2, 2),
            Err(CaptureError::ImageConversionFailed(_))
        ));
    }

    #[test]
    fn crop_rejects_zero_size() {
        let image = bgra_image(4, 4);
        assert!(matches!(
            image.crop(0, 0, 0, 2),
            Err(CaptureError::ImageConversionFailed(_))
        ));
    }

    #[test]
    fn crop_rejects_non_bgra_format() {
        let mut image = bgra_image(4, 4);
        image.format = CapturedImageFormat::Png;
        assert!(matches!(
            image.crop(0, 0, 2, 2),
            Err(CaptureError::ImageConversionFailed(_))
        ));
    }
}
