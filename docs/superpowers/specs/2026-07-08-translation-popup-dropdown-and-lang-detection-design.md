# 翻译弹窗语言下拉对齐原型 + 语言联动增强设计

> 日期：2026-07-08
> 状态：待实现
> 策略：下拉框改 inline 搜索式 combobox + 模型回传检测源语言 + OS 语言作默认目标语言
> 视觉事实来源：`C:\Users\xdj\IdeaProjects\LLM\OpenDesignProjects\shizi\translation-popup.html`（高保真原型）

## 1. 背景与目标

上一版 spec（`2026-07-08-translation-popup-language-and-visual-design.md`）已落地：会话语言状态、语言下拉（浮层 dropdown）、卡片蓝点等。测试后发现两个问题需要本次解决：

1. **下拉框效果不好**：当前用绝对定位浮层 `.lang-dropdown`，被 `.content` 的 `overflow-y: auto` 裁剪（下拉项多时截断、需滚动 content）。高保真原型用 **inline 搜索式 combobox**（`.lang-picker`）规避此问题（原型注释明说「不依赖浮层，避免被弹窗 overflow 截断」），且带搜索框、英文名双列、键盘导航。本次将下拉框对齐原型。
2. **语言联动不完整**：上一版只把 source/target 传进 prompt，但模型并未回传「实际原文是什么语言」，原文右下角 `.lang-badge` 仍是静态「自动检测」文案（上一版 OQ-2 明确未动）；默认目标语言硬编码 `zh-CN`，未读操作系统语言。本次补齐这两点。

**目标**：三个需求一并落地--①下拉框对齐原型 inline 搜索式 combobox；②source=auto 时模型回传检测到的原文语言，弹窗右下角标签动态显示；③首次安装默认目标语言读 OS 语言，不在列表则回退英语。

## 2. 与上一版 spec 的关系

- 上一版「会话语言状态」「`set_session_languages` command」「翻译入口读 session 语言」**全部保留不变**，本次复用。
- 上一版 OQ-2「不动 `.lang-badge`」**本次推翻**：`.lang-badge` 改为动态显示检测语言（需求 2-A 的前端落脚点）。
- 上一版「`.lang-dropdown` 浮层」**本次替换**为 inline `.lang-picker`（需求 1）。
- 上一版「`DEFAULT_TARGET_LANG = "zh-CN"`」**本次改为**读 OS 语言（需求 2-B）。

## 3. 范围

### 改动

**需求 1（纯前端）**：
- `frontend/public/translate.html`：`.lang-toolbar` 后插入 `.lang-picker` 块；`.lang-side` 由 `<div>` 改 `<button type="button">`。
- `frontend/public/translate.css`：删 `.lang-dropdown` / `.lang-dropdown-item`；新增 `.lang-picker*` / `.lang-option*` / `@keyframes langPickerIn`（照搬原型）。
- `frontend/public/translate.js`：`LANGUAGES` 补 `english` 字段；删浮层逻辑；新增 inline picker 逻辑（搜索 + 键盘导航 + toggle）。

**需求 2-A（跨端）**：
- `src-tauri/src/core/translation/types.rs`：`user_prompt` 在 source=auto 时追加检测指令；`TranslationEvent::Finished` 加 `detected_source_lang: Option<String>` 字段。
- `src-tauri/src/core/translation/service.rs`：`translate_with` 加流式首行解析状态机（source=auto 时启用）。
- `frontend/public/translate.js`：监听 Finished 的 `detectedSourceLang`，动态更新 `.lang-badge`。
- `frontend/public/translate.html`：`.lang-badge` 移除静态文案，默认隐藏。

**需求 2-B（后端 config）**：
- `src-tauri/Cargo.toml`：加 `sys-locale` 依赖。
- `src-tauri/src/core/config/types.rs`：`DEFAULT_TARGET_LANG` 改为 `default_target_lang_from_os()` 函数；新增 `map_os_lang_to_list(os: &str) -> String` 纯函数。
- 后端单测：映射函数 + 流式解析 + Finished 序列化。

### 不做（YAGNI）

