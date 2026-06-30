# 截图 OCR 端到端最小闭环 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 把已落地的截图/OCR 零件串联成端到端闭环——`Alt+O` 触发系统 picker 截图 → Windows OCR 识别 → 复用现有翻译链路在 Web 弹窗展示。

**架构：** `shortcuts` 仅按快捷键分流到 `ui::ocr_popup::start_translation_from_ocr`；后者做 busy 预检与取消/错误/成功分流，通过平台分发缝 `platform::capture_and_recognize` 调用 `WindowsScreenCapture` + `WindowsOcrEngine`，成功后复用 `start_translation_from_input`。不新增前端代码或事件类型。

**技术栈：** Rust / Tauri 2 / `tauri-plugin-global-shortcut` / `windows` crate（`Windows.Graphics.Capture` + `Windows.Media.Ocr`）/ `async-trait` / `tokio`。

**规格：** `docs/superpowers/specs/2026-06-30-screenshot-ocr-end-to-end-design.md`

---

## 文件结构

| 文件 | 职责 | 动作 |
|---|---|---|
| `src-tauri/src/app/state.rs` | `AppState` 增加 `is_translation_busy()` peek | 修改 |
| `src-tauri/src/platform/windows/capture.rs` | `WindowsScreenCapture` 实现 `ScreenCapture` trait | 修改 |
| `src-tauri/src/platform/windows/mod.rs` | 暴露 windows 侧 `capture_and_recognize` | 修改 |
| `src-tauri/src/platform/unsupported.rs` | 非 windows 侧 `capture_and_recognize` stub | 修改 |
| `src-tauri/src/platform/mod.rs` | 平台分发缝入口 | 修改 |
| `src-tauri/src/ui/ocr_popup.rs` | OCR 业务编排 + 错误文案 | 创建 |
| `src-tauri/src/ui/mod.rs` | 注册 `ocr_popup` 模块 | 修改 |
| `src-tauri/src/app/shortcuts.rs` | `Alt+O` 分流与注册 | 修改 |

已有不动：`core::ocr_translation::recognize_capture_for_translation`（fake 已测）、`core::capture::ScreenCapture` trait、`core::ocr::OcrEngine` trait、`ui::web_popup::start_translation_from_input` / `show_translation_error`、`lib.rs`。

**关键类型与签名（全计划统一）：**

```rust
// core/ocr_translation.rs（已有，不改）
pub async fn recognize_capture_for_translation<C, O>(
    capture: &C, ocr: &O, hints: OcrHints,
) -> Result<Option<TranslationInput>, OcrTranslationError>
where C: ScreenCapture, O: OcrEngine

// platform/mod.rs（新增）
pub async fn capture_and_recognize(hints: OcrHints)
    -> Result<Option<TranslationInput>, OcrTranslationError>

// app/state.rs（新增）
impl AppState { pub fn is_translation_busy(&self) -> bool }

// ui/ocr_popup.rs（新增）
pub async fn start_translation_from_ocr(app: tauri::AppHandle, state: AppState)
fn friendly_ocr_error(error: OcrTranslationError) -> String
```

`OcrTranslationError` 变体：`Capture(CaptureError)`、`Ocr(OcrError)`。`CaptureError` 变体：`PermissionDenied`、`NoCaptureTarget`、`UnsupportedPlatform`、`BackendUnavailable(String)`、`ImageConversionFailed(String)`。`OcrError` 变体：`EngineUnavailable`、`LanguageUnavailable(String)`、`ImageTooLarge`、`ImageConversionFailed(String)`、`EmptyResult`、`UnsupportedPlatform`。

---

## 任务 1：`AppState::is_translation_busy` peek 方法

**文件：**
- 修改：`src-tauri/src/app/state.rs`
- 测试：`src-tauri/src/app/state.rs`（同文件 `#[cfg(test)] mod tests`）

- [ ] **步骤 1：编写失败的测试**

在 `src-tauri/src/app/state.rs` 的 `mod tests` 末尾追加：

```rust
    #[test]
    fn is_translation_busy_reflects_begin_and_finish() {
        let state = app_state();

        assert!(!state.is_translation_busy(), "初始不应处于 busy");

        state.try_begin_translation().expect("开始翻译");
        assert!(state.is_translation_busy(), "begin 后应处于 busy");

        state.finish_translation().expect("结束翻译");
        assert!(!state.is_translation_busy(), "finish 后应退出 busy");
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib app::state::tests::is_translation_busy_reflects_begin_and_finish`
预期：编译失败，报错 `no method named is_translation_busy found`

