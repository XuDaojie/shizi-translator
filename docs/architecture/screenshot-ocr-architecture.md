# 截图 / OCR 架构设计

## 背景与目标

Shizi 里程碑 1 已完成 MVP 文本翻译闭环：手动输入、`Alt+T` 划词复制、OpenAI-compatible 流式翻译、Mock provider、内嵌设置面板和 `translation:event` 流式展示。

里程碑 2 的目标是在此基础上加入截图 / OCR 能力：用户通过快捷键触发截图，系统识别图片中的文字，再复用现有翻译链路展示结果。

本设计的目标是先明确架构边界和技术路线，避免在实现 OCR 时破坏现有的 UI 解耦方向。核心原则如下：

- 截图、OCR、翻译编排属于 Rust 后端能力。
- UI 只负责展示状态和用户操作，不承载 OCR 或翻译业务逻辑。
- OCR 文本必须通过统一翻译入口进入 `TranslationService`。
- LLM provider 不感知输入来自手动输入、划词还是 OCR。
- Windows 优先落地，同时为后续 macOS Vision Framework 预留平台扩展点。

## 非目标

本设计不覆盖以下内容：

- 不直接实现截图或 OCR 代码。
- 不引入 Slint，也不替换当前 Web 翻译弹窗。
- 不实现完整的取消、重试、历史记录或 usage 统计。
- 不一次性完成 Windows、macOS、Linux 全平台支持。
- 不把 Pot 的截图 / OCR 源码迁移到 Shizi；Pot 只能作为抽象层面的参考。

## 当前 MVP 约束

### 已有可复用能力

- `src-tauri/src/app/shortcuts.rs` 已注册并处理 `Alt+T`，可复用其全局快捷键入口和异步调度模式。
- `src-tauri/src/app/state.rs` 已提供 `translation_busy` 单并发保护，可用于限制 OCR 翻译与普通翻译并发。
- `src-tauri/src/ui/web_popup.rs` 已提供 `start_translation_from_text`，负责读取配置、选择 provider、显示窗口并推送 `translation:event`。
- `src-tauri/src/core/translation/service.rs` 已将 provider 流式输出转换为统一的 `TranslationEvent`。
- `frontend/main.js` 已消费 `Started` / `Delta` / `Finished` / `Failed`，可继续复用统一事件展示。

### 当前架构简化点

当前 MVP 为了快速跑通闭环保留了若干简化：

- `TranslationRequest` 仍是 `source_text + target_lang`，没有显式表达输入来源。
- `web_popup.rs` 同时承担 command、窗口展示、事件 emit 和翻译编排。
- 尚未抽出 `TranslationEventSink` 或 `TranslationPopupPort`。
- 当前 pending source text 只存 `String`，不区分手动输入、划词和 OCR。
- 当前前端只展示文本和翻译状态，不展示输入来源。

这些简化不会阻塞 OCR 设计，但会影响实现顺序。里程碑 2 不需要一次性拆完所有 UI port；更重要的是先设计统一输入模型，防止 OCR 走出一条独立支线。

## 推荐数据流

OCR 翻译应进入与划词、手动输入相同的翻译主链路：

```text
OCR 快捷键
  -> app/shortcuts 分发 OCR intent
  -> ScreenCapture.capture_interactive 或 capture_region
  -> OcrEngine.recognize(CapturedImage, OcrHints)
  -> TranslationInput::OcrText { text, image_id }
  -> TranslationService
  -> TranslationEvent
  -> Web popup 通过 translation:event 展示
```

纯识别（独立文字识别窗口）与截图翻译共享 DXGI 抓帧 + overlay 框选，但在 `submit_capture_region` 处按 `AppState.CapturePurpose`（`Translate` | `RecognizeOnly`）分叉：`Translate` 继续 `start_translation_from_input`；`RecognizeOnly` 只 emit `ocr:recognize-result` 到 `ocr` 窗口，不调用 `TranslationService`。

禁止以下路线：

```text
OCR -> frontend -> frontend calls start_translation
```

原因：这会让前端承载业务编排，并使后续 Slint 替换、平台 OCR 扩展和测试隔离都变困难。

