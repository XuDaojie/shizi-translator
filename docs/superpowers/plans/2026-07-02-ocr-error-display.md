# OCR 错误状态展示与重试一致性 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** OCR 失败时给出带「阶段前缀 + 原因 + 可操作指引」的文案，并让翻译弹窗在不可重试错误（`retryable: false`）下隐藏重试按钮。

**架构：** 纯文案增强 + 前端读字段。后端只改 `ocr_popup.rs` 的 `friendly_ocr_error` 文案表；前端只改 `translate.js` 的 `failed` 分支，把恒置 `canRetry: true` 改为 `payload.retryable !== false`。不扩展 `TranslationEvent::Failed` 结构、不加 `stage` 字段、不新增 IPC、不新增 UI 组件。

**技术栈：** Rust（edition 2021）+ Tauri 2；原生静态前端（无构建步骤，`node --check` 验证语法）。

**关联规格：** [docs/superpowers/specs/2026-07-02-ocr-error-display-design.md](../specs/2026-07-02-ocr-error-display-design.md)

---

## 文件结构

| 文件 | 职责 | 操作 |
|---|---|---|
| `src-tauri/src/ui/ocr_popup.rs` | `friendly_ocr_error`：把 `OcrTranslationError` 映射成用户可读文案 | 修改（重写 `friendly_ocr_error` match 体 + 更新/新增测试） |
| `frontend/translate.js` | 翻译弹窗事件处理：`failed` 分支按 `payload.retryable` 决定是否显示重试按钮 | 修改（约 117-126 行 `failed` 分支一行） |
| `README.md` | 「当前能力」补充 OCR 错误指引；「已知限制」不变 | 修改 |
| `docs/roadmap/progressive-development-plan.md` | 里程碑 2 任务 6 回填 ✅ | 修改 |

---

## 任务 1：后端 `friendly_ocr_error` 文案增强（TDD）

**文件：**
- 修改：`src-tauri/src/ui/ocr_popup.rs:64-92`（`friendly_ocr_error` 函数体）
- 测试：`src-tauri/src/ui/ocr_popup.rs:94-136`（`#[cfg(test)] mod tests`）

本任务按 TDD 先改测试（断言新文案，此时会 FAIL），再改实现让测试通过。所有 8 个用例在同一批测试函数中处理。

- [ ] **步骤 1：改写测试模块，断言新文案**

