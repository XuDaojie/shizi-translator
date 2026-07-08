# 前端任务清单 · 翻译弹窗语言联动与卡片视觉优化

> 主 plan：`docs/superpowers/plans/2026-07-08-translation-popup-language-and-visual.md`（契约段在「后端契约段」）。
> 本文件为前端任务卡切片。前端是**适配层**：把后端标准化数据（语言代码）转换为当前 Tauri UI 视觉表达（显示名、下拉、蓝点）。所有本地化/格式化/视觉由前端完成（§5.5）。

## 任务总览

| task_id | 标题 | owner | depends_on | model_tier |
|---|---|---|---|---|
| FE-1 | settings.ts defaultTargetLang 改 zh-CN + syncFromBackend 回读 targetLang（TDD） | frontend | - | weak |
| FE-2 | TranslatePanel.vue 源语言补 auto + 目标过滤 auto | frontend | - | weak |
| FE-3 | translate.js 语言下拉 + 会话语言接入 + 卡片头部蓝点 + card-sync 清理 | frontend | - | weak |

---

## FE-1：settings.ts defaultTargetLang 改 zh-CN + syncFromBackend 回读 targetLang（TDD）

- **task_id**：FE-1
- **owner**：frontend
- **files_to_write**：
  - `frontend/src/settings/stores/settings.ts`
  - `frontend/src/settings/stores/settings.test.ts`
- **files_to_read**：
  - `frontend/src/settings/stores/settings.ts`（`defaultTargetLang` 默认值 ~line 95、`syncFromBackend` ~line 493-532）
  - `frontend/src/settings/stores/settings.test.ts`（现有测试风格、mock backend payload）
  - `frontend/src/types/config.ts`（AppConfig.targetLang 类型）
  - 主 plan 契约段 C-4、open-questions OQ-4
- **contract_refs**：C-4（target_lang 默认 `zh-CN`，前端 defaultTargetLang 与之同源）
- **depends_on**：-（按契约编码，不依赖后端实现）
- **can_parallel_with**：BE-1、BE-2、BE-3、BE-4、BE-5、FE-2、FE-3
- **model_tier**：weak
- **tier_rationale**：默认弱模型。适配层默认值改值 + 一行回读 + 测试，常规前端。
- **boundary_rationale**：前端 settings store 维护 pre-sync 默认状态（UI 兜底），后端 DEFAULT_TARGET_LANG 维护持久化默认。两处同源手动同步（既有模式，`default_source_lang: 'auto'` 已如此）。`syncFromBackend` 回读是前端把后端标准化数据合并到 UI 状态，属适配层。

### 实现要点

- [ ] **步骤 1：编写失败测试**

在 `settings.test.ts` 末尾加（先读现有测试 imports 与 `useSettings` 用法）。需测两点：①默认 `defaultTargetLang === 'zh-CN'`；②`syncFromBackend` 把 `backend.targetLang` 回读进 `state.translation.defaultTargetLang`。

由于 `syncFromBackend` 依赖 `invokeGetAppConfig`（Tauri invoke），用现有 mock 模式（`vi.mocked(invokeGetAppConfig).mockResolvedValue(...)`）。参考现有「后端非空时按 id 合并」测试（~line 279）的 mock 风格。

```ts
  it('defaultTargetLang 默认为 zh-CN', () => {
    vi.mocked(isTauriReady).mockReturnValue(false);
    const settings = useSettings();
    expect(settings.state.translation.defaultTargetLang).toBe('zh-CN');
  });

  it('syncFromBackend 回读 targetLang 到 defaultTargetLang', async () => {
    vi.mocked(isTauriReady).mockReturnValue(true);
    vi.mocked(invokeGetAppConfig).mockResolvedValue({
      targetLang: 'en-US',
      defaultSourceLang: 'auto',
      autoCopy: true,
      restoreClipboard: true,
      services: [{ id: 'svc-1', serviceType: 'llm', name: 'A', enabled: true, protocol: 'openai_chat', apiKey: 'k', endpoint: 'e', model: 'm', timeoutSeconds: 60, systemPrompt: '', translationPrompt: '', reflectionPrompt: '', reflectionEnabled: false, chainOfThought: 'off' }],
      popupPrecreate: true,
      overlayPrecreate: true,
      collectUsage: true,
      logLevel: 'info',
      shortcuts: {},
    });
    const settings = useSettings();
    await settings.syncFromBackend();
    expect(settings.state.translation.defaultTargetLang).toBe('en-US');
  });
```