- [ ] **步骤 3：编写最少实现代码**

在 `src-tauri/src/app/state.rs` 的 `impl AppState` 块中，紧接 `finish_translation` 方法之后追加：

```rust
    pub fn is_translation_busy(&self) -> bool {
        self.translation_busy
            .lock()
            .map(|busy| *busy)
            .unwrap_or(false)
    }
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib app::state::tests::is_translation_busy_reflects_begin_and_finish`
预期：PASS

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/app/state.rs
git commit -m "feat(state): 增加 translation_busy 的 peek 方法"
```

---

## 任务 2：`WindowsScreenCapture` 实现 `ScreenCapture` trait

**文件：**
- 修改：`src-tauri/src/platform/windows/capture.rs`

注意：`capture_full_screen` 已存在且返回 `Result<Option<CapturedImage>, CaptureError>`，与 `capture_interactive` 签名一致。本任务仅加 trait impl，不改 `capture_full_screen`。

- [ ] **步骤 1：补 import 与 trait impl**

在 `src-tauri/src/platform/windows/capture.rs` 顶部 `use` 区，把第一行：

```rust
use crate::core::capture::{CapturedImage, CaptureError};
```

改为：

```rust
use crate::core::capture::{CaptureRegion, CapturedImage, CaptureError, ScreenCapture};
```

在 `impl WindowsScreenCapture` 块结束之后（`capture_full_screen` 所属的 `impl` 块之后，文件中 `struct BgraBufferLayout` 之前）追加 trait 实现：

```rust
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
```

- [ ] **步骤 2：运行构建验证编译**

运行：`cd src-tauri && cargo build`
预期：编译成功（`ScreenCapture`、`CaptureRegion` 已在 `core::capture` 导出；`async_trait` 已是项目依赖）。

- [ ] **步骤 3：运行全量测试确认无回归**

运行：`cd src-tauri && cargo test`
预期：现有测试全部 PASS，ignored 测试仍跳过。

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/platform/windows/capture.rs
git commit -m "feat(capture): WindowsScreenCapture 接入 ScreenCapture trait"
```

---

## 任务 3：平台分发缝 `capture_and_recognize`（非 windows stub）

先写非 windows stub 与测试，再写 windows 实现。stub 让 `ui::ocr_popup` 可平台无关编写，且非 windows CI 可验证 `UnsupportedPlatform` 行为。

**文件：**
- 修改：`src-tauri/src/platform/unsupported.rs`
- 修改：`src-tauri/src/platform/mod.rs`
- 测试：`src-tauri/src/platform/unsupported.rs`（同文件 `#[cfg(test)] mod tests`）

- [ ] **步骤 1：编写失败的测试（非 windows 行为）**

在 `src-tauri/src/platform/unsupported.rs` 末尾追加：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        capture::CaptureError,
        ocr::OcrHints,
        ocr_translation::OcrTranslationError,
    };

    #[tokio::test]
    async fn capture_and_recognize_unsupported_on_non_windows() {
        let error = capture_and_recognize(OcrHints::default())
            .await
            .expect_err("非 windows 平台应返回错误");

        assert!(matches!(
            error,
            OcrTranslationError::Capture(CaptureError::UnsupportedPlatform)
        ));
    }
}
```

> 说明：此测试带 `#[cfg(not(target_os = "windows"))]` 语义——它在非 windows 平台才有意义；在 windows 开发机上 `cargo test` 不会编译该文件（`platform/mod.rs` 用 `#[cfg(not(target_os="windows"))]` 选择 `unsupported` 模块）。windows 开发机本任务步骤 2 的构建会跳过此文件，步骤 3 的测试在 windows 上不运行此用例，属预期。

- [ ] **步骤 2：编写非 windows stub 实现**

把 `src-tauri/src/platform/unsupported.rs` 改为：

```rust
use crate::core::{
    capture::CaptureError,
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

pub async fn capture_and_recognize(
    _hints: OcrHints,
) -> Result<Option<TranslationInput>, OcrTranslationError> {
    Err(OcrTranslationError::Capture(CaptureError::UnsupportedPlatform))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        capture::CaptureError,
        ocr::OcrHints,
        ocr_translation::OcrTranslationError,
    };

    #[tokio::test]
    async fn capture_and_recognize_unsupported_on_non_windows() {
        let error = capture_and_recognize(OcrHints::default())
            .await
            .expect_err("非 windows 平台应返回错误");

        assert!(matches!(
            error,
            OcrTranslationError::Capture(CaptureError::UnsupportedPlatform)
        ));
    }
}
```

