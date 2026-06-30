# 自建 Overlay 区域框选 截图 OCR 设计规格

## 背景与目标

Shizi 截图 OCR 端到端闭环已通（master, ab2c345）：`Alt+O` → `GraphicsCapturePicker` 全屏单帧 → `WindowsOcrEngine` → `TranslationInput::OcrText` → `start_translation_from_input` → `translation:event`。

当前痛点：

- 每次弹系统 picker 让用户选显示器，不是 Bob/Pot 体验。
- `GraphicsCapture` 会显示捕获指示边框。
- `capture_region` 返回 `UnsupportedPlatform`，未实现。
- 用户取消 picker 当前误报「截图失败」（`BackendUnavailable` 而非 `Ok(None)`）。

本规格目标：把截图 OCR 从「系统 picker 全屏单帧」演进到「自建 overlay 区域框选」，接近 Bob/Pot 的框选体验，复用现有 D3D11 帧提取、`WindowsOcrEngine`、翻译链路。

## 关键决策

1. **后端路线：DXGI Desktop Duplication（路线 B）。**
   - 直接复制桌面输出，无系统 picker、无授权弹窗、无捕获边框、能抓 DComp/加速内容。
   - 单帧采集复杂度可控：首帧即全帧，dirty/move rect 只对连续流有意义，单帧不涉及。
   - 复用现有 D3D11 staging 提取代码（`copy_texture_to_staging` / `bgra_buffer_layout` / `MappedTextureGuard`），去掉 WinRT `IDirect3DDevice` 桥 + frame pool + picker。
   - 多显示器先收口为「光标所在单显示器」。
   - GraphicsCapture `CreateForMonitor`（路线 A′）留作 DXGI 在某些会话失败时的兜底备选，**不在 MVP 实现**。

2. **两段式截图 UX（借鉴 Pot，不照搬代码）：先抓整屏静态图 → 在静态图上拖矩形。**
   - 快照在 overlay 显示前已拍完，overlay 自身不会被截进图里，框选稳定无残影。

3. **裁剪在 Rust 内存完成，不走磁盘 PNG 接力。**
   - `CapturedImage` 已是内存 BGRA bytes，直接按矩形裁剪，免去 PNG 编解码 + 落盘 + `convertFileSrc` + `image` crate 依赖。

4. **编排留 Rust。** overlay 前端只回传原始 CSS 矩形，DPI 换算与裁剪在 Rust。

## 非目标

- 多显示器虚拟桌面拼接、跨屏框选。
- GraphicsCapture 兜底（路线 A′）。
- overlay 的多选区、编辑手柄、放大镜。
- 截图历史记录、重试、usage 统计。
- macOS / Linux 平台（沿用 `UnsupportedPlatform`）。

MVP 只覆盖：光标所在单显示器、单矩形框选、Esc / 右键取消。

## 架构与数据流

```text
Alt+O
  -> app/shortcuts 分流 OCR
  -> ui/ocr_popup::start_translation_from_ocr
     1. busy 预检（沿用）
     2. WindowsScreenCapture::capture_monitor(cursor_monitor) 抓整屏 BGRA 帧存入 AppState（含 scale_factor）
     3. 建 screenshot-overlay 窗口
  -> overlay.html 加载 -> invoke('get_capture_frame') 拿整屏 PNG data URL 显示
  -> 用户拖矩形
     - mouseup -> invoke('submit_capture_region', {x,y,w,h})  // CSS 逻辑像素
     - Esc/右键 -> invoke('cancel_capture')
  -> Rust 监听 submit_capture_region：按 scale_factor 换算物理像素 -> CapturedImage::crop -> recognize -> start_translation_from_input（沿用）-> 关 overlay
  -> Rust 监听 cancel_capture：丢帧、关 overlay、静默
```

禁止路线（与既有架构一致）：

```text
OCR -> frontend -> frontend calls start_translation
```

## 组件设计

### §1 整屏快照后端（DXGI）

新增 `WindowsScreenCapture::capture_monitor(monitor) -> Result<CapturedImage, CaptureError>`：

