use crate::core::capture::{CapturedImage, CaptureError};
use std::time::Duration;
use windows::core::Interface;
use windows::Foundation::IAsyncOperation;
use windows::Graphics::Capture::{
    Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCapturePicker,
};
use windows::Graphics::DirectX::Direct3D11::{IDirect3DDevice, IDirect3DSurface};
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::SizeInt32;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
};
use windows::Win32::Graphics::Dxgi::{IDXGIDevice, IDXGISurface, DXGI_MAPPED_RECT, DXGI_MAP_READ};
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
        let Some(item) = Self::pick_capture_item().await? else {
            return Ok(None);
        };

        let device = Self::create_direct3d_device()?;
        let size = item
            .Size()
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            size,
        )
        .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;
        let session = pool
            .CreateCaptureSession(&item)
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;
        session
            .StartCapture()
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;

        let mut frame = None;
        for _ in 0..20 {
            match pool.TryGetNextFrame() {
                Ok(next_frame) => {
                    frame = Some(next_frame);
                    break;
                }
                Err(_) => std::thread::sleep(Duration::from_millis(50)),
            }
        }
        let frame = frame.ok_or_else(|| {
            CaptureError::BackendUnavailable("未能在超时时间内获取首帧".to_string())
        })?;
        let image = Self::extract_bgra_from_frame(frame, size)?;

        let _ = session.Close();
        let _ = pool.Close();
        Ok(Some(image))
    }

    fn extract_bgra_from_frame(
        frame: Direct3D11CaptureFrame,
        size: SizeInt32,
    ) -> Result<CapturedImage, CaptureError> {
        let surface: IDirect3DSurface = frame
            .Surface()
            .map_err(|error| CaptureError::ImageConversionFailed(error.to_string()))?;
        let dxgi_surface: IDXGISurface = surface
            .cast()
            .map_err(|error| CaptureError::ImageConversionFailed(error.to_string()))?;

        let width = size.Width as u32;
        let height = size.Height as u32;
        let row_len = (width as usize) * 4;
        let mut bytes = Vec::with_capacity(row_len * height as usize);

        unsafe {
            let mut mapped: DXGI_MAPPED_RECT = std::mem::zeroed();
            dxgi_surface
                .Map(&mut mapped, DXGI_MAP_READ)
                .map_err(|error| CaptureError::ImageConversionFailed(error.to_string()))?;
            let row_pitch = mapped.Pitch as usize;
            for row in 0..height as usize {
                let src = mapped.pBits.add(row * row_pitch);
                bytes.extend_from_slice(std::slice::from_raw_parts(src, row_len));
            }
            let _ = dxgi_surface.Unmap();
        }

        Ok(CapturedImage {
            bytes,
            width,
            height,
            format: crate::core::capture::CapturedImageFormat::Bgra8,
        })
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
    async fn capture_full_screen_can_be_invoked() {
        let _ = WindowsScreenCapture.capture_full_screen().await;
    }
}
