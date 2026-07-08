# 翻译弹窗语言联动与卡片视觉优化设计

> 日期：2026-07-08
> 状态：待实现
> 策略：后端会话语言状态 + 语言代码统一 + 弹窗语言下拉 + 卡片状态视觉

## 1. 背景与目标

用户提出 4 个翻译弹窗相关优化：

1. **语言联动**：翻译弹窗的源/目标语言默认与设置页「翻译模块」一致；弹窗内手动修改只对本次运行生效（不持久化）；软件彻底退出重启后重置为设置页默认；软件保持打开（窗口隐藏）期间保留修改。同时修复弹窗当前显示语言代码而非语言名称的问题。
2. **默认源语言自动检测**：设置页「默认源语言」下拉当前过滤掉了 `auto` 选项，用户选了具体语言后无法选回「自动检测」，需补回。
3. **任务栏图标**：翻译弹窗（main 窗口）无需在 Windows 任务栏显示图标。
4. **卡片状态视觉**：参考高保真原型（`C:\Users\xdj\IdeaProjects\LLM\OpenDesignProjects\shizi\translation-popup.html`），翻译进行中在卡片头部加持续闪动小蓝点，卡片内保留流式光标，提示用户翻译未结束（缓解智谱 AI 长文本段间延迟被误判为截断的问题）。

**目标**：4 个需求一并落地，不新增后端翻译业务逻辑，保持 translate.html 纯静态技术栈。

## 2. 范围

### 改动

- `src-tauri/src/app/state.rs`：AppState 新增会话语言字段 + 读写方法 + 启动初始化。
- `src-tauri/src/ui/web_popup.rs`：`start_translation_from_input` 改用 AppState 会话语言构造批次；新增 `get_session_languages` / `set_session_languages` command。
- `src-tauri/src/lib.rs`：注册两个新 command。
- `src-tauri/src/core/config/types.rs`：`DEFAULT_TARGET_LANG` 由 `"中文"` 改为 `"zh-CN"`（统一为代码）。
- `src-tauri/tauri.conf.json`：main 窗口加 `skipTaskbar: true`。
- `frontend/src/settings/stores/settings.ts`：`defaultTargetLang` 默认值 `'中文'` -> `'zh-CN'`；`syncFromBackend` 补 `targetLang` 回读。
- `frontend/src/settings/panels/TranslatePanel.vue`：源语言下拉补回 `auto`；目标语言下拉过滤 `auto`。
- `frontend/public/translate.js`：维护 `LANGUAGES` 代码↔名称映射；init 读会话语言显示；`langSource`/`langTarget`/`langSwap` 接入下拉与交换；卡片头部加状态蓝点。
- `frontend/public/translate.css`：加 `.result-header-status` / `.result-header-dot` / `@keyframes pulse-dot`（搬原型）。
- `frontend/public/translate-card-sync.js`：删除直接写 config 语言标签的逻辑（语言标签改由会话语言渲染）。
- 后端单测：`DEFAULT_TARGET_LANG` 断言更新；新增会话语言读写单测。

### 不做（YAGNI）

- 不改后端 `TranslationRequest::user_prompt` / `system_prompt` 的 prompt 生成逻辑：`target_lang` 存储值（代码，如 `zh-CN`）直接进 prompt。模型对 `zh-CN`/`en-US` 等 locale 代码可理解；若后续需更友好的「简体中文」名称，单独加后端代码->名称映射，不在本次。
- 不改 `build_batch_requests` 签名（batch.rs）：它已接收 `target_lang`/`source_lang` 参数，只需调用方改传值。
- 不新增后端代码->名称映射表：弹窗显示名称用前端映射，后端 prompt 沿用代码。
- 不碰 overlay.html、设置页其他模块、OCR/翻译业务逻辑。
- 不改 settings 窗口任务栏（保留图标，方便 Alt+Tab）。
- 不迁 Vue（translate 保持纯静态）。

## 3. 总体架构：会话语言状态

### 3.1 问题

后端 `start_translation_from_input`（web_popup.rs:84-90）从 `config.target_lang` / `config.default_source_lang` 取语言，**不接受前端传参**。而划词（Alt+D）、截图 OCR（Alt+E）是后端自动触发的，前端没机会传语言。要让「弹窗临时改语言对所有翻译入口生效」，后端必须持有一份运行时会话语言。

