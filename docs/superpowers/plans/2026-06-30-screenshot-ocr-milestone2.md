# 截图 OCR 第一切片实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 为里程碑 2 的截图 / OCR 翻译建立第一条可测试的后端切片：统一翻译输入模型，新增截图 / OCR core 抽象，完成 fake 编排测试，并验证 Windows Graphics Capture 能力入口。

**架构：** 本切片采用最小拆分：保留 `web_popup.rs` 作为当前 WebView 翻译编排入口，先把 `TranslationRequest` 演进为 `TranslationInput`，再新增 `core::capture`、`core::ocr` 和 OCR workflow。真实截图路线选择 Windows Graphics Capture，但本切片只做可用性探针和接口落点，真实单帧捕获作为下一份计划的输入。

**技术栈：** Rust 2021、Tauri 2、`async-trait`、`thiserror`、`windows` crate（Windows Graphics Capture capability probe）、现有静态 Web 前端。

---

## 文件结构

### 修改文件

- `src-tauri/Cargo.toml`
  - 增加 Windows 平台依赖 `windows`，只启用 Graphics Capture 探针所需 feature。

- `src-tauri/src/core/mod.rs`
  - 导出新增 `capture`、`ocr`、`ocr_translation` 模块。

- `src-tauri/src/core/translation/types.rs`
  - 新增 `TranslationInput`。
  - 将 `TranslationRequest` 从 `source_text` 字段改为 `input` 字段。
  - 保留 `TranslationEvent::Started { source_text }`，避免前端同步大改。

- `src-tauri/src/core/translation/mod.rs`
  - 重新导出 `TranslationInput`。

- `src-tauri/src/core/llm/mock.rs`
  - 使用 `request.source_text()` 或 `request.input.text()` 读取文本。

- `src-tauri/src/core/llm/openai_compatible.rs`
  - 使用统一输入文本构造 prompt。

- `src-tauri/src/ui/web_popup.rs`
  - 新增 `start_translation_from_input`。
  - 保留 `start_translation_from_text` 作为手动输入兼容入口。
  - 划词入口后续传入 `TranslationInput::SelectedText`。

- `src-tauri/src/app/shortcuts.rs`
  - 将 `Alt+T` 读取到的选区文本传入 `TranslationInput::SelectedText`。
  - 本切片不注册 OCR 快捷键，避免用户触发未完成的真实截图链路。

### 新增文件

- `src-tauri/src/core/capture/mod.rs`
  - 定义 `ScreenCapture` trait、`CaptureRegion`、`CapturedImage`、`CapturedImageFormat`、`CaptureError`。

- `src-tauri/src/core/ocr/mod.rs`
  - 定义 `OcrEngine` trait、`OcrHints`、`OcrResult`、`OcrLine`、`OcrWord`、`OcrBoundingBox`、`OcrError`。

- `src-tauri/src/core/ocr_translation.rs`
  - 定义 `recognize_capture_for_translation`，负责串联 capture 和 OCR，输出 `Option<TranslationInput>`。

- `src-tauri/src/platform/mod.rs`
  - 挂载平台模块。

- `src-tauri/src/platform/windows/mod.rs`
  - 挂载 Windows 平台能力模块。

- `src-tauri/src/platform/windows/capture.rs`
  - 定义 `WindowsGraphicsCaptureProbe`，提供 Graphics Capture 可用性检测。

- `src-tauri/src/platform/unsupported.rs`
  - 为非 Windows 平台提供 `GraphicsCaptureProbe` 的不可用实现，保证跨平台编译边界清晰。

---

## 任务 1：引入 TranslationInput 并保持现有翻译链路

**文件：**
- 修改：`src-tauri/src/core/translation/types.rs`
- 修改：`src-tauri/src/core/translation/mod.rs`
- 修改：`src-tauri/src/core/llm/mock.rs`
- 修改：`src-tauri/src/core/llm/openai_compatible.rs`
- 修改：`src-tauri/src/core/translation/service.rs`
- 修改：`src-tauri/src/ui/web_popup.rs`
- 修改：`src-tauri/src/app/shortcuts.rs`

