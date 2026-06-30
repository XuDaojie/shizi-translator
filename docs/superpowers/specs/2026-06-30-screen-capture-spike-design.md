# 全屏单帧截图 Spike 设计规格

## 背景

Windows OCR spike 已验证 `Windows.Media.Ocr` 能识别内存图片。下一步是补齐截图能力，让真实屏幕像素能进入 `CapturedImage`。本切片只验证「能拿到全屏单帧并转成 `CapturedImage`」，不接 OCR、不接快捷键、不接翻译编排。

现有基础：

- `src-tauri/src/platform/windows/capture.rs` 已有 `WindowsGraphicsCaptureProbe::is_supported()`。
- `core::capture` 已定义 `CapturedImage`、`CapturedImageFormat::Bgra8`、`CaptureError`。
- `WindowsOcrEngine` 已能消费 `CapturedImageFormat::Bgra8`。

## 目标

通过 `Windows.Graphics.Capture` 获取一帧全屏像素，转换为 `CapturedImage { format: Bgra8, .. }`。

完成后应具备：

- `WindowsScreenCapture::is_supported()` 检测当前系统是否支持 Graphics Capture。
- `WindowsScreenCapture::capture_full_screen()` 弹 picker 让用户选显示器，返回单帧。
- D3D11 设备创建与 `IDirect3DDevice` 桥接。
- 从 `Direct3D11CaptureFrame` 读取 BGRA 像素到 `Vec<u8>`。
- 用户取消、平台不支持、设备失败、帧读取失败都有明确错误映射。
- 默认忽略的 Windows 集成测试，用于人工验证真实截图链路。
- 不影响现有 `Alt+T` 划词翻译和 OCR engine。

## 非目标

- 不实现 `capture_region` 区域截图。
- 不实现 `ScreenCapture` trait；本切片只提供独立方法和测试。trait 接入留给后续切片。
- 不接 `WindowsOcrEngine`。
- 不接 OCR 快捷键、翻译编排、UI 状态。
- 不做自建选区 overlay，不做多屏，不做 DPI 缩放处理。
- 不做 `FrameArrived` 事件流；只用 `TryGetNextFrame` 取一帧后立即关闭 session 和 frame pool。

## 推荐架构

扩展 `src-tauri/src/platform/windows/capture.rs`，新增：

```rust
pub struct WindowsScreenCapture;

impl WindowsScreenCapture {
    pub fn is_supported() -> bool {
        windows::Graphics::Capture::GraphicsCaptureSession::IsSupported().unwrap_or(false)
    }

    pub async fn capture_full_screen(&self) -> Result<Option<CapturedImage>, CaptureError>;
}
```

`capture_full_screen` 返回 `Option<CapturedImage>`，与现有 `ScreenCapture::capture_interactive` 的「用户取消 = `Ok(None)`」语义一致。

内部私有函数：

- `pick_capture_item() -> Result<Option<GraphicsCaptureItem>, CaptureError>`：调用 `GraphicsCapturePicker`，用户取消返回 `Ok(None)`。
- `create_direct3d_device() -> Result<IDirect3DDevice, CaptureError>`：创建 D3D11 设备并桥接为 WinRT `IDirect3DDevice`。这是本切片核心风险点。
- `extract_bgra_from_frame(frame: Direct3D11CaptureFrame) -> Result<CapturedImage, CaptureError>`：从 frame surface 读取 BGRA 像素。

## 数据流

```text
WindowsScreenCapture::capture_full_screen
  -> pick_capture_item (GraphicsCapturePicker)
  -> GraphicsCaptureItem
  -> create_direct3d_device (D3D11 Device + CreateDirect3D11DeviceFromDXGIDevice)
  -> IDirect3DDevice
  -> Direct3D11CaptureFramePool::CreateFreeThreaded(Bgra8, 2, item size)
  -> frame_pool.CreateCaptureSession(item) + session.StartCapture
  -> frame_pool.TryGetNextFrame()
  -> Direct3D11CaptureFrame.Surface (IDXGISurface)
  -> extract_bgra_from_frame
  -> CapturedImage { bytes, width, height, format: Bgra8 }
  -> session.Close / frame_pool.Close
```