- 不改 `build_batch_requests` 签名（已接收 source_lang/target_lang）。
- 不改 `LlmProvider` trait / 各 provider（mock/openai/claude）的 `stream_translate`--流式解析状态机放在 `TranslationService::translate_with` 共用层，provider 只产出原始 Delta。
- 不强制模型返回语言 code：模型返回自然语言中文名（如「英语」），前端直接显示，不与下拉标签强制对齐。
- 不做存量用户 config 迁移：config.json 已存在的 target_lang 保留原值，from_env 只在首次安装触发。
- 不改设置页 TranslatePanel 的语言下拉（保持上一版：源含 auto、目标过滤 auto）。
- 不碰 overlay.html、OCR 链路、卡片蓝点/流式光标等上一版成果。
- 不给前端下拉/picker 加 vitest 单测（纯静态，与 overlay 一致，靠手动验证）。

## 4. 需求 1：下拉框对齐原型 inline 搜索式 combobox

视觉与交互事实来源为原型 `translation-popup.html` 的 `.lang-picker` 实现（原型 line 224-298 CSS、line 654-660 HTML、line 1130-1269 JS）。本需求把当前浮层 dropdown 替换为该形态。

### 4.1 HTML（translate.html）

`.lang-toolbar` 之后、`.results` 之前插入（与原型同位，inline 占据文档流）：

```html
<div class="lang-picker" id="langPicker" hidden>
  <div class="lang-picker-search">
    <svg class="lang-picker-search-icon" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="7"/><line x1="20" y1="20" x2="16.65" y2="16.65"/></svg>
    <input type="text" class="lang-picker-input" id="langPickerInput" placeholder="搜索语言…" autocomplete="off" spellcheck="false" />
  </div>
  <ul class="lang-picker-list" id="langPickerList"></ul>
</div>
```

`.lang-side` 由 `<div>` 改 `<button type="button">`（对齐原型语义，CSS 已支持 flex 布局）。

### 4.2 CSS（translate.css）

- **删** `.lang-dropdown` / `.lang-dropdown-item`（浮层废弃）。
- **新增**（照搬原型，保持类名与样式一致）：
  - `.lang-picker`：白底卡片 + `border-radius: var(--radius-md)` + `0.5px solid var(--border)` + `box-shadow: var(--shadow-card)` + `overflow: hidden` + `animation: langPickerIn .15s ease`
  - `.lang-picker[hidden] { display: none }`
  - `.lang-picker-search`：flex + gap 8px + padding 7px 10px + `border-bottom: 0.5px solid var(--border)` + `background: var(--bg-soft)`
  - `.lang-picker-search-icon`：13x13 + `color: var(--fg-3)`
  - `.lang-picker-input`：flex 1 + 无边框无背景 + `font-size: 0.75rem` + outline none
  - `.lang-picker-list`：`max-height: 220px` + `overflow-y: auto` + `padding: 4px 0` + 滚动条样式
  - `.lang-option`：flex + `justify-content: space-between` + gap 12px + padding 6px 12px + `font-size: 0.75rem` + `transition: background .08s`
  - `.lang-option:hover, .lang-option.is-active`：`background: var(--bg-soft)`
  - `.lang-option.is-selected`：`color: var(--accent)` + `font-weight: 600`
  - `.lang-option.is-selected .lang-option-english`：`color: var(--accent); opacity: .7`
  - `.lang-option-native`：flex-shrink 0 + 省略号
  - `.lang-option-english`：`color: var(--fg-3)` + `font-size: 0.6875rem`
  - `@keyframes langPickerIn`：`from { opacity: 0; transform: translateY(-4px) }` -> `to { opacity: 1; transform: translateY(0) }`

### 4.3 JS（translate.js）

**LANGUAGES 补 `english` 字段**（10 项 value 不变，与 `frontend/src/settings/tokens.ts` 同源）：

```js
const LANGUAGES = [
  { value: 'auto',  label: '自动检测', english: 'Auto Detect' },
  { value: 'zh-CN', label: '简体中文', english: 'Chinese (Simplified)' },
  { value: 'zh-TW', label: '繁體中文', english: 'Chinese (Traditional)' },
  { value: 'en-US', label: 'English', english: 'English' },
  { value: 'ja-JP', label: '日本語',   english: 'Japanese' },
  { value: 'ko-KR', label: '한국어',   english: 'Korean' },
  { value: 'fr-FR', label: 'Français', english: 'French' },
  { value: 'de-DE', label: 'Deutsch',  english: 'German' },
  { value: 'es-ES', label: 'Español',  english: 'Spanish' },
  { value: 'ru-RU', label: 'Русский',  english: 'Russian' },
];
```

