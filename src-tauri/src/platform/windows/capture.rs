use crate::core::capture::{CaptureRegion, CapturedImage, CaptureError, ScreenCapture};
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
        let result = Self::capture_result_from_frame(frame, size);

        let _ = session.Close();
        let _ = pool.Close();
        result
    }

    fn capture_result_from_frame(
        frame: Option<Direct3D11CaptureFrame>,
        size: SizeInt32,
    ) -> Result<Option<CapturedImage>, CaptureError> {
        match frame {
            Some(frame) => Self::extract_bgra_from_frame(frame, size).map(Some),
            None => Err(CaptureError::BackendUnavailable(
                "未能在超时时间内获取首帧".to_string(),
            )),
        }
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

        let layout = Self::bgra_buffer_layout(size)?;
        let mut bytes = Vec::with_capacity(layout.capacity);

        unsafe {
            let mut mapped: DXGI_MAPPED_RECT = std::mem::zeroed();
            dxgi_surface
                .Map(&mut mapped, DXGI_MAP_READ)
                .map_err(|error| CaptureError::ImageConversionFailed(error.to_string()))?;
            let _guard = MappedSurfaceGuard::new(&dxgi_surface);
            let row_pitch = mapped.Pitch as usize;
            Self::validate_mapped_surface(mapped.pBits, row_pitch, layout.row_len)?;
            for row in 0..layout.height as usize {
                let offset = row.checked_mul(row_pitch).ok_or_else(|| {
                    CaptureError::ImageConversionFailed("映射后的截图行偏移溢出".to_string())
                })?;
                let src = mapped.pBits.add(offset);
                bytes.extend_from_slice(std::slice::from_raw_parts(src, layout.row_len));
            }
        }

        Ok(CapturedImage {
            bytes,
            width: layout.width,
            height: layout.height,
            format: crate::core::capture::CapturedImageFormat::Bgra8,
        })
    }

    fn bgra_buffer_layout(size: SizeInt32) -> Result<BgraBufferLayout, CaptureError> {
        if size.Width <= 0 || size.Height <= 0 {
            return Err(CaptureError::ImageConversionFailed(
                "截图尺寸必须为正数".to_string(),
            ));
        }

        let width = size.Width as u32;
        let height = size.Height as u32;
        let row_len = (width as usize).checked_mul(4).ok_or_else(|| {
            CaptureError::ImageConversionFailed("截图行字节数溢出".to_string())
        })?;
        let capacity = row_len.checked_mul(height as usize).ok_or_else(|| {
            CaptureError::ImageConversionFailed("截图缓冲区大小溢出".to_string())
        })?;
        Ok(BgraBufferLayout {
            width,
            height,
            row_len,
            capacity,
        })
    }

    fn validate_mapped_surface(
        bits: *mut u8,
        row_pitch: usize,
        row_len: usize,
    ) -> Result<(), CaptureError> {
        if bits.is_null() {
            return Err(CaptureError::ImageConversionFailed(
                "映射后的截图像素指针为空".to_string(),
            ));
        }
        if row_pitch < row_len {
            return Err(CaptureError::ImageConversionFailed(
                "映射后的截图行跨度小于行字节数".to_string(),
            ));
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl ScreenCapture for WindowsScreenCapture {
    async fn capture_region(&self, _region: CaptureRegion) -> Result<CapturedImage, CaptureError> {
        // ponytail: 区域截图留给 DXGI/自建 overlay 阶段，MVP 仅交互式 picker
        Err(CaptureError::UnsupportedPlatform)
    }

    async fn capture_interactive(&self) -> Result<Option<CapturedImage>, CaptureError> {
        self.capture_full_screen().await
    }
}

struct BgraBufferLayout {
    width: u32,
    height: u32,
    row_len: usize,
    capacity: usize,
}

struct MappedSurfaceGuard<'a> {
    surface: &'a IDXGISurface,
}

impl<'a> MappedSurfaceGuard<'a> {
    fn new(surface: &'a IDXGISurface) -> Self {
        Self { surface }
    }
}

impl Drop for MappedSurfaceGuard<'_> {
    fn drop(&mut self) {
        unsafe {
            let _ = self.surface.Unmap();
        }
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

    #[test]
    fn bgra_buffer_layout_rejects_non_positive_size() {
        let size = SizeInt32 {
            Width: 0,
            Height: 1,
        };

        let result = WindowsScreenCapture::bgra_buffer_layout(size);

        assert!(matches!(result, Err(CaptureError::ImageConversionFailed(_))));
    }

    #[test]
    fn bgra_buffer_layout_calculates_row_and_capacity() {
        let size = SizeInt32 {
            Width: 3,
            Height: 2,
        };

        let layout = WindowsScreenCapture::bgra_buffer_layout(size).unwrap();

        assert_eq!(layout.width, 3);
        assert_eq!(layout.height, 2);
        assert_eq!(layout.row_len, 12);
        assert_eq!(layout.capacity, 24);
    }

    #[test]
    fn capture_result_from_frame_returns_error_when_first_frame_times_out() {
        let size = SizeInt32 {
            Width: 1,
            Height: 1,
        };

        let result = WindowsScreenCapture::capture_result_from_frame(None, size);

        assert!(matches!(result, Err(CaptureError::BackendUnavailable(_))));
    }

    #[test]
    fn validate_mapped_surface_rejects_null_bits() {
        let result = WindowsScreenCapture::validate_mapped_surface(std::ptr::null_mut(), 4, 4);

        assert!(matches!(result, Err(CaptureError::ImageConversionFailed(_))));
    }

    #[test]
    fn validate_mapped_surface_rejects_short_row_pitch() {
        let mut byte = 0_u8;
        let result = WindowsScreenCapture::validate_mapped_surface(&mut byte, 3, 4);

        assert!(matches!(result, Err(CaptureError::ImageConversionFailed(_))));
    }

    #[tokio::test]
    #[ignore]
    async fn capture_full_screen_returns_bgra_image_when_user_picks_display() {
        if !WindowsScreenCapture::is_supported() {
            return;
        }

        let image = WindowsScreenCapture
            .capture_full_screen()
            .await
            .expect("截图链路应成功")
            .expect("用户应选择显示器");

        assert_eq!(
            image.format,
            crate::core::capture::CapturedImageFormat::Bgra8
        );
        let expected_len = image
            .width
            .checked_mul(image.height)
            .and_then(|pixels| pixels.checked_mul(4))
            .map(|bytes| bytes as usize)
            .expect("截图尺寸应可计算 BGRA 字节长度");
        assert_eq!(image.bytes.len(), expected_len);
    }
}