> 注：mock payload 的 `services` 非空才会走「后端非空」合并分支（含 targetLang 回读）。执行者按现有测试的 ServiceInstanceConfig 字段补全（参考 settings.test.ts 现有 mock）。

- [ ] **步骤 2：运行测试验证失败**
  - `npm run test`
  - 预期：FAIL（`defaultTargetLang` 当前是 `'中文'`；`syncFromBackend` 未回读 targetLang）。

- [ ] **步骤 3：改 `defaultTargetLang` 默认值**

`settings.ts` ~line 95：

```ts
      defaultSourceLang: 'auto',
      defaultTargetLang: 'zh-CN',
```

- [ ] **步骤 4：`syncFromBackend` 补 targetLang 回读**

`settings.ts` ~line 517-518（`defaultSourceLang` 回读之后）加：

```ts
    state.translation.defaultSourceLang =
      backend.defaultSourceLang ?? state.translation.defaultSourceLang
    state.translation.defaultTargetLang =
      backend.targetLang ?? state.translation.defaultTargetLang
```

- [ ] **步骤 5：运行测试验证通过**
  - `npm run test`
  - 预期：PASS（含新增 2 个测试 + 原有全过）。

> **关于现有 mock payload 的 `targetLang: '中文'`**：settings.test.ts 多处 mock backend 用 `targetLang: '中文'`（line 250/285/352/394/437/483/520）。这些是 mock 后端返回值（测试各分支逻辑），不断言默认。**保留不动**（它们测的是合并逻辑，targetLang 值任意均可行）。若执行者倾向统一可改为 `'zh-CN'`，非强制。

- [ ] **步骤 6：typecheck 验证**
  - `npm run typecheck`
  - 预期：PASS。

- [ ] **步骤 7：Commit**（PM 串行 commit）

### acceptance

- `npm run test` 全绿（含 `defaultTargetLang 默认为 zh-CN` 与 `syncFromBackend 回读 targetLang`）。
- `npm run typecheck` 通过。
- `settings.ts` 的 `defaultTargetLang` 默认 `'zh-CN'`。
- `syncFromBackend` 在后端非空分支回读 `backend.targetLang` 到 `state.translation.defaultTargetLang`。

---

## FE-2：TranslatePanel.vue 源语言补 auto + 目标过滤 auto

- **task_id**：FE-2
- **owner**：frontend
- **files_to_write**：
  - `frontend/src/settings/panels/TranslatePanel.vue`
- **files_to_read**：
  - `frontend/src/settings/panels/TranslatePanel.vue`（当前 source 过滤 auto、target 不过滤）
  - `frontend/src/settings/tokens.ts`（LANGUAGES 定义，含 auto）
  - 主 plan 契约段 C-4
- **contract_refs**：C-4（default_source_lang 含 auto 语义）
- **depends_on**：-
- **can_parallel_with**：BE-1..BE-5、FE-1、FE-3
- **model_tier**：weak
- **tier_rationale**：默认弱模型。两行模板过滤条件对调，机械改动。
- **boundary_rationale**：设置页 UI 选项过滤，纯适配层（LANGUAGES 来自 tokens.ts，含 auto 选项）。后端 config 接受任何字符串，是否在下拉展示 auto 是前端 UI 决策。

### 实现要点

当前 `TranslatePanel.vue` line 25-32：