### 3.2 方案对比

- **方案 A（采用）**：AppState 新增会话语言字段 `session_source_lang` / `session_target_lang`（RwLock 保护）。启动时从 config 初始化；前端 `set_session_languages` 写入；所有翻译入口优先读会话语言。新增 `get_session_languages` 供弹窗 init 读取。
- 方案 B：扩展 `start_translation` 加可选参数——划词/OCR 路径无法传参，等于「仅手动生效」，已被否决。
- 方案 C：前端 localStorage + 翻译前传参——划词/OCR 后端触发时前端无法介入，失效。

选 A。AppState 已有 translation 锁、pending_source_text 等运行时状态，风格一致。

### 3.3 生命周期

```
软件启动
  -> AppState::new(config_store)
  -> 会话语言 = config.defaultSourceLang / config.targetLang  （初始化）
弹窗 initCards
  -> invoke('get_session_languages') -> { sourceLang, targetLang }
  -> 代码->名称映射后显示在 langSource / langTarget 标签
用户弹窗改语言
  -> invoke('set_session_languages', { sourceLang, targetLang })
  -> 写 AppState（不持久化），弹窗立即更新标签
翻译触发（手动 / 划词 / OCR / 重试）
  -> start_translation_from_input 从 AppState 读会话语言构造批次
软件退出
  -> 会话语言丢失（仅内存）
软件重启
  -> 从 config 重新初始化  ✓ 重置为设置页默认
```

### 3.4 边界：设置页改 config 默认语言时

会话语言**不随之改变**（直到重启）。即用户在设置页改了「默认目标语言」，本次运行弹窗仍用旧会话语言；重启后才生效。这符合需求 1「软件打开时与设置一致」「重启重置」。设置页 `save_app_config` 广播的 `app-config:changed`，弹窗监听后只刷新服务卡片，**不重置会话语言**。

## 4. 需求 1：语言联动

### 4.1 后端

**AppState（state.rs）**：

```rust
pub struct AppState {
    // ... 现有字段
    session_source_lang: RwLock<String>,
    session_target_lang: RwLock<String>,
}
```

- `AppState::new(config_store)` 内从 `config_store.get()` 读 `default_source_lang` / `target_lang` 初始化会话语言（get 失败则回退 `"auto"` / `"zh-CN"`）。
- 新增方法：
  - `session_languages(&self) -> (String, String)`：读会话源/目标语言。
  - `set_session_languages(&self, source: String, target: String) -> Result<(), String>`：写会话语言。

**新 command（web_popup.rs）**：

```rust
#[tauri::command]
pub async fn get_session_languages(
    state: tauri::State<'_, AppState>,
) -> Result<SessionLanguages, String> {
    let (source_lang, target_lang) = state.session_languages();
    Ok(SessionLanguages { source_lang, target_lang })
}

#[tauri::command]
pub async fn set_session_languages(
    source_lang: String,
    target_lang: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state.set_session_languages(source_lang, target_lang)
}
```

`SessionLanguages` 用 `#[serde(rename_all = "camelCase")]` 序列化为 `{ sourceLang, targetLang }`。

**翻译入口（web_popup.rs:84-90）**：

```rust
let (source_lang, target_lang) = state.session_languages();
let requests = batch::build_batch_requests(
    input.clone(),
    target_lang,        // 原为 config.target_lang.clone()
    source_lang,        // 原为 config.default_source_lang.clone()
    &config.services,
    &batch_id,
)?;
```

**lib.rs**：`invoke_handler` 加 `get_session_languages` / `set_session_languages`。

### 4.2 前端弹窗（translate.js）

**LANGUAGES 映射**：在 translate.js 顶部定义，注释标记「与 `frontend/src/settings/tokens.ts` LANGUAGES 同源，新增语言两处同步」：

```js
const LANGUAGES = [
  { value: 'auto', label: '自动检测' },
  { value: 'zh-CN', label: '简体中文' },
  { value: 'zh-TW', label: '繁體中文' },
  { value: 'en-US', label: 'English' },
  { value: 'ja-JP', label: '日本語' },
  { value: 'ko-KR', label: '한국어' },
  { value: 'fr-FR', label: 'Français' },
  { value: 'de-DE', label: 'Deutsch' },
  { value: 'es-ES', label: 'Español' },
  { value: 'ru-RU', label: 'Русский' },
];
const LANG_LABEL = (code) => LANGUAGES.find((l) => l.value === code)?.label ?? code;
```