## D3D11 设备桥接

`Direct3D11CaptureFramePool` 需要一个 `IDirect3DDevice`。在 Win32 桌面应用中，必须：

1. 用 `D3D11CreateDevice` 创建 `ID3D11Device`。
2. 用 `CreateDirect3D11DeviceFromDXGIDevice` 把 `ID3D11Device` 桥接为 WinRT `IDirect3DDevice`。

需要补充 windows crate feature（具体 feature 名以 windows 0.58 为准）：

- `Win32_Graphics_Direct3D11`
- `Win32_Graphics_Dxgi`
- `Graphics_DirectX_Direct3D11`
- D3D11 与 WinRT 互操作所需的 feature，例如 `Win32_System_WinRT` 或 `Win32_System_WinRT_Direct3D11Interop`

如果 windows 0.58 的 feature 名或 interop API 与预期不符，spike 会阻塞，届时按实际 API 调整或升级处理，不在规格中预先猜测具体路径。

## picker 选择

`GraphicsCapturePicker` 在桌面应用中需要关联 owner window handle。Shizi 主窗口在翻译流程中可能处于隐藏状态；spike 阶段允许 picker 以当前激活窗口或无 owner 形式弹出。如果 picker 因 owner 缺失失败，按 `CaptureError::UnsupportedPlatform` 或 `BackendUnavailable` 映射，不在此切片内做窗口句柄管理。

## 像素读取

`Direct3D11CaptureFrame::Surface` 返回 `IDirect3DSurface`，需 QueryInterface 到 `IDXGISurface`，再 `Map` 读取像素。读取要点：

- 以 `DXGI_MAP_READ` 映射。
- 处理 row pitch：surface 的 row pitch 可能大于 `width * 4`，拷贝时按行 stride 处理，输出紧密排列的 BGRA buffer。
- 读取后 `Unmap`。
- frame pool 创建时指定 `DirectXPixelFormat::Bgra8`，保证帧格式为 BGRA8。

如果 surface 映射失败或格式不是 BGRA8，返回 `CaptureError::ImageConversionFailed`。

## 错误映射

- 平台不支持 Graphics Capture：`CaptureError::UnsupportedPlatform`
- 用户取消 picker：`Ok(None)`
- picker 弹出失败：`CaptureError::BackendUnavailable`
- D3D11 设备创建失败：`CaptureError::BackendUnavailable`
- 设备桥接失败：`CaptureError::ImageConversionFailed`
- frame pool / session 创建失败：`CaptureError::BackendUnavailable`
- 帧读取或像素拷贝失败：`CaptureError::ImageConversionFailed`

## 测试策略

### 单元测试

- `is_supported()` 可调用且不 panic。
- 纯逻辑函数（如有）按需测试。D3D11 / picker / frame 链路无法在 CI 单测中运行，不强测。

### 默认忽略的 Windows 集成测试

新增 `#[tokio::test] #[ignore]` 测试：

- 调用 `WindowsScreenCapture::capture_full_screen()`。
- 需用户手动在 picker 中选择显示器。
- 断言返回 `Ok(Some(image))`，且 `image.format == Bgra8`、`image.bytes.len() == image.width * image.height * 4`。
- 不断言像素内容。

运行方式：

```bash
cd src-tauri && cargo test capture_full_screen -- --ignored
```

## 成功标准

- `cargo build` 通过，含 D3D11 桥接所需 windows feature。
- `cargo test` 通过，忽略的集成测试不影响默认测试。
- 默认忽略测试在 Windows 上能弹 picker、拿到一帧、转成 `CapturedImage`，且 `bytes.len() == width * height * 4`。
- `node --check frontend/main.js` 通过。
- 不影响现有手动输入、`Alt+T` 划词翻译和 `WindowsOcrEngine`。

## 后续切片

本切片完成后：

1. 为 `WindowsScreenCapture` 实现 `ScreenCapture` trait。
2. 串联 `ScreenCapture -> WindowsOcrEngine -> TranslationInput::OcrText -> TranslationService`。
3. 新增 OCR 快捷键与最小 UI 状态。
4. 评估区域截图 / overlay / 多屏 / DPI。
