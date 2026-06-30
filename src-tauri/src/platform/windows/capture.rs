use crate::core::capture::{CapturedImage, CaptureError};
use windows::core::Interface;
use windows::Foundation::IAsyncOperation;
use windows::Graphics::Capture::{GraphicsCaptureItem, GraphicsCapturePicker};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::System::WinRT::Direct3D11::CreateDirect3D11DeviceFromDXGIDevice;

pub struct WindowsScreenCapture;

impl WindowsScreenCapture {
    pub fn is_supported() -> bool {
        windows::Graphics::Capture::GraphicsCaptureSession::IsSupported().unwrap_or(false)
    }

    pub(crate) fn create_direct3d_device() -> Result<IDirect3DDevice, CaptureError> {
        let mut d3d_device: Option<ID3D11Device> = None;

        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                None,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                7,
                Some(&mut d3d_device),
                None,
                None,
            )
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;
        }

        let d3d_device = d3d_device
            .ok_or_else(|| CaptureError::BackendUnavailable("D3D11 设备为空".to_string()))?;
        let dxgi_device: IDXGIDevice = d3d_device
            .cast()
            .map_err(|error| CaptureError::ImageConversionFailed(error.to_string()))?;

        let inspectable = unsafe {
            CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device)
                .map_err(|error| CaptureError::ImageConversionFailed(error.to_string()))?
        };

        inspectable
            .cast()
            .map_err(|error| CaptureError::ImageConversionFailed(error.to_string()))
    }

    pub(crate) async fn pick_capture_item() -> Result<Option<GraphicsCaptureItem>, CaptureError> {
        let picker = GraphicsCapturePicker::new()
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;
        let operation: IAsyncOperation<GraphicsCaptureItem> = picker
            .PickSingleItemAsync()
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;
        let item = operation
            .get()
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;
        Ok(Some(item))
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

    #[test]
    fn create_direct3d_device_returns_device_or_error() {
        let _ = WindowsScreenCapture::create_direct3d_device();
    }

    #[tokio::test]
    #[ignore]
    async fn pick_capture_item_can_be_invoked() {
        let _ = WindowsScreenCapture::pick_capture_item().await;
    }
}