未知代码回退显示原值（友好降级，兼容旧 config 里残留的「中文」名称）。

**会话语言状态**：

```js
let sessionSourceLang = 'auto';
let sessionTargetLang = 'zh-CN';
```

**initCards**：调 `get_session_languages` 读会话语言，更新标签：

```js
async function initCards() {
  if (!invoke) return;
  try {
    const [config, langs] = await Promise.all([
      invoke('get_app_config'),
      invoke('get_session_languages'),
    ]);
    if (config?.logLevel) logger.setLevel(config.logLevel);
    sessionSourceLang = langs.sourceLang ?? 'auto';
    sessionTargetLang = langs.targetLang ?? 'zh-CN';
    renderLangLabels();
    refreshCardsFromConfig(config);
  } catch { return; }
}
```

**renderLangLabels**：

```js
function renderLangLabels() {
  langSource.querySelector('.lang-label').textContent = LANG_LABEL(sessionSourceLang);
  langTarget.querySelector('.lang-label').textContent = LANG_LABEL(sessionTargetLang);
}
```

**语言下拉**：`langSource` / `langTarget` 点击弹出轻量下拉（纯 JS，原型 chevron 样式）。下拉项 = LANGUAGES（源含 `auto`，目标过滤 `auto`）。选择后：

```js
async function selectLang(side, code) {
  if (side === 'source') sessionSourceLang = code;
  else sessionTargetLang = code;
  renderLangLabels();
  try { await invoke('set_session_languages', { sourceLang: sessionSourceLang, targetLang: sessionTargetLang }); }
  catch (e) { showToast(String(e)); }
}
```

**交换按钮**：交换源/目标语言。若任一方为 `auto` 则 toast「自动检测不支持交换」并跳过（避免 `auto` 作为目标污染 prompt，与目标下拉过滤 `auto` 的策略一致）：

```js
async function swapLangs() {
  if (sessionSourceLang === 'auto' || sessionTargetLang === 'auto') {
    showToast('自动检测不支持交换');
    return;
  }
  [sessionSourceLang, sessionTargetLang] = [sessionTargetLang, sessionSourceLang];
  renderLangLabels();
  await invoke('set_session_languages', { sourceLang: sessionSourceLang, targetLang: sessionTargetLang });
}
```

目标语言永远不会是 `auto`：下拉过滤 `auto` + 交换跳过 `auto`，双重保证。

**translate-card-sync.js**：删除第 35-45 行写 `langSource`/`langTarget` 标签的逻辑（语言标签改由 `renderLangLabels` 管理）。`syncServiceCards` 的 `deps` 不再需要 `langSource`/`langTarget`（调用方清理）。

**HTML（translate.html）**：`langSource` / `langTarget` 加 chevron svg（与原型一致），改为可点击按钮（已有 `.lang-side` 样式，CSS 已支持）。

## 5. 需求 2：源语言自动检测选项 + 语言代码统一

### 5.1 TranslatePanel.vue

```vue
<!-- 源语言：补回 auto -->
<SettingSelect v-model="state.translation.defaultSourceLang" :options="languageOptions" />
<!-- 目标语言：过滤 auto -->
<SettingSelect
  v-model="state.translation.defaultTargetLang"
  :options="languageOptions.filter((l) => l.value !== 'auto')"
/>
```

### 5.2 默认值统一为代码

- `frontend/src/settings/stores/settings.ts`：`defaultTargetLang: '中文'` -> `'zh-CN'`。
- `src-tauri/src/core/config/types.rs`：`DEFAULT_TARGET_LANG` 由 `"中文"` -> `"zh-CN"`；`normalized` 的回退值随之改（`normalize_string(self.target_lang, DEFAULT_TARGET_LANG)`）。
- 测试 `normalized_fills_ui_runtime_defaults`（types.rs:408 `assert_eq!(config.target_lang, "中文")`）更新为 `"zh-CN"`。

### 5.3 syncFromBackend 补 targetLang 回读

`frontend/src/settings/stores/settings.ts` 的 `syncFromBackend`（第 517-518 行附近）补：

```js
state.translation.defaultTargetLang =
  backend.targetLang ?? state.translation.defaultTargetLang;
```

