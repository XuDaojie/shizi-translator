# 翻译来源展示 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 让翻译弹窗在译文输出区上方展示输入来源徽章（划词→「来自划词」、OCR→「来自 OCR」、手动→不显示），完成 milestone 2 任务 5。

**架构：** 后端在 `TranslationInput` 上新增 `kind() -> &'static str`（复用 serde tag 字面值 `manualText`/`selectedText`/`ocrText`，零转换），`TranslationEvent::Started` 新增 `source_type: String` 字段，`web_popup.rs` 构造 `Started` 时填入 `request.input.kind()`。前端 `Started` 分支读 `payload.sourceType` 切换徽章显隐与文案，结束/取消/失败/清空时隐藏徽章。不触动 TranslationInput 枚举、provider、TranslationService、配置与重试/取消链路。

**技术栈：** Rust（edition 2021, serde）、Tauri 2、原生静态前端（HTML/JS/CSS，无构建）。

**关联规格：** [docs/superpowers/specs/2026-07-01-translation-source-display-design.md](../specs/2026-07-01-translation-source-display-design.md)

---

## 文件结构

| 文件 | 职责 | 动作 |
|---|---|---|
| `src-tauri/src/core/translation/types.rs` | `TranslationInput`/`TranslationEvent` 类型定义与单测 | 修改：新增 `kind()`、`Started.source_type` 字段、补/新增测试 |
| `src-tauri/src/ui/web_popup.rs` | 构造 `TranslationEvent::Started` 的唯一出口 | 修改：构造处填 `source_type` |
| `frontend/index.html` | 主窗口 DOM | 修改：输出区上方加 `#sourceBadge` 元素 |
| `frontend/main.js` | 前端事件渲染 | 修改：`Started` 分支显隐徽章，结束/取消/失败/清空时隐藏 |
| `frontend/style.css` | 样式 | 修改：新增 `.source-badge` 样式，调整输出区布局 |

---

## 任务 1：TranslationInput::kind() 方法

**文件：**
- 修改：`src-tauri/src/core/translation/types.rs`（`impl TranslationInput` 块，约 32-39 行；`#[cfg(test)] mod tests` 内新增测试）

- [x] **步骤 1：编写失败的测试**

在 `src-tauri/src/core/translation/types.rs` 的 `mod tests` 内、`translation_input_text_returns_inner_text` 测试之后新增：

```rust
    #[test]
    fn translation_input_kind_returns_serde_tag_literal() {
        assert_eq!(
            TranslationInput::ManualText("x".to_string()).kind(),
            "manualText"
        );
        assert_eq!(
            TranslationInput::SelectedText("x".to_string()).kind(),
            "selectedText"
        );
        assert_eq!(
            TranslationInput::OcrText {
                text: "x".to_string(),
                image_id: None,
            }
            .kind(),
            "ocrText"
        );
    }
```

- [x] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test translation_input_kind_returns_serde_tag_literal`
预期：编译失败，报错 `no method named kind found for struct/enum TranslationInput`。

- [x] **步骤 3：编写最少实现代码**

在 `src-tauri/src/core/translation/types.rs` 的 `impl TranslationInput` 块内（`text()` 方法之后）新增 `kind()`：

```rust
    pub fn kind(&self) -> &'static str {
        match self {
            Self::ManualText(_) => "manualText",
            Self::SelectedText(_) => "selectedText",
            Self::OcrText { .. } => "ocrText",
        }
    }
```

- [x] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test translation_input_kind_returns_serde_tag_literal`
预期：PASS。

- [x] **步骤 5：Commit**

```bash
git add src-tauri/src/core/translation/types.rs
git commit -m "feat(translation): TranslationInput 新增 kind() 返回来源字面值"
```

---

## 任务 2：TranslationEvent::Started 增加 source_type 字段

本任务同时改 `types.rs`（字段 + 测试）与 `web_popup.rs`（构造处填充），两者必须同 commit，否则编译失败。

**文件：**
- 修改：`src-tauri/src/core/translation/types.rs`（`TranslationEvent::Started` 变体，约 44-47 行；`started_event_serializes_with_frontend_field_names` 测试，约 101-115 行）
- 修改：`src-tauri/src/ui/web_popup.rs`（构造 `Started` 处，约 111-117 行）