```vue
      <SettingSelect
        v-model="state.translation.defaultSourceLang"
        :options="languageOptions.filter((l) => l.value !== 'auto')"
      />
    </SettingRow>
    <SettingRow title="默认目标语言" description="最常用的目标语种,可在翻译时临时切换。">
      <SettingSelect v-model="state.translation.defaultTargetLang" :options="languageOptions" />
```

- [ ] **步骤 1：源语言去掉 auto 过滤（补回 auto 选项）**

```vue
      <SettingSelect
        v-model="state.translation.defaultSourceLang"
        :options="languageOptions"
      />
```

- [ ] **步骤 2：目标语言加 auto 过滤**

```vue
      <SettingRow title="默认目标语言" description="最常用的目标语种,可在翻译时临时切换。">
      <SettingSelect
        v-model="state.translation.defaultTargetLang"
        :options="languageOptions.filter((l) => l.value !== 'auto')"
      />
```

> 注：`languageOptions`（line 16）已含 auto（来自 LANGUAGES）。改后源语言下拉含「自动检测」，目标语言下拉不含。

- [ ] **步骤 3：typecheck 验证**
  - `npm run typecheck`
  - 预期：PASS。

- [ ] **步骤 4：Commit**（PM 串行 commit）

### acceptance

- `npm run typecheck` 通过。
- 设置页源语言下拉含「自动检测」选项；目标语言下拉无「自动检测」选项。
- 手动验证（spec §10 第 6 项）：`npm run tauri dev` 设置页观察。

---

## FE-3：translate.js 语言下拉 + 会话语言接入 + 卡片头部蓝点 + card-sync 清理

- **task_id**：FE-3
- **owner**：frontend
- **files_to_write**：
  - `frontend/public/translate.js`
  - `frontend/public/translate.html`
  - `frontend/public/translate.css`
  - `frontend/public/translate-card-sync.js`
- **files_to_read**：
  - `frontend/public/translate.js`（initCards、getCard、renderTranslationEvent、refreshCardsFromConfig、setStreamCursor）
  - `frontend/public/translate.html`（langSource/langTarget/langSwap 元素）
  - `frontend/public/translate.css`（.lang-side、.result-card-header、.stream-cursor）
  - `frontend/public/translate-card-sync.js`（syncServiceCards 写 langSource/langTarget 标签 ~line 35-45）
  - `frontend/src/settings/tokens.ts`（LANGUAGES 同源参考）
  - 高保真原型 `C:\Users\xdj\IdeaProjects\LLM\OpenDesignProjects\shizi\translation-popup.html`（卡片头部蓝点、chevron、pulse-dot 参考）
  - 主 plan 契约段 C-1 / C-2 / C-3 / C-5、实现澄清第 4 条、open-questions OQ-1 / OQ-2
- **contract_refs**：C-1（SessionLanguages JS 视图）、C-2（get_session_languages）、C-3（set_session_languages）、C-5（translation:event -> 蓝点映射）
- **depends_on**：-（按契约编码；手动验证需 BE-3/BE-4 就绪）
- **can_parallel_with**：BE-1..BE-5、FE-1、FE-2
- **model_tier**：weak
- **tier_rationale**：默认弱模型。纯前端适配层：语言代码->显示名映射、下拉 UI、DOM 视觉。无后端契约变更，无跨模块状态机。下拉组件用纯 JS 实现（原型风格），sonnet 可胜任。
- **boundary_rationale**：
  - LANGUAGES 代码↔名称映射归前端（显示名 locale 敏感，§5.5 速查表「Markdown 渲染、相对时间」同类）。
  - 卡片头部蓝点是纯视觉（translation:event 已是 UI 无关契约 C-5，蓝点是其视觉映射），归前端。
  - `translate-card-sync.js` 删语言标签逻辑：原逻辑把 config 的 `defaultSourceLang`/`targetLang` 直接写标签，现改为会话语言驱动（由 translate.js `renderLangLabels`），card-sync 不再触碰语言标签。归前端（两文件都是前端）。
  - 不改后端：会话语言状态、command、translation:event 均后端契约，前端只消费。