**删浮层逻辑**：`activeDropdown` / `openDropdown` / `closeDropdown` / `onDropdownOutsideClick` / `onDropdownEsc`。

**新增 inline picker 逻辑**（照原型 `openLangPicker` / `closeLangPicker` / `renderLangList` / `selectLang`）：
- `activeLangType`（null | 'source' | 'target'）
- `openLangPicker(side)`：toggle（同 side 再点关闭）；设 placeholder（搜索源/目标语言）；`renderLangList('')`；`picker.hidden = false`；`requestAnimationFrame(() => input.focus())`
- `closeLangPicker()`：`picker.hidden = true` + `activeLangType = null`
- `renderLangList(q)`：target 去 auto；按 `label` 或 `english` 小写包含过滤；渲染 `<li class="lang-option">` 含 `<span class="lang-option-native">` + `<span class="lang-option-english">`；当前项加 `is-selected`；首个 `is-selected` 或首项加 `is-active`
- `selectLang(side, code)`：更新 `sessionSourceLang`/`sessionTargetLang` -> `renderLangLabels()` -> `invoke('set_session_languages', ...)` -> `closeLangPicker()`
- input `keydown`：ArrowDown/Up 移动 `is-active` + `scrollIntoView({block:'nearest'})`；Enter 选 `is-active`；Escape 关闭
- list `click`：`closest('.lang-option')` -> `selectLang`
- document `click`：外部点击（不含 `.lang-side` 与 picker 内）关闭
- `swapLangs` 时若 picker 开着则关闭

**保留不变**：`swapLangs` 的「含 auto 跳过 + toast」逻辑（比原型更安全）；`set_session_languages` IPC 调用；`renderLangLabels` / `LANG_LABEL`。

## 5. 需求 2-A：模型返回检测到的源语言

### 5.1 prompt 改造（types.rs `user_prompt`）

当 `self.prompts.source_lang == "auto"` 时，在 `user_prompt` 返回值末尾追加：

```
\n\n请先在第一行用【源语言：语言名称】输出你检测到的原文语言（如：英语、日语、中文），换行后再输出译文。
```

source 是具体语言时不追加（模型正常翻译，不检测）。

> 注：`prompts.source_lang` 由 `build_batch_requests` 写入会话源语言（[batch.rs:30](src-tauri/src/core/translation/batch.rs:30)），故 `request.prompts.source_lang == "auto"` 等价于「用户选了自动检测」。

### 5.2 流式解析状态机（service.rs `translate_with`）

在 `translate_with` 的 `stream_translate` 回调里，source=auto 时启用首行解析。状态：

- `pending_header: String`：累积首行字符，直到遇到首个 `\n`
- `header_parsed: bool`：首行是否已解析完毕
- `detected: Option<String>`：解析到的语言名

**Delta 处理**（`request.prompts.source_lang == "auto"` 时）：

```
若 !header_parsed:
    pending_header.push_str(&text)
    若 pending_header 含 '\n':
        按 '\n' 分割一次 -> (首行, 剩余)
        首行用正则 /【源语言：(.+?)】/ 匹配:
            匹配成功 -> detected = Some(捕获的语言名)
            匹配失败 -> 首行内容作为 Delta 补发（不吞译文）
        剩余作为 Delta 发出
        header_parsed = true
    否则: 不发 Delta（继续累积）
否则:
    text 直接作为 Delta 发出
```

source 非 auto 时：Delta 直接透传，不走状态机。

**Finished 处理**：把 `detected` 放进 `TranslationEvent::Finished { detected_source_lang: detected, .. }`。

**边界**：
- 译文极短无 `\n`：Finished 时 `pending_header` 仍累积，解析无匹配 -> 内容补作 Delta（在 Finished 前补发），`detected = None`。
- 模型首行无标记 -> 降级，首行作 Delta 补发，`detected = None`，译文完整。
- 标记跨多个 Delta chunk -> 状态机累积到 `\n` 才解析，正确拼接。