## 输入模型演进

当前 `TranslationRequest`：

```rust
pub struct TranslationRequest {
    pub session_id: TranslationSessionId,
    pub source_text: String,
    pub target_lang: String,
}
```

建议演进为：

```rust
pub struct TranslationRequest {
    pub session_id: TranslationSessionId,
    pub input: TranslationInput,
    pub target_lang: String,
}

pub enum TranslationInput {
    ManualText(String),
    SelectedText(String),
    OcrText {
        text: String,
        image_id: Option<String>,
    },
}
```

`TranslationInput` 应提供统一取文本的方法，例如 `text()`，供 `TranslationService` 和 provider 使用。provider 只看到最终文本，不需要知道输入来源。

实现阶段可以分两步：

1. 先引入 `TranslationInput`，保持对外事件 payload 仍包含 `source_text`，避免一次性改动前端。
2. 再按需要让 UI 展示输入来源，例如「划词」「截图 OCR」「手动输入」。

## ScreenCapture 抽象

截图能力应通过 trait 隔离平台实现：

```rust
#[async_trait::async_trait]
pub trait ScreenCapture: Send + Sync {
    async fn capture_region(&self, region: CaptureRegion) -> Result<CapturedImage, CaptureError>;

    async fn capture_interactive(&self) -> Result<Option<CapturedImage>, CaptureError>;
}
```

建议类型：

```rust
pub struct CaptureRegion {
    pub display_id: Option<String>,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
}

pub struct CapturedImage {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub format: CapturedImageFormat,
}

pub enum CapturedImageFormat {
    Bgra8,
    Rgba8,
    Png,
}
```

`capture_interactive` 返回 `Result<Option<CapturedImage>, CaptureError>` 的原因是用户取消截图不是错误。调用方应将 `Ok(None)` 视为用户取消，不进入 OCR，也不弹出翻译失败。

## Windows 截图技术路线

### 路线 A：Windows.Graphics.Capture

Microsoft 官方说明 `Windows.Graphics.Capture` 可通过系统 picker 捕获显示器或窗口，并提供安全、易用的系统选择 UI。

优点：

- 用户授权路径清晰。
- 系统 picker 降低自建 overlay 的交互复杂度。
- 更适合作为 Windows MVP 的低风险起点。

风险：

- 用户体验受系统 picker 约束，未必等同于 Bob / Pot 式区域框选。
- 桌面应用中使用 picker 时需要处理 owner window handle 关联。
- 需要验证如何获取单帧并转换为 OCR 可用的 `SoftwareBitmap`。

### 路线 B：DXGI Desktop Duplication

Microsoft 官方说明 Desktop Duplication API 可通过 `AcquireNextFrame` 获取桌面图像，返回 DXGI surface，并需要处理 dirty rect、move rect、旋转和指针等信息。

优点：

- 更适合自定义区域截图和未来高性能捕获。
- 可以支撑后续自建 overlay 和精细裁剪。

风险：

- 实现复杂度高，需要处理 GPU surface 到 CPU bitmap 的转换。
- 多显示器、屏幕旋转、DPI 缩放、鼠标指针都需要显式处理。
- 初期引入容易拖慢里程碑 2 的主目标。

### 推荐分阶段策略

1. MVP 优先验证 `Windows.Graphics.Capture` 是否能满足「用户选择区域或目标 → 获取单帧 → OCR」链路。
2. 同时保留 `ScreenCapture` trait，使后续可以用 DXGI 或自建 overlay 替换实现。
3. 不在第一版把截图体验绑定到某个具体 crate 或某段 Win32 实现。

## OcrEngine 抽象

OCR 能力应独立于截图能力：

```rust
#[async_trait::async_trait]
pub trait OcrEngine: Send + Sync {
    async fn recognize(&self, image: CapturedImage, hints: OcrHints) -> Result<OcrResult, OcrError>;
}
```

建议类型：