### 实现要点

#### 3.1 translate.js：LANGUAGES 映射 + 会话语言状态

- [ ] **步骤 1：顶部加 LANGUAGES 映射**（在 `import` 之后、`const invoke` 之前）

```js
// 语言代码↔名称映射。与 frontend/src/settings/tokens.ts LANGUAGES 同源，
// 新增语言两处同步。translate.js 为纯静态不能 import Vue src，故复制。
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

> 未知代码回退原值（兼容旧 config 残留 `"中文"`：显示「中文」）。

- [ ] **步骤 2：会话语言状态变量**（在 `let pinned = false;` 附近）

```js
let sessionSourceLang = 'auto';
let sessionTargetLang = 'zh-CN';
```

#### 3.2 translate.js：initCards 读会话语言

- [ ] **步骤 3：initCards 并发读 config + 会话语言**

当前 `initCards`（~line 604）：

```js
async function initCards() {
  if (!invoke) return;
  try {
    const config = await invoke('get_app_config');
    if (config?.logLevel) logger.setLevel(config.logLevel);
    refreshCardsFromConfig(config);
  } catch {
    return;
  }
}
```

改为：

```js
async function initCards() {
  if (!invoke) return;
  try {
    const [config, langs] = await Promise.all([
      invoke('get_app_config'),
      invoke('get_session_languages'),
    ]);
    if (config?.logLevel) logger.setLevel(config.logLevel);
    sessionSourceLang = langs?.sourceLang ?? 'auto';
    sessionTargetLang = langs?.targetLang ?? 'zh-CN';
    renderLangLabels();
    refreshCardsFromConfig(config);
  } catch {
    return;
  }
}
```

> `get_session_languages` 失败时 `Promise.all` reject -> catch 静默降级（用默认 `auto`/`zh-CN`，不打扰用户，spec §9）。

#### 3.3 translate.js：renderLangLabels + 下拉 + 交换

- [ ] **步骤 4：renderLangLabels**

```js
function renderLangLabels() {
  langSource.querySelector('.lang-label').textContent = LANG_LABEL(sessionSourceLang);
  langTarget.querySelector('.lang-label').textContent = LANG_LABEL(sessionTargetLang);
}
```

- [ ] **步骤 5：selectLang + 下拉 UI**

实现轻量下拉（纯 JS，原型 chevron 风格）。下拉为动态创建的 `<div class="lang-dropdown">`，绝对定位在 lang-side 下方。点击 lang-side 打开，点击项调用 selectLang，点击外部/Escape 关闭。

```js
let activeDropdown = null;

function closeDropdown() {
  if (activeDropdown) {
    activeDropdown.remove();
    activeDropdown = null;
    document.removeEventListener('mousedown', onDropdownOutsideClick, true);
    document.removeEventListener('keydown', onDropdownEsc, true);
  }
}

function onDropdownOutsideClick(e) {
  if (activeDropdown && !activeDropdown.contains(e.target) && !e.target.closest('.lang-side')) {
    closeDropdown();
  }
}

function onDropdownEsc(e) {
  if (e.key === 'Escape') closeDropdown();
}

function openDropdown(side) {
  closeDropdown();
  const options = side === 'source'
    ? LANGUAGES
    : LANGUAGES.filter((l) => l.value !== 'auto');
  const current = side === 'source' ? sessionSourceLang : sessionTargetLang;
  const dd = document.createElement('div');
  dd.className = 'lang-dropdown';
  options.forEach((opt) => {
    const item = document.createElement('button');
    item.type = 'button';
    item.className = 'lang-dropdown-item' + (opt.value === current ? ' selected' : '');
    item.textContent = opt.label;
    item.addEventListener('click', () => {
      selectLang(side, opt.value);
      closeDropdown();
    });
    dd.appendChild(item);
  });
  const anchor = side === 'source' ? langSource : langTarget;
  anchor.parentElement.appendChild(dd);  // 挂到 .lang-toolbar 以便定位
  // 定位：anchor 下方
  const rect = anchor.getBoundingClientRect();
  const parentRect = anchor.parentElement.getBoundingClientRect();
  dd.style.left = (rect.left - parentRect.left) + 'px';
  dd.style.top = (rect.bottom - parentRect.top) + 'px';
  dd.style.minWidth = rect.width + 'px';
  activeDropdown = dd;
  document.addEventListener('mousedown', onDropdownOutsideClick, true);
  document.addEventListener('keydown', onDropdownEsc, true);
}