### 5.3 事件 schema 变更（types.rs `TranslationEvent::Finished`）

```rust
Finished {
    session_id: TranslationSessionId,
    #[serde(flatten)]
    service: TranslationServiceMeta,
    full_text: String,
    usage: Option<TokenUsage>,
    detected_source_lang: Option<String>,  // 新增
}
```

- `#[serde(rename_all = "camelCase")]` 已在 enum 级，序列化为 `detectedSourceLang`。
- 非 auto 或解析失败为 `None` -> JSON `null`。
- `Started` / `Delta` / `Failed` / `Cancelled` 不变。

### 5.4 前端标签动态化（translate.js + translate.html）

**translate.html**：`.lang-badge` 移除静态「自动检测」文案，改为空 span（默认隐藏）。

**translate.js**：
- 新增 `setSourceBadge(text or null)`：`text` 为 null 时隐藏 `.lang-badge`，否则设 textContent 并显示。
- `renderLangLabels()` 末尾调用：`sessionSourceLang == 'auto'` 时显示「检测中…」占位；否则 `setSourceBadge(null)` 隐藏。
- `renderTranslationEvent` 的 `started`（new batch 重置块）：若 source=auto 设「检测中…」，否则隐藏。
- `renderTranslationEvent` 的 `finished`：取 `event.detectedSourceLang`，若非 null 且 source=auto 则 `setSourceBadge(detectedSourceLang)`；若为 null（模型未按格式）则 `setSourceBadge(null)` 隐藏（降级，不保留「检测中…」避免翻译完成后仍显示占位造成困惑）。
- 多服务批翻译：取首个非 null 的 `detectedSourceLang`（各服务检测应一致；不一致取首个）。

## 6. 需求 2-B：默认目标语言读 OS 语言

### 6.1 新增依赖

`src-tauri/Cargo.toml` 的 `[dependencies]` 加：

```toml
sys-locale = "0.3"
```

`sys_locale::get_locale()` 同步返回 `Option<String>`（如 `"zh-CN"`、`"en-US"`），跨平台无 unsafe。

### 6.2 from_env 改造（types.rs）

将 `const DEFAULT_TARGET_LANG: &str = "zh-CN"` 替换为函数：

```rust
fn default_target_lang_from_os() -> String {
    let os = sys_locale::get_locale().unwrap_or_else(|| "en-US".to_string());
    map_os_lang_to_list(&os)
}
```

`AppConfig::from_env()` 中 `target_lang` 改为 `default_target_lang_from_os()`。`normalized` 的 `normalize_string(self.target_lang, ...)` 回退值改为常量 `const FALLBACK_TARGET_LANG: &str = "en-US"`（**不在 normalize 里调 `sys_locale`**，避免每次 save 都查 OS；normalize 的 fallback 只在 target_lang 为空时触发，极少）。原 `DEFAULT_TARGET_LANG` 常量拆为两个职责：`default_target_lang_from_os()`（from_env 用，读 OS）+ `FALLBACK_TARGET_LANG = "en-US"`（normalize 兜底，不读 OS）。

> 实现澄清：`from_env` 是同步函数，`sys_locale::get_locale()` 同步，兼容。`normalized` 在 `ConfigStore::load` 和 `save` 时调用，此时 `target_lang` 已有值（非空），`normalize_string` 不会触发 fallback，故 OS 映射只在 `from_env` 首次构造时生效。

### 6.3 语言映射纯函数

```rust
fn map_os_lang_to_list(os: &str) -> String {
    let lower = os.to_lowercase();
    // 精确匹配
    let codes = ["zh-CN", "zh-TW", "en-US", "ja-JP", "ko-KR", "fr-FR", "de-DE", "es-ES", "ru-RU"];
    if codes.contains(&lower.as_str()) { return lower; }
    // 主语言前缀匹配
    let main = lower.split('-').next().unwrap_or("");
    match main {
        "zh" => if lower.contains("hant") || lower.contains("tw") { "zh-TW" } else { "zh-CN" },
        "en" => "en-US",
        "ja" => "ja-JP",
        "ko" => "ko-KR",
        "fr" => "fr-FR",
        "de" => "de-DE",
        "es" => "es-ES",
        "ru" => "ru-RU",
        _ => "en-US",
    }.to_string()
}
```