```rust
pub struct OcrHints {
    pub preferred_languages: Vec<String>,
}

pub struct OcrResult {
    pub text: String,
    pub lines: Vec<OcrLine>,
    pub engine: String,
}

pub struct OcrLine {
    pub text: String,
    pub words: Vec<OcrWord>,
}

pub struct OcrWord {
    pub text: String,
    pub bounding_box: OcrBoundingBox,
}
```

第一版 UI 可以只使用 `OcrResult.text`。保留 `lines` / `words` 是为了后续支持高亮、调试和更好的文本拼接，不要求第一版展示。

## 引擎选择

截图识别在运行时只使用**当前唯一启用**的一项文字识别服务（设置页「服务 → 文字识别」）：

| 引擎 | 协议 / 实现 | 本版本 |
| --- | --- | --- |
| Windows 媒体 OCR | `Windows.Media.Ocr`（离线、系统自带） | 默认可启用，可与视觉渠道互斥切换 |
| OpenAI 兼容视觉 | 多模态 vision 模型（只抽文字，再进翻译批次） | 可启用；启用后关闭其它 OCR |
| Claude 视觉 | Messages 视觉协议 | **不可启用**（`runtimeSupported: false`） |

规则摘要：

- 至少保留一项启用的 OCR；不允许全部关闭。
- 启用某项时互斥：仅该项 `enabled`，其余 OCR 实例关闭。
- OCR 与翻译服务实例独立配置；识别出的纯文本仍走统一 `TranslationInput` / 多服务翻译批次。
- 引擎解析与调用在 Rust 侧完成，前端只管理实例启用状态与配置。

### 文字识别窗：会话级渠道与 PDF 首页

独立文字识别窗口在上述「唯一启用」规则之上做了**薄扩展**（不影响 `Alt+S` 截图翻译）：

1. **会话级临时渠道**：OCR 窗前端持有 `selectedOcrServiceId`（不落盘）。识别相关 command 透传可选 `service_id`；`resolve_ocr_engine_for(services, Some(id))` 按实例 id 建引擎并**忽略** `enabled`。无 id 时仍走唯一启用规则。
2. **截图纯识别槽**：`start_ocr_capture` 将临时 id 写入 `AppState.ocr_session_service_id`；`submit_capture_region(RecognizeOnly)` 读取后清除；`Translate` 路径永不读该槽。
3. **打开 PDF**：文件选择器支持 `pdf`；经扩展名/魔数分支后，Windows 上用 `Windows.Data.Pdf` 栅格化**第 1 页**为 `CapturedImage`，再进现有 `recognize_image_full`。`OcrRunMeta` 可选 `sourcePage` / `sourcePageCount` 供前端提示页数。剪贴板仍仅位图，不扩展 PDF。

## Windows OCR 技术路线

### Windows.Media.Ocr

Microsoft 官方说明 `Windows.Media.Ocr::OcrEngine` 提供 OCR 能力，调用 `RecognizeAsync(SoftwareBitmap)` 扫描图片文本，并返回 `OcrResult`。结果包含行、词以及对应的位置和尺寸信息。

Rust 侧可通过 `windows` crate 调用 `windows::Media::Ocr::OcrEngine`，关键 API 包括：

- `RecognizeAsync`
- `AvailableRecognizerLanguages`
- `IsLanguageSupported`
- `TryCreateFromLanguage`
- `TryCreateFromUserProfileLanguages`
- `MaxImageDimension`

推荐语言策略：

1. 优先尝试用户配置或系统用户语言。
2. 如果目标是中英混合，优先检测中文与英文 OCR 语言是否可用。
3. 如果指定语言不可用，返回明确的 `LanguageUnavailable` 错误，不静默降级到错误语言。
4. 设计阶段记录语言包安装风险，实现阶段再决定是否提供引导文案。

关键待验证点：

- `CapturedImage` 如何可靠转换为 `SoftwareBitmap`。
- `MaxImageDimension` 超限时是裁剪、缩放还是返回错误。
- 中文、英文、中英混合截图的识别质量。
- WinRT async 与 Tauri async runtime 的交互方式。

## 错误模型

建议区分截图错误、OCR 错误和翻译错误。

### 截图错误

