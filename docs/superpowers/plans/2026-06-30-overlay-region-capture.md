# 自建 Overlay 区域框选 截图 OCR 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 把截图 OCR 从「系统 picker 全屏单帧」演进到「自建 overlay 区域框选」，接近 Bob/Pot 体验。

**架构：** `Alt+O` → DXGI Desktop Duplication 抓光标所在显示器整屏 BGRA 帧 → 存入 `AppState` → 建独立 overlay 窗口加载 `overlay.html` → 前端 canvas 显示整屏 + 鼠标框选 → 提交 CSS 矩形 → Rust 按 `scale_factor` 换算物理像素 → 内存裁剪 BGRA → `WindowsOcrEngine` → 复用 `start_translation_from_input` 翻译链路。编排全在 Rust，前端只回传原始 CSS 矩形。

**技术栈：** Rust + Tauri 2 + `windows` crate 0.58（DXGI Desktop Duplication、D3D11 staging texture、Windows.Media.Ocr）+ 原生静态 HTML/JS overlay（无构建）。

**规格依据：** `docs/superpowers/specs/2026-06-30-overlay-region-capture-design.md`（commit a986411）

---

## 文件结构

**新增：**
- `frontend/overlay.html` — overlay 前端：canvas 显示整屏 BGRA、鼠标框选、Esc/右键取消、四个 invoke。单一职责：框选交互 + 回传 CSS 矩形。
- `src-tauri/src/ui/overlay.rs` — overlay 窗口建窗 + 四个 Tauri command（取帧 meta/bytes、提交矩形、取消）。单一职责：overlay 窗口生命周期与前后端桥。

**修改：**
- `src-tauri/src/core/capture/mod.rs` — 加 `CapturedImage::crop`（纯 BGRA 行切片）+ `css_rect_to_physical`（DPI 换算纯函数）。
- `src-tauri/src/platform/windows/capture.rs` — DXGI `capture_monitor`，删除 GraphicsCapturePicker 路径与 `owner_hwnd`，复用 staging 提取。
- `src-tauri/src/platform/windows/mod.rs` — 平台缝：`capture_screen` + `recognize_region`。
- `src-tauri/src/platform/unsupported.rs` — 非 Windows 对应 stub。
- `src-tauri/src/platform/mod.rs` — re-export `capture_screen` / `recognize_region`。
- `src-tauri/src/core/ocr_translation.rs` — 加 `recognize_cropped_for_translation`。
- `src-tauri/src/app/state.rs` — `AppState` 存整屏帧（set/take + meta）。
- `src-tauri/src/ui/ocr_popup.rs` — `start_translation_from_ocr` 改为「抓帧 + 建 overlay」；`friendly_ocr_error` 复用。
- `src-tauri/src/ui/mod.rs` — 声明 `pub mod overlay;`。
- `src-tauri/src/app/shortcuts.rs` — 入口不变（仍调 `start_translation_from_ocr`），仅去掉 owner_hwnd 透传。
- `src-tauri/src/lib.rs` — 注册四个新 command。
- `src-tauri/capabilities/default.json` — `windows` 列表加 `screenshot-overlay`。

---

## 任务 1：`CapturedImage::crop` 内存裁剪（纯 Rust）

**文件：**
- 修改：`src-tauri/src/core/capture/mod.rs`
- 测试：同文件 `#[cfg(test)] mod tests`

- [ ] **步骤 1：编写失败的测试**

在 `src-tauri/src/core/capture/mod.rs` 的 `mod tests` 内追加：

```rust
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
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib core::capture`
预期：FAIL，报错 `no method named crop`。

- [ ] **步骤 3：编写最少实现代码**

在 `src-tauri/src/core/capture/mod.rs` 的 `impl` 区域（紧跟 `CapturedImage` 结构体定义后）加：

```rust
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
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib core::capture`
预期：PASS，4 个新测试 + 现有测试全过。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/core/capture/mod.rs
git commit -m "feat(capture): CapturedImage::crop 内存 BGRA 行切片裁剪"
```

---

## 任务 2：`css_rect_to_physical` DPI 换算（纯 Rust）

**文件：**
- 修改：`src-tauri/src/core/capture/mod.rs`
- 测试：同文件 `mod tests`

- [ ] **步骤 1：编写失败的测试**

在 `mod tests` 内追加：

```rust
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
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib core::capture::tests::css_rect`
预期：FAIL，报错 `cannot find function css_rect_to_physical`。

- [ ] **步骤 3：编写最少实现代码**

在 `src-tauri/src/core/capture/mod.rs` 顶层（结构体定义区域后、`impl` 附近）加：

```rust
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
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib core::capture`
预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/core/capture/mod.rs
git commit -m "feat(capture): css_rect_to_physical DPI 矩形换算纯函数"
```

---

## 任务 3：`AppState` 存整屏帧