async function selectLang(side, code) {
  if (side === 'source') sessionSourceLang = code;
  else sessionTargetLang = code;
  renderLangLabels();
  try {
    await invoke('set_session_languages', { sourceLang: sessionSourceLang, targetLang: sessionTargetLang });
  } catch (e) {
    showToast(String(e));
  }
}
```

> 乐观更新：先改标签再 invoke；invoke 失败 toast，标签已更新（下次翻译用 AppState 旧值，spec §9）。

- [ ] **步骤 6：swapLangs**

```js
async function swapLangs() {
  if (sessionSourceLang === 'auto' || sessionTargetLang === 'auto') {
    showToast('自动检测不支持交换');
    return;
  }
  [sessionSourceLang, sessionTargetLang] = [sessionTargetLang, sessionSourceLang];
  renderLangLabels();
  try {
    await invoke('set_session_languages', { sourceLang: sessionSourceLang, targetLang: sessionTargetLang });
  } catch (e) {
    showToast(String(e));
  }
}
```

- [ ] **步骤 7：绑定 langSource/langTarget/langSwap 事件**（在 `pinBtn.addEventListener` 附近）

```js
langSource.addEventListener('click', () => openDropdown('source'));
langTarget.addEventListener('click', () => openDropdown('target'));
langSwap.addEventListener('click', swapLangs);
```

#### 3.4 translate.js：卡片头部蓝点

- [ ] **步骤 8：getCard 内加 `.result-header-status`**

`getCard` 的 `card.innerHTML` 数组（~line 156-194），在 `result-engine-name` span 之后、`result-collapse-btn` 之前插入：

```js
    '  <span class="result-engine-name">' + displayName + '</span>',
    '  <span class="result-header-status" hidden><span class="result-header-dot"></span></span>',
    '  <button class="result-collapse-btn" title="折叠">',
```

- [ ] **步骤 9：setHeaderDot 辅助函数**（在 `setStreamCursor` 附近）

```js
function setHeaderDot(card, visible) {
  const dot = card.el.querySelector('.result-header-status');
  if (dot) dot.hidden = !visible;
}
```

- [ ] **步骤 10：renderTranslationEvent 接入蓝点**

按 spec §8.1 映射表：
- `started`（isNewBatch 重置块）：`setHeaderDot(c, false)`（重置时隐藏，~line 340-350 重置循环内）
- `started`（单卡分支）：`setHeaderDot(card, true)`（在 `setStreamCursor(card, true)` 之后）
- `delta`：不改动（保持 started 设的可见）
- `finished`：`setHeaderDot(card, false)`（在 `setStreamCursor(card, false)` 附近）
- `failed`：`setHeaderDot(card, false)`
- `cancelled`：`setHeaderDot(card, false)`

具体插入点：
- `case 'started'` 的 `if (isNewBatch) { ... }` 块内，`resultCards.forEach(function (c) { ... })` 循环里加 `setHeaderDot(c, false);`（与 `c.text.textContent = ''` 同级）。
- `case 'started'` 末尾（`setStreamCursor(card, true);` 之后）加 `setHeaderDot(card, true);`
- `case 'finished'`（`setStreamCursor(card, false);` 附近）加 `setHeaderDot(card, false);`
- `case 'failed'`（`setStreamCursor(card, false);` 附近）加 `setHeaderDot(card, false);`
- `case 'cancelled'`（`setStreamCursor(card, false);` 附近）加 `setHeaderDot(card, false);`

#### 3.5 translate.js：refreshCardsFromConfig 清理 langSource/langTarget deps

- [ ] **步骤 11：refreshCardsFromConfig 不再传 langSource/langTarget 给 syncServiceCards**

当前 `refreshCardsFromConfig`（~line 569-595）两处 `syncServiceCards(config, {...})` 调用都传了 `langSource, langTarget`。删除这两处 deps（语言标签改由 `renderLangLabels` 管理，card-sync 不再触碰）。

两处调用改为：

```js
    syncServiceCards(config, {
      resultCards,
      getCard,
      updateCardMeta,
      resultsList,
      allowCreate: false,
      allowRemove: false,
    });
