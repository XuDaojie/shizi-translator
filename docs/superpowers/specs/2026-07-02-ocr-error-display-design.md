# OCR 错误状态展示与重试一致性 设计规格

- 日期：2026-07-02
- 阶段：里程碑 2（系统原生能力扩展）收尾任务 6
- 类型：增量改进（前端展示 + 后端文案）

## 背景与动机

里程碑 2 截图 OCR 主链路已落地（`Alt+O` → DXGI 抓帧 → overlay 框选 → Windows.Media.Ocr → 翻译弹窗），但 roadmap 里程碑 2 任务 6「加入 OCR 错误状态展示，例如无语言包、权限不足、识别为空」尚未真正落地为用户可感知的体验。当前存在的实际缺口：

1. **重试按钮恒显但点击必报错**：`show_translation_error` 发出的 `Failed` 事件携带 `retryable: false`，但 [translate.js](../../../frontend/translate.js) 的 `failed` 分支无视 `payload.retryable`，恒置 `canRetry: true`。OCR 失败（无语言包 / 识别为空 / 区域过大）路径根本未缓存 `last_translation_input`，点击「重试」会触发 `retry_translation` 报「没有可重试的翻译」，再弹一次错，形成误导循环。
2. **缺阶段区分**：OCR 失败与 LLM 翻译失败在弹窗中都显示为同样的红字消息，用户无法直观判断是截图 OCR 阶段挂了还是模型调用阶段挂了。
3. **缺可操作指引**：现有友好文案只说「缺少 OCR 语言包」「未识别到文本」，未告诉用户下一步该做什么（去哪装语言包、要不要重新截图）。

## 目标

- OCR 失败时给出**阶段前缀 + 原因 + 可操作指引**的可操作错误文案。
- 翻译弹窗在不可重试错误（OCR 失败）下**隐藏重试按钮**，避免误导。
- 不改动 `TranslationEvent::Failed` 事件结构，不新增 IPC，不新增 UI 组件（YAGNI）。

## 非目标

- 不引入独立错误展示组件、阶段徽章、错误码体系。
- 不为「重新截图」单独加按钮或 IPC 通路（OCR 失败统一引导用户重新按 `Alt+O`）。
- 不修复截图 OCR 的多屏 / 混合 DPI 已知问题（属另一独立任务）。
- 不调整 LLM 翻译失败的展示逻辑（本次仅让其重试按钮尊重 `retryable` 字段，文案不动）。

## 方案

采用纯文案增强 + 前端读 `retryable` 字段，不扩展事件结构。

### 1. 后端文案增强

文件：[src-tauri/src/ui/ocr_popup.rs](../../../src-tauri/src/ui/ocr_popup.rs) `friendly_ocr_error`

把干巴巴的原因扩成「阶段前缀 + 原因 + 可操作指引」格式。OCR 类错误统一加 `OCR 识别失败：` 前缀，Capture 类错误统一加 `截图失败：` 前缀。

| 错误变体 | 新文案 |
|---|---|
| `Ocr(EmptyResult)` | `OCR 识别失败：未识别到文本。请重新按 Alt+O 框选更清晰的区域。` |
| `Ocr(LanguageUnavailable(_))` | `OCR 识别失败：缺少 OCR 语言包。请在「Windows 设置 > 时间和语言 > 语言」安装对应 OCR 语言包后重试。` |
| `Ocr(ImageTooLarge)` | `OCR 识别失败：截图区域过大，请缩小区域后重新按 Alt+O 截图。` |
| `Ocr(EngineUnavailable)` | `OCR 识别失败：系统 OCR 能力不可用。` |
| `Ocr(ImageConversionFailed(_))` | `OCR 识别失败：图像转换失败，请重新截图。` |
| `Ocr(UnsupportedPlatform)` | `OCR 识别失败：当前平台暂不支持截图 OCR。` |
| `Capture(UnsupportedPlatform)` | `截图失败：当前平台暂不支持截图 OCR。` |
| `Capture(NoCaptureTarget)` | `截图失败：未选择截图区域或窗口。` |
| `Capture(PermissionDenied)` | `截图失败：无法访问屏幕捕获权限。` |
| `Capture(BackendUnavailable(detail))` | `截图失败，请稍后重试（{detail}）` |
| `Capture(ImageConversionFailed(detail))` | `截图失败：图像转换失败（{detail}）` |

