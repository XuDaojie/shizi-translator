# 翻译弹窗 UI 重写实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 按 OpenDesign 原型整套重写 `frontend/public/translate.html` / `translate.js` / `translate.css`，去原生标题栏改为自绘标题栏，接现有后端，最小后端封装。

**架构：** 后端仅新增 `trigger_ocr_translation` command（薄封装 `start_translation_from_ocr`，先 hide 弹窗再截图）+ 窗口配置去标题栏/透明/禁 resize + capabilities 补两个窗口权限；前端整套重写为单卡片 + 多卡预留数据结构，顶部工具栏 `data-tauri-drag-region` 拖拽，宽 452/.popup 420 固定、高自适应（ResizeObserver → `setSize`），图钉/截图翻译/设置/朗读/复制接真实，收藏/书签/语言栏 toast 占位，取消/重试挂状态栏左侧文字按钮。

**技术栈：** Tauri 2（`withGlobalTauri:true`，前端走 `window.__TAURI__`）+ 纯静态 HTML/JS/CSS（不迁 Vue）+ Rust 后端。

**规格文档：** [docs/superpowers/specs/2026-07-03-translation-popup-rewrite-design.md](../specs/2026-07-03-translation-popup-rewrite-design.md)

---

## 关键上下文（供执行者查阅）

- **后端事件结构**（`TranslationEvent`，`#[serde(rename_all="camelCase", tag="type")]`）：
  - `started` → `{ type:"started", sessionId, sourceText, sourceType }`（sourceType ∈ `manualText`/`selectedText`/`ocrText`）
  - `delta` → `{ type:"delta", sessionId, text }`
  - `finished` → `{ type:"finished", sessionId, fullText, usage:{ inputTokens, outputTokens } | null }`
  - `failed` → `{ type:"failed", sessionId, message, retryable }`
  - `cancelled` → `{ type:"cancelled", sessionId }`
- **AppConfig**（`#[serde(rename_all="camelCase")]`）：`invoke('get_app_config')` 返回对象含 `provider` 字段（`"openai-compatible"` / `"claude"` / `"mock"`）。
- **`start_translation_from_ocr` 签名**：`(app: tauri::AppHandle, state: AppState)`（`AppState` 实现Clone，`state.inner().clone()` 可得）。command 内拿到的是 `tauri::State<'_, AppState>`。
- **OCR 完成后弹窗重新 show**：`submit_capture_region`（overlay.rs:155-162）在 OCR 成功后调用 `show_translation_popup` + `start_translation_from_input`，所以 `trigger_ocr_translation` 内先 hide 弹窗是安全的——框选完成后会重新 show 并定位。
- **前端 Tauri API 注入**：`tauri.conf.json` 已设 `withGlobalTauri:true`，`window.__TAURI__.core.invoke` / `window.__TAURI__.event.listen` / `window.__TAURI__.window.getCurrentWindow` 均可用。`getCurrentWindow()` 返回 `Window` 实例，有 `setAlwaysOnTop(bool)` 与 `setSize({ type:"Logical", width, height })`。
- **现有 `translate.js` 用 `type="module" defer`**（见 translate.html:8），module 作用域隔离，**不能用内联 `onclick`**，所有按钮用 `id` + `addEventListener` 绑定。

---

## 文件结构

### 后端（修改）

- `src-tauri/src/app/popup_window.rs` — 翻译弹窗窗口装配。改 `build_popup` 窗口装饰 + `show_popup` 定位尺寸常量。
- `src-tauri/src/ui/ocr_popup.rs` — OCR 翻译编排。新增 `trigger_ocr_translation` command。
- `src-tauri/src/lib.rs` — 应用装配入口。`invoke_handler` 注册 `trigger_ocr_translation`。
- `src-tauri/capabilities/default.json` — Tauri 权限清单。补两个窗口权限。

### 前端（整套重写）

- `frontend/public/translate.css` — 弹窗全部样式。
- `frontend/public/translate.html` — 弹窗结构。
- `frontend/public/translate.js` — 弹窗交互。

### 文档（收尾同步）

- `README.md` / `docs/roadmap/progressive-development-plan.md` / `CLAUDE.md` / `AGENTS.md`

---

## 任务 1：后端窗口去标题栏 + 尺寸常量

**文件：**
- 修改：`src-tauri/src/app/popup_window.rs:94-95`（`show_popup` 内 `POPUP_W`/`POPUP_H` 常量）
- 修改：`src-tauri/src/app/popup_window.rs:109-117`（`build_popup` 窗口配置）

- [ ] **步骤 1：修改 `build_popup` 窗口配置**