- 枚举 DXGI Output，选光标所在显示器（`GetCursorPos` + Output 描述比对，MVP 简化为取主显示器或光标所在 Output）。
- `DuplicateOutput` → `AcquireNextFrame`（首帧即全帧，带超时，沿用现有 `capture_full_screen` 的 20×50ms 轮询模式）。
- 复用 `copy_texture_to_staging` + `Map` + `bgra_buffer_layout` + `MappedTextureGuard` 读 BGRA。

重构：

- 拆出 `create_d3d11_device() -> ID3D11Device` 共用；DXGI 路径不需要 WinRT `IDirect3DDevice` 桥。
- 删除 `pick_capture_item` / `capture_full_screen` / GraphicsCapturePicker + `IInitializeWithWindow` + `owner_hwnd` 字段。
- `WindowsScreenCapture::new` 不再需要 owner 句柄（overlay 由独立窗口承载）。
- `WindowsGraphicsCaptureProbe` 保留（探针仍有诊断价值）。

失败映射：

- `DuplicateOutput` / `AcquireNextFrame` 失败 → `BackendUnavailable`。
- 锁屏 / 屏保 / 非交互会话失败自然落到此分支，前端文案复用现有「截图失败」。

### §2 区域裁剪（核心层，纯 Rust）

`core/capture/mod.rs` 新增：

```rust
impl CapturedImage {
    /// 按 BGRA 行切片裁剪。x/y/w/h 为物理像素，越界返回 ImageConversionFailed。
    pub fn crop(&self, x: u32, y: u32, w: u32, h: u32) -> Result<CapturedImage, CaptureError>;
}
```

- 纯 BGRA 行切片，按 `width * 4` 跳行，无平台依赖，**可单测**。
- `ScreenCapture::capture_region` 在 Windows 上实现为 `capture_monitor` + `crop`，让 trait 完整。
- overlay 路径：先 `capture_monitor` 拿整屏给 overlay 显示，框选后对**同一帧内存** `crop`，不重新采集。

### §3 overlay 窗口（UI 层）

运行时建窗（`ui/overlay.rs`）：

- label `screenshot-overlay`。
- 覆盖目标显示器：位置/尺寸取自该显示器物理尺寸，`always_on_top` + `decorations=false` + `skip_taskbar` + `resizable=false`。
- 不透明：窗口显示整屏截图，框外用半透明遮罩 div 压暗，无需窗口级透明。
- 加载 `frontend/overlay.html`。
- 与 `capture_monitor` 选同一台显示器（光标所在）。

### §4 overlay 前端（原生静态 HTML/JS，无构建）

`frontend/overlay.html` + 内联 JS：

- `<img>` 铺满整屏快照。
- 鼠标拖拽画矩形（`cursor-crosshair`），框外半透明遮罩。
- 交互：左键拖拽选区；mouseup 提交；**Esc / 右键取消**（补 Pot 缺失的 Esc）。
- 通信：
  - 加载时 `invoke('get_capture_frame')` 拿整屏图（PNG data URL，Rust 侧编码一次）。
  - mouseup → `invoke('submit_capture_region', {x, y, w, h})`（CSS 逻辑像素，相对 overlay 窗口左上）。
  - Esc / 右键 → `invoke('cancel_capture')`。

### §5 编排重构（Rust 入口）

`start_translation_from_ocr` 改为事件驱动：

1. busy 预检（沿用 `state.is_translation_busy()`）。
2. `capture_monitor` 抓整屏 → 存入 `AppState`（带显示器 `scale_factor`）。
3. 建 overlay 窗口。
4. 监听 `submit_capture_region`：按 `scale_factor` 把 CSS 矩形换算物理像素 → `crop` → `recognize` → `start_translation_from_input`（沿用）→ 关 overlay。
5. 监听 `cancel_capture`：丢帧、关 overlay、静默（沿用 `Ok(None)` 语义）。

`recognize_capture_for_translation` 的 `capture_interactive` 内联链路被替换；`FakeCapture` 改为提供整屏帧 + `crop`，编排测试仍可纯 Rust 跑。