**文件：**
- 修改：`src-tauri/src/app/state.rs`
- 测试：同文件 `mod tests`

- [ ] **步骤 1：编写失败的测试**

先在 `mod tests` 内追加（顶部已有 `app_state()` helper）：

```rust
    #[test]
    fn pending_capture_frame_round_trips() {
        use crate::core::capture::{CapturedImage, CapturedImageFormat};
        let state = app_state();
        let frame = CapturedImage {
            bytes: vec![1, 2, 3, 4],
            width: 1,
            height: 1,
            format: CapturedImageFormat::Bgra8,
        };

        state.set_pending_capture(frame.clone(), 1.5).expect("写入截图帧");

        let meta = state.pending_capture_meta().expect("读取 meta").expect("应有 meta");
        assert_eq!(meta, (1, 1, 1.5));

        let taken = state.take_pending_capture().expect("取出帧").expect("应有帧");
        assert_eq!(taken.0, frame);
        assert_eq!(taken.1, 1.5);

        assert!(state.take_pending_capture().expect("再次取出").is_none());
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib app::state`
预期：FAIL，`no method named set_pending_capture`。

- [ ] **步骤 3：编写最少实现代码**

在 `src-tauri/src/app/state.rs`：

顶部 use 区改为：

```rust
use std::sync::{Arc, Mutex};

use crate::core::capture::CapturedImage;
use crate::core::config::ConfigStore;
```

`AppState` 结构体加字段（在 `translation_busy` 后）：

```rust
    // overlay 截图链路：抓到的整屏帧 + 显示器 scale_factor，等待框选裁剪。
    pending_capture: Arc<Mutex<Option<(CapturedImage, f64)>>>,
```

`AppState::new` 的初始化加：

```rust
            pending_capture: Arc::new(Mutex::new(None)),
```

`impl AppState` 内追加方法：

```rust
    pub fn set_pending_capture(&self, frame: CapturedImage, scale_factor: f64) -> Result<(), String> {
        let mut slot = self
            .pending_capture
            .lock()
            .map_err(|_| "截图帧状态锁已损坏".to_string())?;
        *slot = Some((frame, scale_factor));
        Ok(())
    }

    pub fn pending_capture_meta(&self) -> Result<Option<(u32, u32, f64)>, String> {
        let slot = self
            .pending_capture
            .lock()
            .map_err(|_| "截图帧状态锁已损坏".to_string())?;
        Ok(slot.as_ref().map(|(frame, scale)| (frame.width, frame.height, *scale)))
    }

    pub fn pending_capture_bytes(&self) -> Result<Option<Vec<u8>>, String> {
        let slot = self
            .pending_capture
            .lock()
            .map_err(|_| "截图帧状态锁已损坏".to_string())?;
        Ok(slot.as_ref().map(|(frame, _)| frame.bytes.clone()))
    }

    pub fn take_pending_capture(&self) -> Result<Option<(CapturedImage, f64)>, String> {
        let mut slot = self
            .pending_capture
            .lock()
            .map_err(|_| "截图帧状态锁已损坏".to_string())?;
        Ok(slot.take())
    }
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib app::state`
预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/app/state.rs
git commit -m "feat(state): AppState 暂存 overlay 整屏帧与 scale_factor"
```

---

## 任务 4：`recognize_cropped_for_translation` 编排（纯 Rust，fake 可测）

**文件：**
- 修改：`src-tauri/src/core/ocr_translation.rs`
- 测试：同文件 `mod tests`

- [ ] **步骤 1：编写失败的测试**

在 `src-tauri/src/core/ocr_translation.rs` 的 `mod tests` 内追加（`FakeOcr` / `image()` 已存在，但 `image()` 现为 1x1 Rgba8；新增一个 BGRA helper）：

```rust
    fn bgra_4x4() -> CapturedImage {
        CapturedImage {
            bytes: vec![128; 4 * 4 * 4],
            width: 4,
            height: 4,
            format: CapturedImageFormat::Bgra8,
        }
    }

    #[tokio::test]
    async fn cropped_workflow_returns_ocr_input() {
        let frame = bgra_4x4();
        let input = recognize_cropped_for_translation(
            &frame,
            (1, 1, 2, 2),
            &FakeOcr { text: " Hi ".to_string() },
            OcrHints::default(),
        )
        .await
        .expect("裁剪 OCR workflow 应成功")
        .expect("应返回 OCR 输入");

        assert_eq!(input.text(), "Hi");
    }

    #[tokio::test]
    async fn cropped_workflow_rejects_empty_text() {
        let frame = bgra_4x4();
        let error = recognize_cropped_for_translation(
            &frame,
            (0, 0, 2, 2),
            &FakeOcr { text: "   ".to_string() },
            OcrHints::default(),
        )
        .await
        .expect_err("空文本应报错");

        assert!(matches!(
            error,
            OcrTranslationError::Ocr(crate::core::ocr::OcrError::EmptyResult)
        ));
    }

    #[tokio::test]
    async fn cropped_workflow_propagates_crop_error() {
        let frame = bgra_4x4();
        let error = recognize_cropped_for_translation(
            &frame,
            (3, 3, 5, 5),
            &FakeOcr { text: "x".to_string() },
            OcrHints::default(),
        )
        .await
        .expect_err("越界裁剪应报错");

        assert!(matches!(
            error,
            OcrTranslationError::Capture(crate::core::capture::CaptureError::ImageConversionFailed(_))
        ));
    }