- [ ] **步骤 3：在 `platform/mod.rs` 暴露分发入口**

`src-tauri/src/platform/mod.rs` 当前内容：

```rust
#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(not(target_os = "windows"))]
pub mod unsupported;
```

改为：

```rust
#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(not(target_os = "windows"))]
pub mod unsupported;

pub use crate::core::ocr_translation::OcrTranslationError;

#[cfg(target_os = "windows")]
pub use windows::capture_and_recognize;

#[cfg(not(target_os = "windows"))]
pub use unsupported::capture_and_recognize;
```

- [ ] **步骤 4：运行构建验证编译（windows 开发机会因缺 windows 侧实现而失败——预期）**

运行：`cd src-tauri && cargo build`
预期（windows 开发机）：编译失败，报错 `cannot find function capture_and_recognize in module windows` 或类似——因为任务 4 才补 windows 侧实现。这是本任务与任务 4 之间的已知中间态。

> 若希望本任务独立可编译，可暂在 `platform/windows/mod.rs` 加临时 `pub async fn capture_and_recognize(...) -> unimplemented!()`，但任务 4 会立即覆盖它。**推荐跳过临时实现，直接进入任务 4**，两任务合并 commit。下面步骤 5 的 commit 在任务 4 完成后一起执行。

- [ ] **步骤 5：暂不 commit**（与任务 4 合并提交）

---

## 任务 4：平台分发缝 `capture_and_recognize`（windows 实现）

**文件：**
- 修改：`src-tauri/src/platform/windows/mod.rs`

- [ ] **步骤 1：编写 windows 侧实现**

`src-tauri/src/platform/windows/mod.rs` 当前内容：

```rust
pub mod capture;
pub mod ocr;
```

改为：

```rust
pub mod capture;
pub mod ocr;

use crate::core::{
    ocr::OcrHints,
    ocr_translation::{recognize_capture_for_translation, OcrTranslationError},
    translation::TranslationInput,
};
use capture::WindowsScreenCapture;
use ocr::WindowsOcrEngine;

pub async fn capture_and_recognize(
    hints: OcrHints,
) -> Result<Option<TranslationInput>, OcrTranslationError> {
    recognize_capture_for_translation(&WindowsScreenCapture, &WindowsOcrEngine, hints).await
}
```

> 注：`WindowsScreenCapture` 与 `WindowsOcrEngine` 均为单元结构体，可直接构造传引用。`recognize_capture_for_translation` 接受 `&C: ScreenCapture` / `&O: OcrEngine`，trait 已在任务 2 为 `WindowsScreenCapture` 落地，`WindowsOcrEngine` 的 trait impl 已存在于 `platform/windows/ocr.rs`。

- [ ] **步骤 2：运行构建验证编译**

运行：`cd src-tauri && cargo build`
预期：编译成功，任务 3 步骤 4 的失败已消除。

- [ ] **步骤 3：运行全量测试**

运行：`cd src-tauri && cargo test`
预期：现有测试全部 PASS。非 windows 用例在 windows 开发机上不编译/不运行，属预期。

- [ ] **步骤 4：Commit（任务 3 + 任务 4 合并）**

```bash
git add src-tauri/src/platform/unsupported.rs src-tauri/src/platform/mod.rs src-tauri/src/platform/windows/mod.rs
git commit -m "feat(platform): 新增 capture_and_recognize 平台分发缝"
```

---

## 任务 5：`ui::ocr_popup` OCR 业务编排与错误文案

**文件：**
- 创建：`src-tauri/src/ui/ocr_popup.rs`
- 修改：`src-tauri/src/ui/mod.rs`
- 测试：`src-tauri/src/ui/ocr_popup.rs`（同文件 `#[cfg(test)] mod tests`，仅测 `friendly_ocr_error` 纯函数）

- [ ] **步骤 1：编写失败的测试（错误文案映射）**

创建 `src-tauri/src/ui/ocr_popup.rs`：

