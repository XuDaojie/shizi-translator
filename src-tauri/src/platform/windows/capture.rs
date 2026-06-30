use crate::core::capture::{CapturedImage, CaptureError};

pub struct WindowsScreenCapture;

impl WindowsScreenCapture {
    pub fn is_supported() -> bool {
        windows::Graphics::Capture::GraphicsCaptureSession::IsSupported().unwrap_or(false)
    }

    pub async fn capture_full_screen(&self) -> Result<Option<CapturedImage>, CaptureError> {
        Err(CaptureError::UnsupportedPlatform)
    }
}

pub struct WindowsGraphicsCaptureProbe;

impl WindowsGraphicsCaptureProbe {
    pub fn is_supported() -> bool {
        windows::Graphics::Capture::GraphicsCaptureSession::IsSupported().unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphics_capture_probe_returns_boolean() {
        let _supported: bool = WindowsGraphicsCaptureProbe::is_supported();
    }

    #[test]
    fn screen_capture_is_supported_returns_boolean() {
        let _supported: bool = WindowsScreenCapture::is_supported();
    }
}
