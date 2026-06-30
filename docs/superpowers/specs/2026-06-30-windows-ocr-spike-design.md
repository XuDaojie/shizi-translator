# Windows OCR Spike 设计规格

## 背景

截图 / OCR 第一切片已经完成了后端底座：

- `TranslationInput::OcrText` 已进入翻译输入模型。
- `ScreenCapture` 与 `OcrEngine` core 抽象已存在。
- fake 可测的 OCR workflow 已能验证截图取消、OCR 空文本和 OCR 文本进入翻译输入。
- `WindowsGraphicsCaptureProbe` 已能检测当前系统是否支持 Windows Graphics Capture。

下一步选择先做 Windows OCR spike，而不是先做真实截图。原因是截图链路最终仍要把图片转换为 `SoftwareBitmap` 并交给 `Windows.Media.Ocr`。如果 OCR 输入格式和 WinRT 调用路径没有先验证，真实截图接入后会把问题埋到更深处。

## 目标

本切片目标是验证 Rust 后端能通过 `windows` crate 调用 `Windows.Media.Ocr`，并把一张内存图片识别成 `OcrResult`，或明确映射为现有 `OcrError`。

完成后应具备：

- Windows OCR 引擎可用性检测。
- `CapturedImage` 到 OCR 输入的最小转换路径。
- `WindowsOcrEngine` 对 `OcrEngine` trait 的实现。
- 对语言不可用、图片格式不支持、OCR 空结果等错误的明确映射。
- 不影响现有手动翻译、`Alt+T` 划词翻译和 fake OCR workflow。

## 非目标

本切片不实现以下能力：

- 不接 `Windows.Graphics.Capture` 真实截图。
- 不新增 OCR 快捷键。
- 不接 Web UI 状态。
- 不做端到端截图翻译。
- 不实现 macOS / Linux OCR。
- 不做 OCR 识别质量优化。
- 不支持 PNG 解码；`CapturedImageFormat::Png` 在本切片中返回格式转换错误。

## 推荐架构

新增 Windows OCR 平台模块：

```text
src-tauri/src/platform/windows/ocr.rs
```

模块职责：

- 检测 `Windows.Media.Ocr::OcrEngine` 是否可用。
- 根据 `OcrHints` 选择 OCR 语言。
- 将 `CapturedImage` 转换为 `SoftwareBitmap`。
- 调用 `RecognizeAsync`。
- 将 WinRT OCR 结果转换为 `crate::core::ocr::OcrResult`。
- 将平台错误映射为 `crate::core::ocr::OcrError`。

公开类型建议：

```rust
pub struct WindowsOcrEngine;

impl WindowsOcrEngine {
    pub fn is_available() -> bool;
}

#[async_trait::async_trait]
impl OcrEngine for WindowsOcrEngine {
    async fn recognize(
        &self,
        image: CapturedImage,
        hints: OcrHints,
    ) -> Result<OcrResult, OcrError>;
}
```

## 输入格式策略

本切片只支持 raw pixel 格式：

- `CapturedImageFormat::Rgba8`
- `CapturedImageFormat::Bgra8`

`CapturedImageFormat::Png` 暂不支持，返回：

```rust
OcrError::ImageConversionFailed("暂不支持 PNG OCR 输入".to_string())
```

原因：PNG 解码属于独立能力，和验证 `Windows.Media.Ocr` 调用路径不是同一个风险点。为了保持切片小而可验证，本阶段不引入 PNG 解码。

## 语言选择策略

语言选择按以下顺序：

1. 如果 `OcrHints.preferred_languages` 非空，按顺序尝试创建 `Windows.Globalization.Language` 并检测 `OcrEngine::IsLanguageSupported`。
2. 第一个被支持的语言用于 `OcrEngine::TryCreateFromLanguage`。
3. 如果 hints 中没有可用语言，返回 `OcrError::LanguageUnavailable(<language>)`。
4. 如果 hints 为空，使用 `OcrEngine::TryCreateFromUserProfileLanguages()`。
5. 如果系统用户语言无法创建 OCR engine，返回 `OcrError::EngineUnavailable`。

## OCR 结果转换

`Windows.Media.Ocr::OcrResult` 应转换为 core 层的：

```rust
pub struct OcrResult {
    pub text: String,
    pub lines: Vec<OcrLine>,
    pub engine: String,
}
```

转换规则：

- `text` 使用 Windows OCR 返回的全文文本。
- `lines` 映射每个 OCR line。
- `words` 映射每个 OCR word。
- `bounding_box` 使用 Windows OCR word 的位置和尺寸。
- `engine` 固定为 `windows-media-ocr`。
- 如果全文 `text.trim().is_empty()`，返回 `OcrError::EmptyResult`。

## 错误映射

本切片只做明确、可测试的错误映射：

- OCR engine 无法创建：`OcrError::EngineUnavailable`
- 指定语言不可用：`OcrError::LanguageUnavailable(language)`
- 图片尺寸超过 `OcrEngine::MaxImageDimension()`：`OcrError::ImageTooLarge`
- 图片格式不支持或字节长度不匹配：`OcrError::ImageConversionFailed(message)`
- OCR 成功但文本为空：`OcrError::EmptyResult`

## 测试策略

### 单元测试

优先覆盖不依赖真实 OCR 环境的纯逻辑：

- 语言选择：preferred language 可用 / 不可用。
- 图片格式：`Png` 返回 `ImageConversionFailed`。
- 图片字节长度与尺寸不匹配返回 `ImageConversionFailed`。
- `MaxImageDimension` 超限返回 `ImageTooLarge`。

### Windows 集成测试

新增一个默认忽略的 Windows-only 测试，用于人工验证真实 OCR：

```bash
cd src-tauri && cargo test windows_ocr -- --ignored
```

该测试用代码生成一张小尺寸 bitmap，调用 `WindowsOcrEngine::recognize`。测试目标是验证 WinRT OCR 调用路径可用；不把识别准确率作为强断言，避免字体渲染、语言包和系统环境差异造成脆弱测试。

## 成功标准

- `cargo test` 通过。
- `cargo build` 通过。
- `node --check frontend/main.js` 通过。
- `WindowsOcrEngine::is_available()` 可调用且不 panic。
- `WindowsOcrEngine` 实现 `OcrEngine` trait。
- 不支持的格式、语言不可用、图片过大、OCR 空结果都有明确错误。
- 不影响现有手动输入、`Alt+T` 划词翻译和 fake OCR workflow。

## 后续切片

本切片完成后，下一步再接真实截图：

1. 使用 `Windows.Graphics.Capture` 获取单帧。
2. 将截图帧转换为 `CapturedImage`。
3. 串联 `ScreenCapture -> WindowsOcrEngine -> TranslationInput::OcrText -> TranslationService`。
4. 新增 OCR 快捷键与最小 UI 状态。
