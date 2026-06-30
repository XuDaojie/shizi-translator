# 全屏单帧截图 Spike 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 通过 `Windows.Graphics.Capture` 获取一帧全屏像素，转换为 `CapturedImage { format: Bgra8, .. }`，不接 OCR / 快捷键 / 翻译编排。

**架构：** 扩展 `src-tauri/src/platform/windows/capture.rs`，新增 `WindowsScreenCapture::capture_full_screen()`。流程为 `GraphicsCapturePicker` 选显示器 → D3D11 设备桥接为 `IDirect3DDevice` → `Direct3D11CaptureFramePool::CreateFreeThreaded` → `TryGetNextFrame` → 从 `IDXGISurface` 读 BGRA 像素 → `CapturedImage`。

**技术栈：** Rust 2021、Tauri 2、`windows` crate 0.58（D3D11 / DXGI / Graphics Capture / Direct3D11 互操作）、`tokio` 测试运行时。

---

## 风险与处理约定

本计划的核心风险是 **D3D11 设备桥接** 和 **IDXGISurface 像素读取**，windows 0.58 的确切 feature 名与 API 签名存在不确定性。处理约定：

- 实现者必须按实际编译错误和 windows-rs 文档调整 feature 名与 API 调用，不要照搬计划中的 feature 名猜测。
- 如果某个 API 签名在 windows 0.58 中不存在或语义不明，实现者应返回 `BLOCKED` 并附上具体错误，不要硬改或 `unsafe` 绕过。
- 所有 D3D11 / DXGI / surface 调用必须处理 `Result`，错误映射为 `CaptureError`，不允许 `unwrap()` / `expect()` 进入生产路径。

---

## 文件结构

### 修改文件

- `src-tauri/Cargo.toml`
  - 扩展 Windows 专用 `windows` crate features，加入 D3D11、DXGI、Direct3D11 互操作所需 feature。

- `src-tauri/src/platform/windows/capture.rs`
  - 在现有 `WindowsGraphicsCaptureProbe` 基础上新增 `WindowsScreenCapture` 及其截图链路。
  - 新增默认忽略的 Windows 集成测试。

### 不修改

- `src-tauri/src/core/capture/mod.rs`：`CapturedImage` / `CaptureError` / `ScreenCapture` 已定义，本切片不改 core 抽象。
- `src-tauri/src/platform/windows/ocr.rs`：OCR engine 已完成，本切片不接 OCR。
- 前端、快捷键、翻译编排：本切片不改。

---

## 任务 1：扩展 windows crate D3D11 互操作 features

**文件：**
- 修改：`src-tauri/Cargo.toml`

- [ ] **步骤 1：扩展 Windows 依赖 features**