修复当前只回读 sourceLang、漏 targetLang 的 bug（需求 1 联动的前提）。

## 6. 需求 3：任务栏图标

`src-tauri/tauri.conf.json` 的 main 窗口加 `"skipTaskbar": true`：

```json
{
  "label": "main",
  "url": "translate.html",
  "title": "Shizi - 翻译助手",
  "width": 420,
  "height": 480,
  "resizable": false,
  "decorations": false,
  "transparent": true,
  "skipTaskbar": true,
  "center": true
}
```

main 窗口由 Tauri 配置创建（`popup_window.rs::ensure_popup_window` 是空函数），改配置即可。settings 窗口（`show_settings_window`）不动，保留任务栏图标。用户通过托盘菜单 / 快捷键 / 划词唤起翻译弹窗。

## 7. 需求 4：卡片视觉

### 7.1 卡片头部蓝点

**translate.js `getCard`**：在 `result-card-header` 引擎名后、折叠按钮前，加状态蓝点：

```js
card.innerHTML = [
  '<div class="result-card-header">',
  '  <svg class="result-engine-icon" viewBox="0 0 20 20"></svg>',
  '  <span class="result-engine-name">' + displayName + '</span>',
  '  <span class="result-header-status" hidden><span class="result-header-dot"></span></span>',
  '  <button class="result-collapse-btn" title="折叠">...</button>',
  '</div>',
  // ... 其余不变
].join('\n');
```

**renderTranslationEvent**：卡片 `status === 'translating'` 时显示蓝点（移除 `hidden`）；`finished`/`failed`/`cancelled` 时隐藏（加 `hidden`）。

```js
function setHeaderDot(card, visible) {
  const dot = card.el.querySelector('.result-header-status');
  if (dot) dot.hidden = !visible;
}
```

- `started`：`setHeaderDot(card, true)`。
- `finished`/`failed`/`cancelled`：`setHeaderDot(card, false)`。
- `delta`：不改动（保持 `started` 设的可见状态）。

**translate.css**（搬原型）：

```css
.result-header-status {
  display: none;
  align-items: center;
  margin-left: auto;
  margin-right: 2px;
}
.result-header-status:not([hidden]) { display: inline-flex; }
.result-header-dot {
  width: 6px; height: 6px;
  border-radius: 50%;
  background: var(--accent);
  flex-shrink: 0;
  animation: pulse-dot 1.2s ease-in-out infinite;
}
@keyframes pulse-dot {
  0%, 100% { opacity: 1; transform: scale(1); }
  50%      { opacity: 0.4; transform: scale(0.85); }
}
```

注：`margin-left: auto` 把蓝点推到右侧（折叠按钮前），与原型一致。`result-engine-name` 已 `flex: 1`，蓝点自然贴右。

### 7.2 流式光标

`setStreamCursor` 已实现（translate.js:274），`started`/`delta` 时 `setStreamCursor(card, true)`，`finished`/`failed`/`cancelled` 时 `false`。**保持不变**。translating 期间光标持续显示，符合需求。

## 8. 数据流

```
弹窗 init
  -> get_session_languages + get_app_config
  -> 渲染语言标签 + 服务卡片
用户改语言 / 交换
  -> set_session_languages -> AppState 更新 -> 弹窗标签更新
翻译（任意入口）
  -> start_translation_from_input
  -> state.session_languages() 取会话语言
  -> build_batch_requests(input, target, source, services, batch_id)
  -> emit translation:event { Started/Delta/Finished/Failed/Cancelled }
  -> translate.js renderTranslationEvent
     -> 卡片头部蓝点（translating 显示）
     -> result-text 流式追加 + stream-cursor
     -> 状态栏 / 取消重试 / token
设置页改默认语言
  -> save_app_config -> app-config:changed
  -> 弹窗只刷新服务卡片，不动会话语言
```

### 8.1 translation:event -> 卡片视觉映射

| 事件 | 头部蓝点 | 流式光标 |
|---|---|---|
| `started` | 显示 | 显示 |
| `delta` | 显示（保持） | 显示 |
| `finished` | 隐藏 | 隐藏 |
| `failed` | 隐藏 | 隐藏 |
| `cancelled` | 隐藏 | 隐藏 |

## 9. 错误处理

