use crate::core::capture::{CaptureError, CaptureRegion, CapturedImage, ScreenCapture};
use std::time::Duration;
use windows::core::Interface;
use windows::Foundation::IAsyncOperation;
use windows::Graphics::Capture::{
    Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCapturePicker,
};
use windows::Graphics::DirectX::Direct3D11::{IDirect3DDevice, IDirect3DSurface};
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::SizeInt32;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, ID3D11Device, ID3D11DeviceContext,
    ID3D11Texture2D, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::UI::Shell::IInitializeWithWindow;

pub struct WindowsScreenCapture {
    // ponytail: 存 isize 而非 HWND，让结构体满足 Send+Sync（ScreenCapture trait 要求）。
    // HWND 是裸指针不可跨线程；用时转回 HWND。
    owner_hwnd: isize,
}

impl WindowsScreenCapture {
    pub fn new(owner_hwnd: isize) -> Self {
        Self { owner_hwnd }
    }

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

    pub(crate) async fn pick_capture_item(
        &self,
    ) -> Result<Option<GraphicsCaptureItem>, CaptureError> {
        let picker = GraphicsCapturePicker::new()
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;
        // 桌面应用必须为 picker 关联 owner window handle，否则 PickSingleItemAsync 会失败。
        let owner = HWND(self.owner_hwnd as *mut core::ffi::c_void);
        unsafe {
            picker
                .cast::<IInitializeWithWindow>()
                .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?
                .Initialize(owner)
                .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;
        }
        let operation: IAsyncOperation<GraphicsCaptureItem> = picker
            .PickSingleItemAsync()
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;
        let item = match operation.get() {
            Ok(item) => item,
            Err(error) => return Self::map_picker_result_error(error),
        };
        Ok(Some(item))
    }

    fn map_picker_result_error(
        error: windows::core::Error,
    ) -> Result<Option<GraphicsCaptureItem>, CaptureError> {
        if error.code().is_ok() {
            Ok(None)
        } else {
            Err(CaptureError::BackendUnavailable(error.to_string()))
        }
    }

    pub async fn capture_full_screen(&self) -> Result<Option<CapturedImage>, CaptureError> {
        let Some(item) = Self::pick_capture_item(self).await? else {
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
        let texture = Self::texture_from_direct3d_surface(&surface)?;
        let layout = Self::bgra_buffer_layout(size)?;
        let staging = Self::copy_texture_to_staging(&texture, layout.width, layout.height)?;

        unsafe {
            let context = texture
                .GetDevice()
                .and_then(|device| device.GetImmediateContext())
                .map_err(|error| CaptureError::ImageConversionFailed(error.to_string()))?;
            let mut mapped: D3D11_MAPPED_SUBRESOURCE = std::mem::zeroed();
            context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .map_err(|error| CaptureError::ImageConversionFailed(error.to_string()))?;
            let _guard = MappedTextureGuard::new(&context, &staging);
            let row_pitch = mapped.RowPitch as usize;
            Self::validate_mapped_surface(mapped.pData as *mut u8, row_pitch, layout.row_len)?;
            let mut bytes = Vec::with_capacity(layout.capacity);
            for row in 0..layout.height as usize {
                let offset = row.checked_mul(row_pitch).ok_or_else(|| {
                    CaptureError::ImageConversionFailed("映射后的截图行偏移溢出".to_string())
                })?;
                let src = (mapped.pData as *const u8).add(offset);
                bytes.extend_from_slice(std::slice::from_raw_parts(src, layout.row_len));
            }

            Ok(CapturedImage {
                bytes,
                width: layout.width,
                height: layout.height,
                format: crate::core::capture::CapturedImageFormat::Bgra8,
            })
        }
    }

    fn texture_from_direct3d_surface(
        surface: &IDirect3DSurface,
    ) -> Result<ID3D11Texture2D, CaptureError> {
        let access: IDirect3DDxgiInterfaceAccess = surface
            .cast()
            .map_err(|error| CaptureError::ImageConversionFailed(error.to_string()))?;
        unsafe {
            access
                .GetInterface::<ID3D11Texture2D>()
                .map_err(|error| CaptureError::ImageConversionFailed(error.to_string()))
        }
    }

    fn copy_texture_to_staging(
        texture: &ID3D11Texture2D,
        width: u32,
        height: u32,
    ) -> Result<ID3D11Texture2D, CaptureError> {
        unsafe {
            let device = texture
                .GetDevice()
                .map_err(|error| CaptureError::ImageConversionFailed(error.to_string()))?;
            let context = device
                .GetImmediateContext()
                .map_err(|error| CaptureError::ImageConversionFailed(error.to_string()))?;
            let desc = Self::staging_texture_desc(width, height);
            let mut staging = None;
            device
                .CreateTexture2D(&desc, None, Some(&mut staging))
                .map_err(|error| CaptureError::ImageConversionFailed(error.to_string()))?;
            let staging = staging.ok_or_else(|| {
                CaptureError::ImageConversionFailed("CPU 可读截图纹理为空".to_string())
            })?;
            context.CopyResource(&staging, texture);
            Ok(staging)
        }
    }

    fn staging_texture_desc(width: u32, height: u32) -> D3D11_TEXTURE2D_DESC {
        D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        }
    }