```

注意：`mod tests` 顶部 `use` 需含 `CapturedImageFormat`，现有导入已是 `capture::{CaptureError, CaptureRegion, CapturedImage, CapturedImageFormat, ScreenCapture}`，无需改。

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib core::ocr_translation`
预期：FAIL，`cannot find function recognize_cropped_for_translation`。

- [ ] **步骤 3：编写最少实现代码**

在 `src-tauri/src/core/ocr_translation.rs` 顶层加函数（保留现有 `recognize_capture_for_translation` 不动）：

```rust
/// overlay 路径：对已抓到的整屏帧按物理像素矩形裁剪后 OCR，转成翻译输入。
pub async fn recognize_cropped_for_translation<O>(
    frame: &CapturedImage,
    region: (u32, u32, u32, u32),
    ocr: &O,
    hints: OcrHints,
) -> Result<Option<TranslationInput>, OcrTranslationError>
where
    O: OcrEngine,
{
    let (x, y, w, h) = region;
    let cropped = frame.crop(x, y, w, h)?;
    let result = ocr.recognize(cropped, hints).await?;
    let text = result.text.trim().to_string();
    if text.is_empty() {
        return Err(OcrError::EmptyResult.into());
    }
    Ok(Some(TranslationInput::OcrText {
        text,
        image_id: None,
    }))
}
```

顶部 `use` 需含 `CapturedImage`（现为 `capture::{CaptureError, ScreenCapture}`），改为：

```rust
use crate::core::{
    capture::{CaptureError, CapturedImage, ScreenCapture},
    ocr::{OcrEngine, OcrError, OcrHints},
    translation::TranslationInput,
};
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib core::ocr_translation`
预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/core/ocr_translation.rs
git commit -m "feat(ocr): recognize_cropped_for_translation 裁剪后 OCR 编排"
```

---

## 任务 5：DXGI `capture_monitor` 后端

> 此任务接触 Win32 unsafe + DXGI，逻辑无法纯单测，主要靠编译 + `#[ignore]` 人工集成测试。保留可单测的纯函数（layout/校验），unsafe 部分尽量薄。

**文件：**
- 修改：`src-tauri/src/platform/windows/capture.rs`
- 修改：`src-tauri/Cargo.toml`（windows features 增补）

- [ ] **步骤 1：增补 windows crate features**

在 `src-tauri/Cargo.toml` 的 `[target.'cfg(windows)'.dependencies]` 的 windows features 列表加（DXGI Output 复制需要）：

```toml
  "Win32_Graphics_Dxgi",
  "Win32_Graphics_Gdi",
  "Win32_UI_WindowsAndMessaging",
```

（`Win32_Graphics_Dxgi` 已有则保留；新增 `Win32_Graphics_Gdi`（HMONITOR/MonitorFromPoint）与 `Win32_UI_WindowsAndMessaging`（GetCursorPos）。）

- [ ] **步骤 2：编写 staging 提取的纯函数测试（复用现有 layout 测试）**

现有 `bgra_buffer_layout` / `validate_mapped_surface` / `staging_texture_desc` 测试保留可用。无需新纯函数测试——`capture_monitor` 本身靠集成测试。直接进入实现。

- [ ] **步骤 3：重写 capture.rs**

将 `src-tauri/src/platform/windows/capture.rs` 改为以下内容（删除 GraphicsCapturePicker/owner_hwnd/frame pool，新增 DXGI Output 复制；保留 `bgra_buffer_layout`/`staging_texture_desc`/`validate_mapped_surface`/`MappedTextureGuard`/`copy_texture_to_staging` 等可复用件）：