把 [popup_window.rs:109-117](src-tauri/src/app/popup_window.rs#L109-L117) 的 `build_popup` 函数体中 `WebviewWindowBuilder` 链改为：

```rust
fn build_popup(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    let window = WebviewWindowBuilder::new(app, POPUP_LABEL, WebviewUrl::App("translate.html".into()))
        .title("Shizi 翻译")
        .inner_size(452.0, 512.0)       // 窗口逻辑宽 452 = .popup 420 + body padding 32；初始高 512 = 首屏内容 + padding
        .decorations(false)              // 去原生标题栏，顶部工具栏作为自绘标题栏
        .transparent(true)               // 圆角 + 阴影需要透明背景
        .resizable(false)                // 宽固定、高由前端 setSize 控制，禁用原生边框 resize
        .visible(false)
        .build()
        .map_err(|e| format!("创建翻译弹窗失败: {e}"))?;

    // 关闭事件：隐藏而非销毁（托盘驻留模型）
    let win_clone = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = win_clone.hide();
        }
    });

    Ok(window)
}
```

- [ ] **步骤 2：修改 `show_popup` 尺寸常量**

把 [popup_window.rs:94-95](src-tauri/src/app/popup_window.rs#L94-L95) 的两个常量改为：

```rust
        const POPUP_W: f64 = 452.0;
        const POPUP_H: f64 = 512.0;
```

- [ ] **步骤 3：编译验证**

运行：`cd src-tauri && cargo build`
预期：编译通过，无错误。

- [ ] **步骤 4：单测验证（现有不破坏）**

运行：`cd src-tauri && cargo test`
预期：全部通过。`compute_popup_position` 的现有单测用独立辅助函数 `popup_400x300()`，不依赖常量，不受影响。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/app/popup_window.rs
git commit -m "refactor(popup): 翻译弹窗去原生标题栏 + 透明窗口 + 尺寸常量调整为 452x512"
```

---

## 任务 2：后端 trigger_ocr_translation command + 注册 + 权限

**文件：**
- 修改：`src-tauri/src/ui/ocr_popup.rs`（新增 command + use 引入）
- 修改：`src-tauri/src/lib.rs:15-24`（use 引入）+ `lib.rs:38-51`（invoke_handler）
- 修改：`src-tauri/capabilities/default.json:5-8`（permissions 数组）

- [ ] **步骤 1：在 `ocr_popup.rs` 新增 `trigger_ocr_translation` command**

在 [ocr_popup.rs:1-8](src-tauri/src/ui/ocr_popup.rs#L1-L8) 的 `use` 块中，把 `use tauri::Manager;` 上方的 `use crate::{...};` 改为引入 `popup_window`：

```rust
use crate::{
    app::{popup_window, state::AppState},
    core::{capture::CaptureError, ocr::OcrError, ocr_translation::OcrTranslationError},
    platform::capture_screen,
    ui::{overlay, web_popup::show_translation_error},
};

use tauri::Manager;
```

然后在 `start_translation_from_ocr` 函数之后、`friendly_ocr_error` 之前插入新 command（`start_translation_from_ocr` 已有 `app: tauri::AppHandle` 参数，`Manager::get_webview_window` 可用）：

```rust
/// 翻译弹窗「截图翻译」按钮入口：先隐藏弹窗避免被抓进截图帧，再复用 Alt+O 的 OCR 链路。
/// 框选完成后 submit_capture_region 内部 show_translation_popup 会重新 show 并定位弹窗。
#[tauri::command]
pub async fn trigger_ocr_translation(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    if let Some(popup) = app.get_webview_window(popup_window::POPUP_LABEL) {
        let _ = popup.hide();
    }
    start_translation_from_ocr(app, state.inner().clone()).await;
    Ok(())
}
```

- [ ] **步骤 2：在 `lib.rs` 注册 command**

把 [lib.rs:15-24](src-tauri/src/lib.rs#L15-L24) 的 `use ui::{...}` 改为引入 `trigger_ocr_translation`：

```rust
use ui::{
    config::{get_app_config, save_app_config, open_settings},
    ocr_popup::trigger_ocr_translation,
    overlay::{
        cancel_capture, ensure_overlay, get_capture_frame_bytes, get_capture_frame_meta,
        show_overlay, submit_capture_region,
    },
    web_popup::{
        cancel_translation, retry_translation, start_translation, take_pending_source_text,
    },
};
```

然后在 [lib.rs:38-51](src-tauri/src/lib.rs#L38-L51) 的 `generate_handler!` 宏里，在 `start_translation,` 之后加一行 `trigger_ocr_translation,`：

```rust
        .invoke_handler(tauri::generate_handler![
            start_translation,
            trigger_ocr_translation,
            cancel_translation,
            retry_translation,
            take_pending_source_text,
            get_app_config,
            save_app_config,
            open_settings,
            get_capture_frame_meta,
            get_capture_frame_bytes,
            submit_capture_region,
            cancel_capture,
            show_overlay,
        ])
```

- [ ] **步骤 3：在 `capabilities/default.json` 补窗口权限**

把 [capabilities/default.json](src-tauri/capabilities/default.json) 的 `permissions` 数组改为：

```json
  "permissions": [
    "core:default",
    "global-shortcut:default",
    "core:window:allow-set-always-on-top",
    "core:window:allow-set-size"
  ]
```

- [ ] **步骤 4：编译验证（注册由编译期保证）**

运行：`cd src-tauri && cargo build`
预期：编译通过。`generate_handler!` 宏在编译期校验所有 command 函数存在且签名正确。

- [ ] **步骤 5：单测验证**

运行：`cd src-tauri && cargo test`
预期：全部通过。`friendly_ocr_error` 现有单测不受影响。

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/ui/ocr_popup.rs src-tauri/src/lib.rs src-tauri/capabilities/default.json
git commit -m "feat(ocr): 新增 trigger_ocr_translation command + 窗口置顶/尺寸权限"
```

---

## 任务 3：前端样式与结构重写（translate.css + translate.html）

**文件：**
- 重写：`frontend/public/translate.css`（整文件覆盖）
- 重写：`frontend/public/translate.html`（整文件覆盖）

- [ ] **步骤 1：重写 `translate.css`**

用以下完整内容覆盖 `frontend/public/translate.css`（从原型移植；`body` 改透明 + padding 16px 留阴影；`.content` 加 `flex:1` + `overflow-y:auto`；新增 `.source-badge` / `.meta-badges` / `.status-action`；去掉原型的居中展示样式与 mock 用 `.skeleton`）：

```css
*,*::before,*::after{box-sizing:border-box;margin:0;padding:0}

:root {
  --bg-popup:      #F5F2EC;
  --bg-card:       #FFFFFF;
  --bg-soft:       #FAF8F3;
  --bg-soft-2:     #F0EDE5;

  --fg:            #1F1E1B;
  --fg-2:          #5B584F;
  --fg-3:          #94918A;

  --border:        #E6E2D8;
  --border-2:      #D8D3C5;

  --accent:        #0078D4;
  --accent-hover:  #106EBE;
  --accent-soft:   rgba(0,120,212,0.08);
  --success:       #107C10;
  --warning:       #CA5010;
  --danger:        #b42318;

  --radius-sm:     5px;
  --radius-md:     9px;
  --radius-lg:     14px;

  --font-family:   "Segoe UI Variable", "Segoe UI", -apple-system, BlinkMacSystemFont, "Helvetica Neue", "Microsoft YaHei", system-ui, sans-serif;

  --shadow-popup:  0 8px 24px rgba(28,25,23,0.10), 0 1px 2px rgba(28,25,23,0.04);
  --shadow-card:   0 1px 2px rgba(28,25,23,0.04);
  --shadow-card-h: 0 2px 8px rgba(28,25,23,0.07);

  font-size: 16px;
}

body {
  font-family: var(--font-family);
  background: transparent;
  color: var(--fg);
  padding: 16px;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  font-feature-settings: "ss01" 1, "tnum" 1;
  overflow: hidden;
}

/* === 弹窗外壳 === */
.popup {
  width: 420px;
  background: var(--bg-popup);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-popup);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  position: relative;
}

/* === 顶部工具栏（自绘标题栏，data-tauri-drag-region 拖拽） === */
.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 3px 6px;
  min-height: 26px;
}
.toolbar-left, .toolbar-right { display: flex; align-items: center; gap: 1px; }
.toolbar-btn {
  width: 22px; height: 22px;
  border: none; background: transparent;
  border-radius: 4px;
  cursor: pointer;
  display: flex; align-items: center; justify-content: center;
  color: var(--fg-2);
  transition: background .15s, color .15s;
}
.toolbar-btn:hover  { background: rgba(28,25,23,0.05); color: var(--fg); }
.toolbar-btn.active { color: var(--accent); }
.toolbar-btn:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
.toolbar-btn svg { width: 13px; height: 13px; stroke-width: 1.6; }

/* === 内容区（超出时滚动） === */
.content {
  padding: 0 10px 10px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

/* === 原文卡片 === */
.source-card {
  background: var(--bg-card);
  border-radius: var(--radius-md);
  border: 0.5px solid var(--border);
  box-shadow: var(--shadow-card);
  padding: 10px 12px 8px;
  transition: box-shadow .15s, border-color .15s;
}
.source-card:focus-within {
  border-color: var(--accent);
  box-shadow: 0 0 0 1px var(--accent), var(--shadow-card-h);
}
.source-input {
  display: block;
  width: 100%;
  border: none; background: transparent;
  font-family: var(--font-family);
  font-size: 0.8125rem;
  line-height: 1.55;
  color: var(--fg);
  resize: none; outline: none;
  padding: 0;
  min-height: 2.75rem;
  overflow: hidden;
  user-select: text;
}
.source-input::placeholder { color: var(--fg-3); }

.source-meta {
  display: flex;
  align-items: center;
  gap: 3px;
  margin-top: 8px;
  padding-top: 6px;
  border-top: 0.5px solid var(--border);
}
.meta-btn {
  width: 24px; height: 24px;
  border: none; background: transparent;
  border-radius: 5px;
  cursor: pointer;
  display: flex; align-items: center; justify-content: center;
  color: var(--fg-2);
  transition: background .15s, color .15s;
}
.meta-btn:hover { background: rgba(28,25,23,0.05); color: var(--fg); }
.meta-btn:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
.meta-btn svg { width: 12px; height: 12px; stroke-width: 1.6; }
.meta-btn.copied { color: var(--success); }

.meta-badges {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 4px;
}
.source-badge {
  display: inline-flex;
  align-items: center;
  font-size: 0.6875rem;
  color: var(--fg-2);
  background: var(--bg-soft-2);
  padding: 2px 8px;
  border-radius: 10px;
  line-height: 1.5;
  font-weight: 500;
}
.source-badge:empty { display: none; }
.lang-badge {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  font-size: 0.6875rem;
  color: var(--accent);
  background: var(--accent-soft);
  padding: 2px 8px;
  border-radius: 10px;
  line-height: 1.5;
  font-weight: 600;
}

/* === 翻译工具栏 === */
.lang-toolbar {
  background: var(--bg-card);
  border-radius: var(--radius-md);
  border: 0.5px solid var(--border);
  box-shadow: var(--shadow-card);
  display: flex;
  align-items: center;
  height: 32px;
  padding: 0 2px;
}
.lang-side {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
  padding: 0 10px;
  background: transparent;
  border: none;
  font-family: var(--font-family);
  font-size: 0.75rem;
  color: var(--fg);
  cursor: pointer;
  user-select: none;
  transition: background .15s, color .15s;
  min-width: 0;
  height: 28px;
  border-radius: 6px;
}
.lang-side:hover  { background: var(--bg-soft); }
.lang-side:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
.lang-side .lang-label { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.lang-side .lang-chevron { width: 10px; height: 10px; color: var(--fg-2); flex-shrink: 0; }
.lang-swap {
  width: 28px; height: 28px;
  flex-shrink: 0;
  display: flex; align-items: center; justify-content: center;
  background: transparent; border: none;
  color: var(--fg-2);
  cursor: pointer;
  border-radius: 6px;
  transition: background .15s, color .15s;
}
.lang-swap:hover  { background: var(--bg-soft); color: var(--accent); }
.lang-swap:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
.lang-swap svg { width: 12px; height: 12px; }

/* === 翻译结果区 === */
.results {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.result-card {
  background: var(--bg-card);
  border-radius: var(--radius-md);
  border: 0.5px solid var(--border);
  box-shadow: var(--shadow-card);
  overflow: hidden;
  transition: box-shadow .2s, border-color .2s;
}
.result-card:hover {
  box-shadow: var(--shadow-card-h);
  border-color: var(--border-2);
}
.result-card-header {
  display: flex;
  align-items: center;
  padding: 6px 12px;
  gap: 6px;
  cursor: pointer;
  user-select: none;
}
.result-engine-icon {
  width: 14px; height: 14px;
  border-radius: 3px;
  flex-shrink: 0;
}
.result-engine-name {
  font-size: 0.6875rem;
  font-weight: 500;
  color: var(--fg-2);
  flex: 1;
}
.result-collapse-btn {
  width: 20px; height: 20px;
  border: none; background: transparent;
  border-radius: 4px;
  cursor: pointer;
  display: flex; align-items: center; justify-content: center;
  color: var(--fg-2);
  transition: background .15s;
}
.result-collapse-btn:hover { background: rgba(28,25,23,0.05); }
.result-collapse-btn svg { width: 11px; height: 11px; transition: transform .25s ease; }
.result-card.collapsed .result-collapse-btn svg { transform: rotate(-90deg); }

.result-card-body {
  display: grid;
  grid-template-rows: 1fr;
  transition: grid-template-rows .3s ease, padding .3s ease, opacity .2s ease;
  padding: 0 12px 9px;
}
.result-card-body-inner {
  overflow: hidden;
  min-height: 0;
}
.result-card.collapsed .result-card-body {
  grid-template-rows: 0fr;
  padding-top: 0;
  padding-bottom: 0;
  opacity: 0;
}
.result-text {
  font-size: 0.8125rem;
  line-height: 1.6;
  color: var(--fg);
  white-space: pre-wrap;
  word-break: break-word;
  min-height: 1em;
}
.result-actions {
  display: flex;
  align-items: center;
  gap: 3px;
  margin-top: 6px;
}
.result-action-btn {
  width: 22px; height: 22px;
  border: none; background: transparent;
  border-radius: 4px;
  cursor: pointer;
  display: flex; align-items: center; justify-content: center;
  color: var(--fg-2);
  transition: background .15s, color .15s;
}
.result-action-btn:hover  { background: rgba(28,25,23,0.05); color: var(--fg); }
.result-action-btn.copied { color: var(--success); }
.result-action-btn:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
.result-action-btn svg { width: 12px; height: 12px; stroke-width: 1.6; }
.result-tokens {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  gap: 7px;
  font-size: 0.625rem;
  color: var(--fg-3);
  font-variant-numeric: tabular-nums;
  user-select: none;
  letter-spacing: 0.01em;
}
.result-tokens .tok {
  display: inline-flex;
  align-items: center;
  gap: 2px;
}
.result-tokens .tok svg {
  width: 9px;
  height: 9px;
  opacity: 0.55;
  stroke-width: 2;
}
.result-tokens .tok-sep {
  width: 1px;
  height: 9px;
  background: var(--border);
}

/* === 状态栏 === */
.status-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 14px;
  border-top: 0.5px solid var(--border);
  font-size: 0.6875rem;
  color: var(--fg-2);
  background: var(--bg-popup);
}
.status-left { display: flex; align-items: center; gap: 6px; }
.status-dot {
  width: 6px; height: 6px;
  border-radius: 50%;
  background: var(--success);
}
.status-dot.loading {
  background: var(--warning);
  animation: pulse 1s ease-in-out infinite;
}
@keyframes pulse {
  0%,100%{opacity:1}
  50%{opacity:.4}
}
.status-action {
  border: none;
  background: transparent;
  color: var(--fg-2);
  font-family: var(--font-family);
  font-size: 0.6875rem;
  cursor: pointer;
  padding: 0;
  transition: color .15s;
}
.status-action:hover { color: var(--accent); }
.status-action:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }

/* === 拖拽把手（视觉保留，resizable:false 下无功能） === */
.resize-handle {
  position: absolute;
  bottom: 0; right: 0;
  width: 14px; height: 14px;
  cursor: nwse-resize;
  opacity: 0;
  transition: opacity .2s;
}
.popup:hover .resize-handle { opacity: .4; }
.resize-handle::before {
  content: '';
  position: absolute;
  bottom: 3px; right: 3px;
  width: 7px; height: 7px;
  border-right: 1.5px solid var(--fg-3);
  border-bottom: 1.5px solid var(--fg-3);
  border-radius: 0 0 2px 0;
}

/* === 流式光标 === */
.stream-cursor {
  display: inline-block;
  width: 1px;
  height: 0.95em;
  background: var(--accent);
  margin-left: 1px;
  vertical-align: text-bottom;
  animation: blink 1s steps(1) infinite;
}
@keyframes blink {
  0%,49%{opacity:1}
  50%,100%{opacity:0}
}

/* === Toast === */
.toast {
  position: fixed;
  bottom: 24px;
  left: 50%;
  transform: translateX(-50%) translateY(20px);
  background: var(--fg);
  color: var(--bg-card);
  font-size: 0.8125rem;
  padding: 8px 18px;
  border-radius: 20px;
  opacity: 0;
  transition: opacity .25s, transform .25s;
  pointer-events: none;
  z-index: 100;
}
.toast.show {
  opacity: 1;
  transform: translateX(-50%) translateY(0);
}
```

- [ ] **步骤 2：重写 `translate.html`**

用以下完整内容覆盖 `frontend/public/translate.html`（单 `.result-card`；`.toolbar` 加 `data-tauri-drag-region`；按钮用 `id` 绑定；`.source-badge` 显示来源徽章，`.lang-badge` 固定「自动检测」；状态栏含 `#statusAction` 文字按钮；引擎图标默认灰色占位，由 JS 的 `loadEngineMeta` 覆盖）：

```html
<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Shizi - 翻译</title>
  <link rel="stylesheet" href="translate.css">
  <script type="module" src="translate.js" defer></script>
</head>
<body>
  <div class="popup" id="popup">
    <!-- 顶部工具栏（自绘标题栏，data-tauri-drag-region 拖拽） -->
    <div class="toolbar" data-tauri-drag-region>
      <div class="toolbar-left">
        <button class="toolbar-btn" id="pinBtn" title="固定窗口">
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="17" x2="12" y2="22"/><path d="M5 17h14v-1.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6h1a2 2 0 0 0 0-4H8a2 2 0 0 0 0 4h1v4.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24Z"/></svg>
        </button>
      </div>
      <div class="toolbar-right">
        <button class="toolbar-btn" id="favBtn" title="收藏">
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>
        </button>
        <button class="toolbar-btn" id="ocrBtn" title="截图翻译">
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M6 9l6-6 6 6M6 15l6 6 6-6"/></svg>
        </button>
        <button class="toolbar-btn" id="bookmarkBtn" title="书签">
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M19 21l-7-5-7 5V5a2 2 0 012-2h10a2 2 0 012 2z"/></svg>
        </button>
        <button class="toolbar-btn" id="settingsBtn" title="设置">
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z"/></svg>
        </button>
      </div>
    </div>

    <div class="content">
      <!-- 原文卡片 -->
      <div class="source-card">
        <textarea class="source-input" id="sourceText" placeholder="输入要翻译的文本…" rows="3"></textarea>
        <div class="source-meta">
          <button class="meta-btn" id="speakSourceBtn" title="朗读原文">
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><path d="M19.07 4.93a10 10 0 010 14.14M15.54 8.46a5 5 0 010 7.07"/></svg>
          </button>
          <button class="meta-btn" id="copySourceBtn" title="复制原文">
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/></svg>
          </button>
          <div class="meta-badges">
            <span class="source-badge" id="sourceBadge"></span>
            <span class="lang-badge">自动检测</span>
          </div>
        </div>
      </div>

      <!-- 翻译工具栏（纯视觉占位） -->
      <div class="lang-toolbar">
        <button class="lang-side" id="langSource">
          <span class="lang-label">自动检测</span>
          <svg class="lang-chevron" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>
        </button>
        <button class="lang-swap" id="langSwap" title="交换语言">
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M7 16l-4-4 4-4"/><path d="M17 8l4 4-4 4"/><line x1="3" y1="12" x2="21" y2="12"/></svg>
        </button>
        <button class="lang-side" id="langTarget">
          <span class="lang-label">简体中文</span>
          <svg class="lang-chevron" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>
        </button>
      </div>

      <!-- 翻译结果区（单卡片 + 预留多卡数据结构） -->
      <div class="results" id="results">
        <div class="result-card" id="resultCard">
          <div class="result-card-header" id="resultHeader">
            <svg class="result-engine-icon" id="resultEngineIcon" viewBox="0 0 20 20"><rect width="20" height="20" rx="5" fill="#94918A"/></svg>
            <span class="result-engine-name" id="resultEngineName">翻译</span>
            <button class="result-collapse-btn" id="collapseBtn" title="折叠">
              <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>
            </button>
          </div>
          <div class="result-card-body">
            <div class="result-card-body-inner">
              <div class="result-text" id="resultText"></div>
              <div class="result-actions" id="resultActions" style="visibility:hidden">
                <button class="result-action-btn" id="speakResultBtn" title="朗读翻译">
                  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><path d="M19.07 4.93a10 10 0 010 14.14M15.54 8.46a5 5 0 010 7.07"/></svg>
                </button>
                <button class="result-action-btn" id="copyResultBtn" title="复制翻译">
                  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/></svg>
                </button>
                <span class="result-tokens" id="resultTokens" title="输入 / 输出 Token" style="display:none">
                  <span class="tok tok-input">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="19" x2="12" y2="5"/><polyline points="5 12 12 5 19 12"/></svg>
                    <span class="tok-value">0</span>
                  </span>
                  <span class="tok-sep"></span>
                  <span class="tok tok-output">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19"/><polyline points="19 12 12 19 5 12"/></svg>
                    <span class="tok-value">0</span>
                  </span>
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 底部状态栏 -->
    <div class="status-bar">
      <div class="status-left">
        <span class="status-dot" id="statusDot"></span>
        <span id="statusText">就绪</span>
        <button class="status-action" id="statusAction" style="display:none"></button>
      </div>
      <span id="charCount">0 字</span>
    </div>

    <div class="resize-handle"></div>
  </div>

  <div class="toast" id="toast"></div>
</body>
</html>
```

- [ ] **步骤 3：手动验证视觉骨架**

运行：`npm run tauri dev`（或调试 `SHIZI_LLM_PROVIDER=mock npm run tauri dev`），触发划词翻译 `Alt+T` 打开弹窗。

预期：
- 弹窗无 Windows 原生标题栏，圆角 + 阴影可见，顶部工具栏可见。
- 工具栏按钮（图钉/收藏/截图/书签/设置）可见，但点击无功能（JS 还未重写）。
- 原文卡 / 语言栏 / 结果卡 / 状态栏布局与原型一致。
- 长按工具栏空白区可拖动整窗（`data-tauri-drag-region`）。

- [ ] **步骤 4：Commit**

```bash
git add frontend/public/translate.css frontend/public/translate.html
git commit -m "refactor(translate): 重写翻译弹窗样式与结构对齐 OpenDesign 原型（去标题栏+单卡片+自绘工具栏）"
```

---

## 任务 4：前端交互逻辑重写（translate.js）

**文件：**
- 重写：`frontend/public/translate.js`（整文件覆盖）

- [ ] **步骤 1：重写 `translate.js`**

用以下完整内容覆盖 `frontend/public/translate.js`。核心模块：① DOM 获取 + Tauri API；② 工具函数（toast/autoResize/speakText/copyText）；③ 引擎图标映射 + `loadEngineMeta`；④ 事件渲染（`renderTranslationEvent` + `setStatus` + `setStreamCursor`）；⑤ 翻译触发（手动/取消/重试）；⑥ 工具栏按钮绑定；⑦ 窗口管理（`adjustHeight` + `ResizeObserver` + `initMaxHeight`）；⑧ 初始化。

```js
const invoke = window.__TAURI__?.core?.invoke;
const listen = window.__TAURI__?.event?.listen;
const getCurrentWindow = window.__TAURI__?.window?.getCurrentWindow;

const popupEl = document.getElementById('popup');
const sourceText = document.getElementById('sourceText');
const speakSourceBtn = document.getElementById('speakSourceBtn');
const copySourceBtn = document.getElementById('copySourceBtn');
const sourceBadge = document.getElementById('sourceBadge');
const pinBtn = document.getElementById('pinBtn');
const favBtn = document.getElementById('favBtn');
const ocrBtn = document.getElementById('ocrBtn');
const bookmarkBtn = document.getElementById('bookmarkBtn');
const settingsBtn = document.getElementById('settingsBtn');
const langSource = document.getElementById('langSource');
const langSwap = document.getElementById('langSwap');
const langTarget = document.getElementById('langTarget');
const resultCard = document.getElementById('resultCard');
const resultHeader = document.getElementById('resultHeader');
const resultEngineIcon = document.getElementById('resultEngineIcon');
const resultEngineName = document.getElementById('resultEngineName');
const collapseBtn = document.getElementById('collapseBtn');
const resultText = document.getElementById('resultText');
const resultActions = document.getElementById('resultActions');
const speakResultBtn = document.getElementById('speakResultBtn');
const copyResultBtn = document.getElementById('copyResultBtn');
const resultTokens = document.getElementById('resultTokens');
const tokInputValue = resultTokens.querySelector('.tok-input .tok-value');
const tokOutputValue = resultTokens.querySelector('.tok-output .tok-value');
const statusDot = document.getElementById('statusDot');
const statusText = document.getElementById('statusText');
const statusAction = document.getElementById('statusAction');
const charCount = document.getElementById('charCount');
const toastEl = document.getElementById('toast');

let isTranslating = false;
let currentSessionId = null;
let pinned = false;

/* === Toast === */
let toastTimer = null;
function showToast(msg) {
  toastEl.textContent = msg;
  toastEl.classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toastEl.classList.remove('show'), 1800);
}

/* === 原文区 === */
function autoResize() {
  sourceText.style.height = 'auto';
  sourceText.style.height = sourceText.scrollHeight + 'px';
}
function updateCharCount() {
  charCount.textContent = `${sourceText.value.length} 字`;
}
sourceText.addEventListener('input', () => {
  autoResize();
  updateCharCount();
});
sourceText.addEventListener('keydown', (e) => {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    startManualTranslation();
  }
});

/* === 朗读 === */
function speakText(text, lang) {
  if (!('speechSynthesis' in window)) {
    showToast('当前浏览器不支持语音朗读');
    return;
  }
  window.speechSynthesis.cancel();
  const utter = new SpeechSynthesisUtterance(text);
  utter.lang = lang;
  utter.rate = 0.95;
  window.speechSynthesis.speak(utter);
}

/* === 复制 === */
function copyText(text, btn) {
  navigator.clipboard.writeText(text).then(() => {
    btn.classList.add('copied');
    showToast('已复制到剪贴板');
    setTimeout(() => btn.classList.remove('copied'), 1500);
  }).catch(() => {
    showToast('复制失败');
  });
}

/* === 引擎图标/名映射 === */
const ENGINE_META = {
  'openai-compatible': {
    icon: '<svg viewBox="0 0 20 20"><rect width="20" height="20" rx="5" fill="#10A37F"/><circle cx="10" cy="10" r="6" fill="none" stroke="#fff" stroke-width="1.2"/><path d="M7.5 10c0-1.38 1.12-2.5 2.5-2.5s2.5 1.12 2.5 2.5" stroke="#fff" stroke-width="1.2" fill="none" stroke-linecap="round"/></svg>',
    name: 'OpenAI 翻译',
  },
  'claude': {
    icon: '<svg viewBox="0 0 20 20"><rect width="20" height="20" rx="5" fill="#D97757"/><text x="10" y="14.5" text-anchor="middle" font-size="12" font-weight="700" fill="#fff" font-family="Segoe UI, system-ui, sans-serif">C</text></svg>',
    name: 'Claude 翻译',
  },
  'mock': {
    icon: '<svg viewBox="0 0 20 20"><rect width="20" height="20" rx="5" fill="#94918A"/><text x="10" y="14.5" text-anchor="middle" font-size="12" font-weight="700" fill="#fff" font-family="Segoe UI, system-ui, sans-serif">M</text></svg>',
    name: 'Mock 翻译',
  },
};

async function loadEngineMeta() {
  if (!invoke) return;
  try {
    const config = await invoke('get_app_config');
    const meta = ENGINE_META[config.provider] ?? ENGINE_META['openai-compatible'];
    resultEngineIcon.innerHTML = meta.icon;
    resultEngineName.textContent = meta.name;
  } catch (error) {
    showToast(String(error));
  }
}

/* === 来源徽章 === */
function setSourceBadge(sourceType) {
  switch (sourceType) {
    case 'selectedText':
      sourceBadge.textContent = '来自划词';
      break;
    case 'ocrText':
      sourceBadge.textContent = '来自 OCR';
      break;
    default:
      sourceBadge.textContent = '';
      break;
  }
}

/* === 翻译事件渲染 === */
function getSessionId(payload) {
  const sessionId = payload?.sessionId;
  if (typeof sessionId === 'string') return sessionId;
  if (sessionId && typeof sessionId === 'object') return sessionId[0] ?? sessionId['0'] ?? null;
  return null;
}
function shouldHandleSessionEvent(payload) {
  const sessionId = getSessionId(payload);
  return !currentSessionId || !sessionId || sessionId === currentSessionId;
}

function setStatus({ text, loading, action }) {
  statusText.textContent = text;
  statusDot.classList.toggle('loading', loading);
  if (action) {
    statusAction.textContent = action.label;
    statusAction.style.display = '';
    statusAction.onclick = action.onClick;
  } else {
    statusAction.style.display = 'none';
    statusAction.onclick = null;
  }
}

function setStreamCursor(visible) {
  const existing = resultText.querySelector('.stream-cursor');
  if (existing) existing.remove();
  if (visible) {
    const cursor = document.createElement('span');
    cursor.className = 'stream-cursor';
    resultText.appendChild(cursor);
  }
}

function scrollResultToBottom() {
  resultText.scrollTop = resultText.scrollHeight;
}

function renderTranslationEvent(payload) {
  switch (payload.type) {
    case 'started':
      currentSessionId = getSessionId(payload);
      sourceText.value = payload.sourceText ?? sourceText.value;
      autoResize();
      updateCharCount();
      setSourceBadge(payload.sourceType);
      resultText.textContent = '';
      resultText.style.color = '';
      resultActions.style.visibility = 'hidden';
      resultTokens.style.display = 'none';
      setStreamCursor(true);
      isTranslating = true;
      setStatus({
        text: '翻译中…',
        loading: true,
        action: { label: '取消', onClick: cancelTranslation },
      });
      break;
    case 'delta':
      if (!shouldHandleSessionEvent(payload)) return;
      resultText.appendChild(document.createTextNode(payload.text ?? ''));
      setStreamCursor(true);
      scrollResultToBottom();
      break;
    case 'finished':
      if (!shouldHandleSessionEvent(payload)) return;
      resultText.textContent = payload.fullText ?? resultText.textContent;
      resultText.style.color = '';
      setStreamCursor(false);
      if (payload.usage) {
        tokInputValue.textContent = payload.usage.inputTokens;
        tokOutputValue.textContent = payload.usage.outputTokens;
        resultTokens.style.display = '';
      } else {
        resultTokens.style.display = 'none';
      }
      resultActions.style.visibility = 'visible';
      currentSessionId = null;
      isTranslating = false;
      setSourceBadge(null);
      setStatus({
        text: '翻译完成',
        loading: false,
        action: { label: '重试', onClick: retryTranslation },
      });
      scrollResultToBottom();
      break;
    case 'failed':
      if (currentSessionId && !shouldHandleSessionEvent(payload)) return;
      resultText.textContent = payload.message ?? '翻译失败';
      resultText.style.color = 'var(--danger)';
      setStreamCursor(false);
      resultActions.style.visibility = 'hidden';
      resultTokens.style.display = 'none';
      currentSessionId = null;
      isTranslating = false;
      setSourceBadge(null);
      setStatus({
        text: '翻译失败',
        loading: false,
        action: payload.retryable !== false
          ? { label: '重试', onClick: retryTranslation }
          : null,
      });
      break;
    case 'cancelled':
      if (!shouldHandleSessionEvent(payload)) return;
      resultText.appendChild(document.createTextNode('\n[已取消]'));
      resultText.style.color = 'var(--fg-3)';
      setStreamCursor(false);
      currentSessionId = null;
      isTranslating = false;
      setSourceBadge(null);
      setStatus({
        text: '已取消',
        loading: false,
        action: { label: '重试', onClick: retryTranslation },
      });
      break;
    default:
      break;
  }
  adjustHeight();
}

if (listen) {
  listen('translation:event', (event) => {
    renderTranslationEvent(event.payload);
  });
}

/* === 翻译触发 === */
async function startManualTranslation() {
  if (isTranslating) return;
  const text = sourceText.value.trim();
  if (!text) {
    showToast('请输入要翻译的文本');
    return;
  }
  if (!invoke) {
    showToast('Tauri API 未就绪，请在桌面应用中运行');
    return;
  }
  try {
    await invoke('start_translation', { text });
  } catch (error) {
    showToast(String(error));
  }
}

async function cancelTranslation() {
  if (!invoke) return;
  try {
    await invoke('cancel_translation');
  } catch (error) {
    showToast(String(error));
  }
}

async function retryTranslation() {
  if (isTranslating) return;
  if (!invoke) {
    showToast('Tauri API 未就绪');
    return;
  }
  try {
    await invoke('retry_translation');
  } catch (error) {
    showToast(String(error));
  }
}

/* === 工具栏按钮 === */
async function togglePin() {
  if (!getCurrentWindow) {
    showToast('窗口 API 未就绪');
    return;
  }
  pinned = !pinned;
  pinBtn.classList.toggle('active', pinned);
  try {
    await getCurrentWindow().setAlwaysOnTop(pinned);
    showToast(pinned ? '窗口已固定' : '取消固定');
  } catch (error) {
    pinned = !pinned;
    pinBtn.classList.toggle('active', pinned);
    showToast(String(error));
  }
}

function toggleFav() {
  const active = favBtn.classList.toggle('active');
  const svg = favBtn.querySelector('svg');
  if (svg) svg.setAttribute('fill', active ? 'currentColor' : 'none');
  showToast(active ? '已收藏' : '取消收藏');
}

async function triggerOcr() {
  if (!invoke) {
    showToast('Tauri API 未就绪');
    return;
  }
  try {
    await invoke('trigger_ocr_translation');
  } catch (error) {
    showToast(String(error));
  }
}

async function openSettings() {
  if (!invoke) return;
  try {
    await invoke('open_settings');
  } catch (error) {
    showToast(String(error));
  }
}

function toggleCollapse() {
  resultCard.classList.toggle('collapsed');
  adjustHeight();
}

pinBtn.addEventListener('click', togglePin);
favBtn.addEventListener('click', toggleFav);
ocrBtn.addEventListener('click', triggerOcr);
bookmarkBtn.addEventListener('click', () => showToast('功能开发中'));
settingsBtn.addEventListener('click', openSettings);
resultHeader.addEventListener('click', (e) => {
  // 折叠按钮自身的 click 不触发折叠（避免双触发）
  if (e.target.closest('.result-collapse-btn')) return;
  toggleCollapse();
});
collapseBtn.addEventListener('click', (e) => {
  e.stopPropagation();
  toggleCollapse();
});
speakSourceBtn.addEventListener('click', () => speakText(sourceText.value, 'en-US'));
copySourceBtn.addEventListener('click', () => copyText(sourceText.value, copySourceBtn));
speakResultBtn.addEventListener('click', () => speakText(resultText.textContent, 'zh-CN'));
copyResultBtn.addEventListener('click', () => copyText(resultText.textContent, copyResultBtn));
langSource.addEventListener('click', () => showToast('功能开发中'));
langSwap.addEventListener('click', () => showToast('功能开发中'));
langTarget.addEventListener('click', () => showToast('功能开发中'));

/* === 待回填原文 === */
async function applyPendingSourceText() {
  if (!invoke) return;
  try {
    const text = await invoke('take_pending_source_text');
    if (text) {
      sourceText.value = text;
      autoResize();
      updateCharCount();
    }
  } catch (error) {
    showToast(String(error));
  }
}
window.addEventListener('focus', applyPendingSourceText);

/* === 高度自适应 === */
let resizeRaf = null;
let lastHeight = 0;
function adjustHeight() {
  if (resizeRaf) cancelAnimationFrame(resizeRaf);
  resizeRaf = requestAnimationFrame(() => {
    const h = popupEl.offsetHeight;
    if (h === lastHeight) return;
    lastHeight = h;
    if (getCurrentWindow) {
      getCurrentWindow()
        .setSize({ type: 'Logical', width: 452, height: h + 32 })
        .catch(() => {});
    }
  });
}
function initMaxHeight() {
  // 窗口高度上限 = 屏幕逻辑高 × 80%；超出则 .content 区滚动（CSS 已设 overflow-y:auto）
  const maxPopupH = Math.floor(window.screen.availHeight * 0.8) - 32;
  popupEl.style.maxHeight = maxPopupH + 'px';
}
const resizeObserver = new ResizeObserver(adjustHeight);
resizeObserver.observe(popupEl);

/* === 初始化 === */
initMaxHeight();
autoResize();
updateCharCount();
loadEngineMeta();
applyPendingSourceText();
```

- [ ] **步骤 2：手动验证清单**

运行：`SHIZI_LLM_PROVIDER=mock npm run tauri dev`（mock 无需 API Key，便于验证流式）。

逐项验证（spec 第 8 节）：

1. **划词翻译流式显示**：在他处选中文本按 `Alt+T`，弹窗出现，原文回填，结果区逐字流式追加 + 光标闪烁，完成显示 token。
2. **OCR 翻译流式显示**：按 `Alt+O` 框选区域，OCR 后弹窗显示原文 + 流式译文。
3. **手动输入 Enter 翻译**：弹窗内输入文本按 Enter（Shift+Enter 换行），触发翻译。
4. **图钉置顶切换**：点图钉按钮，active 态变蓝，窗口置顶；再点取消。
5. **截图翻译按钮触发 overlay**：点工具栏截图按钮，弹窗隐藏，overlay 出现可框选。
6. **拖拽标题栏**：长按工具栏空白区拖动整窗。
7. **高度自适应**：长文本翻译时弹窗高度增长至屏幕 80% 后 `.content` 滚动；短文本时收紧。
8. **取消/重试**：翻译中点状态栏「取消」中止；失败/完成后点「重试」重发。
9. **朗读/复制**：原文/译文的朗读按钮发声，复制按钮变绿 + toast。
10. **折叠卡片**：点结果卡头或折叠按钮，卡片体收起/展开（grid 动画）。
11. **引擎图标/名**：mock 模式显示灰色 M 图标 + "Mock 翻译"；切 openai-compatible 显示绿色 OpenAI 图标。
12. **占位按钮 toast**：收藏/书签/语言栏三按钮点后 toast「功能开发中」/「已收藏」等。

- [ ] **步骤 3：Commit**

```bash
git add frontend/public/translate.js
git commit -m "feat(translate): 重写翻译弹窗交互（事件渲染+窗口管理+功能对接+引擎映射）"
```

---

## 任务 5：文档同步（收尾硬门禁）

**文件：**
- 修改：`README.md:16`（翻译弹窗描述）
- 修改：`docs/roadmap/progressive-development-plan.md:429-435`（「前端体验优化」小节）
- 修改：`CLAUDE.md` 架构关键点 + `AGENTS.md` 对应段落（同步）

- [ ] **步骤 1：更新 `README.md` 翻译弹窗描述**

把 [README.md:16](README.md#L16) 那一条改为（强调去原生标题栏 + 自绘标题栏 + 新视觉）：

```markdown
- 独立设置页与独立翻译弹窗：主窗口承载设置页（Vue 3 + Tailwind v4 + reka-ui + @lucide/vue），含通用/翻译/快捷键/服务/历史/高级 6 个分类面板，支持多服务实例管理；划词 / OCR 触发时弹出独立翻译弹窗并跟随光标定位，两者互不耦合。翻译弹窗已去除 Windows 原生标题栏，改为自绘顶部工具栏（图钉/收藏/截图翻译/书签/设置）作为标题栏并支持拖拽，宽固定 420px、高度随内容自适应（最高 80% 屏幕高），视觉对齐 OpenDesign 原型。
```

- [ ] **步骤 2：更新 `docs/roadmap/progressive-development-plan.md`**

在 [progressive-development-plan.md:429-435](docs/roadmap/progressive-development-plan.md#L429-L435) 的「前端体验优化（Tauri UI 路线）」列表中，把「翻译页 Vue 迁移（后续）」上方插入一条已完成项：

```markdown
- **翻译弹窗 UI 打磨**（已完成，2026-07）：按 OpenDesign 原型整套重写 `frontend/public/translate.html` / `translate.js` / `translate.css`——去 Windows 原生标题栏改自绘工具栏（`data-tauri-drag-region` 拖拽）、`decorations:false`+`transparent:true`+`resizable:false`、宽 452/.popup 420 固定 + 高自适应（ResizeObserver → `setSize`）、单卡片 + 预留多卡数据结构、图钉/截图翻译/设置/朗读/复制接真实后端、收藏/书签/语言栏 toast 占位、取消/重试挂状态栏文字按钮；后端仅新增 `trigger_ocr_translation` 薄封装 + 两个窗口权限。
```

- [ ] **步骤 3：同步 `CLAUDE.md` 与 `AGENTS.md` 架构关键点**

在 `CLAUDE.md` 的「架构关键点」的「托盘驻留模型」条目之后，新增一条「翻译弹窗窗口」：

```markdown
- **翻译弹窗窗口**：`build_popup` 配置 `decorations(false)` + `transparent(true)` + `resizable(false)` + `inner_size(452, 512)`，去除 Windows 原生标题栏；顶部 `.toolbar` 加 `data-tauri-drag-region` 实现自绘标题栏拖拽（Tauri 2 原生，零 JS）。`.popup` 宽 420px 固定，`body` 设 `padding:16px` + `background:transparent` 留阴影空间；高度由前端 `ResizeObserver` 监听 `.popup` 内容高度变化后 `getCurrentWindow().setSize({ type:"Logical", width:452, height:h+32 })` 动态调整，上限屏幕高 80%（超出由 `.content` `overflow-y:auto` 滚动）。图钉按钮 `setAlwaysOnTop` 需 `core:window:allow-set-always-on-top` 权限，`setSize` 需 `core:window:allow-set-size`，均已在 `capabilities/default.json` 授权。
```

同步更新 `AGENTS.md` 对应段落（CLAUDE.md 与 AGENTS.md 内容必须一致，见开发说明第 1 条）。

- [ ] **步骤 4：Commit**

```bash
git add README.md docs/roadmap/progressive-development-plan.md CLAUDE.md AGENTS.md
git commit -m "docs: 同步翻译弹窗 UI 重写文档（去标题栏+自绘工具栏+高度自适应）"
```

---

## 自检

### 1. 规格覆盖度

逐章对照 spec：

- §2 范围改动清单：5 项全部覆盖（popup_window.rs → 任务 1；ocr_popup.rs + lib.rs → 任务 2；capabilities → 任务 2；translate.html/js/css → 任务 3+4）。
- §3 窗口装饰与拖拽：build_popup 配置（任务 1 步骤 1）+ 阴影空间 body padding（任务 3 CSS）+ drag-region（任务 3 HTML）+ 尺寸策略宽 452 高自适应（任务 4 `adjustHeight`/`initMaxHeight` + 任务 1 常量）。
- §4 UI 结构与功能对接：工具栏 5 按钮（任务 4 `togglePin`/`toggleFav`/`triggerOcr`/`bookmark`/`openSettings`）+ 原文卡（任务 4 `autoResize`/`speakText`/`copyText`）+ 语言栏占位（任务 4）+ 结果区单卡 + 引擎映射（任务 4 `loadEngineMeta`/`ENGINE_META`）+ 状态栏取消/重试（任务 4 `setStatus`）+ resize handle 视觉（任务 3 CSS）。
- §5 后端改动：`trigger_ocr_translation`（任务 2）+ lib.rs 注册（任务 2）+ capabilities（任务 2）。
- §6 数据流：事件 → UI 映射全部在任务 4 `renderTranslationEvent` 实现；弹窗加载 `take_pending_source_text` + `get_app_config`（任务 4 `applyPendingSourceText`/`loadEngineMeta`）；截图翻译按钮时序（任务 2 command hide + 现有 submit 重新 show）。
- §7 错误处理：failed 红色 + retryable（任务 4）、invoke 异常 catch toast（任务 4 各 try/catch）、朗读不支持（任务 4 `speakText`）、复制失败（任务 4 `copyText`）。OCR 失败走现有 `friendly_ocr_error` 经 `translation:event Failed` 推送，任务 4 `failed` 分支渲染。
- §8 测试：后端 `cargo test`（任务 1/2 步骤 4/5）+ 前端手动验证清单（任务 4 步骤 2，12 项含 spec 的 10 项）。
- §9 文档同步：任务 5。
- §10 风险：transparent 阴影裁剪（任务 3 body padding 16px 缓解）；高度自适应闪烁（任务 4 `requestAnimationFrame` 节流 + `lastHeight` 比较避免重复 setSize）；窗口权限（任务 2 capabilities）；截图时序（任务 2 command 先 hide）；drag-region 冲突（任务 4 `collapseBtn` `stopPropagation`）。

**遗漏：无。**

### 2. 占位符扫描

- 无「TODO」「待定」「后续实现」。
- 所有代码步骤均含完整代码块。
- 所有命令含预期输出。
- `ENGINE_META` 三种 provider 均有完整 SVG + 名称，无省略。

### 3. 类型一致性

- 后端 `trigger_ocr_translation` 签名 `(app: tauri::AppHandle, state: tauri::State<'_, AppState>)` 与 lib.rs `generate_handler!` 注册一致；内部调用 `start_translation_from_ocr(app, state.inner().clone())` 与现有签名 `(app: AppHandle, state: AppState)` 匹配。
- 前端 DOM id（`pinBtn`/`ocrBtn`/`statusAction`/`resultEngineIcon` 等）在 HTML 与 JS 中逐一对应，已交叉核对。
- 事件字段 `sessionId`/`sourceText`/`sourceType`/`fullText`/`usage.inputTokens`/`usage.outputTokens`/`message`/`retryable` 与后端 `TranslationEvent` 的 camelCase 序列化一致。
- `setStatus` 的 `action` 参数结构 `{ label, onClick }` 在所有调用点一致。