```

与

```js
  syncServiceCards(config, {
    resultCards,
    getCard,
    updateCardMeta,
    resultsList,
  });
```

#### 3.6 translate-card-sync.js：删语言标签逻辑

- [ ] **步骤 12：删 syncServiceCards 末尾的语言标签写入**

`translate-card-sync.js` 当前 line 35-45：

```js
  const sourceLabel = deps.langSource?.querySelector('.lang-label');
  if (sourceLabel) {
    sourceLabel.textContent = !config?.defaultSourceLang || config.defaultSourceLang === 'auto'
      ? '自动检测'
      : config.defaultSourceLang;
  }

  const targetLabel = deps.langTarget?.querySelector('.lang-label');
  if (targetLabel) {
    targetLabel.textContent = config?.targetLang || '中文';
  }
```

整段删除。`deps.langSource`/`deps.langTarget` 不再被读取（调用方 FE-3 步骤 11 已停止传入）。

#### 3.7 translate.html：langSource/langTarget 加 chevron

- [ ] **步骤 13：langSource 加 chevron svg**

`translate.html` ~line 47-49 当前：

```html
        <div class="lang-side" id="langSource">
          <span class="lang-label">自动检测</span>
        </div>
```

改为（加 chevron svg，与原型一致；保持 `<div>` 即可，CSS 补 pointer）：

```html
        <div class="lang-side" id="langSource">
          <span class="lang-label">自动检测</span>
          <svg class="lang-chevron" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>
        </div>
```

- [ ] **步骤 14：langTarget 加 chevron svg**

```html
        <div class="lang-side" id="langTarget">
          <span class="lang-label">简体中文</span>
          <svg class="lang-chevron" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>
        </div>
