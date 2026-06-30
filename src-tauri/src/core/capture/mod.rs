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
        let expected = src_row_len
            .checked_mul(self.height as usize)
            .ok_or_else(|| CaptureError::ImageConversionFailed("缓冲区大小溢出".to_string()))?;
        if self.bytes.len() < expected {
            return Err(CaptureError::ImageConversionFailed(
                "缓冲区与声明尺寸不匹配".to_string(),
            ));
        }
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

/// 把 overlay 前端回传的 CSS 逻辑像素矩形按 scale_factor 换算为物理像素。
/// 返回 (x, y, w, h)，均向下取整。
pub fn css_rect_to_physical(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    scale_factor: f64,
) -> (u32, u32, u32, u32) {
    (
        (x * scale_factor) as u32,
        (y * scale_factor) as u32,
        (w * scale_factor) as u32,
        (h * scale_factor) as u32,
    )
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

    #[test]
    fn css_rect_scales_at_1x() {
        assert_eq!(css_rect_to_physical(10.0, 20.0, 30.0, 40.0, 1.0), (10, 20, 30, 40));
    }

    #[test]
    fn css_rect_scales_at_1_5x() {
        // 10*1.5=15, 20*1.5=30, 30*1.5=45, 40*1.5=60
        assert_eq!(css_rect_to_physical(10.0, 20.0, 30.0, 40.0, 1.5), (15, 30, 45, 60));
    }

    #[test]
    fn css_rect_scales_at_2x() {
        assert_eq!(css_rect_to_physical(5.0, 6.0, 7.0, 8.0, 2.0), (10, 12, 14, 16));
    }

    #[test]
    fn css_rect_floors_fractional_pixels() {
        // 3.3*1.0=3.3 -> 3；尺寸 floor 后若为 0 由调用方 crop 拒绝
        assert_eq!(css_rect_to_physical(3.3, 3.9, 1.6, 1.2, 1.0), (3, 3, 1, 1));
    }
}
