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
}