```rust
use crate::core::capture::{CaptureError, CaptureRegion, CapturedImage, CapturedImageFormat, ScreenCapture};
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
    IDXGIAdapter1, IDXGIDevice, IDXGIFactory1, IDXGIOutput, IDXGIOutput1, IDXGIOutputDuplication,
    CreateDXGIFactory1, DXGI_OUTDUPL_FRAME_INFO,
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
            .map_err(|e| CaptureError::BackendUnavailable(e.to_string()))?;
        }
        let device = device.ok_or_else(|| CaptureError::BackendUnavailable("D3D11 设备为空".into()))?;
        let context = context.ok_or_else(|| CaptureError::BackendUnavailable("D3D11 上下文为空".into()))?;
        Ok((device, context))
    }

    /// 找光标所在显示器对应的 DXGI Output（找不到则取第一个 adapter 的第一个 output）。
    fn duplicate_cursor_output(
        device: &ID3D11Device,
    ) -> Result<(IDXGIOutputDuplication, D3D11_TEXTURE2D_DESC), CaptureError> {
        unsafe {
            let mut cursor = POINT::default();
            let _ = GetCursorPos(&mut cursor);
            let target_monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTOPRIMARY);

            let factory: IDXGIFactory1 =
                CreateDXGIFactory1().map_err(|e| CaptureError::BackendUnavailable(e.to_string()))?;

            let mut adapter_idx = 0;
            while let Ok(adapter) = factory.EnumAdapters1(adapter_idx) {
                adapter_idx += 1;
                let adapter: IDXGIAdapter1 = adapter;
                let mut output_idx = 0;
                while let Ok(output) = adapter.EnumOutputs(output_idx) {
                    output_idx += 1;
                    let output: IDXGIOutput = output;
                    let desc = output
                        .GetDesc()
                        .map_err(|e| CaptureError::BackendUnavailable(e.to_string()))?;
                    // 命中光标所在显示器，或退而取第一个 output
                    if desc.Monitor == target_monitor || (adapter_idx == 1 && output_idx == 1) {
                        let output1: IDXGIOutput1 = output
                            .cast()
                            .map_err(|e| CaptureError::BackendUnavailable(e.to_string()))?;
                        if desc.Monitor != target_monitor {
                            continue; // 仅暂存第一个；优先继续找命中的
                        }
                        let dupl = output1
                            .DuplicateOutput(device)
                            .map_err(|e| CaptureError::BackendUnavailable(e.to_string()))?;
                        let mut dupl_desc = Default::default();
                        dupl.GetDesc(&mut dupl_desc);
                        let tex_desc = D3D11_TEXTURE2D_DESC {
                            Width: dupl_desc.ModeDesc.Width,
                            Height: dupl_desc.ModeDesc.Height,
                            ..Default::default()
                        };
                        return Ok((dupl, tex_desc));
                    }
                }
            }
            Err(CaptureError::BackendUnavailable("未找到可复制的显示器输出".into()))
        }
    }

    pub async fn capture_monitor(&self) -> Result<CapturedImage, CaptureError> {
        let (device, context) = Self::create_d3d11_device()?;
        let (dupl, _desc) = Self::duplicate_cursor_output(&device)?;

        // AcquireNextFrame 首帧即全帧；带超时轮询。
        let mut acquired: Option<(ID3D11Texture2D, u32, u32)> = None;
        for _ in 0..20 {
            let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource = None;
            let hr = unsafe { dupl.AcquireNextFrame(50, &mut frame_info, &mut resource) };
            match hr {
                Ok(()) => {
                    let resource = resource
                        .ok_or_else(|| CaptureError::BackendUnavailable("DXGI 帧资源为空".into()))?;
                    let texture: ID3D11Texture2D = resource
                        .cast()
                        .map_err(|e| CaptureError::ImageConversionFailed(e.to_string()))?;
                    let mut tex_desc = D3D11_TEXTURE2D_DESC::default();
                    unsafe { texture.GetDesc(&mut tex_desc) };
                    acquired = Some((texture, tex_desc.Width, tex_desc.Height));
                    break;
                }
                Err(_) => {
                    unsafe { let _ = dupl.ReleaseFrame(); }
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }

        let (texture, width, height) = acquired.ok_or_else(|| {
            CaptureError::BackendUnavailable("未能在超时时间内获取桌面帧".into())
        })?;

        let result = Self::extract_bgra(&device, &context, &texture, width, height);
        unsafe { let _ = dupl.ReleaseFrame(); }
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
                .map_err(|e| CaptureError::ImageConversionFailed(e.to_string()))?;
            let _guard = MappedTextureGuard { context, texture: &staging };
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
                .map_err(|e| CaptureError::ImageConversionFailed(e.to_string()))?;
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
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        }
    }

    fn validate_mapped_surface(bits: *mut u8, row_pitch: usize, row_len: usize) -> Result<(), CaptureError> {
        if bits.is_null() {
            return Err(CaptureError::ImageConversionFailed("映射像素指针为空".into()));
        }
        if row_pitch < row_len {
            return Err(CaptureError::ImageConversionFailed("行跨度小于行字节数".into()));
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
```

> 实现者注意：`windows` 0.58 中 `AcquireNextFrame` / `EnumAdapters1` / `EnumOutputs` 的精确签名（可变引用 vs 返回值、`Option` 包裹）以本地 `cargo build` 报错为准微调，保持逻辑不变。`duplicate_cursor_output` 的「退而取第一个 output」分支若 borrow 复杂，可简化为：第一轮只找命中 monitor 的，找不到再单独取 (0,0) output。优先保证命中光标显示器。

