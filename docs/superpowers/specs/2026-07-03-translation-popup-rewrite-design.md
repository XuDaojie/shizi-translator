# 翻译弹窗 UI 重写设计

> 日期：2026-07-03
> 状态：待实现
> 策略：按 OpenDesign 原型整套重写 translate.html/js/css + 窗口去标题栏 + 最小后端封装

## 1. 背景与目标

OpenDesign 产出了翻译弹窗高保真原型（`C:\Users\xdj\IdeaProjects\LLM\OpenDesignProjects\shizi\translation-popup.html`），纯原生 HTML/JS/CSS 实现，与 shizi 翻译弹窗技术栈一致。本任务按原型重写翻译弹窗，做视觉打磨。

**目标**：

1. 用原型整套 UI 重写 `frontend/public/translate.html` / `translate.js` / `translate.css`，视觉与交互对齐原型。
2. 去掉 Windows 原生标题栏，顶部工具栏（图钉/设置所在行）作为新标题栏，保留长按拖动功能，样式完全重绘。
3. 已实现能力接现有后端 command（翻译流式、取消重试、设置、截图 OCR），未实现能力（收藏/书签/语言栏）toast 占位，不新增后端业务逻辑。

## 2. 范围

### 改动

- `frontend/public/translate.html` / `translate.js` / `translate.css`：按原型整套重写，保持纯静态（不迁 Vue，符合 roadmap 约定）。
- `src-tauri/src/app/popup_window.rs`：`build_popup` 窗口配置（decorations/transparent/inner_size）+ `compute_popup_position` 尺寸常量。
- `src-tauri/src/ui/ocr_popup.rs`：新增 `trigger_ocr_translation` command（薄封装现有 `start_translation_from_ocr`）。
- `src-tauri/src/lib.rs`：注册 `trigger_ocr_translation`。
- `src-tauri/capabilities/default.json`：补 `core:window:allow-set-always-on-top` + `core:window:allow-set-size`。

### 不做（YAGNI）

- 不碰 overlay.html、设置页、后端翻译/OCR 业务逻辑。
- 不做语言选择/交换/源语言检测（后端无能力，toast 占位）。
- 不做多引擎并行（后端单 provider，单卡片 + 预留多卡数据结构）。
- 不新增 save 之外的后端 command（`trigger_ocr_translation` 是必要封装，非新能力）。
- 不迁 Vue（translate 保持纯静态）。

## 3. 窗口装饰与拖拽

### 3.1 build_popup 配置（popup_window.rs:109-117）

```rust
WebviewWindowBuilder::new(app, POPUP_LABEL, WebviewUrl::App("translate.html".into()))
    .title("Shizi 翻译")
    .inner_size(452.0, 512.0)       // 窗口逻辑宽 452 = .popup 420 + body padding 32；初始高 512 = 首屏内容 + padding
    .decorations(false)              // 去原生标题栏
    .transparent(true)               // 圆角 + 阴影需要透明背景
    .resizable(false)                // 尺寸由前端控制，禁用原生边框 resize
    .visible(false)
    .build()
```

- `transparent(true)`：让 CSS `border-radius` 与 `box-shadow` 可见。
- `resizable(false)`：宽固定、高由前端动态 setSize，禁用原生边框 resize。
- `always_on_top` 不在 build 时设，由前端图钉按钮动态切换。

### 3.2 阴影空间

原型 `--shadow-popup: 0 8px 24px`。`transparent` 窗口下 `box-shadow` 超出窗口边界会被裁剪。处理：`body` 设 `padding: 16px` + `background: transparent`，窗口逻辑尺寸 = `.popup` 外尺寸 + 32px。

- `.popup` 宽 420px（与原型一致），窗口逻辑宽 452px。
- 高度自适应时 `setSize` 的 height 含此 padding。

### 3.3 拖拽

- 顶部 `.toolbar` 元素加 `data-tauri-drag-region` 属性（Tauri 2 原生，零 JS）。
- 工具栏内 `.toolbar-btn` 点击不影响拖拽（Tauri 2 的 drag-region 对子元素交互自动让路）。
- 拖拽体验与原生标题栏一致：长按 toolbar 空白区拖动整窗。

### 3.4 窗口尺寸策略（宽固定 420，高自适应）

