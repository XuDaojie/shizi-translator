# 截图 OCR 端到端最小闭环设计

## 背景与目标

里程碑 2 前序切片已完成架构阶段 1-4：

- `TranslationInput`（含 `OcrText`）已落地，手动输入与划词翻译共用统一输入模型。
- `ScreenCapture` / `OcrEngine` core 抽象已建立。
- `WindowsOcrEngine` 已实现 `OcrEngine`，支持 BGRA/RGBA 到 Windows OCR 输入的转换。
- `WindowsScreenCapture::capture_full_screen()` 已验证 `Windows.Graphics.Capture` 单帧截图，但**尚未接入 `ScreenCapture` trait**。
- `recognize_capture_for_translation` 编排函数已存在，使用 fake capture / fake OCR 验证过截图取消、OCR 空文本和 OCR 文本进入翻译输入。
- `start_translation_from_input` 已接受 `TranslationInput::OcrText`。

本切片的目标是把上述零件串联成端到端最小闭环（架构阶段 5）：

```text
Alt+O 快捷键
  -> app/shortcuts 分流到 OCR 入口
  -> ui::ocr_popup::start_translation_from_ocr
  -> platform::capture_and_recognize (Windows: WindowsScreenCapture + WindowsOcrEngine)
  -> recognize_capture_for_translation
  -> TranslationInput::OcrText
  -> start_translation_from_input
  -> TranslationService
  -> translation:event (Started/Delta/Finished/Failed)
  -> Web popup
```

禁止路线（沿用架构文档）：OCR → frontend → frontend calls start_translation。

## 非目标

- 不实现 `capture_region` 的真实区域框选（MVP 仅交互式 picker，DXGI/overlay 留给后续切片）。
- 不引入 OCR 语言配置项或语言引导文案。
- 不新增前端代码或新事件类型；OCR 前置状态/失败统一复用现有 `translation:event` 的 `Failed` 与 `Started`。
- 不回填 `image_id`，`TranslationInput::OcrText` 的 `image_id` 保持 `None`。
- 不做取消、重试、历史记录、usage 统计。
- 不修改 `pot-desktop/`。

## 现状关键事实

- `WindowsScreenCapture::capture_full_screen() -> Result<Option<CapturedImage>, CaptureError>` 已实现，返回类型与 `ScreenCapture::capture_interactive` 一致；用户在系统 picker 取消时当前实现会返回 picker 错误而非 `Ok(None)`——这是已知 spike 简化，本切片在 trait 实现层记录该行为，不强制本切片修复（见「待验证与已知简化」）。
- `platform::unsupported` 仅有 `GraphicsCaptureProbe` stub，无 `ScreenCapture`/`OcrEngine` 实现。`WindowsScreenCapture`/`WindowsOcrEngine` 仅存在于 `platform::windows`。因此需要一条平台分发缝。
- `handle_global_shortcut` 当前只处理 `Alt+T` 划词分支。
- `start_translation_from_input` 内部已做 `try_begin_translation` 与空文本校验，并复用 `show_window` + `emit_translation_event`。OCR 入口在成功路径上直接调用它，无需重写翻译编排。
- `capabilities/default.json` 已含 `global-shortcut:default`，新增 `Alt+O` 无需额外授权。
- `OcrHints::default()`（空 preferred_languages）会让 `WindowsOcrEngine` 回退到 `TryCreateFromUserProfileLanguages`，中文/英文系统通常可用，MVP 默认即可。

## 架构方案

### 模块边界

| 单元 | 职责 |
|---|---|
| `core::ocr_translation::recognize_capture_for_translation` | 已有：capture → OCR → 取消/空文本/成功 → `TranslationInput`。fake 已测，本切片不重写。 |
| `platform::capture_and_recognize`（新增） | 平台分发缝：Windows 走 `WindowsScreenCapture`+`WindowsOcrEngine`，非 Windows 返回 `UnsupportedPlatform`。 |
| `ui::ocr_popup::start_translation_from_ocr`（新增） | OCR 业务编排：busy 预检 → 调平台缝 → 取消/错误/成功分流 → 复用 `start_translation_from_input`。 |
| `app::shortcuts` | 仅按快捷键分流，不承载 OCR 业务。 |
| `app::state::AppState` | 增加 `is_translation_busy()` peek 方法。 |