- [ ] **步骤 4：编译 + 纯函数测试**

运行：`cd src-tauri && cargo build`
预期：编译通过（如签名不符按报错微调）。
运行：`cd src-tauri && cargo test --lib platform::windows::capture`
预期：PASS（纯函数测试）。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/platform/windows/capture.rs src-tauri/Cargo.toml
git commit -m "feat(capture): DXGI Desktop Duplication 抓光标显示器整屏帧"
```

---

## 任务 6：平台缝 `capture_screen` / `recognize_region`

**文件：**
- 修改：`src-tauri/src/platform/windows/mod.rs`
- 修改：`src-tauri/src/platform/unsupported.rs`
- 修改：`src-tauri/src/platform/mod.rs`

- [ ] **步骤 1：改 windows/mod.rs**

将 `src-tauri/src/platform/windows/mod.rs` 改为：

```rust
pub mod capture;
pub mod ocr;

use crate::core::{
    capture::{CaptureError, CapturedImage},
    ocr::OcrHints,
    ocr_translation::{recognize_cropped_for_translation, OcrTranslationError},
    translation::TranslationInput,
};
use capture::WindowsScreenCapture;
use ocr::WindowsOcrEngine;

/// 抓光标所在显示器整屏帧 + 该显示器 scale_factor。
pub async fn capture_screen() -> Result<CapturedImage, CaptureError> {
    WindowsScreenCapture::new().capture_monitor().await
}

/// 对已抓帧按物理像素矩形裁剪并 OCR。
pub async fn recognize_region(
    frame: &CapturedImage,
    region: (u32, u32, u32, u32),
    hints: OcrHints,
) -> Result<Option<TranslationInput>, OcrTranslationError> {
    recognize_cropped_for_translation(frame, region, &WindowsOcrEngine, hints).await
}
```

> 说明：旧 `capture_and_recognize` 删除（overlay 路径不再用「抓帧即识别」）。

- [ ] **步骤 2：改 unsupported.rs**

将 `src-tauri/src/platform/unsupported.rs` 改为：

```rust
use crate::core::{
    capture::{CaptureError, CapturedImage},
    ocr::OcrHints,
    ocr_translation::OcrTranslationError,
    translation::TranslationInput,
};

pub struct GraphicsCaptureProbe;

impl GraphicsCaptureProbe {
    pub fn is_supported() -> bool {
        false
    }
}

pub async fn capture_screen() -> Result<CapturedImage, CaptureError> {
    Err(CaptureError::UnsupportedPlatform)
}

pub async fn recognize_region(
    _frame: &CapturedImage,
    _region: (u32, u32, u32, u32),
    _hints: OcrHints,
) -> Result<Option<TranslationInput>, OcrTranslationError> {
    Err(OcrTranslationError::Capture(CaptureError::UnsupportedPlatform))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn capture_screen_unsupported_on_non_windows() {
        assert!(matches!(
            capture_screen().await,
            Err(CaptureError::UnsupportedPlatform)
        ));
    }
}
```

- [ ] **步骤 3：改 platform/mod.rs**

将 `src-tauri/src/platform/mod.rs` 改为：

```rust
#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(not(target_os = "windows"))]
pub mod unsupported;

pub use crate::core::ocr_translation::OcrTranslationError;

#[cfg(target_os = "windows")]
pub use windows::{capture_screen, recognize_region};

#[cfg(not(target_os = "windows"))]
pub use unsupported::{capture_screen, recognize_region};
```

- [ ] **步骤 4：编译**

运行：`cd src-tauri && cargo build`
预期：报错集中在 `ocr_popup.rs`（仍引用旧 `capture_and_recognize`）——下个任务修复。先确认 `platform` 模块自身编译无误：`cd src-tauri && cargo build 2>&1 | grep -i "platform"` 应无 platform 内部错误。

- [ ] **步骤 5：Commit**（与任务 7 一起提交，因 ocr_popup 此刻编译不过）

跳过独立 commit，进入任务 7。

---

## 任务 7：`ocr_popup` 事件驱动编排 + `overlay.rs` 建窗与 command

**文件：**
- 新增：`src-tauri/src/ui/overlay.rs`
- 修改：`src-tauri/src/ui/ocr_popup.rs`
- 修改：`src-tauri/src/ui/mod.rs`

- [ ] **步骤 1：写 overlay.rs（建窗 + 四个 command）**

新建 `src-tauri/src/ui/overlay.rs`：

```rust
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

use crate::{
    app::state::AppState,
    core::ocr::OcrHints,
    platform::recognize_region,
    ui::web_popup::{show_translation_error, start_translation_from_input},
};

pub const OVERLAY_LABEL: &str = "screenshot-overlay";