| 场景 | 处理 |
|---|---|
| `get_session_languages` 失败 | 弹窗 init 用前端默认 `auto`/`zh-CN`，toast 不打扰（静默降级） |
| `set_session_languages` 失败 | toast 显示 `String(error)`，标签已先更新（乐观更新）；下次翻译用 AppState 旧值 |
| AppState 初始化时 config 读失败 | 会话语言回退 `auto`/`zh-CN` |
| 交换时一方为 `auto` | toast「自动检测不支持交换」，跳过 |
| `start_translation_from_input` 读会话语言失败 | 不应发生（RwLock 不会毒化除非 panic）；若毒化，回退 config |

## 10. 测试

### 后端单测（cargo test）

- `state.rs`：
  - `session_languages_init_from_config`：AppState::new 后会话语言 == config 的 default_source_lang / target_lang。
  - `set_session_languages_updates_state`：set 后 get 返回新值。
  - `set_session_languages_persists_until_reset`：set 后不随 config 变化（会话语言独立于 config）。
- `types.rs`：`normalized_fills_ui_runtime_defaults` 断言 `target_lang == "zh-CN"`。
- 现有 `build_batch_requests` / `TranslationRequest` 测试保持（签名不变）。

### 前端

- `settings.test.ts`：`defaultTargetLang` 默认 `'zh-CN'`；`syncFromBackend` 回读 targetLang。
- `config.test.ts`：`projectToAppConfig` 映射 `targetLang` = `defaultTargetLang`（已是，确认不回归）。
- 翻译弹窗纯静态，无 vitest 单测（与 overlay 一致），靠 `tauri dev` 手动验证。

### 手动验证清单

1. 启动软件，弹窗源=自动检测、目标=简体中文（与设置页默认一致）。
2. 弹窗下拉切目标为 English，翻译一段中文 -> 译成英文。
3. 划词翻译（Alt+D）-> 用弹窗临时设的 English 目标（验证划词也用会话语言）。
4. 关闭弹窗（hide）再唤起 -> 临时语言保留。
5. 退出软件重启 -> 语言重置为设置页默认。
6. 设置页源语言下拉可选「自动检测」；目标语言下拉无「自动检测」。
7. 设置页改默认目标语言 -> 弹窗本次运行不跟变；重启后跟变。
8. 任务栏无翻译弹窗图标；settings 窗口有任务栏图标。
9. 翻译长文本：卡片头部蓝点持续闪动，卡片内光标闪烁；翻译完成后蓝点消失、光标消失。
10. 交换语言按钮：非 auto 时交换；含 auto 时 toast 提示。

## 11. 文档同步（收尾硬门禁）

- spec：本设计文档。
- README.md：翻译弹窗语言联动行为、任务栏图标、卡片状态视觉。
- docs/roadmap/progressive-development-plan.md：标注相关项完成。
- CLAUDE.md / AGENTS.md：架构关键点补「会话语言状态」「main 窗口 skipTaskbar」；前后端通信补 `get_session_languages` / `set_session_languages` command。
- plugins.md：无新插件，不动。

## 12. 风险

- **后端 prompt 收到代码**：`target_lang` 存代码（如 `zh-CN`），prompt 生成「翻译为zh-CN」。模型可理解，但不如「简体中文」友好。缓解：本次接受；后续可加后端映射单独优化。旧 config 里 `target_lang="中文"` 残留时，prompt「翻译为中文」仍正常（弹窗显示也回退原值「中文」）。
- **会话语言与 config 不一致**：设置页改 config 后会话语言不跟变（直到重启）。已在 spec 写明，符合需求 1。手动验证清单第 7 项覆盖。
- **交换让目标变 auto**：采用「含 auto 跳过 + toast」策略，避免 `auto` 目标污染 prompt。
- **RwLock 毒化**：会话语言 RwLock 若 panic 毒化，读返回错误。缓解：读写逻辑简单，不持锁调用复杂代码；毒化时回退 config（`session_languages` 内部 `unwrap_or` 回退）。
- **skipTaskbar 后窗口找不到**：用户可能困惑窗口去向。缓解：托盘菜单 + 快捷键 + 划词唤起；托盘已是驻留模型核心入口。
- **Tauri 2 skipTaskbar 字段**：WindowConfig 支持 `skipTaskbar`（bool）。编译期校验配置。