理由：与现有 `web_popup` 拥有翻译入口、`shortcuts` 只分流的边界保持一致；编排核心已 fake 测，UI 层薄、可手动验证；不抽 `OcrPipeline` 结构体（单一 capture+ocr 组合用不上，YAGNI）。

### 1. `WindowsScreenCapture` 接入 `ScreenCapture` trait

在 `platform/windows/capture.rs` 增加：

```rust
#[async_trait::async_trait]
impl ScreenCapture for WindowsScreenCapture {
    async fn capture_region(&self, _region: CaptureRegion) -> Result<CapturedImage, CaptureError> {
        Err(CaptureError::UnsupportedPlatform) // ponytail: 区域截图留给 DXGI/overlay 阶段，MVP 仅交互式
    }

    async fn capture_interactive(&self) -> Result<Option<CapturedImage>, CaptureError> {
        self.capture_full_screen().await
    }
}
```

保留 `capture_full_screen` 固有方法，不破坏现有 ignored spike 测试。`capture_region` 返回 `UnsupportedPlatform` 是有意的 MVP 简化，注释标明升级路径。

### 2. 平台分发缝 `platform::capture_and_recognize`

新增统一函数，签名与 `recognize_capture_for_translation` 对齐但内置具体实现：

```rust
pub async fn capture_and_recognize(
    hints: OcrHints,
) -> Result<Option<TranslationInput>, OcrTranslationError>
```

- `#[cfg(target_os = "windows")]`：调 `recognize_capture_for_translation(&WindowsScreenCapture, &WindowsOcrEngine, hints)`。
- `#[cfg(not(target_os = "windows"))]`：返回 `Err(OcrTranslationError::Capture(CaptureError::UnsupportedPlatform))`。

这迫使 trait impl 落地，并让 `ui::ocr_popup` 平台无关。函数放在 `platform/mod.rs` 或 `platform/windows/mod.rs` + `platform/unsupported.rs`，按现有 windows/unsupported 拆分惯例。

### 3. `ui::ocr_popup::start_translation_from_ocr`

```rust
pub async fn start_translation_from_ocr(app: tauri::AppHandle, state: AppState) {
    if state.is_translation_busy() {
        show_translation_error(&app, "正在翻译中，请稍后再试");
        return;
    }

    match capture_and_recognize(OcrHints::default()).await {
        Ok(None) => {}  // 用户取消，静默
        Ok(Some(input)) => {
            if let Err(error) = start_translation_from_input(input, app, state) {
                show_translation_error(&app, error);  // start_translation_from_input 内部已 try_begin，此处仅兜底
            }
        }
        Err(error) => show_translation_error(&app, friendly_ocr_error(error)),
    }
}
```

错误文案映射（`friendly_ocr_error`）：

| 错误 | 文案 |
|---|---|
| `OcrTranslationError::Capture(UnsupportedPlatform)` | 当前平台暂不支持截图 OCR |
| `OcrTranslationError::Capture(NoCaptureTarget)` | 未选择截图区域或窗口 |
| `OcrTranslationError::Capture(BackendUnavailable(_))` | 截图失败，请稍后重试 |
| `OcrTranslationError::Capture(ImageConversionFailed(_))` | 截图图像转换失败 |
| `OcrTranslationError::Capture(PermissionDenied)` | 无法访问屏幕捕获权限 |
| `OcrTranslationError::Ocr(EngineUnavailable)` | 系统 OCR 能力不可用 |
| `OcrTranslationError::Ocr(LanguageUnavailable(_))` | 缺少 OCR 语言包 |
| `OcrTranslationError::Ocr(ImageTooLarge)` | 截图区域过大，请缩小区域 |
| `OcrTranslationError::Ocr(EmptyResult)` | 未识别到文本 |
| `OcrTranslationError::Ocr(ImageConversionFailed(_))` | OCR 图像转换失败 |