- 宽度：窗口 452 固定，`.popup` 420 固定。
- 高度：JS `ResizeObserver` 监听 `.popup` 内容高度变化 → `window.setSize({ width: 452, height: popupHeight + 32 })`，上限 = 屏幕逻辑高 × 80%，超出则 `.content` 区 `overflow-y: auto`。
- 初始高度 512（首屏：工具栏 26 + 原文卡 ~120 + 语言栏 32 + 1 结果卡 ~200 + 状态栏 28 + padding/gap + body padding 32）。
- `compute_popup_position` 的 `POPUP_W` 改 452、`POPUP_H` 改 512（定位用，实际高度动态变化不影响定位钳制）。

## 4. UI 结构与功能对接

### 4.1 顶部工具栏（`.toolbar`，`data-tauri-drag-region`）

| 按钮 | 行为 |
|---|---|
| 图钉（左） | `togglePin`：`window.getCurrentWindow().setAlwaysOnTop(b)`，active 态 accent 色 |
| 收藏 | `showToast('功能开发中')` |
| 截图翻译 | `invoke('trigger_ocr_translation')`（command 内部先 hide 本弹窗再截图） |
| 书签 | `showToast('功能开发中')` |
| 设置 | `invoke('open_settings')` |

### 4.2 原文卡片（`.source-card`）

- `.source-input` textarea：`started` 事件回填 `sourceText`；支持手动输入 + Enter（非 Shift）触发 `invoke('start_translation', { text })`（原 translateBtn.click 改为直接 invoke，新 UI 无翻译按钮）。
- 朗读按钮：Web Speech API `speakText(text, lang)`（浏览器原生）。
- 复制按钮：`navigator.clipboard.writeText`。
- 语言徽章：固定文案「自动检测」（纯视觉）。
- 自动高度：`input` 事件 `autoResize`（原型逻辑）。

### 4.3 语言工具栏（`.lang-toolbar`）

- 源语言 / 交换 / 目标语言三按钮均 `showToast('功能开发中')`，纯视觉占位。

### 4.4 翻译结果区（`.results`，单卡片 + 预留多卡）

- 数据结构：`results: [{ engineIcon, engineName, text, usage, collapsed }]`，当前长度恒为 1，预留多卡扩展。
- 引擎图标/名：弹窗加载时 `invoke('get_app_config')` 读 `provider`，映射：
  - `openai-compatible` → OpenAI 图标 / "OpenAI 翻译"
  - `claude` → Claude 图标 / "Claude 翻译"
  - `mock` → 通用图标 / "Mock 翻译"
- 卡片头：引擎图标 + 名称 + 折叠按钮（`toggleCollapse`，grid 动画）。
- 卡片体：`.result-text`（流式追加 + `stream-cursor`）+ `.result-actions`（朗读/复制 + token 显示）。
- token：`finished` 事件显示 `input → output`；`collectUsage` 关闭时不显示。

### 4.5 状态栏（`.status-bar`）与取消/重试

原型状态栏无取消/重试按钮，但后端已有 `cancel_translation` / `retry_translation`，去掉是功能退化。最小侵入方案：状态栏左侧 statusText 之后挂文字按钮。

```
[● 翻译中…  取消]              186 字
[● 翻译完成  重试]              186 字
[● 翻译失败  重试]              186 字
```

- 翻译中（`started`/`delta`）：显示「取消」→ `invoke('cancel_translation')`。
- 完成/失败/取消后：显示「重试」→ `invoke('retry_translation')`。
- 文字按钮样式：`color: var(--fg-2)`，hover `var(--accent)`，无边框。
- 右侧 `charCount`：原文 textarea 字数，`input` 事件更新。

### 4.6 resize handle

- 原型右下角 `.resize-handle`：保留视觉（hover 显现），无功能（`resizable(false)`，高自适应不需要用户拖拽）。

## 5. 后端改动

### 5.1 trigger_ocr_translation command（ocr_popup.rs）

```rust
#[tauri::command]
pub async fn trigger_ocr_translation(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // 先隐藏翻译弹窗，避免被抓进截图帧
    if let Some(popup) = app.get_webview_window(popup_window::POPUP_LABEL) {
        let _ = popup.hide();
    }
    start_translation_from_ocr(app, state.inner().clone()).await;
    Ok(())
}
```

- `start_translation_from_ocr` 签名 `(app: AppHandle, state: AppState)` 已兼容，直接调用。
- 后续 `start_translation_from_input`（框选完成后）内部 `show_translation_popup` 重新 show 并定位。
- 截图翻译时序：command 内 `hide popup` → `capture_screen` → `open_overlay` → 框选 → OCR → 翻译 → 重新 show。

