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
}