- [ ] **步骤 1：编写失败的 TranslationInput 单元测试**

在 `src-tauri/src/core/translation/types.rs` 末尾添加：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translation_input_text_returns_inner_text() {
        assert_eq!(TranslationInput::ManualText("manual".to_string()).text(), "manual");
        assert_eq!(TranslationInput::SelectedText("selected".to_string()).text(), "selected");
        assert_eq!(
            TranslationInput::OcrText {
                text: "ocr".to_string(),
                image_id: Some("image-1".to_string()),
            }
            .text(),
            "ocr"
        );
    }

    #[test]
    fn translation_request_source_text_reads_input_text() {
        let request = TranslationRequest {
            session_id: TranslationSessionId("session-1".to_string()),
            input: TranslationInput::SelectedText("hello".to_string()),
            target_lang: "中文".to_string(),
        };

        assert_eq!(request.source_text(), "hello");
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
cd src-tauri && cargo test translation_input
```

预期：编译失败，报错包含 `use of undeclared type TranslationInput` 或 `no method named source_text`。

- [ ] **步骤 3：实现 TranslationInput 与 source_text helper**

将 `src-tauri/src/core/translation/types.rs` 改为：

```rust
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationSessionId(pub String);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "source")]
pub enum TranslationInput {
    ManualText(String),
    SelectedText(String),
    OcrText {
        text: String,
        image_id: Option<String>,
    },
}