```rust
use crate::{
    app::state::AppState,
    core::{
        capture::CaptureError,
        ocr::OcrError,
        ocr_translation::OcrTranslationError,
    },
    platform::capture_and_recognize,
    ui::web_popup::{show_translation_error, start_translation_from_input},
};

use crate::core::ocr::OcrHints;

pub async fn start_translation_from_ocr(app: tauri::AppHandle, state: AppState) {
    // ponytail: OCR 阶段不持有 translation_busy；picker 模态天然串行，
    // 翻译阶段仍由 start_translation_from_input 内部 try_begin_translation 保护。
    // busy peek 与 OCR→翻译间存在微小竞态窗口，MVP 可接受；后续可让 OCR 入口占住 busy。
    if state.is_translation_busy() {
        show_translation_error(&app, "正在翻译中，请稍后再试");
        return;
    }

    match capture_and_recognize(OcrHints::default()).await {
        Ok(None) => {} // 用户取消截图，静默
        Ok(Some(input)) => {
            if let Err(error) = start_translation_from_input(input, app, state) {
                show_translation_error(&app, error);
            }
        }
        Err(error) => show_translation_error(&app, friendly_ocr_error(error)),
    }
}

fn friendly_ocr_error(error: OcrTranslationError) -> String {
    match error {
        OcrTranslationError::Capture(CaptureError::UnsupportedPlatform) => {
            "当前平台暂不支持截图 OCR".to_string()
        }
        OcrTranslationError::Capture(CaptureError::NoCaptureTarget) => "未选择截图区域或窗口".to_string(),
        OcrTranslationError::Capture(CaptureError::PermissionDenied) => "无法访问屏幕捕获权限".to_string(),
        OcrTranslationError::Capture(CaptureError::BackendUnavailable(_)) => "截图失败，请稍后重试".to_string(),
        OcrTranslationError::Capture(CaptureError::ImageConversionFailed(_)) => "截图图像转换失败".to_string(),
        OcrTranslationError::Ocr(OcrError::EngineUnavailable) => "系统 OCR 能力不可用".to_string(),
        OcrTranslationError::Ocr(OcrError::LanguageUnavailable(_)) => "缺少 OCR 语言包".to_string(),
        OcrTranslationError::Ocr(OcrError::ImageTooLarge) => "截图区域过大，请缩小区域".to_string(),
        OcrTranslationError::Ocr(OcrError::EmptyResult) => "未识别到文本".to_string(),
        OcrTranslationError::Ocr(OcrError::ImageConversionFailed(_)) => "OCR 图像转换失败".to_string(),
        OcrTranslationError::Ocr(OcrError::UnsupportedPlatform) => "当前平台暂不支持截图 OCR".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{capture::CaptureError, ocr::OcrError, ocr_translation::OcrTranslationError};

    #[test]
    fn friendly_error_maps_empty_result() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Ocr(OcrError::EmptyResult)),
            "未识别到文本"
        );
    }

    #[test]
    fn friendly_error_maps_unsupported_platform() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Capture(CaptureError::UnsupportedPlatform)),
            "当前平台暂不支持截图 OCR"
        );
    }

    #[test]
    fn friendly_error_maps_language_unavailable() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Ocr(OcrError::LanguageUnavailable(
                "zh-Hans-CN".to_string()
            ))),
            "缺少 OCR 语言包"
        );
    }

    #[test]
    fn friendly_error_maps_backend_unavailable() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Capture(CaptureError::BackendUnavailable(
                "boom".to_string()
            ))),
            "截图失败，请稍后重试"
        );
    }
}
```

- [ ] **步骤 2：在 `ui/mod.rs` 注册模块**

`src-tauri/src/ui/mod.rs` 当前内容：

```rust
pub mod config;
pub mod web_popup;
```

改为：

```rust
pub mod config;
pub mod ocr_popup;
pub mod web_popup;
```

- [ ] **步骤 3：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib ui::ocr_popup::tests`
预期：4 个测试 PASS。

> 若报 `cannot find function start_translation_from_input` 等签名不匹配，核对 `web_popup.rs` 导出的函数名（`start_translation_from_input`、`show_translation_error` 均为 `pub`）。

- [ ] **步骤 4：运行全量构建与测试**

运行：`cd src-tauri && cargo build && cargo test`
预期：编译成功，全部非 ignored 测试 PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/ui/ocr_popup.rs src-tauri/src/ui/mod.rs
git commit -m "feat(ui): 新增 OCR 翻译编排入口与错误文案映射"
```

---

## 任务 6：`shortcuts.rs` 分流 `Alt+T` / `Alt+O` 并注册 `Alt+O`