把 `src-tauri/src/ui/ocr_popup.rs` 现有的 `#[cfg(test)] mod tests { ... }` 整块（第 94-136 行）替换为：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{capture::CaptureError, ocr::OcrError, ocr_translation::OcrTranslationError};

    #[test]
    fn friendly_error_maps_empty_result() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Ocr(OcrError::EmptyResult)),
            "OCR 识别失败：未识别到文本。请重新按 Alt+O 框选更清晰的区域。"
        );
    }

    #[test]
    fn friendly_error_maps_language_unavailable() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Ocr(OcrError::LanguageUnavailable(
                "zh-Hans-CN".to_string()
            ))),
            "OCR 识别失败：缺少 OCR 语言包。请在「Windows 设置 > 时间和语言 > 语言」安装对应 OCR 语言包后重试。"
        );
    }

    #[test]
    fn friendly_error_maps_image_too_large() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Ocr(OcrError::ImageTooLarge)),
            "OCR 识别失败：截图区域过大，请缩小区域后重新按 Alt+O 截图。"
        );
    }

    #[test]
    fn friendly_error_maps_engine_unavailable() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Ocr(OcrError::EngineUnavailable)),
            "OCR 识别失败：系统 OCR 能力不可用。"
        );
    }

    #[test]
    fn friendly_error_maps_ocr_image_conversion_failed() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Ocr(OcrError::ImageConversionFailed(
                "boom".to_string()
            ))),
            "OCR 识别失败：图像转换失败，请重新截图。"
        );
    }

    #[test]
    fn friendly_error_maps_ocr_unsupported_platform() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Ocr(OcrError::UnsupportedPlatform)),
            "OCR 识别失败：当前平台暂不支持截图 OCR。"
        );
    }

    #[test]
    fn friendly_error_maps_capture_unsupported_platform() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Capture(
                CaptureError::UnsupportedPlatform
            )),
            "截图失败：当前平台暂不支持截图 OCR。"
        );
    }

    #[test]
    fn friendly_error_maps_capture_no_target() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Capture(CaptureError::NoCaptureTarget)),
            "截图失败：未选择截图区域或窗口。"
        );
    }

    #[test]
    fn friendly_error_maps_capture_permission_denied() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Capture(
                CaptureError::PermissionDenied
            )),
            "截图失败：无法访问屏幕捕获权限。"
        );
    }

    #[test]
    fn friendly_error_maps_backend_unavailable() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Capture(
                CaptureError::BackendUnavailable("boom".to_string())
            )),
            "截图失败，请稍后重试（boom）"
        );
    }

    #[test]
    fn friendly_error_maps_capture_image_conversion_failed() {
        assert_eq!(
            friendly_ocr_error(OcrTranslationError::Capture(
                CaptureError::ImageConversionFailed("boom".to_string())
            )),
            "截图失败：图像转换失败（boom）"
        );
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib ui::ocr_popup::tests`
预期：FAIL，多个用例因文案不匹配报 `assertion failed`（如 `friendly_error_maps_empty_result` 期望新文案，实际得到 `"未识别到文本"`）。

- [ ] **步骤 3：重写 `friendly_ocr_error` 实现以通过测试**

把 `src-tauri/src/ui/ocr_popup.rs` 第 64-92 行的 `friendly_ocr_error` 函数整体替换为：

```rust
pub fn friendly_ocr_error(error: OcrTranslationError) -> String {
    match error {
        OcrTranslationError::Capture(CaptureError::UnsupportedPlatform) => {
            "截图失败：当前平台暂不支持截图 OCR。".to_string()
        }
        OcrTranslationError::Capture(CaptureError::NoCaptureTarget) => {
            "截图失败：未选择截图区域或窗口。".to_string()
        }
        OcrTranslationError::Capture(CaptureError::PermissionDenied) => {
            "截图失败：无法访问屏幕捕获权限。".to_string()
        }
        OcrTranslationError::Capture(CaptureError::BackendUnavailable(detail)) => {
            format!("截图失败，请稍后重试（{detail}）")
        }
        OcrTranslationError::Capture(CaptureError::ImageConversionFailed(detail)) => {
            format!("截图失败：图像转换失败（{detail}）")
        }
        OcrTranslationError::Ocr(OcrError::EngineUnavailable) => {
            "OCR 识别失败：系统 OCR 能力不可用。".to_string()
        }
        OcrTranslationError::Ocr(OcrError::LanguageUnavailable(_)) => {
            "OCR 识别失败：缺少 OCR 语言包。请在「Windows 设置 > 时间和语言 > 语言」安装对应 OCR 语言包后重试。".to_string()
        }
        OcrTranslationError::Ocr(OcrError::ImageTooLarge) => {
            "OCR 识别失败：截图区域过大，请缩小区域后重新按 Alt+O 截图。".to_string()
        }
        OcrTranslationError::Ocr(OcrError::EmptyResult) => {
            "OCR 识别失败：未识别到文本。请重新按 Alt+O 框选更清晰的区域。".to_string()
        }
        OcrTranslationError::Ocr(OcrError::ImageConversionFailed(_)) => {
            "OCR 识别失败：图像转换失败，请重新截图。".to_string()
        }
        OcrTranslationError::Ocr(OcrError::UnsupportedPlatform) => {
            "OCR 识别失败：当前平台暂不支持截图 OCR。".to_string()
        }
    }
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib ui::ocr_popup::tests`
预期：PASS，11 个用例全绿。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/ui/ocr_popup.rs
git commit -m "feat(ocr): friendly_ocr_error 增强为阶段前缀+原因+可操作指引"
```

---

## 任务 2：前端 `failed` 分支按 `retryable` 控制重试按钮

**文件：**
- 修改：`frontend/translate.js:117-126`（`handleTranslationEvent` 的 `case 'failed'` 分支）

- [ ] **步骤 1：修改 `failed` 分支**

把 `frontend/translate.js` 中（约第 117-126 行）的 `case 'failed'` 分支：

```js
    case 'failed':
      if (currentSessionId && !shouldHandleSessionEvent(payload)) return;
      outputText.textContent = payload.message ?? '翻译失败';
      outputText.style.color = '#b42318';
      currentSessionId = null;
      hideSourceBadge();
      hideUsageFooter();
      setActionButtons({ translating: false, canRetry: true });
      scrollOutputToBottom();
      break;
```

替换为（仅改 `canRetry` 一行）：

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

语义：`payload.retryable === false`（OCR 失败，由 `show_translation_error` 发出）→ 隐藏重试按钮；`payload.retryable === true` 或字段缺省（LLM 失败、旧事件兼容）→ 显示重试按钮。

- [ ] **步骤 2：运行语法检查验证通过**

运行：`node --check frontend/translate.js`
预期：无输出，退出码 0（语法正确）。

- [ ] **步骤 3：Commit**

```bash
git add frontend/translate.js
git commit -m "fix(translation): 弹窗 failed 分支按 retryable 控制重试按钮显隐"
```

---

## 任务 3：全量验证

**文件：** 无修改，仅运行验证命令。

- [ ] **步骤 1：Rust 全量测试**

运行：`cd src-tauri && cargo test`
预期：全部用例 PASS（含本计划新增的 `friendly_ocr_error` 11 个用例，无回归）。

- [ ] **步骤 2：Rust 构建**

运行：`cd src-tauri && cargo build`
预期：编译成功，无 warning（除既有无关 warning）。

- [ ] **步骤 3：前端语法检查**

运行：`node --check frontend/translate.js`
预期：退出码 0。

- [ ] **步骤 4：手动验证（可选，需 Windows 环境）**

mock provider 下触发：
1. OCR 空结果（框选纯色区域）→ 弹窗显示「OCR 识别失败：未识别到文本。请重新按 Alt+O 框选更清晰的区域。」且**重试按钮隐藏**。
2. LLM 翻译失败（mock 返回错误）→ 弹窗显示失败消息且**重试按钮显示**（不回归）。

无 commit（本任务仅验证）。

---

## 任务 4：文档同步

**文件：**
- 修改：`README.md`（当前能力区）
- 修改：`docs/roadmap/progressive-development-plan.md:219`

- [ ] **步骤 1：README「当前能力」补充 OCR 错误指引**

在 `README.md` 第 18 行「翻译取消与重试」条目之后插入一条新能力条目（保持与上下文一致的列表项格式）：

找到现有条目：

```markdown
- 翻译取消与重试：流式翻译过程中可取消，失败或取消后可一键重试。
```

在其后新增一行：

```markdown
- 翻译取消与重试：流式翻译过程中可取消，失败或取消后可一键重试。
- OCR 错误指引：截图 OCR 失败（缺语言包 / 识别为空 / 区域过大等）时给出带阶段前缀与可操作指引的错误文案，并隐藏无意义的重试按钮。
```

- [ ] **步骤 2：roadmap 里程碑 2 任务 6 回填**

修改 `docs/roadmap/progressive-development-plan.md` 第 219 行：

```markdown
任务 6：加入 OCR 错误状态展示，例如无语言包、权限不足、识别为空。
```

改为：

```markdown
任务 6：加入 OCR 错误状态展示，例如无语言包、权限不足、识别为空。 ✅
```

- [ ] **步骤 3：Commit**

```bash
git add README.md docs/roadmap/progressive-development-plan.md
git commit -m "docs: 同步 OCR 错误指引能力与 roadmap 任务 6 状态"
```

---

## 自检

**1. 规格覆盖度：**
- 后端文案增强（spec §1）→ 任务 1（11 个变体全覆盖，含 spec 表中全部 11 行）✅
- 前端 `failed` 分支读 `retryable`（spec §2）→ 任务 2 ✅
- 不改动项（spec §3：`show_translation_error` / `TranslationEvent::Failed` / 无新 IPC / 无新 UI 组件）→ 计划未触碰这些，符合 ✅
- Rust 单元测试（spec 测试章节）→ 任务 1 含 11 个用例（spec 列出 8 个命名用例 + 补全 Capture 三个变体共 11）✅
- 前端验证（`node --check` + 手动）→ 任务 2 步骤 2 + 任务 3 步骤 4 ✅
- 验证命令（`cargo test` / `cargo build` / `node --check`）→ 任务 3 ✅
- 文档同步（README + roadmap）→ 任务 4 ✅
- 验收标准逐条 → 任务 1+2 满足文案与重试按钮、任务 3 验证不回归、任务 4 文档 ✅

无遗漏。

**2. 占位符扫描：** 无 TODO / 「待定」/ 「类似任务 N」/ 无代码描述步骤。所有代码步骤均含完整代码块。✅

**3. 类型一致性：**
- `friendly_ocr_error` 签名 `(OcrTranslationError) -> String` 不变 ✅
- 用到的枚举变体 `OcrTranslationError::{Capture, Ocr}`、`CaptureError::{UnsupportedPlatform, NoCaptureTarget, PermissionDenied, BackendUnavailable(String), ImageConversionFailed(String)}`、`OcrError::{EngineUnavailable, LanguageUnavailable(String), ImageTooLarge, EmptyResult, ImageConversionFailed(String), UnsupportedPlatform}` 均与现有代码（ocr_popup.rs:1-6 import + 64-92 实现）一致 ✅
- 前端 `payload.retryable` 字段名与后端 `show_translation_error` 发出的 `retryable: false` 一致（spec §3 已确认该字段已存在）✅
- `setActionButtons({ translating, canRetry })` 调用签名与 `finished`/`cancelled` 分支既有用法一致 ✅

无问题。

---

## 执行交接

计划已完成并保存到 `docs/superpowers/plans/2026-07-02-ocr-error-display.md`。两种执行方式：

**1. 子代理驱动（推荐）** - 每个任务调度一个新的子代理，任务间进行审查，快速迭代

**2. 内联执行** - 在当前会话中使用 executing-plans 执行任务，批量执行并设有检查点

选哪种方式？