/// 在光标所在显示器上铺满建 overlay 窗口。整屏帧须已存入 AppState。
pub fn open_overlay(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = existing.close();
    }
    let window = WebviewWindowBuilder::new(app, OVERLAY_LABEL, WebviewUrl::App("overlay.html".into()))
        .title("Shizi 截图")
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .fullscreen(true)
        .build()
        .map_err(|e| e.to_string())?;
    let _ = window.set_focus();
    Ok(())
}

fn close_overlay(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = window.close();
    }
}

#[tauri::command]
pub async fn get_capture_frame_meta(
    state: tauri::State<'_, AppState>,
) -> Result<Option<(u32, u32, f64)>, String> {
    state.pending_capture_meta()
}

#[tauri::command]
pub async fn get_capture_frame_bytes(
    state: tauri::State<'_, AppState>,
) -> Result<tauri::ipc::Response, String> {
    let bytes = state.pending_capture_bytes()?.unwrap_or_default();
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
pub async fn cancel_capture(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let _ = state.take_pending_capture();
    close_overlay(&app);
    Ok(())
}

/// 前端回传 CSS 逻辑像素矩形（相对 overlay 左上）。
#[tauri::command]
pub async fn submit_capture_region(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    use crate::core::capture::css_rect_to_physical;

    close_overlay(&app);

    let Some((frame, scale)) = state.take_pending_capture()? else {
        return Ok(()); // 帧已被取消/消费，静默
    };
    let region = css_rect_to_physical(x, y, w, h, scale);
    if region.2 == 0 || region.3 == 0 {
        return Ok(()); // 选区过小，静默
    }

    let app_state = state.inner().clone();
    match recognize_region(&frame, region, OcrHints::default()).await {
        Ok(None) => {}
        Ok(Some(input)) => {
            if let Err(error) = start_translation_from_input(input, app.clone(), &app_state) {
                show_translation_error(&app, error);
            }
        }
        Err(error) => show_translation_error(&app, crate::ui::ocr_popup::friendly_ocr_error(error)),
    }
    Ok(())
}
```

> 实现者注意：`WebviewWindowBuilder` 在 Tauri 2 中需 `tauri::Manager` 在作用域；`fullscreen(true)` 在多显示器下默认主屏，MVP 可接受（与 `capture_monitor` 的「光标显示器」可能在多屏下不一致，记为已知限制，见末尾风险）。若需精确定位光标显示器，后续用 `.position()/.inner_size()` 按 monitor 设。

- [ ] **步骤 2：改 ocr_popup.rs（抓帧 + 建 overlay；friendly_ocr_error 改 pub）**

将 `src-tauri/src/ui/ocr_popup.rs` 顶部 use 与 `start_translation_from_ocr` 改为：

```rust
use crate::{
    app::state::AppState,
    core::{capture::CaptureError, ocr::OcrError, ocr_translation::OcrTranslationError},
    platform::capture_screen,
    ui::{overlay::open_overlay, web_popup::show_translation_error},
};

use crate::app::window::show_window;

pub async fn start_translation_from_ocr(app: tauri::AppHandle, state: AppState) {
    if state.is_translation_busy() {
        show_translation_error(&app, "正在翻译中，请稍后再试");
        return;
    }

    // 先抓整屏帧（overlay 显示前拍完，避免把 overlay 截进图里）
    let frame = match capture_screen().await {
        Ok(frame) => frame,
        Err(error) => {
            show_translation_error(&app, friendly_ocr_error(OcrTranslationError::Capture(error)));
            return;
        }
    };

    // scale_factor 取主窗口缩放（MVP 简化；多屏精确缩放留后续）
    let scale = app
        .get_webview_window("main")
        .and_then(|w| w.scale_factor().ok())
        .unwrap_or(1.0);

    if let Err(error) = state.set_pending_capture(frame, scale) {
        show_translation_error(&app, error);
        return;
    }

    // 不显示主窗口；overlay 自身承载交互
    let _ = show_window; // 保留 import 兼容；overlay 不需要主窗口可见
    if let Err(error) = open_overlay(&app) {
        let _ = state.take_pending_capture();
        show_translation_error(&app, format!("无法打开截图窗口：{error}"));
    }
}
```

并将原 `fn friendly_ocr_error` 的可见性改为 `pub fn friendly_ocr_error`（供 overlay.rs 调用），函数体不变。

> 清理：删除顶部对 `tauri::Manager` / `get_webview_window` 中 owner_hwnd 相关旧代码、删除对 `capture_and_recognize` 的引用。`show_window` 若不再需要可一并删掉其 import（上面的 `let _ = show_window;` 仅为说明，实现时直接移除未用 import 更干净）。

- [ ] **步骤 3：改 ui/mod.rs**

在 `src-tauri/src/ui/mod.rs` 加：

```rust
pub mod overlay;
```

- [ ] **步骤 4：编译 + 测试**

运行：`cd src-tauri && cargo build`
预期：编译通过。
运行：`cd src-tauri && cargo test --lib ui::ocr_popup`
预期：现有 `friendly_ocr_error` 测试仍 PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/ui/overlay.rs src-tauri/src/ui/ocr_popup.rs src-tauri/src/ui/mod.rs src-tauri/src/platform/
git commit -m "feat(ocr): overlay 事件驱动编排，抓帧后建框选窗口"
```

---

## 任务 8：注册 command + capabilities

**文件：**
- 修改：`src-tauri/src/lib.rs`
- 修改：`src-tauri/capabilities/default.json`

- [ ] **步骤 1：改 lib.rs**

`src-tauri/src/lib.rs` 顶部 use 加：

```rust
use ui::overlay::{cancel_capture, get_capture_frame_bytes, get_capture_frame_meta, submit_capture_region};
```

`invoke_handler` 的 `generate_handler!` 列表加四项：

```rust
        .invoke_handler(tauri::generate_handler![
            start_translation,
            take_pending_source_text,
            get_app_config,
            save_app_config,
            get_capture_frame_meta,
            get_capture_frame_bytes,
            submit_capture_region,
            cancel_capture,
        ])
```

- [ ] **步骤 2：改 capabilities**

`src-tauri/capabilities/default.json` 的 `windows` 改为含 overlay 窗口（command 权限由 core:default 覆盖本地 invoke，窗口需在 capability 作用域内）：

```json
{
  "identifier": "default",
  "description": "Default capabilities",
  "windows": ["main", "screenshot-overlay"],
  "permissions": [
    "core:default",
    "global-shortcut:default"
  ]
}
```

- [ ] **步骤 3：编译**

运行：`cd src-tauri && cargo build`
预期：编译通过。

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/capabilities/default.json
git commit -m "feat(ocr): 注册 overlay 四命令并授权 overlay 窗口"
```

---

## 任务 9：`overlay.html` 前端（canvas 框选）

**文件：**
- 新增：`frontend/overlay.html`

- [ ] **步骤 1：写 overlay.html**

新建 `frontend/overlay.html`：

```html
<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <title>Shizi 截图</title>
  <style>
    html, body { margin: 0; padding: 0; overflow: hidden; cursor: crosshair; user-select: none; }
    #frame { position: fixed; top: 0; left: 0; }
    #mask { position: fixed; top: 0; left: 0; right: 0; bottom: 0; background: rgba(0,0,0,0.35); }
    #sel { position: fixed; border: 1px solid #38bdf8; background: rgba(56,189,248,0.12); display: none; }
  </style>
</head>
<body>
  <canvas id="frame"></canvas>
  <div id="mask"></div>
  <div id="sel"></div>
  <script>
    const invoke = window.__TAURI__.core.invoke;
    const canvas = document.getElementById('frame');
    const sel = document.getElementById('sel');
    let downX = 0, downY = 0, moving = false, down = false;

    async function init() {
      const meta = await invoke('get_capture_frame_meta');
      if (!meta) { await invoke('cancel_capture'); return; }
      const [w, h, scale] = meta;
      const buf = await invoke('get_capture_frame_bytes'); // ArrayBuffer，BGRA
      const bgra = new Uint8Array(buf);
      const rgba = new Uint8ClampedArray(w * h * 4);
      for (let i = 0; i < w * h; i++) {
        rgba[i*4]   = bgra[i*4+2]; // R <- B
        rgba[i*4+1] = bgra[i*4+1]; // G
        rgba[i*4+2] = bgra[i*4];   // B <- R
        rgba[i*4+3] = bgra[i*4+3]; // A
      }
      canvas.width = w; canvas.height = h;            // 物理像素
      canvas.style.width = (w / scale) + 'px';        // CSS 逻辑像素
      canvas.style.height = (h / scale) + 'px';
      canvas.getContext('2d').putImageData(new ImageData(rgba, w, h), 0, 0);
    }

    function updateSel(x1, y1, x2, y2) {
      const left = Math.min(x1, x2), top = Math.min(y1, y2);
      const width = Math.abs(x2 - x1), height = Math.abs(y2 - y1);
      sel.style.display = 'block';
      sel.style.left = left + 'px';
      sel.style.top = top + 'px';
      sel.style.width = width + 'px';
      sel.style.height = height + 'px';
    }

    window.addEventListener('mousedown', (e) => {
      if (e.button !== 0) { invoke('cancel_capture'); return; } // 右键/中键取消
      down = true; moving = false; downX = e.clientX; downY = e.clientY;
    });
    window.addEventListener('mousemove', (e) => {
      if (!down) return;
      moving = true; updateSel(downX, downY, e.clientX, e.clientY);
    });
    window.addEventListener('mouseup', (e) => {
      if (!down) return;
      down = false;
      const x = Math.min(downX, e.clientX), y = Math.min(downY, e.clientY);
      const w = Math.abs(e.clientX - downX), h = Math.abs(e.clientY - downY);
      if (!moving || w < 3 || h < 3) { invoke('cancel_capture'); return; }
      invoke('submit_capture_region', { x, y, w, h });
    });
    window.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') invoke('cancel_capture');
    });

    init().catch(() => invoke('cancel_capture'));
  </script>