**文件：**
- 修改：`src-tauri/src/app/shortcuts.rs`

`tauri-plugin-global-shortcut` 的 `with_handler` 闭包签名是 `|app, shortcut: &Shortcut, event|`，`Shortcut` 实现 `Display`，`register("Alt+T")` 解析出的触发 shortcut 字符串形式为 `"Alt+T"`，故用 `shortcut.to_string()` 字符串比较分流。

- [ ] **步骤 1：改写 `shortcuts.rs`**

把 `src-tauri/src/app/shortcuts.rs` 整体替换为：

```rust
use std::{thread, time::Duration};

use tauri::{Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::{
    app::state::AppState,
    core::{selection::copy_selected_text, translation::TranslationInput},
    ui::{
        ocr_popup::start_translation_from_ocr,
        web_popup::{show_translation_error, start_translation_from_input},
    },
};

pub fn register_global_shortcuts(app: &tauri::App) -> Result<(), tauri_plugin_global_shortcut::Error> {
    app.global_shortcut().register("Alt+T")?;
    app.global_shortcut().register("Alt+O")
}

pub fn handle_global_shortcut(
    app: &tauri::AppHandle,
    shortcut: &tauri_plugin_global_shortcut::Shortcut,
    event: tauri_plugin_global_shortcut::ShortcutEvent,
) {
    if event.state != ShortcutState::Released {
        return;
    }

    match shortcut.to_string().as_str() {
        "Alt+O" => {
            let app_handle = app.clone();
            let state: State<'_, AppState> = app_handle.state();
            let state = state.inner().clone();
            tauri::async_runtime::spawn(async move {
                start_translation_from_ocr(app_handle, state).await;
            });
        }
        _ => handle_selection_translate(app),
    }
}

fn handle_selection_translate(app: &tauri::AppHandle) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        thread::sleep(Duration::from_millis(40));

        let selected_text = match copy_selected_text() {
            Ok(text) => text,
            Err(error) => {
                show_translation_error(&app_handle, error.to_string());
                return;
            }
        };

        let state: State<'_, AppState> = app_handle.state();
        if let Err(error) = state.set_pending_source_text(selected_text.clone()) {
            show_translation_error(&app_handle, error);
            return;
        }

        if let Err(error) = start_translation_from_input(
            TranslationInput::SelectedText(selected_text),
            app_handle.clone(),
            state.inner(),
        ) {
            show_translation_error(&app_handle, error);
        }
    });
}
```

> 改动要点：①`handle_global_shortcut` 第二参数从 `_shortcut`（忽略）改为 `shortcut: &Shortcut` 并按字符串分流；②划词流程抽取为 `handle_selection_translate` 保持原行为不变；③`Alt+O` 分支克隆 `AppState`（`AppState: Clone`）传入 async 任务。

- [ ] **步骤 2：更新 `lib.rs` 的 handler 闭包参数**

`src-tauri/src/lib.rs` 第 23-27 行的 handler：

```rust
                .with_handler(|app, _shortcut, event| {
                    handle_global_shortcut(app, event);
                })
```

改为：

```rust
                .with_handler(|app, shortcut, event| {
                    handle_global_shortcut(app, shortcut, event);
                })
```

- [ ] **步骤 3：运行构建验证编译**

运行：`cd src-tauri && cargo build`
预期：编译成功。

- [ ] **步骤 4：运行全量测试**