impl TranslationInput {
    pub fn text(&self) -> &str {
        match self {
            Self::ManualText(text) | Self::SelectedText(text) => text,
            Self::OcrText { text, .. } => text,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationRequest {
    pub session_id: TranslationSessionId,
    pub input: TranslationInput,
    pub target_lang: String,
}

impl TranslationRequest {
    pub fn source_text(&self) -> &str {
        self.input.text()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum TranslationEvent {
    Started {
        session_id: TranslationSessionId,
        source_text: String,
    },
    Delta {
        session_id: TranslationSessionId,
        text: String,
    },
    Finished {
        session_id: TranslationSessionId,
        full_text: String,
    },
    Failed {
        session_id: TranslationSessionId,
        message: String,
        retryable: bool,
    },
}
```

保留步骤 1 中的测试模块。

- [ ] **步骤 4：更新 translation 模块导出**

将 `src-tauri/src/core/translation/mod.rs` 改为：

```rust
pub mod service;
pub mod types;

pub use service::TranslationService;
pub use types::{TranslationEvent, TranslationInput, TranslationRequest, TranslationSessionId};
```

- [ ] **步骤 5：更新 LLM provider 读取文本的代码**

在 `src-tauri/src/core/llm/mock.rs` 中，将：

```rust
request.source_text.clone(),
```

改为：

```rust
request.source_text().to_string(),
```

在 `src-tauri/src/core/llm/openai_compatible.rs` 中，将：

```rust
request.target_lang, request.source_text
```

改为：

```rust
request.target_lang,
request.source_text()
```

- [ ] **步骤 6：更新 TranslationService 事件中的 source_text**

在 `src-tauri/src/core/translation/service.rs` 中，将 `request.source_text` 的读取改为：

```rust
source_text: request.source_text().to_string(),
```

如果当前文件没有直接读取 `source_text`，保持不变。

- [ ] **步骤 7：更新 Web popup 翻译入口**

在 `src-tauri/src/ui/web_popup.rs` 中引入 `TranslationInput`：

```rust
translation::{TranslationEvent, TranslationInput, TranslationRequest, TranslationService, TranslationSessionId},
```

新增：

```rust
pub fn start_translation_from_input(
    input: TranslationInput,
    app: tauri::AppHandle,
    state: &AppState,
) -> Result<String, String> {
    let source_text = input.text().trim().to_string();
    if source_text.is_empty() {
        return Err("请输入要翻译的文本".to_string());
    }

    let input = match input {
        TranslationInput::ManualText(_) => TranslationInput::ManualText(source_text.clone()),
        TranslationInput::SelectedText(_) => TranslationInput::SelectedText(source_text.clone()),
        TranslationInput::OcrText { image_id, .. } => TranslationInput::OcrText {
            text: source_text.clone(),
            image_id,
        },
    };

    let config = state.config_store.get().map_err(|error| error.to_string())?;
    let provider: Arc<dyn LlmProvider> = match config.provider.as_str() {
        "mock" => Arc::new(MockLlmProvider),
        _ => Arc::new(OpenAiCompatibleProvider::new(OpenAiCompatibleConfig::from(
            config.openai_compatible,
        ))),
    };
    let translation_service = TranslationService::new(provider);

    let session_id = create_session_id()?;
    let request = TranslationRequest {
        session_id: TranslationSessionId(session_id.clone()),
        input,
        target_lang: config.target_lang,
    };

    state.try_begin_translation()?;

    show_window(&app);
    thread::sleep(Duration::from_millis(120));
    emit_translation_event(
        &app,
        TranslationEvent::Started {
            session_id: request.session_id.clone(),
            source_text: request.source_text().to_string(),
        },
    )
    .map_err(|error| {
        let _ = state.finish_translation();
        error.to_string()
    })?;
    let app_handle = app.clone();
    let state_for_task = state.clone();

    tauri::async_runtime::spawn(async move {
        let failed_session_id = request.session_id.clone();
        let result = translation_service
            .translate_with(request, |event| {
                let _ = emit_translation_event(&app_handle, event);
            })
            .await;

        if let Err(error) = result {
            let retryable = error.retryable();
            let _ = emit_translation_event(
                &app_handle,
                TranslationEvent::Failed {
                    session_id: failed_session_id,
                    message: error.to_string(),
                    retryable,
                },
            );
        }
        let _ = state_for_task.finish_translation();
    });

    Ok(session_id)
}
```

再将原 `start_translation_from_text` 改为：

```rust
pub fn start_translation_from_text(
    text: String,
    app: tauri::AppHandle,
    state: &AppState,
) -> Result<String, String> {
    start_translation_from_input(TranslationInput::ManualText(text), app, state)
}
```

- [ ] **步骤 8：让划词入口标记 SelectedText**

在 `src-tauri/src/app/shortcuts.rs` 中引入：

```rust
use crate::core::translation::TranslationInput;
```

将 `start_translation_from_text(selected_text, app_handle.clone(), state.inner())` 改为：

```rust
start_translation_from_input(
    TranslationInput::SelectedText(selected_text),
    app_handle.clone(),
    state.inner(),
)
```

并将 import 从：

```rust
ui::web_popup::{show_translation_error, start_translation_from_text},
```

改为：

```rust
ui::web_popup::{show_translation_error, start_translation_from_input},
```

- [ ] **步骤 9：运行测试验证通过**

运行：

```bash
cd src-tauri && cargo test translation_input
```

预期：两个测试通过。

- [ ] **步骤 10：运行完整 Rust 测试**

运行：

```bash
cd src-tauri && cargo test
```

预期：所有 Rust 单元测试通过。

- [ ] **步骤 11：Commit**

```bash
git add src-tauri/src/core/translation/types.rs \
  src-tauri/src/core/translation/mod.rs \
  src-tauri/src/core/llm/mock.rs \
  src-tauri/src/core/llm/openai_compatible.rs \
  src-tauri/src/core/translation/service.rs \
  src-tauri/src/ui/web_popup.rs \
  src-tauri/src/app/shortcuts.rs

git commit -m "$(cat <<'EOF'
refactor(translation): 引入统一翻译输入模型

Co-Authored-By: Claude Sonnet 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## 任务 2：新增截图 core 抽象

**文件：**
- 创建：`src-tauri/src/core/capture/mod.rs`
- 修改：`src-tauri/src/core/mod.rs`

- [ ] **步骤 1：编写截图抽象测试**

创建 `src-tauri/src/core/capture/mod.rs`，先写测试和最小类型引用：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_image_reports_dimensions() {
        let image = CapturedImage {
            bytes: vec![0, 1, 2, 3],
            width: 1,
            height: 1,
            format: CapturedImageFormat::Rgba8,
        };

        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
        assert_eq!(image.bytes.len(), 4);
    }

    #[test]
    fn user_cancel_is_not_capture_error() {
        let result: Result<Option<CapturedImage>, CaptureError> = Ok(None);
        assert!(result.expect("用户取消不是错误").is_none());
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
cd src-tauri && cargo test captured_image
```

预期：编译失败，报错包含 `cannot find struct CapturedImage`。

- [ ] **步骤 3：实现截图抽象类型和 trait**

将 `src-tauri/src/core/capture/mod.rs` 改为：

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct CaptureRegion {
    pub display_id: Option<String>,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CapturedImage {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub format: CapturedImageFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapturedImageFormat {
    Bgra8,
    Rgba8,
    Png,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CaptureError {
    #[error("无法访问屏幕捕获权限")]
    PermissionDenied,
    #[error("未选择截图区域或窗口")]
    NoCaptureTarget,
    #[error("当前平台暂不支持截图 OCR")]
    UnsupportedPlatform,
    #[error("截图后端不可用：{0}")]
    BackendUnavailable(String),
    #[error("截图图像转换失败：{0}")]
    ImageConversionFailed(String),
}

#[async_trait::async_trait]
pub trait ScreenCapture: Send + Sync {
    async fn capture_region(&self, region: CaptureRegion) -> Result<CapturedImage, CaptureError>;

    async fn capture_interactive(&self) -> Result<Option<CapturedImage>, CaptureError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_image_reports_dimensions() {
        let image = CapturedImage {
            bytes: vec![0, 1, 2, 3],
            width: 1,
            height: 1,
            format: CapturedImageFormat::Rgba8,
        };

        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
        assert_eq!(image.bytes.len(), 4);
    }

    #[test]
    fn user_cancel_is_not_capture_error() {
        let result: Result<Option<CapturedImage>, CaptureError> = Ok(None);
        assert!(result.expect("用户取消不是错误").is_none());
    }
}
```

- [ ] **步骤 4：导出 capture 模块**

在 `src-tauri/src/core/mod.rs` 增加：

```rust
pub mod capture;
```

- [ ] **步骤 5：运行测试验证通过**

运行：

```bash
cd src-tauri && cargo test captured_image
```

预期：两个测试通过。

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/core/mod.rs src-tauri/src/core/capture/mod.rs

git commit -m "$(cat <<'EOF'
feat(capture): 添加截图能力抽象

Co-Authored-By: Claude Sonnet 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## 任务 3：新增 OCR core 抽象

**文件：**
- 创建：`src-tauri/src/core/ocr/mod.rs`
- 修改：`src-tauri/src/core/mod.rs`

- [ ] **步骤 1：编写 OCR 抽象测试**

创建 `src-tauri/src/core/ocr/mod.rs`，先写测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ocr_result_detects_empty_text_after_trim() {
        let result = OcrResult {
            text: "  \n ".to_string(),
            lines: vec![],
            engine: "fake".to_string(),
        };

        assert!(result.is_empty_text());
    }

    #[test]
    fn ocr_result_keeps_non_empty_text() {
        let result = OcrResult {
            text: "Hello".to_string(),
            lines: vec![],
            engine: "fake".to_string(),
        };

        assert!(!result.is_empty_text());
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
cd src-tauri && cargo test ocr_result
```

预期：编译失败，报错包含 `cannot find struct OcrResult`。

- [ ] **步骤 3：实现 OCR 抽象类型和 trait**

将 `src-tauri/src/core/ocr/mod.rs` 改为：

```rust
use crate::core::capture::CapturedImage;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OcrHints {
    pub preferred_languages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OcrResult {
    pub text: String,
    pub lines: Vec<OcrLine>,
    pub engine: String,
}

impl OcrResult {
    pub fn is_empty_text(&self) -> bool {
        self.text.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OcrLine {
    pub text: String,
    pub words: Vec<OcrWord>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OcrWord {
    pub text: String,
    pub bounding_box: OcrBoundingBox,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OcrBoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum OcrError {
    #[error("系统 OCR 能力不可用")]
    EngineUnavailable,
    #[error("缺少 OCR 语言包：{0}")]
    LanguageUnavailable(String),
    #[error("截图区域过大，请缩小区域")]
    ImageTooLarge,
    #[error("OCR 图像转换失败：{0}")]
    ImageConversionFailed(String),
    #[error("未识别到文本")]
    EmptyResult,
    #[error("当前平台暂不支持 OCR")]
    UnsupportedPlatform,
}

#[async_trait::async_trait]
pub trait OcrEngine: Send + Sync {
    async fn recognize(&self, image: CapturedImage, hints: OcrHints) -> Result<OcrResult, OcrError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ocr_result_detects_empty_text_after_trim() {
        let result = OcrResult {
            text: "  \n ".to_string(),
            lines: vec![],
            engine: "fake".to_string(),
        };

        assert!(result.is_empty_text());
    }

    #[test]
    fn ocr_result_keeps_non_empty_text() {
        let result = OcrResult {
            text: "Hello".to_string(),
            lines: vec![],
            engine: "fake".to_string(),
        };

        assert!(!result.is_empty_text());
    }
}
```

- [ ] **步骤 4：导出 OCR 模块**

在 `src-tauri/src/core/mod.rs` 增加：

```rust
pub mod ocr;
```

- [ ] **步骤 5：运行测试验证通过**

运行：

```bash
cd src-tauri && cargo test ocr_result
```

预期：两个测试通过。

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/core/mod.rs src-tauri/src/core/ocr/mod.rs

git commit -m "$(cat <<'EOF'
feat(ocr): 添加 OCR 能力抽象

Co-Authored-By: Claude Sonnet 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## 任务 4：新增 OCR 翻译编排 workflow

**文件：**
- 创建：`src-tauri/src/core/ocr_translation.rs`
- 修改：`src-tauri/src/core/mod.rs`

- [ ] **步骤 1：编写 fake 编排测试**

创建 `src-tauri/src/core/ocr_translation.rs`，先写测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        capture::{CaptureError, CapturedImage, CapturedImageFormat, CaptureRegion, ScreenCapture},
        ocr::{OcrEngine, OcrHints, OcrResult},
    };

    struct FakeCapture {
        image: Option<CapturedImage>,
    }

    #[async_trait::async_trait]
    impl ScreenCapture for FakeCapture {
        async fn capture_region(&self, _region: CaptureRegion) -> Result<CapturedImage, CaptureError> {
            self.image
                .clone()
                .ok_or(CaptureError::NoCaptureTarget)
        }

        async fn capture_interactive(&self) -> Result<Option<CapturedImage>, CaptureError> {
            Ok(self.image.clone())
        }
    }

    struct FakeOcr {
        text: String,
    }

    #[async_trait::async_trait]
    impl OcrEngine for FakeOcr {
        async fn recognize(
            &self,
            _image: CapturedImage,
            _hints: OcrHints,
        ) -> Result<OcrResult, crate::core::ocr::OcrError> {
            Ok(OcrResult {
                text: self.text.clone(),
                lines: vec![],
                engine: "fake".to_string(),
            })
        }
    }

    fn image() -> CapturedImage {
        CapturedImage {
            bytes: vec![0, 1, 2, 3],
            width: 1,
            height: 1,
            format: CapturedImageFormat::Rgba8,
        }
    }

    #[tokio::test]
    async fn workflow_returns_ocr_translation_input() {
        let input = recognize_capture_for_translation(
            &FakeCapture { image: Some(image()) },
            &FakeOcr { text: " Hello ".to_string() },
            OcrHints::default(),
        )
        .await
        .expect("OCR workflow 应成功")
        .expect("应返回 OCR 输入");

        assert_eq!(input.text(), "Hello");
    }

    #[tokio::test]
    async fn workflow_returns_none_when_user_cancels_capture() {
        let input = recognize_capture_for_translation(
            &FakeCapture { image: None },
            &FakeOcr { text: "Hello".to_string() },
            OcrHints::default(),
        )
        .await
        .expect("用户取消不是错误");

        assert!(input.is_none());
    }

    #[tokio::test]
    async fn workflow_rejects_empty_ocr_text() {
        let error = recognize_capture_for_translation(
            &FakeCapture { image: Some(image()) },
            &FakeOcr { text: "  ".to_string() },
            OcrHints::default(),
        )
        .await
        .expect_err("空 OCR 文本应返回错误");

        assert!(matches!(error, OcrTranslationError::Ocr(crate::core::ocr::OcrError::EmptyResult)));
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
cd src-tauri && cargo test workflow_
```

预期：编译失败，报错包含 `cannot find function recognize_capture_for_translation`。如果报错包含 `use of unresolved crate tokio`，执行步骤 3 的 dev dependency 后再确认测试仍因目标函数缺失而失败。

- [ ] **步骤 3：补充 tokio 测试依赖**

在 `src-tauri/Cargo.toml` 增加：

```toml
[dev-dependencies]
tokio = { version = "1", features = ["macros", "rt"] }
```

- [ ] **步骤 4：实现 OCR workflow**

在测试前面加入：

```rust
use crate::core::{
    capture::{CaptureError, ScreenCapture},
    ocr::{OcrEngine, OcrError, OcrHints},
    translation::TranslationInput,
};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum OcrTranslationError {
    #[error(transparent)]
    Capture(#[from] CaptureError),
    #[error(transparent)]
    Ocr(#[from] OcrError),
}

pub async fn recognize_capture_for_translation<C, O>(
    capture: &C,
    ocr: &O,
    hints: OcrHints,
) -> Result<Option<TranslationInput>, OcrTranslationError>
where
    C: ScreenCapture,
    O: OcrEngine,
{
    let Some(image) = capture.capture_interactive().await? else {
        return Ok(None);
    };

    let result = ocr.recognize(image, hints).await?;
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

- [ ] **步骤 5：导出 OCR workflow 模块**

在 `src-tauri/src/core/mod.rs` 增加：

```rust
pub mod ocr_translation;
```

- [ ] **步骤 6：运行 workflow 测试验证通过**

运行：

```bash
cd src-tauri && cargo test workflow_
```

预期：3 个测试通过。

- [ ] **步骤 7：运行完整 Rust 测试**

运行：

```bash
cd src-tauri && cargo test
```

预期：所有 Rust 单元测试通过。

- [ ] **步骤 8：Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock \
  src-tauri/src/core/mod.rs \
  src-tauri/src/core/ocr_translation.rs

git commit -m "$(cat <<'EOF'
feat(ocr): 添加截图识别翻译编排

Co-Authored-By: Claude Sonnet 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## 任务 5：添加 Windows Graphics Capture 可用性探针

**文件：**
- 修改：`src-tauri/Cargo.toml`
- 创建：`src-tauri/src/platform/mod.rs`
- 创建：`src-tauri/src/platform/windows/mod.rs`
- 创建：`src-tauri/src/platform/windows/capture.rs`
- 创建：`src-tauri/src/platform/unsupported.rs`
- 修改：`src-tauri/src/lib.rs`

- [ ] **步骤 1：编写平台探针测试**

创建 `src-tauri/src/platform/windows/capture.rs`：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphics_capture_probe_returns_boolean() {
        let _supported: bool = WindowsGraphicsCaptureProbe::is_supported();
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
cd src-tauri && cargo test graphics_capture_probe_returns_boolean
```

预期：编译失败，报错包含 `use of undeclared type WindowsGraphicsCaptureProbe`。

- [ ] **步骤 3：添加 windows crate 依赖**

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 中增加 Windows 专用依赖：

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = ["Graphics_Capture"] }
```

如果 Cargo 解析提示 `windows` 版本与当前 toolchain 不兼容，改用 `0.62`，并保留相同 feature。修改版本后必须重新运行本任务测试。

- [ ] **步骤 4：实现 Windows 探针**

将 `src-tauri/src/platform/windows/capture.rs` 改为：

```rust
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
}
```

- [ ] **步骤 5：添加平台模块导出**

创建 `src-tauri/src/platform/windows/mod.rs`：

```rust
pub mod capture;
```

创建 `src-tauri/src/platform/unsupported.rs`：

```rust
pub struct GraphicsCaptureProbe;

impl GraphicsCaptureProbe {
    pub fn is_supported() -> bool {
        false
    }
}
```

创建 `src-tauri/src/platform/mod.rs`：

```rust
#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(not(target_os = "windows"))]
pub mod unsupported;
```

在 `src-tauri/src/lib.rs` 顶部增加：

```rust
mod platform;
```

- [ ] **步骤 6：运行平台探针测试**

运行：

```bash
cd src-tauri && cargo test graphics_capture_probe_returns_boolean
```

预期：Windows 上测试通过；非 Windows 上该测试不编译进目标，完整测试不应因平台模块失败。

- [ ] **步骤 7：运行完整验证**

运行：

```bash
cd src-tauri && cargo test
cd src-tauri && cargo build
node --check frontend/main.js
```

预期：Rust 测试通过，Rust debug 构建通过，前端语法检查通过。

- [ ] **步骤 8：Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock \
  src-tauri/src/lib.rs \
  src-tauri/src/platform/mod.rs \
  src-tauri/src/platform/windows/mod.rs \
  src-tauri/src/platform/windows/capture.rs \
  src-tauri/src/platform/unsupported.rs

git commit -m "$(cat <<'EOF'
feat(capture): 添加 Windows 截图能力探针

Co-Authored-By: Claude Sonnet 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## 任务 6：文档同步与最终验证

**文件：**
- 修改：`docs/architecture/screenshot-ocr-architecture.md`

- [ ] **步骤 1：补充第一切片落地状态**

在 `docs/architecture/screenshot-ocr-architecture.md` 的「分阶段实施建议」后增加：

```markdown
## 第一切片落地状态

第一切片选择「Windows Graphics Capture + 最小拆分」路线：

- 已引入 `TranslationInput`，手动输入与划词翻译共用统一输入模型。
- 已新增 `ScreenCapture` 与 `OcrEngine` core 抽象。
- 已新增 fake 可测的 OCR workflow，用于验证截图取消、OCR 空文本和 OCR 文本进入翻译输入。
- 已新增 Windows Graphics Capture 可用性探针，用于确认当前系统是否支持后续真实截图接入。

真实单帧截图、`SoftwareBitmap` 转换和 `Windows.Media.Ocr` 接入将按下一份计划继续。
```

- [ ] **步骤 2：运行最终验证命令**

运行：

```bash
cd src-tauri && cargo test
cd src-tauri && cargo build
node --check frontend/main.js
git status --short
```

预期：

- `cargo test` 通过。
- `cargo build` 通过。
- `node --check frontend/main.js` 无输出且退出码为 0。
- `git status --short` 只显示文档变更。

- [ ] **步骤 3：Commit**

```bash
git add docs/architecture/screenshot-ocr-architecture.md

git commit -m "$(cat <<'EOF'
docs(architecture): 同步截图 OCR 第一切片状态

Co-Authored-By: Claude Sonnet 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## 自检清单

- [ ] 规格覆盖度：计划覆盖 `TranslationInput`、`ScreenCapture`、`OcrEngine`、OCR workflow、Windows Graphics Capture 探针、文档同步。
- [ ] 禁止数据流检查：没有让 OCR 走 `frontend -> start_translation` 的独立业务链路。
- [ ] 类型一致性：`TranslationInput::OcrText { text, image_id }`、`CapturedImage`、`OcrHints`、`OcrResult` 在各任务中命名一致。
- [ ] 范围控制：本计划不实现真实单帧截图，不实现 `Windows.Media.Ocr`，不拆 `TranslationPopupPort`。
- [ ] 验证闭环：每个代码任务都有失败测试、通过测试和 commit 步骤。