### 5.2 lib.rs 注册

`invoke_handler` 加 `trigger_ocr_translation`。

### 5.3 capabilities/default.json 补权限

```json
"permissions": [
  "core:default",
  "global-shortcut:default",
  "core:window:allow-set-always-on-top",
  "core:window:allow-set-size"
]
```

## 6. 数据流

```
划词 Alt+T / OCR Alt+O / 手动翻译
  → start_translation(_from_input)
  → emit translation:event { Started/Delta/Finished/Failed/Cancelled }
  → translate.js renderTranslationEvent(payload)
  → 更新原文卡 / result-text 流式 / 状态栏 / 取消重试按钮 / token
```

- 弹窗加载：`invoke('take_pending_source_text')` 回填划词/OCR 原文 + `invoke('get_app_config')` 读 provider 映射引擎图标/名。
- 截图翻译按钮：`invoke('trigger_ocr_translation')` → 后端 hide popup → 截图 → overlay → OCR → 翻译 → 重新 show。

### 6.1 translation:event → UI 映射

| 事件 | UI 行为 |
|---|---|
| `started` | 回填原文、sourceBadge（划词/OCR 徽章）、状态栏「翻译中…」、显示取消按钮、清空 result-text、显示 stream-cursor |
| `delta` | 追加到 result-text、自动滚动 |
| `finished` | 设 fullText、显示 actions + token、状态栏「翻译完成」+ 重试按钮、隐藏 cursor |
| `failed` | result-text 红色显示 `payload.message`、状态栏「翻译失败」+ 重试按钮（`retryable !== false`）、隐藏 cursor |
| `cancelled` | result-text 追加「[已取消]」、状态栏更新、重试按钮 |

## 7. 错误处理

| 场景 | 处理 |
|---|---|
| 翻译 `failed` | result-text 红色显示 `payload.message`，状态栏「翻译失败」+ 重试按钮（`retryable !== false` 时） |
| OCR 失败 | 现有 `friendly_ocr_error` 文案经 `translation:event Failed` 推送，弹窗重新 show 后在 result-text 显示 |
| invoke 异常 | `catch` 后 toast 显示 `String(error)` |
| 朗读不支持 | `speechSynthesis` 不存在 → toast「当前浏览器不支持语音朗读」 |
| 复制失败 | `clipboard.writeText` reject → toast「复制失败」 |

## 8. 测试

- **后端**：`trigger_ocr_translation` 注册到 `invoke_handler`（编译期保证）；现有 `start_translation_from_ocr` / `friendly_ocr_error` 单测保持。不新增单测（command 是薄封装）。
- **前端**：纯静态 HTML/JS/CSS，无 vitest 单测（与 overlay 一致）；靠 `tauri dev` 手动验证。
- **手动验证清单**：
  1. 划词翻译流式显示
  2. OCR 翻译流式显示
  3. 手动输入 Enter 翻译
  4. 图钉置顶切换
  5. 截图翻译按钮触发 overlay
  6. 拖拽标题栏
  7. 高度自适应
  8. 取消/重试
  9. 朗读/复制
  10. 折叠卡片

## 9. 文档同步（收尾硬门禁）

- spec：本设计文档。
- README.md：更新翻译弹窗描述（去原生标题栏、自绘标题栏、新视觉）。
- docs/roadmap/progressive-development-plan.md：标注翻译弹窗 UI 打磨完成。
- CLAUDE.md / AGENTS.md：窗口配置变更（decorations/transparent）同步架构关键点。

## 10. 风险

- **transparent 窗口阴影裁剪**：Windows 上 `transparent(true)` + `box-shadow` 超出窗口边界会裁剪。缓解：body padding 16px 留阴影空间，窗口尺寸含 padding。
- **高度自适应闪烁**：频繁 `setSize` 可能有闪烁。缓解：ResizeObserver 防抖（requestAnimationFrame 节流），且仅在高度变化超过阈值时 setSize。
- **Tauri 2 窗口权限**：`set-always-on-top` / `set-size` 需 capabilities 显式授权。缓解：capabilities/default.json 已列出，编译期校验。
- **截图翻译时序**：弹窗未及时 hide 会被抓进截图帧。缓解：command 内部先 hide 再 capture，时序由后端控制。
- **drag-region 与按钮冲突**：Tauri 2 的 `data-tauri-drag-region` 对子元素交互自动让路，已验证行为。若个别按钮误触，补 `stopPropagation`。