运行：`cd src-tauri && cargo test`
预期：全部非 ignored 测试 PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/app/shortcuts.rs src-tauri/src/lib.rs
git commit -m "feat(shortcuts): 注册 Alt+O 并按快捷键分流划词与 OCR"
```

---

## 任务 7：端到端手动验证与文档同步

**文件：**
- 修改：`docs/architecture/screenshot-ocr-architecture.md`（更新落地状态）

- [ ] **步骤 1：前端语法检查**

运行：`node --check frontend/main.js`
预期：无输出（语法正确）。本切片不改前端，仅确认无回归。

- [ ] **步骤 2：全量自动化验证**

运行：`cd src-tauri && cargo test && cargo build`
预期：全部非 ignored 测试 PASS，release/debug 构建成功。

- [ ] **步骤 3：手动验证（Windows）**

启动：`npm run tauri dev`，逐项验证并记录结果：

1. `Alt+O` 触发 → 系统 picker 弹出 → 选择一个显示器/窗口 → 含中文文本的截图识别并进入翻译弹窗流式展示。
2. 含英文、中英混合文本的截图均能识别并翻译。
3. 在系统 picker 中取消 → 不崩溃、不弹窗、不进入翻译。
4. OCR 识别为空（截一张纯图片无文字区域）→ 弹窗显示「未识别到文本」。
5. 翻译进行中（`Alt+T` 划词翻译未完成时）按 `Alt+O` → picker 弹出前即提示「正在翻译中，请稍后再试」。
6. OCR 成功后译文正常流式产出（provider 不感知输入来源）。
7. `Alt+T` 划词翻译功能不回归。

> 已知简化：当前 `capture_full_screen` 在用户取消系统 picker 时返回 `BackendUnavailable` 错误而非 `Ok(None)`，故验证项 3 可能表现为弹窗提示「截图失败，请稍后重试」而非完全静默。若验证时确认此行为影响体验，记录到「待验证与已知简化」，本切片不强制修复（spec 已声明）。

- [ ] **步骤 4：同步架构文档落地状态**

在 `docs/architecture/screenshot-ocr-architecture.md` 的「全屏单帧截图 Spike 落地状态」章节之后，新增章节：

```markdown
## 截图 OCR 端到端闭环落地状态

截图 OCR 端到端最小闭环已串联完成：

- 已将 `WindowsScreenCapture` 接入 `ScreenCapture` trait（`capture_interactive` 委托 `capture_full_screen`，`capture_region` 暂返回 `UnsupportedPlatform`）。
- 已新增 `platform::capture_and_recognize` 平台分发缝，Windows 侧串联 `WindowsScreenCapture` + `WindowsOcrEngine`，非 Windows 返回 `UnsupportedPlatform`。
- 已新增 `ui::ocr_popup::start_translation_from_ocr`，负责 busy 预检、用户取消静默、OCR 错误文案映射，成功后复用 `start_translation_from_input`。
- 已注册 `Alt+O` 全局快捷键并在 `handle_global_shortcut` 中按快捷键分流划词与 OCR。
- 不新增前端代码或事件类型；OCR 前置失败统一经 `translation:event::Failed` 展示。

已知简化（未在本切片修复）：`capture_full_screen` 在用户取消系统 picker 时返回 `BackendUnavailable` 而非 `Ok(None)`，用户取消当前会触发「截图失败」提示而非静默。区域截图（`capture_region`）仍未实现，留给 DXGI/自建 overlay 阶段。
```

- [ ] **步骤 5：Commit**

```bash
git add docs/architecture/screenshot-ocr-architecture.md
git commit -m "docs(architecture): 同步截图 OCR 端到端闭环落地状态"
```

---

## 自检

**1. 规格覆盖度：**
- trait 接入 → 任务 2 ✓
- 平台分发缝 → 任务 3+4 ✓
- `start_translation_from_ocr` + busy peek + 错误文案 → 任务 1+5 ✓
- shortcuts 分流 + 注册 Alt+O → 任务 6 ✓
- 不新增事件/前端 → 任务 5/6 设计体现，任务 7 步骤 1 确认 ✓
- 测试策略（is_translation_busy、capture_and_recognize 非 windows、friendly_ocr_error、手动验证）→ 任务 1/3/5/7 ✓
- 文档同步 → 任务 7 步骤 4 ✓

**2. 占位符扫描：** 无 TODO/待定；任务 3 步骤 4 的「暂不 commit」是明确的合并提交策略，非占位；任务 7 步骤 3 的已知简化有 spec 依据。✓

**3. 类型一致性：**
- `capture_and_recognize(hints: OcrHints) -> Result<Option<TranslationInput>, OcrTranslationError>` 在任务 3/4/5 一致 ✓
- `is_translation_busy(&self) -> bool` 任务 1 定义、任务 5 使用 ✓
- `start_translation_from_ocr(app: tauri::AppHandle, state: AppState)` 任务 5 定义、任务 6 调用（传 `app_handle, state`）✓
- `handle_global_shortcut(app, shortcut: &Shortcut, event)` 任务 6 定义、`lib.rs` 任务 6 步骤 2 调用一致 ✓
- `friendly_ocr_error(error: OcrTranslationError) -> String` 任务 5 定义并测试 ✓
- `OcrTranslationError`/`CaptureError`/`OcrError` 变体在任务 5 的 match 中穷尽（含所有变体）✓

无遗漏，类型一致。