```rust
pub enum CaptureError {
    PermissionDenied,
    NoCaptureTarget,
    UnsupportedPlatform,
    BackendUnavailable,
    ImageConversionFailed,
}
```

处理建议：

- 用户取消：返回 `Ok(None)`，不作为错误。
- 权限不足：显示「无法访问屏幕捕获权限」。
- 无捕获目标：显示「未选择截图区域或窗口」。
- 平台不支持：显示「当前平台暂不支持截图 OCR」。

### OCR 错误

```rust
pub enum OcrError {
    EngineUnavailable,
    LanguageUnavailable,
    ImageTooLarge,
    ImageConversionFailed,
    EmptyResult,
    UnsupportedPlatform,
}
```

处理建议：

- OCR 引擎不可用：提示系统 OCR 不可用。
- 语言不可用：提示缺少对应 OCR 语言包。
- 图片过大：提示截图区域过大，建议缩小区域。
- 识别为空：提示「未识别到文本」，不进入 LLM 翻译。

### 翻译错误

翻译错误继续使用现有 `TranslationEvent::Failed { message, retryable }`。OCR 阶段不应改变 provider 错误模型。

## 用户可见状态

第一版建议复用现有翻译弹窗，增加最小状态表达：

- 截图中：用户正在选择截图区域或窗口。
- 识别中：已获得截图，正在 OCR。
- 未识别到文本：OCR 成功但文本为空。
- 翻译中：OCR 文本已进入 `TranslationService`。
- OCR 不可用：系统 OCR 能力不可用或语言包缺失。

如果当前前端暂不适合新增独立状态事件，实现阶段可以先将 OCR 前置失败映射为 `TranslationEvent::Failed`，但设计上应保留更细状态的扩展空间。

## 快捷键与 Tauri 权限

现有全局快捷键统一由 `src-tauri/src/app/shortcuts.rs` 注册、解析和分发。启动时从 `AppConfig.shortcuts` 读取配置；设置页保存配置时先重注册快捷键，成功后再写入 `ConfigStore`，因此划词翻译、截图 OCR 翻译、剪贴板翻译无需重启即可生效。`open-settings` 为程序快捷键（前端窗口聚焦时处理，不注册全局）；`word-lookup` 绑定会保存，但本阶段不注册触发。

Tauri 官方文档说明 global shortcut 插件默认不启用危险能力，需要通过 capabilities 显式授权。当前项目已有 `src-tauri/capabilities/default.json`，新增快捷键或前端 command 时需要同步检查权限配置。

注意：Tauri capabilities 主要限制前端 WebView 能访问哪些 command 和 plugin 权限，不能替代 Rust 后端自身的边界设计。截图和 OCR 的核心能力仍应留在 Rust 侧。

## 测试策略

### 单元测试

优先测试纯 Rust 数据流：

- `TranslationInput::text()` 能正确返回三类输入文本。
- OCR 空结果不会进入翻译。
- 用户取消截图不会产生失败翻译事件。
- busy 状态下 OCR 翻译入口返回明确错误。

### Fake / Mock

建议实现阶段准备两个 fake：

- `FakeScreenCapture`：返回固定 `CapturedImage` 或 `Ok(None)`。
- `FakeOcrEngine`：返回固定文本、空文本或指定错误。

这样可以在没有真实截图权限、没有 OCR 语言包的环境中验证编排逻辑。

### 手动验证

里程碑 2 实现后至少验证：

- Windows 上触发 OCR 快捷键。
- 用户取消截图不崩溃、不进入翻译。
- 含中文、英文、中英混合文本的截图能识别并进入翻译。
- OCR 为空时显示可理解提示。
- OCR 成功后 provider 不知道输入来源，仍按普通文本翻译。
- 现有 `Alt+T` 划词翻译不回归。

## 分阶段实施建议

### 阶段 1：输入模型与编排边界

- 引入 `TranslationInput`。
- 保持现有 `translation:event` payload 兼容。
- 让手动输入和划词都通过统一输入模型进入翻译。

### 阶段 2：截图 / OCR trait 与 fake 实现