注意：OCR 阶段不持有 `translation_busy`。系统 picker 是模态的，天然串行；翻译阶段仍由 `start_translation_from_input` 内部 `try_begin_translation` 保护。busy peek 与 OCR→翻译之间存在微小竞态窗口（picker 关闭到 try_begin 之间），MVP 可接受，注释标注升级路径（如 OCR 入口本身占住 busy）。

### 4. `AppState::is_translation_busy`

```rust
pub fn is_translation_busy(&self) -> bool {
    self.translation_busy.lock().map(|busy| *busy).unwrap_or(false)
}
```

peek 方法，供 OCR 入口在不获取 busy 锁的情况下判断是否值得启动 picker。测试策略明确要求「busy 下 OCR 入口返回明确错误」，peek 让 OCR 入口能在弹出 picker 之前就拒绝。

### 5. `shortcuts.rs` 分流

`handle_global_shortcut` 按快捷键参数分支（`ShortcutEvent` 携带具体 shortcut）：

- `Alt+T` → 现有划词流程（不变）。
- `Alt+O` → `tauri::async_runtime::spawn(start_translation_from_ocr(app_handle, state))`。

`register_global_shortcuts` 增注册 `Alt+O`。需确认 `tauri-plugin-global-shortcut` handler 能拿到触发快捷键以分流（见「待验证」）。

### 6. UI / 事件

不新增事件类型，不新增前端代码。

- OCR 前置失败统一经 `show_translation_error` 发 `translation:event::Failed`。
- 成功路径复用 `start_translation_from_input` 的 `Started/Delta/Finished`。
- picker 期间无弹窗；用户取消无任何弹窗。

## 测试策略

### 单元测试

- `AppState::is_translation_busy`：初始 false；begin 后 true；finish 后 false。
- `capture_and_recognize`：非 Windows 平台返回 `OcrTranslationError::Capture(CaptureError::UnsupportedPlatform)`（`#[cfg(not(windows))]` 可在 CI 验证）。Windows 平台真实调用依赖系统环境，仅靠现有 ignored 集成测试覆盖，不新增默认运行的 Windows 测试。
- 现有 `recognize_capture_for_translation` fake 测试已覆盖取消/空文本/成功，不重写。

### 手动验证

- Windows 上 `Alt+O` 触发 → 系统 picker 选屏 → 含中文、英文、中英混合文本的截图能识别并进入翻译。
- 用户取消截图不崩溃、不弹窗、不进入翻译。
- OCR 为空时显示「未识别到文本」。
- busy 状态下 `Alt+O` 在弹出 picker 前即拒绝，提示「正在翻译中，请稍后再试」。
- OCR 成功后 provider 不感知输入来源，按普通文本翻译。
- 现有 `Alt+T` 划词翻译不回归。

### 验证命令

```bash
cd src-tauri && cargo test
cd src-tauri && cargo build
node --check frontend/main.js
```

## 待验证与已知简化

- `tauri-plugin-global-shortcut` handler 是否能在单一 handler 内按触发的 shortcut 字符串分流 `Alt+T` / `Alt+O`；若不能，则改用 `with_handler` 注册两个独立 shortcut → 各自 handler（实现阶段确认，不影响设计边界）。
- `WindowsScreenCapture::capture_full_screen` 当前在用户取消系统 picker 时返回 picker 错误（`BackendUnavailable`）而非 `Ok(None)`。这意味着「用户取消」在当前 spike 下会被映射为「截图失败，请稍后重试」而非静默。本切片在 trait impl 层记录此行为；是否在本切片内把取消修正为 `Ok(None)`，留待实现阶段按改动量决定（ ponytail：若修正只需在 `pick_capture_item` 区分 `None` 与错误则顺手修，否则记为后续）。
- 系统 picker owner window handle、多显示器、DPI 缩放对坐标的影响：MVP 用 picker 全屏单帧，不涉及区域坐标，本切片不处理。

## 进入实现计划的门槛

本设计已确认：方案 A 模块边界、busy 用 peek 而非持有、错误文案映射。下一步调用 writing-plans 创建实现计划。