- 精确匹配列表 code -> 用
- `zh-Hans`/`zh-CN`/`zh-SG` -> zh-CN；`zh-Hant`/`zh-TW`/`zh-HK` -> zh-TW
- 其他主语言按前缀映射
- 都不匹配 -> `en-US`（符合「不在列表则英语」）

## 7. 数据流

```
启动:
  config.json 不存在 -> AppConfig::from_env()
    -> target_lang = default_target_lang_from_os()
    -> sys_locale::get_locale() -> map_os_lang_to_list -> code（或 en-US）
    -> 持久化到 config.json
  AppState::new -> session_target_lang = config.target_lang

弹窗翻译（source=auto）:
  build_batch_requests(input, target, "auto", services, batch_id)
    -> TranslationRequest { prompts.source_lang: "auto", target_lang, ... }
  TranslationService::translate_with(request, ...):
    user_prompt 末尾追加检测指令
    provider.stream_translate 产出 Delta:
      状态机累积首行 -> 解析【源语言：xxx】 -> detected=Some("xxx")
      剩余作 Delta 发出 -> 前端流式渲染纯译文
    Finished { full_text, detected_source_lang: Some("xxx") }
  前端 renderTranslationEvent finished:
    setSourceBadge("xxx")  // .lang-badge 显示检测语言

弹窗翻译（source=具体语言）:
  prompts.source_lang != "auto" -> user_prompt 不追加检测指令
  状态机不启用 -> Delta 直接透传
  Finished { detected_source_lang: None }
  前端: .lang-badge 隐藏

弹窗下拉选语言:
  点 langSource/langTarget -> openLangPicker(toggle) -> inline picker 显示
    -> .content 撑高 -> ResizeObserver 调高弹窗（不裁剪）
  搜索/↑↓/Enter -> selectLang -> set_session_languages -> closeLangPicker
```

## 8. 错误处理

| 场景 | 处理 |
|---|---|
| 模型不按格式输出首行标记 | 解析失败，首行作 Delta 补发，`detected=None`，.lang-badge 保持「检测中…」或隐藏 |
| 模型首行就是译文（无标记） | 同上降级，译文完整不丢 |
| 标记跨多个 Delta chunk | 状态机累积到 `\n` 才解析，正确拼接 |
| 译文极短无 `\n` | Finished 前 pending_header 补作 Delta，`detected=None` |
| `sys_locale::get_locale()` 返回 None | fallback `"en-US"` -> map -> `en-US` |
| OS 语言不在列表（如泰语 `th-TH`） | map 主语言 `th` 不匹配 -> `en-US` |
| 多服务检测结果不一致 | 前端取首个非 None |
| `set_session_languages` 失败 | toast 提示，标签已乐观更新（与上一版一致） |
| picker 打开时翻译触发 | picker 不阻断翻译；新 batch 重置卡片时 picker 仍开着无妨 |

## 9. 测试

### 后端单测（cargo test）

**types.rs**：
- `map_os_lang_to_list` 各分支：`zh-CN`->`zh-CN`、`zh-Hans`->`zh-CN`、`zh-Hant`->`zh-TW`、`en-GB`->`en-US`、`fr-FR`->`fr-FR`、`th-TH`->`en-US`、`"xx-YY"`->`en-US`、空串->`en-US`。
- `from_env_target_lang_uses_os_or_fallback`：`from_env().target_lang` 在 `map_os_lang_to_list` 的值域内（依赖测试机 locale，断言「是列表 code 之一或 en-US」）。
- `finished_event_serializes_with_detected_source_lang`：Finished 含 `detectedSourceLang` 字段，序列化为 camelCase。
- `finished_event_detected_source_lang_null_when_none`：None 时 JSON `null`。
- `user_prompt_appends_detection_instruction_when_auto`：source_lang="auto" 时 user_prompt 含「【源语言：语言名称】」。
- `user_prompt_no_append_when_specific_source`：source_lang="en-US" 时不含检测指令。
- 上一版 `from_env_default_target_lang_is_zh_cn` 测试需调整（默认值不再硬编码 zh-CN，改为「OS 映射结果」）。