- 新增 `core/capture` 和 `core/ocr` 抽象。
- 使用 fake capture / fake OCR 验证完整编排，不接 Windows API。

### 阶段 3：Windows OCR spike

- 验证 `windows` crate 调用 `Windows.Media.Ocr`。
- 验证 `CapturedImage` 到 `SoftwareBitmap` 的转换。
- 验证 OCR 语言检测和 `MaxImageDimension`。

### 阶段 4：Windows 截图 spike

- 优先验证 `Windows.Graphics.Capture` 获取单帧。
- 如果系统 picker 体验不满足需求，再评估 DXGI 或自建 overlay。

### 阶段 5：接入真实 OCR 快捷键

- 新增 OCR 快捷键入口。
- 串联 capture、OCR、`TranslationInput::OcrText` 和 `TranslationService`。
- 前端展示最小 OCR 状态和错误提示。

## 第一切片落地状态

第一切片选择「Windows Graphics Capture + 最小拆分」路线：

- 已引入 `TranslationInput`，手动输入与划词翻译共用统一输入模型。
- 已新增 `ScreenCapture` 与 `OcrEngine` core 抽象。
- 已新增 fake 可测的 OCR workflow，用于验证截图取消、OCR 空文本和 OCR 文本进入翻译输入。
- 已新增 Windows Graphics Capture 可用性探针，用于确认当前系统是否支持后续真实截图接入。

真实单帧截图、`SoftwareBitmap` 转换和 `Windows.Media.Ocr` 接入将按下一份计划继续。

## Windows OCR Spike 落地状态

Windows OCR spike 已验证 `Windows.Media.Ocr` 接入路径：

- 已新增 `WindowsOcrEngine`，实现 `OcrEngine` trait。
- 已支持 `CapturedImageFormat::Rgba8` / `Bgra8` 到 Windows OCR 输入的转换。
- 已明确映射语言不可用、图片过大、格式不支持和 OCR 空文本错误。
- 已添加默认忽略的 Windows OCR 集成测试，用于人工验证真实 WinRT OCR 调用路径。

真实截图获取、OCR 快捷键和端到端截图翻译仍留给后续切片。

## 全屏单帧截图 Spike 落地状态

全屏单帧截图 spike 已验证 `Windows.Graphics.Capture` 接入路径：

- 已新增 `WindowsScreenCapture::capture_full_screen()`，用于通过系统 picker 获取一帧屏幕图像。
- 已完成 D3D11 设备创建与 `IDirect3DDevice` 桥接。
- 已实现 `GraphicsCapturePicker` 选择入口。
- 已使用 `Direct3D11CaptureFramePool::CreateFreeThreaded` 获取单帧。
- 已从 `IDXGISurface` 读取 BGRA 像素，并转换为 `CapturedImageFormat::Bgra8`。
- 已补充 `Map` / `Unmap` 资源守卫、尺寸校验、row pitch 校验和首帧超时资源释放。
- 已添加默认忽略的 Windows 集成测试，用于人工验证全屏单帧截图链路。

尚未完成：将 `WindowsScreenCapture` 接入 `ScreenCapture` trait、串联 OCR、快捷键和翻译弹窗。

## 截图 OCR 端到端闭环落地状态

截图 OCR 端到端最小闭环已串联完成（架构阶段 5）：

- 已将 `WindowsScreenCapture` 接入 `ScreenCapture` trait（`capture_interactive` 委托 `capture_full_screen`，`capture_region` 暂返回 `UnsupportedPlatform`）。
- 已新增 `platform::capture_and_recognize` 平台分发缝，Windows 侧串联 `WindowsScreenCapture` + `WindowsOcrEngine`，非 Windows 返回 `UnsupportedPlatform`。
- 已新增 `ui::ocr_popup::start_translation_from_ocr`，负责 busy 预检、用户取消静默、OCR 错误文案映射，成功后复用 `start_translation_from_input`。
- 默认注册 `Alt+O` 作为截图 OCR 快捷键；用户可在设置页改绑或清空，保存后立即生效。
- 不新增前端代码或事件类型；OCR 前置失败统一经 `translation:event::Failed` 展示。