</body>
</html>
```

> 说明：`get_capture_frame_bytes` 返回 `tauri::ipc::Response`，前端 `invoke` 解析为 `ArrayBuffer`。canvas 物理像素 = 帧尺寸，CSS 尺寸 = 逻辑尺寸（÷scale），框选坐标用 `clientX/Y`（CSS 逻辑像素），与 Rust 侧 `css_rect_to_physical` 一致。

- [ ] **步骤 2：前端语法检查**

运行：`node --check frontend/overlay.html` 不适用（HTML 非 JS）。改为人工核对脚本块；可选用 `node -e "require('fs').readFileSync('frontend/overlay.html','utf8')"` 确认文件可读。

- [ ] **步骤 3：Commit**

```bash
git add frontend/overlay.html
git commit -m "feat(ocr): overlay 前端 canvas 框选与 BGRA 渲染"
```

---

## 任务 10：整体编译 + 全量测试 + 人工验证

**文件：** 无（验证）

- [ ] **步骤 1：全量编译与测试**

运行：`cd src-tauri && cargo build`
预期：PASS。
运行：`cd src-tauri && cargo test`
预期：所有单元测试 PASS（含任务 1-4 新增）。

- [ ] **步骤 2：人工验证（Windows，`npm run tauri dev`）**

逐项确认：
- `Alt+O` → 整屏冻结画面铺满，cursor 为十字。
- 拖矩形 → 蓝色选区跟随；遮罩压暗框外。
- 松开 → overlay 关闭 → 主窗口弹出 → 中/英/中英混合文本进入翻译。
- Esc / 右键 → overlay 关闭，无翻译、无报错。
- 选区过小（<3px）→ 静默关闭，无报错。
- 翻译进行中再按 `Alt+O` → 提示「正在翻译中」。
- `Alt+T` 划词翻译不回归。

- [ ] **步骤 3：可选集成测试**

运行：`cd src-tauri && cargo test --lib capture_monitor_returns_bgra_frame -- --ignored`
预期：在有桌面会话的机器上 PASS。

- [ ] **步骤 4：同步文档**

更新 `docs/architecture/screenshot-ocr-architecture.md` 末尾追加「自建 overlay 区域框选落地状态」小节，简述 DXGI + crop + overlay 链路（与 CLAUDE.md「文档同步时机」一致）。

- [ ] **步骤 5：Commit**

```bash
git add docs/architecture/screenshot-ocr-architecture.md
git commit -m "docs(ocr): 记录自建 overlay 区域框选落地状态"
```

---

## 风险与已知限制

- **多显示器**：`capture_monitor` 按光标定位 Output，但 `WebviewWindowBuilder.fullscreen(true)` 默认建在主屏 + `scale_factor` 取主窗口缩放。光标在副屏时可能错位。MVP 仅保证主屏正确；多屏精确定位（按 monitor `.position()/.inner_size()/.scale_factor()` 建窗）留后续切片。
- **windows 0.58 API 签名**：DXGI `AcquireNextFrame`/`EnumAdapters1`/`EnumOutputs`/`GetDesc` 的可变引用与返回值形态以本地 `cargo build` 为准微调，逻辑不变。
- **DXGI 失败场景**：锁屏/屏保/安全桌面/远程会话下 `DuplicateOutput` 可能失败，统一落 `BackendUnavailable` → 「截图失败，请稍后重试」。
- **scale_factor 来源**：MVP 用主窗口缩放近似目标显示器缩放，单屏一致；混合 DPI 多屏不准，记为已知限制。

---

## 自检结论

- **规格覆盖度**：§1 DXGI 后端→任务5；§2 crop→任务1，capture_region→任务5；§3 overlay 窗口→任务7；§4 overlay 前端→任务9；§5 编排重构→任务7；§6 DPI→任务2；§7 错误/取消→任务7（friendly_ocr_error 复用 + 取消静默）；§8 测试→任务1-4 单测 + 任务10 集成；§9 受影响文件全覆盖。
- **占位符扫描**：无 TODO/待定；unsafe DXGI 部分给出完整代码 + 「以编译为准微调」属合理工程注记，非占位。
- **类型一致性**：`CapturedImage::crop(x,y,w,h)->Result<CapturedImage,CaptureError>`、`css_rect_to_physical(...)->(u32,u32,u32,u32)`、`recognize_cropped_for_translation(&frame,region,&ocr,hints)`、`capture_screen()`/`recognize_region(&frame,region,hints)`、`AppState::{set_pending_capture,pending_capture_meta,pending_capture_bytes,take_pending_capture}`、四个 command 名贯穿任务7/8/9 一致。