- [x] **步骤 1：编写失败的测试**

在 `src-tauri/src/core/translation/types.rs` 中，将现有 `started_event_serializes_with_frontend_field_names` 测试替换为带 `sourceType` 断言的版本，并新增一个验证三种来源字面值的测试。原测试体整体替换为：

```rust
    #[test]
    fn started_event_serializes_with_frontend_field_names() {
        let event = TranslationEvent::Started {
            session_id: TranslationSessionId("session-1".to_string()),
            source_text: "OCR 原文".to_string(),
            source_type: "ocrText".to_string(),
        };

        let payload = serde_json::to_value(event).expect("事件应可序列化");

        assert_eq!(payload["type"], "started");
        assert_eq!(payload["sessionId"], "session-1");
        assert_eq!(payload["sourceText"], "OCR 原文");
        assert_eq!(payload["sourceType"], "ocrText");
        assert!(payload.get("session_id").is_none());
        assert!(payload.get("source_text").is_none());
        assert!(payload.get("source_type").is_none());
    }

    #[test]
    fn started_event_source_type_serializes_for_each_kind() {
        for kind in ["manualText", "selectedText", "ocrText"] {
            let event = TranslationEvent::Started {
                session_id: TranslationSessionId("session-x".to_string()),
                source_text: "x".to_string(),
                source_type: kind.to_string(),
            };

            let payload = serde_json::to_value(event).expect("事件应可序列化");

            assert_eq!(payload["sourceType"], kind);
        }
    }
```

- [x] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test started_event`
预期：编译失败，`TranslationEvent::Started` 缺少 `source_type` 字段（构造处字段不全）。

- [x] **步骤 3：修改 Started 变体加字段**

在 `src-tauri/src/core/translation/types.rs` 的 `TranslationEvent::Started` 变体内新增 `source_type` 字段：

```rust
    Started {
        session_id: TranslationSessionId,
        source_text: String,
        source_type: String,
    },
```

- [x] **步骤 4：修改 web_popup.rs 构造处填充字段**

在 `src-tauri/src/ui/web_popup.rs` 的 `emit_translation_event` 调用处（约 111-117 行），给 `TranslationEvent::Started` 填入 `source_type`：

```rust
    emit_translation_event(
        &app,
        TranslationEvent::Started {
            session_id: request.session_id.clone(),
            source_text: request.source_text().to_string(),
            source_type: request.input.kind().to_string(),
        },
    )
```

- [x] **步骤 5：运行测试验证通过**

运行：`cd src-tauri && cargo test`
预期：全部 PASS，包括 `started_event_serializes_with_frontend_field_names`、`started_event_source_type_serializes_for_each_kind`、`cancelled_event_serializes_with_frontend_field_names` 等既有测试。

- [x] **步骤 6：Commit**

```bash
git add src-tauri/src/core/translation/types.rs src-tauri/src/ui/web_popup.rs
git commit -m "feat(translation): Started 事件携带 sourceType 字段"
```

---

## 任务 3：前端徽章 DOM 与样式

**文件：**
- 修改：`frontend/index.html`（`.output-area`，约 90-92 行）
- 修改：`frontend/style.css`（`.output-area`/`.output-box`，约 200-214 行；新增 `.source-badge`）

- [x] **步骤 1：在 index.html 输出区上方加徽章元素**

将 `frontend/index.html` 中的输出区：

```html
      <div class="output-area">
        <div id="outputText" class="output-box">翻译结果将显示在这里</div>
      </div>
```

替换为：

```html
      <div class="output-area">
        <div id="sourceBadge" class="source-badge hidden"></div>
        <div id="outputText" class="output-box">翻译结果将显示在这里</div>
      </div>
```

- [x] **步骤 2：在 style.css 调整输出区布局并新增徽章样式**

将 `frontend/style.css` 中的 `.output-area` 与 `.output-box` 规则：

```css
.output-area {
  flex: 1;
  min-height: 120px;
}