已知简化（未在本切片修复）：`capture_full_screen` 在用户取消系统 picker 时返回 `BackendUnavailable` 而非 `Ok(None)`，用户取消当前会触发「截图失败」提示而非静默。区域截图（`capture_region`）仍未实现，留给 DXGI/自建 overlay 阶段。

## 风险与待验证清单

- Windows OCR 语言包缺失时的错误形态。
- `SoftwareBitmap` 输入格式与截图输出格式是否需要额外转换。
- `Windows.Graphics.Capture` 在 Tauri 桌面应用中使用 picker 的 owner window handle 处理。
- 多显示器和 DPI 缩放对区域坐标的影响。
- DXGI Desktop Duplication 的复杂度是否超出里程碑 2 的合理范围。
- 当前 `web_popup.rs` 是否需要在 OCR 接入前先拆分，还是可以保留并新增最小编排入口。
- OCR 前置状态是否需要新增事件类型，或先复用 `Failed` / `Started`。

## 官方资料

- [OcrEngine Class (Windows.Media.Ocr)](https://learn.microsoft.com/en-us/uwp/api/windows.media.ocr.ocrengine)
- [OcrEngine.RecognizeAsync(SoftwareBitmap) Method](https://learn.microsoft.com/en-us/uwp/api/windows.media.ocr.ocrengine.recognizeasync)
- [OcrEngine in windows::Media::Ocr - Rust](https://microsoft.github.io/windows-docs-rs/doc/windows/Media/Ocr/struct.OcrEngine.html)
- [Windows.Graphics.Capture Namespace](https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture)
- [Desktop Duplication API](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api)
- [Global Shortcut - Tauri](https://v2.tauri.app/plugin/global-shortcut/)
- [Capabilities - Tauri](https://v2.tauri.app/security/capabilities/)

## 进入实现计划的门槛

开始编码前需要确认：

1. MVP 截图路线优先采用 `Windows.Graphics.Capture`、DXGI，还是两者分阶段组合。
2. Windows OCR 是否确定优先使用 `Windows.Media.Ocr`。
3. 是否需要先做 `CapturedImage` 到 `SoftwareBitmap` 的 spike。
4. `TranslationInput` 是否作为里程碑 2 第一组代码改动先落地。
5. `web_popup.rs` 是否在 OCR 接入前拆分，还是保留现状并新增最小 OCR 编排入口。
6. OCR 失败、取消、无文本、语言包缺失的用户可见文案。

确认以上决策后，再创建单独的实现计划。

## 自建 overlay 区域框选落地状态

> 落地于 `feat/overlay-region-capture` 分支（2026-07）。规格见 `docs/superpowers/specs/2026-06-30-overlay-region-capture-design.md`，计划见 `docs/superpowers/plans/2026-06-30-overlay-region-capture.md`。

### 链路演进

截图 OCR 从「GraphicsCapturePicker 全屏单帧」演进为「自建 overlay 区域框选」，端到端链路：

```text
Alt+O
  -> app/shortcuts 分流 OCR -> ui::ocr_popup::start_translation_from_ocr
     1. translation_busy 预检
     2. try_begin_capture（独立 capture 锁，挡 OCR/recognize 期间二次 Alt+O）
     3. platform::capture_screen() = DXGI Desktop Duplication 抓光标所在显示器整屏 BGRA 帧
     4. AppState::set_pending_capture(frame, scale_factor)
     5. ui::overlay::open_overlay() 建 screenshot-overlay 窗口（fullscreen + always_on_top + 无装饰）
  -> frontend/overlay.html
     - get_capture_frame_meta 拿 (width, height, scale_factor)
     - get_capture_frame_bytes 拿整屏 BGRA ArrayBuffer（tauri::ipc::Response，无 PNG 编码/落盘）
     - canvas 物理像素 = 帧尺寸、CSS 尺寸 = 逻辑尺寸，BGRA→RGBA 逐像素交换后 putImageData
     - 鼠标拖矩形（clientX/Y 为 CSS 逻辑像素）；mouseup 提交、Esc/右键/选区过小取消
  -> submit_capture_region(x,y,w,h)
     1. close_overlay
     2. take_pending_capture 取 (frame, scale)，None 静默
     3. css_rect_to_physical 按 scale 换算物理像素
     4. recognize_region = CapturedImage::crop + WindowsOcrEngine.recognize（recognize 期间持 capture 锁，完成后 finish_capture 让 translation_busy 接管）
     5. start_translation_from_input（沿用）-> translation:event
  -> cancel_capture：take_pending_capture + finish_capture + close_overlay，静默
```

### 关键组件

- **`CapturedImage::crop`**（`core/capture/mod.rs`）：纯 BGRA 行切片裁剪，含格式/零尺寸/溢出/越界/缓冲区长度校验，可单测。
- **`css_rect_to_physical`**（`core/capture/mod.rs`）：CSS 逻辑像素 → 物理像素纯函数，向下取整。
- **`WindowsScreenCapture::capture_monitor`**（`platform/windows/capture.rs`）：DXGI `DuplicateOutput` + `AcquireNextFrame`，`GetCursorPos`+`MonitorFromPoint` 定位光标显示器（兜底第一个 output），尺寸取自 acquired texture 自身 `GetDesc`（防 rotation/DPI 切换错位），复用 D3D11 staging 提取。删除了 GraphicsCapturePicker / owner_hwnd / WinRT IDirect3DDevice 桥。
- **`AppState` 暂存帧 + capture 锁**（`app/state.rs`）：`pending_capture: Arc<Mutex<Option<(CapturedImage, f64)>>>` + 独立 `capture_in_progress` 锁（`try_begin_capture`/`finish_capture` 幂等）。
- **`recognize_cropped_for_translation`**（`core/ocr_translation.rs`）：对已抓帧 crop + recognize + 空文本拒绝，与 `recognize_capture_for_translation` 签名一致但永不返回 `Ok(None)`。
- **`overlay.rs`**（`ui/overlay.rs`）：建窗 + 四个 Tauri command（meta/bytes/submit/cancel）。
- **`overlay.html`**（`frontend/overlay.html`）：原生静态 canvas 框选，无构建。

### 并发与状态机

- `translation_busy` 仅在 `start_translation_from_input` 内置位，无法覆盖 OCR/recognize 阶段。
- 引入独立 `capture_in_progress` 锁：`start_translation_from_ocr` 入口 `try_begin_capture`，overlay 期间持锁；`submit_capture_region` 在 recognize 完成后、`start_translation_from_input` 前 `finish_capture`，让 `translation_busy` 接管；`cancel_capture` 释放。`finish_capture` 幂等，cancel/submit 竞争安全。

### 已知限制（MVP）

- **多显示器**：`capture_monitor` 按光标定位 DXGI Output，但 `WebviewWindowBuilder.fullscreen(true)` 默认建在主屏；光标在副屏时抓帧与建窗可能错位。仅保证主屏正确。
- **scale_factor 来源近似**：取主窗口缩放近似目标显示器缩放，单屏一致，混合 DPI 多屏不准。
- **DXGI 失败场景**：锁屏/屏保/安全桌面/远程会话下 `DuplicateOutput` 可能失败，统一落 `BackendUnavailable` →「截图失败，请稍后重试」。
- 多屏精确定位（按 monitor `.position()/.inner_size()/.scale_factor()` 建窗）、GraphicsCapture 兜底、overlay 多选区/编辑手柄/放大镜、截图历史均为非目标。

### 测试

- 单元测试（纯 Rust）：`crop`、`css_rect_to_physical`、`AppState` 往返与覆盖语义、`recognize_cropped_for_translation` 三分支、capture 锁 `try_begin/finish`、`unsupported` 平台缝。`cargo test` 45 passed。
- `#[ignore]` 集成测试：`capture_monitor_returns_bgra_frame`（需桌面会话，`cargo test -- --ignored`）。
- 人工验证：`npm run tauri dev` 跑 Alt+O 框选 / Esc 取消 / 选区过小 / busy 守卫 / Alt+T 回归。