```

> 不改 `<div class="lang-side">` 为 `<button>`（保持现有结构；CSS 补 pointer + hover 即可点击）。OQ-1 处理。

#### 3.8 translate.css：补 .lang-side 点击样式 + .lang-chevron + 下拉样式 + 蓝点

- [ ] **步骤 15：.lang-side 改 cursor + 加 hover**

`translate.css` ~line 220-238 的 `.lang-side`，`cursor: default` 改 `cursor: pointer`，并加 `:hover`：

```css
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
.lang-side:hover { background: var(--bg-soft); }
.lang-side:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
.lang-side .lang-label { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.lang-side .lang-chevron { width: 10px; height: 10px; color: var(--fg-2); flex-shrink: 0; }
```

> OQ-1：spec 称「CSS 已支持」不准确，此处补齐。

- [ ] **步骤 16：.lang-swap 加 hover**（原型风格，当前缺）

`.lang-swap` ~line 240-250 加 `cursor: pointer` + `:hover`：

```css
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
.lang-swap:hover { background: var(--bg-soft); color: var(--accent); }
.lang-swap:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
.lang-swap svg { width: 12px; height: 12px; }
```

- [ ] **步骤 17：加 .lang-dropdown 样式**（在 `.lang-swap` 之后）

```css
/* === 语言下拉 === */
.lang-dropdown {
  position: absolute;
  z-index: 50;
  background: var(--bg-card);
  border: 0.5px solid var(--border);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-popup);
  padding: 4px;
  max-height: 240px;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--border-2) transparent;
}
.lang-dropdown-item {
  display: block;
  width: 100%;
  text-align: left;
  border: none;
  background: transparent;
  font-family: var(--font-family);
  font-size: 0.75rem;
  color: var(--fg);
  padding: 6px 10px;
  border-radius: 5px;
  cursor: pointer;
  transition: background .12s, color .12s;
}
.lang-dropdown-item:hover { background: var(--bg-soft); color: var(--accent); }
.lang-dropdown-item.selected { color: var(--accent); font-weight: 600; }
.lang-dropdown-item:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
```

> `.lang-toolbar` 需 `position: relative` 以便下拉绝对定位。检查 `.lang-toolbar` 当前是否有 `position`--若无，加 `position: relative;`（~line 210-219）。

- [ ] **步骤 18：加 .result-header-status / .result-header-dot / @keyframes pulse-dot**（搬原型，在 `.result-card-header` 相关样式附近）

```css
/* === 卡片头部翻译中蓝点 === */
.result-header-status {
  display: none;
  align-items: center;
  margin-left: auto;
  margin-right: 2px;
}
.result-header-status:not([hidden]) { display: inline-flex; }
.result-header-dot {
  width: 6px;
  height: 6px;
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

> `margin-left: auto` 把蓝点推到右侧（折叠按钮前）。`result-engine-name` 已 `flex: 1`，蓝点自然贴右。与原型一致。

#### 3.9 验证

- [ ] **步骤 19：构建验证**
  - `npm run build`
  - 预期：BUILD SUCCEEDED（translate.js/html/css 为静态资源不参与 Vite 构建但 build 不报错）。
  - `npm run typecheck`
  - 预期：PASS（translate.js/html/css 不在 typecheck 范围）。

- [ ] **步骤 20：Commit**（PM 串行 commit）

### acceptance

- `npm run build` 通过。
- `npm run typecheck` 通过。
- translate.js 顶部有 `LANGUAGES` 映射与 `LANG_LABEL`，与 `frontend/src/settings/tokens.ts` 同源（注释标明）。
- `initCards` 并发读 `get_app_config` + `get_session_languages`，更新 `sessionSourceLang`/`sessionTargetLang` 并 `renderLangLabels`。
- 点击 `langSource`/`langTarget` 弹出下拉（源含 auto、目标过滤 auto），选择后调 `set_session_languages` 并更新标签。
- `langSwap` 点击：非 auto 交换 + `set_session_languages`；含 auto toast「自动检测不支持交换」并跳过。
- `getCard` 卡片头部含 `<span class="result-header-status" hidden>...`，`started` 显示、`finished`/`failed`/`cancelled` 隐藏（`setHeaderDot`）。
- `translate-card-sync.js` 不再读写 `deps.langSource`/`deps.langTarget`（语言标签逻辑已删）。
- `translate.html` 的 `langSource`/`langTarget` 含 chevron svg。
- `translate.css` 含 `.lang-side:hover`、`.lang-chevron`、`.lang-dropdown*`、`.result-header-status`、`.result-header-dot`、`@keyframes pulse-dot`。
- 手动验证（spec §10 第 1/2/4/9/10 项）：`npm run tauri dev`（需 BE-3/BE-4 就绪）。

### 不做（YAGNI / 范围控制）

- 不改 `.source-meta` 内 `<span class="lang-badge">自动检测</span>`（OQ-2，spec 未提及，保持静态文案）。
- 不改 `sourceBadge`（来自划词/OCR 徽章，与语言无关）。
- 不改 overlay.html、settings 其他模块。
- 不给 translate.js 加 vitest 单测（纯静态，与 overlay 一致，靠手动验证，spec §10）。
- 不把 LANGUAGES 抽到共享文件（translate.js 纯静态不能 import Vue src；tokens.ts 不能被静态页 import。复制 + 注释标明同源是既有模式）。