.output-box {
  height: 100%;
  padding: 10px;
  border: 1px solid #ddd;
  border-radius: 8px;
  font-size: 14px;
  background: #fff;
  color: #999;
  overflow-y: auto;
}
```

替换为：

```css
.output-area {
  flex: 1;
  min-height: 120px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.source-badge {
  align-self: flex-start;
  padding: 2px 8px;
  border-radius: 6px;
  font-size: 11px;
  color: #555;
  background: #eef3f8;
}

.output-box {
  flex: 1;
  padding: 10px;
  border: 1px solid #ddd;
  border-radius: 8px;
  font-size: 14px;
  background: #fff;
  color: #999;
  overflow-y: auto;
}
```

说明：`.output-area` 改为纵向 flex，徽章 `align-self: flex-start` 贴左、`.output-box` 用 `flex: 1` 占满剩余高度（取代原 `height: 100%`），不抢占主视觉。

- [x] **步骤 3：Commit**

```bash
git add frontend/index.html frontend/style.css
git commit -m "feat(frontend): 输出区新增来源徽章 DOM 与样式"
```

---

## 任务 4：前端 main.js 徽章显隐逻辑

**文件：**
- 修改：`frontend/main.js`（元素引用区，约 1-23 行；`renderTranslationEvent`，约 200-241 行；`clearBtn` 监听，约 296-304 行）

- [x] **步骤 1：新增 sourceBadge 元素引用**

在 `frontend/main.js` 顶部元素引用区（`const outputText = ...` 下一行）新增：

```js
const sourceBadge = document.getElementById('sourceBadge');
```

- [x] **步骤 2：新增徽章显隐辅助函数**

在 `frontend/main.js` 的 `resetOutput()` 函数之后新增：

```js
function setSourceBadge(sourceType) {
  switch (sourceType) {
    case 'selectedText':
      sourceBadge.textContent = '来自划词';
      sourceBadge.classList.remove('hidden');
      break;
    case 'ocrText':
      sourceBadge.textContent = '来自 OCR';
      sourceBadge.classList.remove('hidden');
      break;
    default:
      // manualText 或未知值：隐藏（防御）
      sourceBadge.classList.add('hidden');
      sourceBadge.textContent = '';
      break;
  }
}

function hideSourceBadge() {
  sourceBadge.classList.add('hidden');
  sourceBadge.textContent = '';
}
```

- [x] **步骤 3：在 Started 分支调用 setSourceBadge，其余结束态调用 hideSourceBadge**

将 `frontend/main.js` 中 `renderTranslationEvent` 的 `started` / `finished` / `failed` / `cancelled` 四个分支替换为：

```js
    case 'started':
      currentSessionId = getSessionId(payload);
      inputText.value = payload.sourceText ?? inputText.value;
      outputText.textContent = '';
      outputText.style.color = '#333';
      setSourceBadge(payload.sourceType);
      setActionButtons({ translating: true, canRetry: false });
      break;
    case 'delta':
      if (!shouldHandleSessionEvent(payload)) return;
      outputText.textContent += payload.text ?? '';
      outputText.style.color = '#333';
      scrollOutputToBottom();
      break;
    case 'finished':
      if (!shouldHandleSessionEvent(payload)) return;
      outputText.textContent = payload.fullText ?? outputText.textContent;
      outputText.style.color = '#333';
      currentSessionId = null;
      hideSourceBadge();
      setActionButtons({ translating: false, canRetry: true });
      scrollOutputToBottom();
      break;
    case 'failed':
      if (currentSessionId && !shouldHandleSessionEvent(payload)) return;
      outputText.textContent = payload.message ?? '翻译失败';
      outputText.style.color = '#b42318';
      currentSessionId = null;
      hideSourceBadge();
      setActionButtons({ translating: false, canRetry: true });
      scrollOutputToBottom();
      break;
    case 'cancelled':
      if (!shouldHandleSessionEvent(payload)) return;
      outputText.textContent += '\n[已取消]';
      outputText.style.color = '#999';
      currentSessionId = null;
      hideSourceBadge();
      setActionButtons({ translating: false, canRetry: true });
      break;
```

- [x] **步骤 4：在清空按钮调用 hideSourceBadge**

将 `frontend/main.js` 中 `clearBtn` 的监听：

```js
clearBtn.addEventListener('click', () => {
  if (isTranslating) {
    return;
  }
  inputText.value = '';
  currentSessionId = null;
  resetOutput();
  setActionButtons({ translating: false, canRetry: false });
});
```

替换为：

```js
clearBtn.addEventListener('click', () => {
  if (isTranslating) {
    return;
  }
  inputText.value = '';
  currentSessionId = null;
  resetOutput();
  hideSourceBadge();
  setActionButtons({ translating: false, canRetry: false });
});
```

- [x] **步骤 5：前端语法检查**

运行：`node --check frontend/main.js`
预期：无输出（语法正确）。

- [x] **步骤 6：Commit**

```bash
git add frontend/main.js
git commit -m "feat(frontend): Started 事件按来源切换徽章显隐"
```

---

## 任务 5：整体验证与文档同步

**文件：**
- 修改：本计划复选框回填、相关 README/roadmap 当前能力与限制（按协作规范第 2 条）

- [x] **步骤 1：Rust 全量测试**

运行：`cd src-tauri && cargo test`
预期：全部 PASS，含 `kind()`、`Started.sourceType` 序列化、既有 `cancelled` 序列化等测试。

- [x] **步骤 2：Rust release 构建**

运行：`cd src-tauri && cargo build --release`
预期：编译通过，无警告（尤其无 dead_code）。

- [x] **步骤 3：前端语法检查**

运行：`node --check frontend/main.js`
预期：无输出。

- [x] **步骤 4：手动验证（mock provider）**

> 自动化验证（cargo test / cargo build --release / node --check）已于编码执行阶段全部通过；手动验证由用户在桌面环境完成并确认通过。

运行：`SHIZI_LLM_PROVIDER=mock npm run tauri dev`（PowerShell 下用 `$env:SHIZI_LLM_PROVIDER='mock'; npm run tauri dev`）

逐项确认：
- 手动输入文本翻译 → 徽章不出现
- `Alt+T` 划词翻译 → 徽章「来自划词」
- `Alt+O` 截图 OCR 翻译 → 徽章「来自 OCR」
- 翻译 finished / 取消 / 失败 / 清空 → 徽章消失

- [x] **步骤 5：同步文档**

按协作规范第 2 条，回填本计划复选框，并同步相关设计文档（README 当前能力与限制、roadmap 完成状态、milestone 2 任务 5 状态）。

- [x] **步骤 6：收尾**

执行 `finishing-a-development-branch`（或等价 finish 流程）做合并/清理。

> 收尾结论：本仓库为单分支（`master`）无远程的普通仓库，「翻译来源展示」功能已实现并提交、测试通过（62 passed）、文档同步完成。`finishing-a-development-branch` 流程检测后采用「保持现状」——无合并目标与远程，无可执行的合并/PR/清理动作，功能闭环。

---

## 自检

**1. 规格覆盖度**
- 数据契约（`Started.source_type` + `kind()`）→ 任务 1、任务 2。✓
- 后端改动（types.rs 字段/方法/测试、web_popup.rs 填充）→ 任务 1、任务 2。✓
- 前端改动（index.html 徽章、main.js 逻辑、style.css 样式）→ 任务 3、任务 4。✓
- 徽章显示规则表（manual 隐藏 / selected 「来自划词」/ ocr 「来自 OCR」/ 未知隐藏）→ 任务 4 `setSourceBadge`。✓
- 结束/取消/失败/清空隐藏 → 任务 4 四分支 + clearBtn。✓
- 测试（kind() 三变体、Started.sourceType 序列化、既有测试不受影响、node --check、手动验证）→ 任务 1/2/5。✓
- 影响面（不动 provider/Service/配置/重试/取消）→ 计划全程未触及这些文件。✓
- 验收标准 → 任务 5。✓

**2. 占位符扫描**：无 TODO / 「类似任务 N」/ 抽象描述；每个代码步骤均含完整代码块。✓

**3. 类型一致性**
- `kind()` 返回 `&'static str`，任务 2 用 `request.input.kind().to_string()` 转 `String` 填 `source_type`。✓
- `source_type: String` 字段名在 types.rs 定义、web_popup.rs 填充、测试构造处一致。✓
- 序列化字段名 `sourceType`（camelCase，由 `rename_all_fields = "camelCase"` 保证）与前端 `payload.sourceType` 一致。✓
- 前端 `sourceBadge`、`setSourceBadge`、`hideSourceBadge` 命名跨步骤一致。✓
- `TranslationInput::OcrText { text, image_id }` 模式匹配与既有 `text()` 一致。✓