**service.rs**：
- `translate_detects_source_lang_from_header`：mock provider 输出 `【源语言：英语】\n译文内容` -> Delta 为「译文内容」（不含标记），Finished.detectedSourceLang=Some("英语")。
- `translate_fallbacks_when_no_header_marker`：mock 输出 `译文无标记` -> detected=None，Delta 透传完整译文。
- `translate_handles_marker_across_chunks`：mock 分两次 Delta 输出 `【源语言：英` + `语】\n译文` -> 正确拼接解析。
- 非 auto 时不启用状态机：mock 输出原样，detected=None。

> mock provider 改造：为支持状态机测试，mock 在 `request.prompts.source_lang == "auto"` 时输出 `【源语言：英语】\n[Mock 翻译] {source_text} -> {target_lang}`（首行标记 + 换行 + 译文），非 auto 时保持原逻辑 `[Mock 翻译] {source_text} -> {target_lang}`。现有 mock 测试（非 auto 场景）不回归。

### 前端

- `npm run typecheck` 通过（translate.js/html/css 不在 typecheck 范围，但 settings 的改动无）。
- `npm run build` 通过。
- 翻译弹窗纯静态，无 vitest，靠手动验证。

### 手动验证清单（npm run tauri dev）

1. **下拉 inline**：点源语言 -> picker 出现在工具栏下方（不浮层），10 项含自动检测，搜索框获焦；点目标 -> 9 项无自动检测。
2. **搜索**：输「英」出 English；输「japanese」出日本語。
3. **键盘导航**：↑↓ 移动焦点，Enter 选中，Esc 关闭。
4. **toggle/外部点击/swap** 关闭 picker。
5. **不裁剪**：翻译中打开 picker -> 弹窗撑高，下拉项完整可见。
6. **开关动画**：picker 显示有淡入下移。
7. **OS 默认目标**：删 config.json 启动 -> 弹窗目标 = OS 语言（中文 OS->简体中文；英文 OS->English）。
8. **OS 不在列表**：OS 设为泰语 -> 默认英语。
9. **检测源语言**：source=auto 翻译英语原文 -> 右下角 `.lang-badge` 显示「英语」；翻译中显示「检测中…」。
10. **具体源语言**：source 选 English -> 右下角标签隐藏。
11. **模型不按格式**（难手动触发，靠单测）：译文正常显示，标签不显示检测语言。

## 10. 文档同步（收尾硬门禁）

- spec：本设计文档。
- README.md：下拉框 inline 搜索式行为、模型检测源语言、OS 默认目标语言。
- docs/roadmap/progressive-development-plan.md：标注相关项完成。
- CLAUDE.md / AGENTS.md：架构关键点补「下拉 inline combobox」「Finished 事件 detectedSourceLang」「from_env 读 OS 语言」；前后端通信补 `translation:event` Finished 字段变更。
- plugins.md：无新插件，不动。

## 11. 风险

- **模型不遵守首行标记格式**：降级不显示检测语言，译文不受影响。单测覆盖降级路径。手动验证难触发，靠单测保证。
- **标记跨 chunk 解析**：状态机累积到 `\n` 才解析，正确拼接。单测覆盖跨 chunk 场景。
- **OS 语言映射不全**：仅覆盖 9 种主语言 + en-US 兜底，其他语言一律 en-US。符合需求「不在列表则英语」。后续可扩充 `map_os_lang_to_list`。
- **存量用户 config 不变**：旧版用户 target_lang 仍为 zh-CN（或用户改过的值），from_env 不触发。中国用户 OS 中文 -> zh-CN 一致，无感；外国 OS 存量用户需手动改设置。符合「尊重用户已选值」。
- **`sys-locale` 依赖**：轻量纯 Rust crate，无平台特定编译要求。Cargo.toml 加一行。
- **mock provider 测试改造**：mock 需输出首行标记以测状态机。改造时保持现有 mock 测试（非 auto 场景）不回归。
- **多服务检测不一致**：理论上各模型对同一段原文检测一致；不一致时取首个，可接受。
- **`Finished` schema 变更向后兼容**：新增 `detected_source_lang: Option<String>` 字段，旧前端忽略未知字段；但本需求前端同步改，无版本错位。
