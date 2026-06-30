use crate::core::capture::{
    CaptureError, CaptureRegion, CapturedImage, CapturedImageFormat, ScreenCapture,
};
use std::time::Duration;
use windows::core::Interface;
use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ,
    D3D11_MAPPED_SUBRESOURCE, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, ID3D11Device,
    ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1, IDXGIOutput, IDXGIOutput1,
    IDXGIOutputDuplication, DXGI_OUTDUPL_FRAME_INFO,
};
use windows::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTOPRIMARY};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

pub struct WindowsScreenCapture;

impl WindowsScreenCapture {
    pub fn new() -> Self {
        Self
    }

    pub fn is_supported() -> bool {
        true
    }

    fn create_d3d11_device() -> Result<(ID3D11Device, ID3D11DeviceContext), CaptureError> {
        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                None,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                7,
                Some(&mut device),
                None,
                Some(&mut context),
            )
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;
        }
        let device =
            device.ok_or_else(|| CaptureError::BackendUnavailable("D3D11 设备为空".into()))?;
        let context =
            context.ok_or_else(|| CaptureError::BackendUnavailable("D3D11 上下文为空".into()))?;
        Ok((device, context))
    }

    /// 找光标所在显示器对应的 DXGI Output（找不到则取遍历中首个 output）。
    fn duplicate_cursor_output(
        device: &ID3D11Device,
    ) -> Result<(IDXGIOutputDuplication, u32, u32), CaptureError> {
        unsafe {
            let mut cursor = POINT::default();
            let _ = GetCursorPos(&mut cursor);
            let target_monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTOPRIMARY);

            let factory: IDXGIFactory1 = CreateDXGIFactory1()
                .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;

            // 第一轮：找命中光标所在显示器的 output；同时记下遍历中遇到的第一个 output。
            let mut first_output: Option<IDXGIOutput> = None;
            let mut adapter_idx = 0u32;
            loop {
                let adapter: IDXGIAdapter1 = match factory.EnumAdapters1(adapter_idx) {
                    Ok(adapter) => adapter,
                    Err(_) => break,
                };
                adapter_idx += 1;
                let mut output_idx = 0u32;
                loop {
                    let output: IDXGIOutput = match adapter.EnumOutputs(output_idx) {
                        Ok(output) => output,
                        Err(_) => break,
                    };
                    output_idx += 1;
                    let desc = output
                        .GetDesc()
                        .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;
                    if first_output.is_none() {
                        first_output = Some(output.clone());
                    }
                    if desc.Monitor == target_monitor {
                        return Self::duplicate_output(&output, device);
                    }
                }
            }

            // 退而取第一个 output。
            if let Some(output) = first_output {
                return Self::duplicate_output(&output, device);
            }
            Err(CaptureError::BackendUnavailable(
                "未找到可复制的显示器输出".into(),
            ))
        }
    }

    unsafe fn duplicate_output(
        output: &IDXGIOutput,
        device: &ID3D11Device,
    ) -> Result<(IDXGIOutputDuplication, u32, u32), CaptureError> {
        let output1: IDXGIOutput1 = output
            .cast()
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;
        let dupl = output1
            .DuplicateOutput(device)
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;
        let dupl_desc = dupl.GetDesc();
        Ok((dupl, dupl_desc.ModeDesc.Width, dupl_desc.ModeDesc.Height))
    }

    pub async fn capture_monitor(&self) -> Result<CapturedImage, CaptureError> {
        // D3D11/DXGI 均为同步阻塞 API（AcquireNextFrame 轮询 + thread::sleep），
        // 整段丢到 spawn_blocking，避免卡住 tokio worker（含 translation:event 事件循环）。
        tauri::async_runtime::spawn_blocking(Self::capture_monitor_blocking)
            .await
            .map_err(|join_err| CaptureError::BackendUnavailable(join_err.to_string()))?
    }

    fn capture_monitor_blocking() -> Result<CapturedImage, CaptureError> {
        let (device, context) = Self::create_d3d11_device()?;
        let (dupl, _width, _height) = Self::duplicate_cursor_output(&device)?;

        // AcquireNextFrame 首帧即全帧；带超时轮询。
        let mut acquired: Option<ID3D11Texture2D> = None;
        for _ in 0..20 {
            let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource = None;
            let hr = unsafe { dupl.AcquireNextFrame(50, &mut frame_info, &mut resource) };
            match hr {
                Ok(()) => {
                    let resource = resource.ok_or_else(|| {
                        CaptureError::BackendUnavailable("DXGI 帧资源为空".into())
                    })?;
                    let texture: ID3D11Texture2D = resource
                        .cast()
                        .map_err(|error| CaptureError::ImageConversionFailed(error.to_string()))?;
                    acquired = Some(texture);
                    break;
                }
                Err(_) => {
                    // AcquireNextFrame 失败时不能调 ReleaseFrame（无对应 acquire，会触发 INVALID_CALL）。
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }

        let texture = acquired.ok_or_else(|| {
            CaptureError::BackendUnavailable("未能在超时时间内获取桌面帧".into())
        })?;

        // 以 acquired texture 自身实际尺寸为准（rotation / DPI 切换 / 模式刚切换时可能与 dupl_desc 不一致），
        // 覆盖 duplicate_output 返回的 dupl_desc 尺寸。
        let mut tex_desc = D3D11_TEXTURE2D_DESC::default();
        unsafe {
            texture.GetDesc(&mut tex_desc);
        }
        let tex_width = tex_desc.Width;
        let tex_height = tex_desc.Height;

        let result = Self::extract_bgra(&device, &context, &texture, tex_width, tex_height);
        unsafe {
            let _ = dupl.ReleaseFrame();
        }
        result
    }

    fn extract_bgra(
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        texture: &ID3D11Texture2D,
        width: u32,
        height: u32,
    ) -> Result<CapturedImage, CaptureError> {
        let row_len = (width as usize)
            .checked_mul(4)
            .ok_or_else(|| CaptureError::ImageConversionFailed("行字节数溢出".into()))?;
        let capacity = row_len
            .checked_mul(height as usize)
            .ok_or_else(|| CaptureError::ImageConversionFailed("缓冲区大小溢出".into()))?;

        let staging = Self::copy_to_staging(device, context, texture, width, height)?;

        unsafe {
            let mut mapped: D3D11_MAPPED_SUBRESOURCE = std::mem::zeroed();
            context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .map_err(|error| CaptureError::ImageConversionFailed(error.to_string()))?;
            let _guard = MappedTextureGuard {
                context,
                texture: &staging,
            };
            let row_pitch = mapped.RowPitch as usize;
            Self::validate_mapped_surface(mapped.pData as *mut u8, row_pitch, row_len)?;
            let mut bytes = Vec::with_capacity(capacity);
            for row in 0..height as usize {
                let offset = row.checked_mul(row_pitch).ok_or_else(|| {
                    CaptureError::ImageConversionFailed("行偏移溢出".into())
                })?;
                let src = (mapped.pData as *const u8).add(offset);
                bytes.extend_from_slice(std::slice::from_raw_parts(src, row_len));
            }
            Ok(CapturedImage {
                bytes,
                width,
                height,
                format: CapturedImageFormat::Bgra8,
            })
        }
    }

    fn copy_to_staging(
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        texture: &ID3D11Texture2D,
        width: u32,
        height: u32,
    ) -> Result<ID3D11Texture2D, CaptureError> {
        unsafe {
            let desc = Self::staging_texture_desc(width, height);
            let mut staging = None;
            device
                .CreateTexture2D(&desc, None, Some(&mut staging))
                .map_err(|error| CaptureError::ImageConversionFailed(error.to_string()))?;
            let staging = staging
                .ok_or_else(|| CaptureError::ImageConversionFailed("CPU 可读纹理为空".into()))?;
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

    fn validate_mapped_surface(
        bits: *mut u8,
        row_pitch: usize,
        row_len: usize,
    ) -> Result<(), CaptureError> {
        if bits.is_null() {
            return Err(CaptureError::ImageConversionFailed(
                "映射像素指针为空".into(),
            ));
        }
        if row_pitch < row_len {
            return Err(CaptureError::ImageConversionFailed(
                "行跨度小于行字节数".into(),
            ));
        }
        Ok(())
    }
}

impl Default for WindowsScreenCapture {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ScreenCapture for WindowsScreenCapture {
    async fn capture_region(&self, region: CaptureRegion) -> Result<CapturedImage, CaptureError> {
        let frame = self.capture_monitor().await?;
        frame.crop(
            region.x.max(0) as u32,
            region.y.max(0) as u32,
            region.width,
            region.height,
        )
    }

    async fn capture_interactive(&self) -> Result<Option<CapturedImage>, CaptureError> {
        // overlay 路径不走此方法；保留全屏帧以兼容 trait。
        Ok(Some(self.capture_monitor().await?))
    }
}

struct MappedTextureGuard<'a> {
    context: &'a ID3D11DeviceContext,
    texture: &'a ID3D11Texture2D,
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
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_screen_capture_is_send_sync_for_trait() {
        fn assert_send_sync<T: ScreenCapture + Send + Sync>() {}
        assert_send_sync::<WindowsScreenCapture>();
    }

    #[test]
    fn staging_texture_desc_is_cpu_readable_and_unbound() {
        let desc = WindowsScreenCapture::staging_texture_desc(10, 20);
        assert_eq!(desc.Width, 10);
        assert_eq!(desc.Height, 20);
        assert_eq!(desc.Format, DXGI_FORMAT_B8G8R8A8_UNORM);
        assert_eq!(desc.Usage, D3D11_USAGE_STAGING);
        assert_eq!(desc.BindFlags, 0);
        assert_eq!(desc.CPUAccessFlags, D3D11_CPU_ACCESS_READ.0 as u32);
    }

    #[test]
    fn validate_mapped_surface_rejects_null_bits() {
        assert!(matches!(
            WindowsScreenCapture::validate_mapped_surface(std::ptr::null_mut(), 4, 4),
            Err(CaptureError::ImageConversionFailed(_))
        ));
    }

    #[test]
    fn validate_mapped_surface_rejects_short_row_pitch() {
        let mut byte = 0u8;
        assert!(matches!(
            WindowsScreenCapture::validate_mapped_surface(&mut byte, 3, 4),
            Err(CaptureError::ImageConversionFailed(_))
        ));
    }

    #[tokio::test]
    #[ignore]
    async fn capture_monitor_returns_bgra_frame() {
        let capture = WindowsScreenCapture::new();
        let image = capture.capture_monitor().await.expect("应抓到帧");
        assert_eq!(image.format, CapturedImageFormat::Bgra8);
        assert_eq!(image.bytes.len(), (image.width * image.height * 4) as usize);
    }
}