### §6 坐标 / DPI

- `scale_factor = 物理像素 / 逻辑像素`，由目标显示器决定，随帧一起存入 `AppState`。
- Rust 侧换算：`img_x = css_x * scale_factor`，与 `CaptureRegion.scale_factor` 字段对齐。
- 换算逻辑单测（1.0x / 1.5x / 2.0x、取整边界）。

### §7 错误与状态

- 截图失败、OCR 失败、空文本：沿用现有 `translation:event::Failed` 文案映射，不新增事件类型。
- 用户取消：静默，不弹失败（修复当前 picker 取消误报「截图失败」的已知简化）。
- busy：沿用现有「正在翻译中，请稍后再试」。

### §8 测试策略

单元测试（纯 Rust，无平台依赖）：

- `CapturedImage::crop`：行切片正确性、越界拒绝、1px 边界、`width * 4` 溢出保护。
- DPI 换算：1.0x / 1.5x / 2.0x、向下取整边界。
- 编排：`FakeCapture` 给整屏帧 + `crop`，验证
  - 提交矩形 → OCR 输入文本正确；
  - 取消 → `None`；
  - 空文本 → `EmptyResult`；
  - busy → 不进入截图。

集成测试（`#[ignore]`，人工）：

- 真实 DXGI 抓光标所在显示器 → overlay 框选 → 中英混合 OCR → 翻译展示。
- 现有 `Alt+T` 划词翻译不回归。

### §9 受影响文件

改：

- `src-tauri/src/platform/windows/capture.rs`（DXGI 后端、删除 picker/owner）
- `src-tauri/src/core/capture/mod.rs`（`crop`）
- `src-tauri/src/ui/ocr_popup.rs`（事件驱动编排）
- `src-tauri/src/core/ocr_translation.rs`（`FakeCapture` 调整 + 编排复用）
- `src-tauri/src/app/state.rs`（存整屏帧 + scale_factor）
- `src-tauri/src/app/shortcuts.rs`（入口签名随 `new()` 变更）
- `src-tauri/src/lib.rs`（注册 `get_capture_frame` / `submit_capture_region` / `cancel_capture` 命令）
- `src-tauri/capabilities/default.json`（授权新命令）
- `src-tauri/tauri.conf.json`（overlay 窗口声明，或运行时建窗）

新：

- `frontend/overlay.html`
- `src-tauri/src/ui/overlay.rs`（建窗 + 事件编排）

## 分阶段实施建议

1. **区域裁剪 + DXGI 后端**：`CapturedImage::crop`（含单测）→ `capture_monitor`（替换 picker 路径）→ `capture_region` 串通。
2. **overlay 窗口 + 前端**：`overlay.rs` 建窗 + `overlay.html` 框选 + 三个 Tauri command。
3. **编排重构**：`start_translation_from_ocr` 事件驱动 + `AppState` 存帧 + DPI 换算（含单测）。
4. **错误/取消收敛**：取消静默、失败文案沿用、busy 守卫。
5. **人工验证**：`#[ignore]` 集成测试 + `Alt+T` 回归。

## 风险与待验证

- DXGI 在锁屏 / 屏保 / 非交互会话失败时的错误形态（落到 `BackendUnavailable`，文案可接受）。
- 光标所在显示器判定：`GetCursorPos` 与 DXGI Output 描述的坐标对齐（DPI 下）。
- Tauri 2 运行时建 overlay 窗口的 API 形态与 `frontendDist` 多 HTML 加载方式。
- overlay 窗口 always_on_top 在某些 Windows 版本是否需要额外权限。
- 多显示器 MVP 不覆盖，仅光标所在单台。

## 官方资料

- [Desktop Duplication API](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api)
- [IDXGIOutputDuplication::AcquireNextFrame](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutputduplication-acquirenextframe)
- [GetCursorPos](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getcursorpos)
- [Tauri 2 WebviewWindow](https://v2.tauri.app/reference/javascript/api/namespacewebviewwindow/)
- [Capabilities - Tauri](https://v2.tauri.app/security/capabilities/)