### 2. 前端重试一致性

文件：[frontend/translate.js](../../../frontend/translate.js) `failed` 分支（约 117-126 行）

将恒置 `canRetry: true` 改为读 `payload.retryable`：

```js
case 'failed':
  if (currentSessionId && !shouldHandleSessionEvent(payload)) return;
  outputText.textContent = payload.message ?? '翻译失败';
  outputText.style.color = '#b42318';
  currentSessionId = null;
  hideSourceBadge();
  hideUsageFooter();
  setActionButtons({ translating: false, canRetry: payload.retryable !== false });
  scrollOutputToBottom();
  break;
```

语义：

- `payload.retryable === false`（OCR 失败，由 `show_translation_error` 发出）→ 隐藏重试按钮。
- `payload.retryable === true` 或字段缺省（LLM 失败、旧事件兼容）→ 显示重试按钮。

### 3. 不改动项

- `show_translation_error` 已发 `retryable: false`，不动。
- `TranslationEvent::Failed` 结构不动（不加 `stage` 字段）。
- 不新增 IPC command、不新增前端 UI 组件。
- `start_translation_from_ocr` 中 `is_translation_busy` / `try_begin_capture` 失败的 `show_translation_error` 调用复用增强后文案，无须单独处理（这些路径走的是 `friendly_ocr_error` 之外的字符串，文案不变）。

## 数据流

```
submit_capture_region / start_translation_from_ocr
  └─ recognize_region 失败
       └─ friendly_ocr_error(OcrTranslationError) -> "OCR 识别失败：…可操作指引…"
            └─ show_translation_error(app, msg)
                 ├─ emit Failed { message: msg, retryable: false }
                 └─ 前端 failed 分支
                      ├─ outputText 渲染红字文案
                      └─ setActionButtons(canRetry: false) → 隐藏重试按钮
```

## 测试

### Rust 单元测试

文件：[src-tauri/src/ui/ocr_popup.rs](../../../src-tauri/src/ui/ocr_popup.rs) `tests`

更新现有 4 个断言文案，并补齐覆盖：

- `friendly_error_maps_empty_result` → 断言新文案「…未识别到文本。请重新按 Alt+O…」
- `friendly_error_maps_language_unavailable` → 断言新文案「…缺少 OCR 语言包。请在「Windows 设置…」
- `friendly_error_maps_unsupported_platform` → 断言新文案（Capture 路径，`截图失败：` 前缀）
- `friendly_error_maps_backend_unavailable` → 断言新文案（`截图失败，请稍后重试（boom）`）
- 新增 `friendly_error_maps_image_too_large` → 断言「OCR 识别失败：截图区域过大…」
- 新增 `friendly_error_maps_engine_unavailable` → 断言「OCR 识别失败：系统 OCR 能力不可用。」
- 新增 `friendly_error_maps_ocr_image_conversion_failed` → 断言「OCR 识别失败：图像转换失败，请重新截图。」
- 新增 `friendly_error_maps_ocr_unsupported_platform` → 断言「OCR 识别失败：当前平台暂不支持截图 OCR。」

### 前端验证

- `node --check frontend/translate.js` 语法检查通过。
- 手动验证（mock 模式触发 OCR 空结果 / 越界裁剪）：弹窗显示新文案且重试按钮隐藏。

### 验证命令

```bash
cd src-tauri && cargo test
cd src-tauri && cargo build
node --check frontend/translate.js
```

## 文档同步

- README「当前能力」补充：「OCR 失败给出可操作错误指引（缺语言包 / 识别为空 / 区域过大等），并隐藏无意义的重试按钮。」
- roadmap 里程碑 2 任务 6 回填 ✅。
- CLAUDE.md / AGENTS.md 无须改动（架构与命令未变）。

## 验收标准

- OCR 失败（空结果 / 无语言包 / 区域过大 / 引擎不可用 / 图像转换失败）时，翻译弹窗显示带「OCR 识别失败：」前缀的可操作文案。
- OCR 失败时翻译弹窗不显示重试按钮。
- LLM 翻译失败时仍显示重试按钮（行为不回归）。
- `cargo test` 与 `node --check` 通过。
- README 与 roadmap 同步更新。