将 `src-tauri/Cargo.toml` 中 `[target.'cfg(windows)'.dependencies]` 的 `windows` 改为：

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
  "Foundation",
  "Foundation_Collections",
  "Globalization",
  "Graphics_Capture",
  "Graphics_DirectX",
  "Graphics_DirectX_Direct3D11",
  "Graphics_Imaging",
  "Media_Ocr",
  "Storage_Streams",
  "Win32_Graphics_Direct3D11",
  "Win32_Graphics_Dxgi",
  "Win32_System_WinRT",
  "Win32_System_WinRT_Direct3D11Interop",
] }
```

如果某个 feature 名在 windows 0.58 中解析失败，按 cargo 报错调整：尝试 `Win32_System_WinRT_Direct3D11` 或 `Win32_System_WinRT_Graphics_DirectX` 等相邻命名；目标是用 `cargo check` 能解析出 `CreateDirect3D11DeviceFromDXGIDevice`、`ID3D11Device`、`IDXGISurface`、`D3D11CreateDevice`。

- [ ] **步骤 2：运行依赖解析验证**

运行：

```bash
cd src-tauri && cargo check
```

预期：依赖解析成功，`Cargo.lock` 按需更新，现有代码仍能编译。允许现有阶段性 `dead_code` warnings。

- [ ] **步骤 3：Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock

git commit -m "$(cat <<'EOF'
chore(capture): 扩展 D3D11 互操作依赖能力

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## 任务 2：新增 WindowsScreenCapture 骨架与 is_supported

**文件：**
- 修改：`src-tauri/src/platform/windows/capture.rs`

- [ ] **步骤 1：编写失败测试**

在 `src-tauri/src/platform/windows/capture.rs` 的 tests 模块中增加：

```rust
#[test]
fn screen_capture_is_supported_returns_boolean() {
    let _supported: bool = WindowsScreenCapture::is_supported();
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
cd src-tauri && cargo test screen_capture_is_supported
```

预期：编译失败，报错包含 `cannot find type WindowsScreenCapture`。

- [ ] **步骤 3：实现 WindowsScreenCapture 骨架**

在 `src-tauri/src/platform/windows/capture.rs` 顶部新增 import 与结构：

```rust
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
```

保留原有 `WindowsGraphicsCaptureProbe` 不变。

- [ ] **步骤 4：运行测试验证通过**

运行：

```bash
cd src-tauri && cargo test screen_capture_is_supported
```

预期：测试通过。

- [ ] **步骤 5：运行完整测试**

运行：

```bash
cd src-tauri && cargo test
```

预期：所有测试通过。允许阶段性 `dead_code` warnings。

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/platform/windows/capture.rs

git commit -m "$(cat <<'EOF'
feat(capture): 添加 WindowsScreenCapture 骨架

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## 任务 3：实现 D3D11 设备桥接

**文件：**
- 修改：`src-tauri/src/platform/windows/capture.rs`

- [ ] **步骤 1：编写失败测试**

在 tests 模块中增加：

```rust
#[test]
fn create_direct3d_device_returns_device_or_error() {
    let _ = WindowsScreenCapture::create_direct3d_device();
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
cd src-tauri && cargo test create_direct3d_device
```

预期：编译失败，报错包含 `no function create_direct3d_device`。

- [ ] **步骤 3：实现 D3D11 设备桥接**

在 `src-tauri/src/platform/windows/capture.rs` 中实现一个 `pub(crate)` 关联函数（便于测试调用）。目标：创建 `ID3D11Device`，桥接为 WinRT `IDirect3DDevice`。

参考实现骨架（实现者必须按 windows 0.58 实际 API 与编译错误调整，不要照搬）：

```rust
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
};
use windows::Win32::Graphics::Dxgi::{IDXGIDevice, IDXGISurface};
use windows::Win32::System::WinRT::Direct3D11::CreateDirect3D11DeviceFromDXGIDevice;

impl WindowsScreenCapture {
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
}
```

注意事项：

- `D3D11CreateDevice` 的参数顺序与 `D3D11_CREATE_DEVICE_BGRA_SUPPORT` flag 常量名以 windows 0.58 实际签名为准；若签名不同（例如 `flFeatureLevels` / `sdk_version` 形参），按实际调整。
- `CreateDirect3D11DeviceFromDXGIDevice` 返回 `IInspectable`，需 `.cast::<IDirect3DDevice>()`。
- 若 `Win32::System::WinRT::Direct3D11` 路径不存在，尝试 `Win32::System::WinRT::Graphics::DirectX::Direct3D11` 等相邻路径；目标是用 `cargo check` 解析出该函数。
- 若任何 API 在 windows 0.58 中确实不存在或签名无法确定，返回 `BLOCKED` 并附完整编译错误，不要 `unsafe` 猜测。

- [ ] **步骤 4：运行测试验证通过**

运行：

```bash
cd src-tauri && cargo test create_direct3d_device
```

预期：测试通过；函数能在 Windows 上创建设备（CI 无显卡也可通过，仅验证可调用）。

- [ ] **步骤 5：运行完整测试与构建**

运行：

```bash
cd src-tauri && cargo test
cd src-tauri && cargo build
```

预期：测试和构建通过。允许阶段性 `dead_code` warnings。

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/platform/windows/capture.rs

git commit -m "$(cat <<'EOF'
feat(capture): 实现 D3D11 设备桥接

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## 任务 4：实现 GraphicsCapturePicker 选择

**文件：**
- 修改：`src-tauri/src/platform/windows/capture.rs`

- [ ] **步骤 1：编写失败测试**

在 tests 模块中增加（不实际弹窗，只验证函数存在且签名正确）：

```rust
#[tokio::test]
#[ignore]
async fn pick_capture_item_can_be_invoked() {
    let _ = WindowsScreenCapture::pick_capture_item().await;
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
cd src-tauri && cargo test pick_capture_item
```

预期：编译失败，报错包含 `no function pick_capture_item` 或 `no associated function`。

- [ ] **步骤 3：实现 picker 选择**

在 `src-tauri/src/platform/windows/capture.rs` 中实现。`GraphicsCapturePicker::new()` + `PickSingleItemAsync()`，用 `.get()` 阻塞等待结果，用户取消返回 `Ok(None)`。

```rust
use windows::Foundation::IAsyncOperation;
use windows::Graphics::Capture::{GraphicsCaptureItem, GraphicsCapturePicker};

impl WindowsScreenCapture {
    pub(crate) async fn pick_capture_item() -> Result<Option<GraphicsCaptureItem>, CaptureError> {
        let picker = GraphicsCapturePicker::new()
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;
        let operation: IAsyncOperation<GraphicsCaptureItem> = picker
            .PickSingleItemAsync()
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;
        let item = operation
            .get()
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;
        Ok(item)
    }
}
```

注意：

- `PickSingleItemAsync` 在桌面应用中未关联 owner window 时可能失败；失败按 `BackendUnavailable` 映射，不在本切片做窗口句柄管理。
- 用户取消时 `operation.get()` 返回 `Ok(None)`。

- [ ] **步骤 4：运行测试验证通过**

运行：

```bash
cd src-tauri && cargo test pick_capture_item
```

预期：编译通过，默认 ignored 测试不执行。

- [ ] **步骤 5：运行完整测试与构建**

运行：

```bash
cd src-tauri && cargo test
cd src-tauri && cargo build
```

预期：测试和构建通过。允许阶段性 `dead_code` warnings。

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/platform/windows/capture.rs

git commit -m "$(cat <<'EOF'
feat(capture): 实现 GraphicsCapturePicker 选择

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## 任务 5：实现帧捕获与 BGRA 像素读取

**文件：**
- 修改：`src-tauri/src/platform/windows/capture.rs`

- [ ] **步骤 1：编写失败测试**

在 tests 模块中增加（不实际捕获，只验证函数存在）：

```rust
#[tokio::test]
#[ignore]
async fn capture_full_screen_can_be_invoked() {
    let _ = WindowsScreenCapture.capture_full_screen().await;
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
cd src-tauri && cargo test capture_full_screen
```

预期：编译失败或 `capture_full_screen` 仍返回 `Err(UnsupportedPlatform)`（任务 2 的占位实现）。若是后者，测试 ignored 不执行，需在步骤 3 替换实现后才能通过构建。

- [ ] **步骤 3：实现 capture_full_screen 与像素读取**

替换任务 2 中的 `capture_full_screen` 占位实现，并新增 `extract_bgra_from_frame` 私有函数。

```rust
use windows::Graphics::Capture::{
    Direct3D11CaptureFramePool, DirectXPixelFormat, GraphicsCaptureSession, SizeInt32,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DSurface;
use windows::Win32::Graphics::Dxgi::{IDXGISurface, DXGI_MAPPED_RECT, DXGI_MAP_READ};

impl WindowsScreenCapture {
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
            DirectXPixelFormat::Bgra8,
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

        let frame = pool
            .TryGetNextFrame()
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;
        let image = Self::extract_bgra_from_frame(frame, size)?;

        let _ = session.Close();
        let _ = pool.Close();
        Ok(Some(image))
    }

    fn extract_bgra_from_frame(
        frame: windows::Graphics::Capture::Direct3D11CaptureFrame,
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
        let mut bytes = Vec::with_capacity((width * height * 4) as usize);

        unsafe {
            let mut mapped: DXGI_MAPPED_RECT = std::mem::zeroed();
            dxgi_surface
                .Map(&mut mapped, DXGI_MAP_READ)
                .map_err(|error| CaptureError::ImageConversionFailed(error.to_string()))?;
            let row_pitch = mapped.Pitch as usize;
            for row in 0..height as usize {
                let src = mapped.pBits.add(row * row_pitch);
                bytes.extend_from_slice(std::slice::from_raw_parts(src, (width as usize) * 4));
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
```

注意事项：

- `Direct3D11CaptureFramePool::CreateFreeThreaded` 参数顺序为 `device, pixelFormat, numberOfBuffers, size`，以 windows 0.58 实际签名为准。
- `IDXGISurface::Map` / `Unmap` 的签名以实际为准；`DXGI_MAPPED_RECT` 的 `pBits` / `Pitch` 字段名以实际为准。
- `frame.Surface()` 返回 `IDirect3DSurface`，需 `cast` 到 `IDXGISurface`。
- 若 frame 内容尺寸与 item size 不一致，以 frame 实际尺寸优先；本切片暂用 item size，若测试发现不一致再调整。
- 若任何 API 签名无法确定，返回 `BLOCKED` 并附编译错误，不要 `unsafe` 猜测。

- [ ] **步骤 4：运行测试验证通过**

运行：

```bash
cd src-tauri && cargo test capture_full_screen
```

预期：编译通过，默认 ignored 测试不执行。

- [ ] **步骤 5：运行完整测试与构建**

运行：

```bash
cd src-tauri && cargo test
cd src-tauri && cargo build
```

预期：测试和构建通过。允许阶段性 `dead_code` warnings。

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/platform/windows/capture.rs

git commit -m "$(cat <<'EOF'
feat(capture): 实现全屏单帧捕获与 BGRA 读取

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## 任务 6：添加默认忽略的端到端集成测试与最终验证

**文件：**
- 修改：`src-tauri/src/platform/windows/capture.rs`

- [ ] **步骤 1：编写默认忽略集成测试**

在 tests 模块中增加（与任务 5 的 `capture_full_screen_can_be_invoked` 合并或并存；此处要求断言结果形状）：

```rust
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

    assert_eq!(image.format, crate::core::capture::CapturedImageFormat::Bgra8);
    assert_eq!(image.bytes.len(), (image.width * image.height * 4) as usize);
}
```

- [ ] **步骤 2：运行默认测试验证 ignored 测试不执行**

运行：

```bash
cd src-tauri && cargo test capture_full_screen_returns_bgra_image_when_user_picks_display
```

预期：默认 ignored，不执行，0 passed 0 failed（或显示 ignored）。

- [ ] **步骤 3：运行最终验证**

运行：

```bash
cd src-tauri && cargo test
cd src-tauri && cargo build
node --check frontend/main.js
git status --short
```

预期：

- `cargo test` 通过，ignored 测试不计入失败。
- `cargo build` 通过。
- `node --check frontend/main.js` 无输出且退出码为 0。
- `git status --short` 工作区干净（除可能的 `.claude/worktrees/`，已 gitignore）。

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/platform/windows/capture.rs

git commit -m "$(cat <<'EOF'
test(capture): 添加全屏单帧截图集成验证

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## 自检清单

- [ ] 规格覆盖度：计划覆盖 `is_supported`、`capture_full_screen`、D3D11 设备桥接、picker 选择、帧捕获与 BGRA 读取、错误映射、默认忽略集成测试、最终验证。
- [ ] 范围控制：计划不实现 `capture_region`、`ScreenCapture` trait、OCR 接入、快捷键、UI 状态。
- [ ] 类型一致性：`WindowsScreenCapture`、`CapturedImage`、`CaptureError`、`CapturedImageFormat::Bgra8` 命名与现有代码一致。
- [ ] 风险处理：D3D11 桥接与 surface 读取显式标注按实际 API 调整，遇不确定返回 BLOCKED。
- [ ] 验证闭环：每个代码任务都有失败测试、通过测试和 commit 步骤；集成测试默认 ignored。
