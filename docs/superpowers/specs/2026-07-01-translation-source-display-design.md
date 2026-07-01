# 翻译来源展示设计规格

## 目标

让翻译弹窗能向用户展示当前译文的输入来源（划词 / OCR / 手动），完成 milestone 2 任务 5「前端弹窗展示输入来源：Selected / OCR / Manual」。

TranslationInput 抽象已存在且完整（`ManualText` / `SelectedText` / `OcrText`），provider 已通过 `input.text()` 取文本而不感知来源。本规格只补「来源可见性」——把来源类型透传到前端并以徽章展示，不改动 TranslationInput 枚举、provider、TranslationService、配置与重试/取消链路。

## 非目标

- 不引入 TranslationMode（翻译模式：翻译/解释/摘要等）。
- 不改动 TranslationInput 枚举与序列化。
- 不改动 provider 或 TranslationService。
- 不做来源相关的历史记录或统计。

## 现状

`TranslationEvent::Started` 当前只携带 `session_id` 与 `source_text`，前端无从得知来源类型。[frontend/main.js](../../../frontend/main.js) 无任何来源标签逻辑，[index.html](../../../frontend/index.html) 无来源徽章元素。

三类入口（手动 `start_translation_from_text`、划词、OCR `start_translation_from_ocr`）最终都汇入 `start_translation_from_input`，在那里构造 `TranslationRequest` 并 emit `Started`——这是暴露来源信息的唯一自然出口。

`TranslationInput` 已带 `#[serde(rename_all = "camelCase", tag = "type")]`，其序列化 tag 字面为 `manualText` / `selectedText` / `ocrText`。本规格复用这套字面值，避免引入第二套来源命名。

## 设计

### 数据契约

`TranslationEvent::Started` 新增 `source_type` 字段：

```rust
Started {
    session_id: TranslationSessionId,
    source_text: String,
    source_type: String,   // manualText | selectedText | ocrText
},
```

`TranslationInput` 新增 `kind()` 方法（不改枚举本身）：

```rust
impl TranslationInput {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::ManualText(_) => "manualText",
            Self::SelectedText(_) => "selectedText",
            Self::OcrText { .. } => "ocrText",
        }
    }
}
```

序列化后前端收到 `sourceType` 字段（camelCase），值与 TranslationInput 自身 serde tag 字面一致，零转换。

值域类型选择说明：用 `String` 而非新建 `SourceType` 枚举。值域仅三个、单点产出（`kind()`），类型安全收益抵不过新增类型 + 重复 serde 配置的维护成本（YAGNI）。

### 后端改动

- [src-tauri/src/core/translation/types.rs](../../../src-tauri/src/core/translation/types.rs)
  - `TranslationEvent::Started` 新增 `source_type: String` 字段
  - `TranslationInput` 新增 `kind() -> &'static str` 方法
  - 现有 `started_event_serializes_with_frontend_field_names` 测试补 `sourceType` 断言
  - 新增 `kind()` 返回值单测（3 个变体各一）
- [src-tauri/src/ui/web_popup.rs](../../../src-tauri/src/ui/web_popup.rs)
  - 构造 `Started` 处（约 112 行）填 `source_type: request.input.kind().to_string()`
- 其余不变：`take_pending_source_text`、重试链路、provider、TranslationService 均不动

### 前端改动

[index.html](../../../frontend/index.html)：输出区上方加徽章元素，默认隐藏：

```html
<div id="sourceBadge" class="source-badge hidden">来自划词</div>
<div id="outputText" class="output-box">翻译结果将显示在这里</div>
```

[main.js](../../../frontend/main.js) `Started` 分支：
- 读 `payload.sourceType`
- `manualText` → 隐藏徽章
- `selectedText` → 显示「来自划词」
- `ocrText` → 显示「来自 OCR」
- 未知值 → 隐藏徽章（防御）

翻译结束 / 取消 / 失败 / 清空时隐藏徽章。

[style.css](../../../frontend/style.css)：`.source-badge` 样式——小字号、弱底色圆角，贴合输出区上方，不抢占主视觉。

### 徽章显示规则

| 来源 | 徽章 |
|---|---|
| `manualText` | 不显示 |
| `selectedText` | 「来自划词」 |
| `ocrText` | 「来自 OCR」 |
| 未知值 | 不显示（防御） |

手动输入不显示——用户自己输入，无需提醒来源；划词/OCR 是被动触发，徽章帮助用户理解「这段原文从哪来」。

## 测试

- **Rust 单测**：
  - `kind()` 对三个变体返回正确字面值
  - `Started` 序列化包含 `sourceType` 字段且值为 `manualText` / `selectedText` / `ocrText`
  - 现有 `cancelled_event_serializes_with_frontend_field_names` 等测试不受影响
- **前端**：`node --check frontend/main.js` 语法检查（项目无前端测试框架）
- **手动验证**：`SHIZI_LLM_PROVIDER=mock npm run tauri dev`
  - 手动输入翻译 → 徽章不出现
  - `Alt+T` 划词翻译 → 徽章「来自划词」
  - `Alt+O` 截图 OCR 翻译 → 徽章「来自 OCR」
  - 翻译结束/取消/失败/清空 → 徽章消失

## 影响面与兼容性

- **不触动**：TranslationInput 枚举、provider、TranslationService、配置、重试、取消链路。
- **向后兼容**：`Started` 新增字段；前端旧逻辑读不到 `sourceType` 仅表现为无徽章，不报错。徽章是新 DOM 元素，不影响现有布局。
- **无性能影响**：`Started` payload 多一个短字符串字段。

## 验收标准

- `cargo test` 全绿，新增测试覆盖 `kind()` 与 `Started.sourceType` 序列化。
- `node --check frontend/main.js` 通过。
- 手动验证三类来源徽章表现符合上表。
- provider / TranslationService / 配置代码无改动。