    fn bgra_buffer_layout(size: SizeInt32) -> Result<BgraBufferLayout, CaptureError> {
        if size.Width <= 0 || size.Height <= 0 {
            return Err(CaptureError::ImageConversionFailed(
                "截图尺寸必须为正数".to_string(),
            ));
        }

        let width = size.Width as u32;
        let height = size.Height as u32;
        let row_len = (width as usize)
            .checked_mul(4)
            .ok_or_else(|| CaptureError::ImageConversionFailed("截图行字节数溢出".to_string()))?;
        let capacity = row_len
            .checked_mul(height as usize)
            .ok_or_else(|| CaptureError::ImageConversionFailed("截图缓冲区大小溢出".to_string()))?;
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

struct MappedTextureGuard<'a> {
    context: &'a ID3D11DeviceContext,
    texture: &'a ID3D11Texture2D,
}

impl<'a> MappedTextureGuard<'a> {
    fn new(context: &'a ID3D11DeviceContext, texture: &'a ID3D11Texture2D) -> Self {
        Self { context, texture }
    }
}

impl Drop for MappedTextureGuard<'_> {
    fn drop(&mut self) {
        unsafe {
            self.context.Unmap(self.texture, 0);
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
    fn windows_screen_capture_is_send_sync_for_trait() {
        // ScreenCapture: Send + Sync；owner 句柄必须用 isize 而非 HWND（裸指针不可跨线程）。
        fn assert_send_sync<T: ScreenCapture + Send + Sync>() {}
        assert_send_sync::<WindowsScreenCapture>();
    }

    #[test]
    fn windows_screen_capture_stores_owner_hwnd() {
        let capture = WindowsScreenCapture::new(0x1234);
        assert_eq!(capture.owner_hwnd, 0x1234);
    }

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

        assert!(matches!(
            result,
            Err(CaptureError::ImageConversionFailed(_))
        ));
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
    fn staging_texture_desc_is_cpu_readable_and_unbound() {
        let desc = WindowsScreenCapture::staging_texture_desc(10, 20);

        assert_eq!(desc.Width, 10);
        assert_eq!(desc.Height, 20);
        assert_eq!(desc.MipLevels, 1);
        assert_eq!(desc.ArraySize, 1);
        assert_eq!(desc.Format, DXGI_FORMAT_B8G8R8A8_UNORM);
        assert_eq!(desc.SampleDesc.Count, 1);
        assert_eq!(desc.SampleDesc.Quality, 0);
        assert_eq!(desc.Usage, D3D11_USAGE_STAGING);
        assert_eq!(desc.BindFlags, 0);
        assert_eq!(desc.CPUAccessFlags, D3D11_CPU_ACCESS_READ.0 as u32);
        assert_eq!(desc.MiscFlags, 0);
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

        assert!(matches!(
            result,
            Err(CaptureError::ImageConversionFailed(_))
        ));
    }

    #[test]
    fn validate_mapped_surface_rejects_short_row_pitch() {
        let mut byte = 0_u8;
        let result = WindowsScreenCapture::validate_mapped_surface(&mut byte, 3, 4);

        assert!(matches!(
            result,
            Err(CaptureError::ImageConversionFailed(_))
        ));
    }

    #[test]
    fn successful_hresult_picker_error_is_treated_as_cancel() {
        let result = WindowsScreenCapture::map_picker_result_error(
            windows::core::Error::from_hresult(windows::core::HRESULT(0)),
        );

        assert!(result.expect("S_OK picker error 应视为用户取消").is_none());
    }

    #[test]
    fn failed_hresult_picker_error_remains_backend_error() {
        let result = WindowsScreenCapture::map_picker_result_error(
            windows::core::Error::from_hresult(windows::core::HRESULT(0x8000_4005_u32 as i32)),
        );

        assert!(matches!(result, Err(CaptureError::BackendUnavailable(_))));
    }

    #[tokio::test]
    #[ignore]
    async fn capture_full_screen_returns_bgra_image_when_user_picks_display() {
        if !WindowsScreenCapture::is_supported() {
            return;
        }

        // 人工验证：owner_hwnd 在真实应用中由 Tauri 窗口提供；
        // 此 ignored 测试用 0 句柄，picker 可能因无 owner 而失败，仅用于本地手动调试。
        let capture = WindowsScreenCapture::new(0);
        let image = capture
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
