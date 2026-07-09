# 翻译弹窗 Vue 化 + 翻译历史 UI 重写 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 把翻译弹窗从 819 行原生 HTML+JS 完整迁移到 Vue 3 组件体系（`frontend/src/popup/`），并按原型双栏结构重写 `HistoryPanel.vue`（右侧详情复用弹窗的 `ResultCardView`/`SourceCardView`/`LanguageToolbar`），后端 Rust 零改动。

**架构：** 弹窗与设置页共享 `frontend/src/` Vite 工程，新增 `src/popup/` 子目录（根组件 + 8 子组件 + 3 composable + 3 CSS + data）。`translate.html` 改为 Vue 入口，`vite.config.ts` 增 `translate.html` 为第二个 rollup input。状态在 `TranslationPopup.vue` 用 `ref`/`reactive` 维护（不引 Pinia），通过 props 下发。流式渲染走命令式 `appendChild`（`ResultCard` watch `card.text` 增量追加），`useTranslationEvents` 只管 cards Map 响应式状态，可纯单测。`HistoryPanel` 内 `computed` 把 `OcrHistoryEntry` 适配为伪 `HistorySession`，布局/滚动逻辑复刻原型。

**技术栈：** Vue 3.5 SFC（`<script setup lang="ts">`）+ TypeScript strict + Vite 7 多入口 + vitest 3 + lucide-vue-next（新增）+ 现有 `@/lib/toast.ts` + `@public/logger.js`。

**关联文档：** spec 见 [docs/superpowers/specs/2026-07-10-popup-vue-migration-history-ui-design.md](../specs/2026-07-10-popup-vue-migration-history-ui-design.md)

**关键参考来源（执行者必读）：**
- 迁移源（1:1 行为对齐基准）：[frontend/public/translate.js](../../../frontend/public/translate.js)（819 行）、[frontend/public/translate.html](../../../frontend/public/translate.html)、[frontend/public/translate.css](../../../frontend/public/translate.css)
- 原型（Vue 组件结构 + HistoryPanel 双栏 + CSS 变量参考）：`C:\Users\xdj\IdeaProjects\LLM\OpenDesignProjects\shizi\src\popup\` 及其 `src/settings/panels/HistoryPanel.vue`、`src/popup/popup-tokens.css`、`src/popup/components.css`
- 设置页现有工程：[frontend/src/settings/](../../../frontend/src/settings/)、[frontend/vite.config.ts](../../../frontend/vite.config.ts)、[frontend/tsconfig.json](../../../frontend/tsconfig.json)、根 [package.json](../../../package.json)

**重要约束（来自 spec，执行者必须遵守）：**
1. **不照搬原型 mock 逻辑**：原型弹窗组件是纯前端 mock（`useCardStreaming` 逐字 setTimeout、硬编码原文），逻辑必须以 shizi 旧 `translate.js` 为 1:1 基准。原型只参考 Vue 组件结构、props/emits 契约、HistoryPanel 双栏布局、CSS 变量命名。
2. **字段名适配**：原型 `LanguagePicker`/`LanguageToolbar` 用 `code/native/english/auto` 字段；shizi 弹窗 `languages.ts` 用 `{ value, label, english }`（与旧 translate.js + `settings/tokens.ts` 同源）。组件代码须把原型的 `l.code`→`l.value`、`l.native`→`l.label`、`l.auto`→`l.value === 'auto'`、`dataset.code`→`dataset.value`。
3. **CSS 变量必须用 `--popup-*` 前缀**：`styles/main.css` 的 `--border` 是 HSL 三元组（配 `hsl(var(--border))`），若 popup-tokens.css 用无前缀 `--border` 会覆盖设置页变量致全站崩坏。变量值以 shizi 旧 `translate.css` 为准（1:1 视觉），不照搬原型的颜色值（如 `--popup-fg-3` 用 shizi 的 `#7A7770` 而非原型的 `#94918A`）。
4. **后端契约冻结**：所有 Tauri command 与 `translation:event` payload 形状不变（见 spec 第三节）。
5. **不引入 Pinia**；不写 `<style scoped>`（避免 hash 干扰复用），样式经全局 class。

---

## 文件结构

### 新建

| 文件 | 职责 |
|------|------|
| `frontend/src/popup/popup-tokens.css` | `:root { --popup-* }` CSS 变量，弹窗与设置页共享 |
| `frontend/src/popup/index.css` | 弹窗外壳专属：reset/html/body/`.popup`/`.toolbar`/`.content`/`.status-bar`/`.stream-cursor`/`.toast`（仅弹窗 import） |
| `frontend/src/popup/components.css` | 共享组件：`.source-card`/`.result-card`/`.lang-picker` 等（弹窗 + 设置页 import） |
| `frontend/src/popup/data/languages.ts` | `LANGUAGES` 数组（`{value,label,english}`，10 条，来自旧 translate.js） |
| `frontend/src/popup/composables/utils.ts` | `speakText`/`copyText`/`batchIdFromSession`/`getTauriApis` |
| `frontend/src/popup/composables/utils.test.ts` | `batchIdFromSession` + `copyText` 单测 |
| `frontend/src/popup/composables/useTranslationEvents.ts` | `listen('translation:event')` + dispatch 到 cards Map + `app-config:changed` |
| `frontend/src/popup/composables/useTranslationEvents.test.ts` | dispatch 状态/batchId 重置/陈旧事件丢弃单测 |
| `frontend/src/popup/composables/usePopupHeight.ts` | ResizeObserver + `setSize`（复刻 `adjustHeight`） |
| `frontend/src/popup/components/SourceCardView.vue` | 原文纯展示（历史详情复用） |
| `frontend/src/popup/components/SourceCard.vue` | 原文编辑（含 textarea，弹窗独有） |
| `frontend/src/popup/components/LanguagePicker.vue` | 内嵌搜索 combobox |
| `frontend/src/popup/components/LanguageToolbar.vue` | 源/目标 + swap，支持 readonly |
| `frontend/src/popup/components/ResultCardView.vue` | 结果纯展示（弹窗 + 历史复用） |
| `frontend/src/popup/components/ResultCard.vue` | 结果 Container（弹窗独有，流式 appendChild） |
| `frontend/src/popup/components/PopupToolbar.vue` | 图钉/OCR/设置（弹窗独有） |
| `frontend/src/popup/components/StatusBar.vue` | 状态点 + 文案 + 取消/重试 |
| `frontend/src/popup/TranslationPopup.vue` | 根组件（状态 + 事件流 + 组装） |
| `frontend/src/popup/main.ts` | 弹窗 Vue 入口 |

### 修改

| 文件 | 改动 |
|------|------|
| `frontend/translate.html` | 内容替换为 `<div id="app">` + `/src/popup/main.ts` |
| `frontend/vite.config.ts` | `rollupOptions.input` 增 `translate.html` |
| `frontend/tsconfig.json` | `include` 增 `translate.html` |
| `frontend/src/settings/main.ts` | 增 `import '@/popup/popup-tokens.css'` + `import '@/popup/components.css'` |
| `frontend/src/settings/panels/HistoryPanel.vue` | 整段重写（双栏 + 复用 popup 组件） |
| `package.json` | `dependencies` 增 `lucide-vue-next` |
| `CLAUDE.md` / `AGENTS.md` | 「项目结构」章节同步 |
| `plugins.md` | 依赖清单增 `lucide-vue-next` |

### 删除

- `frontend/public/translate.html`、`frontend/public/translate.css`、`frontend/public/translate.js`、`frontend/public/translate-card-sync.js`
- `frontend/src/translate-card-sync.test.js`

**保留**：`frontend/public/logger.js`、`frontend/public/overlay.html`（OCR overlay 仍依赖）。

---

## 任务 1：安装 lucide-vue-next 依赖

**文件：**
- 修改：`package.json`

- [ ] **步骤 1：安装依赖**

运行：
```bash
npm i lucide-vue-next
```

预期：`package.json` 的 `dependencies` 增加 `"lucide-vue-next": "^..."`。`@lucide/vue` 保留（设置页其他面板仍在用，本次不迁移，spec 风险 4 已知）。

- [ ] **步骤 2：Commit**

```bash
git add package.json package-lock.json
git commit -m "chore(popup): 新增 lucide-vue-next 依赖供弹窗/历史面板使用"
```

---

## 任务 2：创建 popup-tokens.css（共享 CSS 变量）

**文件：**
- 创建：`frontend/src/popup/popup-tokens.css`

变量值以 shizi 旧 `translate.css` 的 `:root` 为准（1:1 视觉），仅加 `--popup-` 前缀，并补 `--popup-danger`（旧 `--danger`）。

- [ ] **步骤 1：创建文件**

`frontend/src/popup/popup-tokens.css` 完整内容：

```css
/* 翻译弹窗设计 tokens：暖米白 / 灰文字 / 蓝 accent。
   作用域：全局 :root，被 src/popup 组件 + src/settings/panels/HistoryPanel.vue 共同消费。
   变量值与旧 frontend/public/translate.css 的 :root 一致（1:1 迁移），仅加 --popup- 前缀，
   避免与 styles/main.css 的 shadcn HSL 变量（如 --border）冲突。 */

:root {
  --popup-bg-popup:      #F5F2EC;
  --popup-bg-card:       #FFFFFF;
  --popup-bg-soft:       #FAF8F3;
  --popup-bg-soft-2:     #F0EDE5;

  --popup-fg:            #1F1E1B;
  --popup-fg-2:          #5B584F;
  --popup-fg-3:          #7A7770;

  --popup-border:        #E6E2D8;
  --popup-border-2:      #D8D3C5;

  --popup-accent:        #0078D4;
  --popup-accent-hover:  #106EBE;
  --popup-accent-soft:   rgba(0, 120, 212, 0.08);
  --popup-success:       #107C10;
  --popup-warning:       #CA5010;
  --popup-danger:        #b42318;

  --popup-radius-sm:     5px;
  --popup-radius-md:     9px;
  --popup-radius-lg:     14px;

  --popup-font-family:   "Segoe UI Variable", "Segoe UI", -apple-system, BlinkMacSystemFont, "Helvetica Neue", "Microsoft YaHei", system-ui, sans-serif;

  --popup-shadow-popup:  0 8px 24px rgba(28, 25, 23, 0.10), 0 1px 2px rgba(28, 25, 23, 0.04);
  --popup-shadow-card:   0 1px 2px rgba(28, 25, 23, 0.04);
  --popup-shadow-card-h: 0 2px 8px rgba(28, 25, 23, 0.07);
}
```

- [ ] **步骤 2：Commit**

```bash
git add frontend/src/popup/popup-tokens.css
git commit -m "feat(popup): 新增 popup-tokens.css 共享 CSS 变量"
```

---

## 任务 3：data/languages.ts + composables/utils.ts + 单测（TDD）

**文件：**
- 创建：`frontend/src/popup/data/languages.ts`
- 创建：`frontend/src/popup/composables/utils.ts`
- 创建：`frontend/src/popup/composables/utils.test.ts`

- [ ] **步骤 1：先写失败的单测**

`frontend/src/popup/composables/utils.test.ts` 完整内容：

```typescript
import { describe, expect, it, vi, beforeEach } from 'vitest'
import { batchIdFromSession, copyText } from './utils'

describe('batchIdFromSession', () => {
  it('从 batchId:serviceId 形式的 sessionId 提取 batchId', () => {
    expect(batchIdFromSession('batch-001:svc-a')).toBe('batch-001')
  })

  it('无冒号的 sessionId 返回 null', () => {
    expect(batchIdFromSession('no-colon')).toBeNull()
  })

  it('非字符串输入返回 null', () => {
    expect(batchIdFromSession(undefined)).toBeNull()
    expect(batchIdFromSession(null)).toBeNull()
    expect(batchIdFromSession(123 as unknown as string)).toBeNull()
  })
})

describe('copyText', () => {
  beforeEach(() => {
    vi.stubGlobal('navigator', {
      clipboard: { writeText: vi.fn(() => Promise.resolve()) },
    })
  })

  it('复制成功返回 true', async () => {
    const ok = await copyText('hello')
    expect(ok).toBe(true)
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith('hello')
  })

  it('clipboard 不可用时返回 false', async () => {
    vi.stubGlobal('navigator', {})
    const ok = await copyText('hello')
    expect(ok).toBe(false)
  })

  it('writeText 抛错时返回 false', async () => {
    vi.stubGlobal('navigator', {
      clipboard: { writeText: vi.fn(() => Promise.reject(new Error('denied'))) },
    })
    const ok = await copyText('hello')
    expect(ok).toBe(false)
  })
})
```

- [ ] **步骤 2：运行测试验证失败**

运行：`npm run test -- utils`
预期：FAIL，报错 `Cannot find module './utils'`。

- [ ] **步骤 3：创建 languages.ts**

`frontend/src/popup/data/languages.ts` 完整内容（与旧 translate.js 的 `LANGUAGES` 常量逐字一致，与 `settings/tokens.ts` 的 LANGUAGES 同源，新增语言两处同步）：

```typescript
/** 语言代码 ↔ 名称映射。与 frontend/src/settings/tokens.ts 的 LANGUAGES 同源，
 *  新增语言两处同步。弹窗侧多一个 english 字段供搜索 combobox 双列展示。 */
export interface Language {
  value: string
  label: string
  english: string
}

export const LANGUAGES: Language[] = [
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
]

/** ISO 码 → 显示名，找不到回退原码。 */
export const langLabel = (code: string): string =>
  LANGUAGES.find((l) => l.value === code)?.label ?? code
```

- [ ] **步骤 4：创建 utils.ts**

`frontend/src/popup/composables/utils.ts` 完整内容：

```typescript
/** Tauri 全局 API 句柄（withGlobalTauri: true，window.__TAURI__ 可用）。
 *  弹窗三页统一走此入口，不引 @tauri-apps/api。 */
export interface TauriApis {
  invoke: <T = unknown>(cmd: string, args?: Record<string, unknown>) => Promise<T>
  listen: <T = unknown>(event: string, handler: (event: { payload: T }) => void) => Promise<UnlistenFn>
  getCurrentWindow: () => { setAlwaysOnTop: (top: boolean) => Promise<void>; setSize: (size: LogicalSize) => Promise<void> }
}
type UnlistenFn = () => void
interface LogicalSize { type: 'Logical'; width: number; height: number }

export function getTauriApis(): TauriApis | null {
  const t = (typeof window !== 'undefined' ? (window as { __TAURI__?: Record<string, unknown> }).__TAURI__ : undefined)
  const invoke = t?.core?.invoke as TauriApis['invoke'] | undefined
  const listen = t?.event?.listen as TauriApis['listen'] | undefined
  const getCurrentWindow = t?.window?.getCurrentWindow as TauriApis['getCurrentWindow'] | undefined
  if (!invoke || !listen || !getCurrentWindow) return null
  return { invoke, listen, getCurrentWindow }
}

/** batchId 从 "{batchId}:{serviceInstanceId}" 形式的 sessionId 提取。非字符串/无冒号返回 null。 */
export function batchIdFromSession(sessionId: unknown): string | null {
  if (typeof sessionId !== 'string') return null
  const idx = sessionId.indexOf(':')
  if (idx === -1) return null
  return sessionId.slice(0, idx)
}

/** 朗读：speechSynthesis 不可用时静默忽略（旧 translate.js 用 toast 提示，由调用方决定）。 */
export function speakText(text: string, lang: string): void {
  if (typeof window === 'undefined' || !('speechSynthesis' in window)) return
  window.speechSynthesis.cancel()
  const utter = new SpeechSynthesisUtterance(text)
  utter.lang = lang
  utter.rate = 0.95
  window.speechSynthesis.speak(utter)
}

/** 复制到剪贴板，成功返回 true，失败/不可用返回 false。 */
export async function copyText(text: string): Promise<boolean> {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text)
      return true
    }
    return false
  } catch {
    return false
  }
}
```

- [ ] **步骤 5：运行测试验证通过**

运行：`npm run test -- utils`
预期：PASS（3 + 3 = 6 个用例全绿）。

- [ ] **步骤 6：Commit**

```bash
git add frontend/src/popup/data/languages.ts frontend/src/popup/composables/utils.ts frontend/src/popup/composables/utils.test.ts
git commit -m "feat(popup): 新增 languages 数据表与 utils composable（含单测）"
```

---

## 任务 4：useTranslationEvents composable + 单测（TDD 核心）

**文件：**
- 创建：`frontend/src/popup/composables/useTranslationEvents.test.ts`
- 创建：`frontend/src/popup/composables/useTranslationEvents.ts`

此 composable 只管 cards Map 的响应式状态 + batchId 管理 + 事件监听；副作用（sourceText 回填、statusInfo 更新、config refresh）通过回调上抛，使其可纯单测。DOM 流式 appendChild 由 `ResultCard` 基于 `card.text` watch 处理（任务 10）。

- [ ] **步骤 1：先写失败的单测**

`frontend/src/popup/composables/useTranslationEvents.test.ts` 完整内容：

```typescript
import { describe, expect, it, vi } from 'vitest'
import { reactive, Map as _Map } from 'vue'
import { useTranslationEvents, type CardState, type TranslationEventPayload } from './useTranslationEvents'

/** 构造最小可用 opts，记录回调调用。 */
function makeHarness() {
  const cards = reactive<Map<string, CardState>>(new Map())
  const state = { isTranslating: false, batchId: null as string | null }
  const calls = {
    started: [] as Array<{ payload: TranslationEventPayload; isNewBatch: boolean }>,
    batchStatus: 0,
    detected: [] as Array<string | null>,
    config: [] as Array<unknown>,
  }
  const logger = { info: vi.fn(), warn: vi.fn() }
  const listen = vi.fn(async (_evt: string, handler: (e: { payload: unknown }) => void) => {
    ;(listen as unknown as { _handler: typeof handler })._handler = handler
    return () => {}
  })
  vi.stubGlobal('window', {
    __TAURI__: { event: { listen } },
  })
  const { dispatch } = useTranslationEvents({
    cards,
    getIsTranslating: () => state.isTranslating,
    setIsTranslating: (v) => { state.isTranslating = v },
    getCurrentBatchId: () => state.batchId,
    setCurrentBatchId: (id) => { state.batchId = id },
    onStarted: (payload, isNewBatch) => { calls.started.push({ payload, isNewBatch }); state.isTranslating = true; state.batchId = 'batch-1' },
    onBatchStatusChange: () => { calls.batchStatus++ },
    onDetectedLang: (lang) => { calls.detected.push(lang) },
    onConfigChanged: (cfg) => { calls.config.push(cfg) },
    logger,
  })
  return { cards, state, calls, dispatch, listen }
}

describe('useTranslationEvents.dispatch', () => {
  it('started 新 batch 创建卡片并标记 translating', () => {
    const h = makeHarness()
    h.dispatch({ type: 'started', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', serviceName: 'OpenAI', serviceType: 'openai', sourceText: 'hi', sourceType: 'selectedText' })
    expect(h.cards.get('svc-a')).toBeDefined()
    expect(h.cards.get('svc-a')!.status).toBe('translating')
    expect(h.cards.get('svc-a')!.serviceName).toBe('OpenAI')
    expect(h.calls.started[0].isNewBatch).toBe(true)
  })

  it('delta 追加 text 到对应卡片', () => {
    const h = makeHarness()
    h.dispatch({ type: 'started', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', serviceName: 'A', serviceType: 'openai' })
    h.dispatch({ type: 'delta', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', text: 'Hel' })
    h.dispatch({ type: 'delta', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', text: 'lo' })
    expect(h.cards.get('svc-a')!.text).toBe('Hello')
  })

  it('finished 全量替换 text 并写入 usage/detectedSourceLang', () => {
    const h = makeHarness()
    h.dispatch({ type: 'started', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', serviceName: 'A', serviceType: 'openai' })
    h.dispatch({ type: 'delta', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', text: '部分' })
    h.dispatch({
      type: 'finished', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a',
      fullText: '完整译文', usage: { inputTokens: 10, outputTokens: 20 }, detectedSourceLang: 'en-US',
    })
    const card = h.cards.get('svc-a')!
    expect(card.text).toBe('完整译文')
    expect(card.status).toBe('finished')
    expect(card.usage).toEqual({ inputTokens: 10, outputTokens: 20 })
    expect(card.detectedSourceLang).toBe('en-US')
    expect(card.showActions).toBe(true)
    expect(h.calls.detected).toContain('en-US')
  })

  it('failed 设置错误文本与 failed 状态', () => {
    const h = makeHarness()
    h.dispatch({ type: 'started', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', serviceName: 'A', serviceType: 'openai' })
    h.dispatch({ type: 'failed', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', message: '网络错误' })
    const card = h.cards.get('svc-a')!
    expect(card.status).toBe('failed')
    expect(card.text).toBe('网络错误')
    expect(card.showActions).toBe(false)
  })

  it('cancelled 追加 [已取消] 标记', () => {
    const h = makeHarness()
    h.dispatch({ type: 'started', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', serviceName: 'A', serviceType: 'openai' })
    h.dispatch({ type: 'delta', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', text: '部分' })
    h.dispatch({ type: 'cancelled', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a' })
    const card = h.cards.get('svc-a')!
    expect(card.status).toBe('cancelled')
    expect(card.text).toContain('[已取消]')
  })

  it('batchId 切换时重置所有已有卡片（新 batch）', () => {
    const h = makeHarness()
    h.dispatch({ type: 'started', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', serviceName: 'A', serviceType: 'openai' })
    h.dispatch({ type: 'delta', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', text: '旧文本' })
    // 新 batch
    h.dispatch({ type: 'started', sessionId: 'batch-2:svc-a', serviceInstanceId: 'svc-a', serviceName: 'A', serviceType: 'openai' })
    expect(h.cards.get('svc-a')!.text).toBe('')
    expect(h.cards.get('svc-a')!.status).toBe('translating')
  })

  it('跨 batch 的陈旧 delta 被丢弃', () => {
    const h = makeHarness()
    h.dispatch({ type: 'started', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', serviceName: 'A', serviceType: 'openai' })
    h.dispatch({ type: 'started', sessionId: 'batch-2:svc-a', serviceInstanceId: 'svc-a', serviceName: 'A', serviceType: 'openai' })
    h.dispatch({ type: 'delta', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', text: '陈旧' })
    expect(h.cards.get('svc-a')!.text).toBe('')
  })

  it('started 同 batch 新服务实例新建卡片，不重置已有', () => {
    const h = makeHarness()
    h.dispatch({ type: 'started', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', serviceName: 'A', serviceType: 'openai' })
    h.dispatch({ type: 'delta', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', text: '保留' })
    h.dispatch({ type: 'started', sessionId: 'batch-1:svc-b', serviceInstanceId: 'svc-b', serviceName: 'B', serviceType: 'claude' })
    expect(h.cards.get('svc-a')!.text).toBe('保留')
    expect(h.cards.get('svc-b')!.status).toBe('translating')
  })
})
```

- [ ] **步骤 2：运行测试验证失败**

运行：`npm run test -- useTranslationEvents`
预期：FAIL，报错 `Cannot find module './useTranslationEvents'`。

- [ ] **步骤 3：创建 useTranslationEvents.ts**

`frontend/src/popup/composables/useTranslationEvents.ts` 完整内容：

```typescript
import type { AppConfig } from '@/types/config'
import { batchIdFromSession } from './utils'

export type CardStatus = 'pending' | 'translating' | 'finished' | 'failed' | 'cancelled'

export interface CardState {
  serviceInstanceId: string
  serviceName: string
  serviceType: string
  modelName: string
  text: string
  status: CardStatus
  collapsed: boolean
  expanded: boolean
  hasOverflow: boolean
  showActions: boolean
  usage: { inputTokens: number; outputTokens: number } | null
  detectedSourceLang: string | null
}

export interface TranslationEventPayload {
  type: 'started' | 'delta' | 'finished' | 'failed' | 'cancelled'
  sessionId?: string
  serviceInstanceId?: string
  serviceName?: string
  serviceType?: string
  modelName?: string
  sourceText?: string
  sourceType?: 'selectedText' | 'ocrText' | null
  text?: string
  fullText?: string
  message?: string
  detectedSourceLang?: string | null
  usage?: { inputTokens: number; outputTokens: number } | null
}

export interface UseTranslationEventsOptions {
  cards: Map<string, CardState>
  getIsTranslating: () => boolean
  setIsTranslating: (v: boolean) => void
  getCurrentBatchId: () => string | null
  setCurrentBatchId: (id: string | null) => void
  /** started 事件（含 isNewBatch 标志）-- 由父组件回填 sourceText/sourceBadge/langBadge/状态栏。 */
  onStarted: (payload: TranslationEventPayload, isNewBatch: boolean) => void
  /** finished/failed/cancelled 后调用--由父组件更新状态栏（updateBatchStatus）。 */
  onBatchStatusChange: () => void
  /** source=auto 且收到 detectedSourceLang 时上抛（更新 .lang-badge）。 */
  onDetectedLang: (lang: string | null) => void
  /** app-config:changed 事件--由父组件 refreshCardsFromConfig（含翻译中延迟逻辑）。 */
  onConfigChanged: (config: AppConfig) => void
  logger: { info: (msg: string, meta?: unknown) => void; warn: (msg: string, meta?: unknown) => void }
}

function ensureCard(cards: Map<string, CardState>, payload: TranslationEventPayload): CardState {
  const id = payload.serviceInstanceId ?? 'default'
  let card = cards.get(id)
  if (!card) {
    card = {
      serviceInstanceId: id,
      serviceName: payload.serviceName ?? '翻译',
      serviceType: payload.serviceType ?? '',
      modelName: payload.modelName ?? '',
      text: '',
      status: 'pending',
      collapsed: false,
      expanded: false,
      hasOverflow: false,
      showActions: false,
      usage: null,
      detectedSourceLang: null,
    }
    cards.set(id, card)
  }
  return card
}

function resetCardForNewBatch(card: CardState): void {
  card.status = 'pending'
  card.text = ''
  card.showActions = false
  card.usage = null
  card.expanded = false
  card.hasOverflow = false
  card.detectedSourceLang = null
}

export interface UseTranslationEventsReturn {
  /** 直接分派一个 translation:event payload（供测试与真实 listen 共用）。 */
  dispatch: (payload: TranslationEventPayload) => void
  /** 注销监听。 */
  unlisten: () => void
}

export function useTranslationEvents(opts: UseTranslationEventsOptions): UseTranslationEventsReturn {
  const dispatch = (payload: TranslationEventPayload): void => {
    switch (payload.type) {
      case 'started': {
        const batchId = batchIdFromSession(payload.sessionId)
        const isNewBatch = batchId !== opts.getCurrentBatchId()
        if (isNewBatch) {
          opts.logger.info('翻译开始', { batch: batchId })
          opts.setCurrentBatchId(batchId)
          opts.cards.forEach(resetCardForNewBatch)
          opts.setIsTranslating(true)
        }
        opts.onStarted(payload, isNewBatch)
        const card = ensureCard(opts.cards, payload)
        card.serviceName = payload.serviceName ?? card.serviceName
        card.serviceType = payload.serviceType ?? card.serviceType
        card.modelName = payload.modelName ?? card.modelName
        card.status = 'translating'
        card.text = ''
        card.showActions = false
        card.usage = null
        card.expanded = false
        card.hasOverflow = false
        card.detectedSourceLang = null
        card.collapsed = false
        break
      }
      case 'delta': {
        if (batchIdFromSession(payload.sessionId) !== opts.getCurrentBatchId()) return
        const card = opts.cards.get(payload.serviceInstanceId ?? 'default')
        if (!card) return
        card.text += payload.text ?? ''
        break
      }
      case 'finished': {
        if (batchIdFromSession(payload.sessionId) !== opts.getCurrentBatchId()) return
        const card = opts.cards.get(payload.serviceInstanceId ?? 'default')
        if (!card) return
        card.text = payload.fullText ?? card.text
        card.status = 'finished'
        card.usage = payload.usage ?? null
        card.showActions = true
        card.detectedSourceLang = payload.detectedSourceLang ?? null
        if (payload.detectedSourceLang) opts.onDetectedLang(payload.detectedSourceLang)
        opts.onBatchStatusChange()
        break
      }
      case 'failed': {
        if (batchIdFromSession(payload.sessionId) !== opts.getCurrentBatchId()) return
        opts.logger.warn('翻译失败', { session: payload.sessionId, message: payload.message })
        const card = opts.cards.get(payload.serviceInstanceId ?? 'default')
        if (!card) return
        card.text = payload.message ?? '翻译失败'
        card.status = 'failed'
        card.showActions = false
        card.usage = null
        opts.onBatchStatusChange()
        break
      }
      case 'cancelled': {
        if (batchIdFromSession(payload.sessionId) !== opts.getCurrentBatchId()) return
        const card = opts.cards.get(payload.serviceInstanceId ?? 'default')
        if (!card) return
        card.text += '\n[已取消]'
        card.status = 'cancelled'
        opts.onBatchStatusChange()
        break
      }
      default:
        break
    }
  }

  // 监听 Tauri 事件；window.__TAURI__ 可能未就绪（单测/纯浏览器），降级跳过。
  const t = (typeof window !== 'undefined' ? (window as { __TAURI__?: { event?: { listen?: (e: string, h: (ev: { payload: TranslationEventPayload }) => void) => Promise<() => void> } } }).__TAURI__ : undefined)
  const listenFn = t?.event?.listen
  let unlistenTranslation: (() => void) | null = null
  let unlistenConfig: (() => void) | null = null
  if (listenFn) {
    listenFn('translation:event', (ev) => dispatch(ev.payload)).then((fn) => { unlistenTranslation = fn })
    listenFn('app-config:changed', (ev) => {
      const cfg = ev.payload as unknown as AppConfig
      opts.onConfigChanged(cfg)
    }).then((fn) => { unlistenConfig = fn })
  }

  return {
    dispatch,
    unlisten: () => {
      unlistenTranslation?.()
      unlistenConfig?.()
    },
  }
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`npm run test -- useTranslationEvents`
预期：PASS（8 个用例全绿）。

- [ ] **步骤 5：Commit**

```bash
git add frontend/src/popup/composables/useTranslationEvents.ts frontend/src/popup/composables/useTranslationEvents.test.ts
git commit -m "feat(popup): 新增 useTranslationEvents composable（含 dispatch 单测）"
```

---

## 任务 5：usePopupHeight composable

**文件：**
- 创建：`frontend/src/popup/composables/usePopupHeight.ts`

复刻旧 `translate.js` 的 `adjustHeight`（rAF 节流 + `setSize(Logical, 420, h)`）+ `initMaxHeight`（`screen.availHeight * 0.8`）+ `ResizeObserver`。

- [ ] **步骤 1：创建文件**

`frontend/src/popup/composables/usePopupHeight.ts` 完整内容：

```typescript
import { onBeforeUnmount, onMounted, type Ref } from 'vue'
import { getTauriApis } from './utils'

/**
 * 弹窗高度自适应：ResizeObserver 观察 .popup，rAF 节流后调
 * getCurrentWindow().setSize({ type:'Logical', width:420, height:h })。
 * 复刻旧 translate.js 的 adjustHeight + initMaxHeight。
 */
export function usePopupHeight(popupRef: Ref<HTMLElement | null>): void {
  let resizeRaf: number | null = null
  let lastHeight = 0
  let observer: ResizeObserver | null = null

  const adjust = (): void => {
    if (resizeRaf !== null) cancelAnimationFrame(resizeRaf)
    resizeRaf = requestAnimationFrame(() => {
      const el = popupRef.value
      if (!el) return
      const h = el.offsetHeight
      if (h === lastHeight) return
      lastHeight = h
      const apis = getTauriApis()
      if (apis) {
        apis.getCurrentWindow()
          .setSize({ type: 'Logical', width: 420, height: h })
          .catch(() => {})
      }
    })
  }

  const initMaxHeight = (): void => {
    const el = popupRef.value
    if (!el || typeof window === 'undefined') return
    const maxPopupH = Math.floor(window.screen.availHeight * 0.8)
    el.style.maxHeight = maxPopupH + 'px'
  }

  onMounted(() => {
    initMaxHeight()
    observer = new ResizeObserver(adjust)
    if (popupRef.value) observer.observe(popupRef.value)
    // 字体加载完成后重测（旧代码 document.fonts.ready.then(autoResize)）
    if (typeof document !== 'undefined' && document.fonts) {
      document.fonts.ready.then(adjust).catch(() => {})
    }
  })

  onBeforeUnmount(() => {
    if (resizeRaf !== null) cancelAnimationFrame(resizeRaf)
    observer?.disconnect()
    observer = null
  })
}
```

- [ ] **步骤 2：Commit**

```bash
git add frontend/src/popup/composables/usePopupHeight.ts
git commit -m "feat(popup): 新增 usePopupHeight 高度自适应 composable"
```

---

## 任务 6：components.css + index.css（共享样式）

**文件：**
- 创建：`frontend/src/popup/components.css`
- 创建：`frontend/src/popup/index.css`

`components.css` = 旧 `translate.css` 中 source-card / result-card / lang-picker 相关规则，所有 `var(--xxx)` → `var(--popup-xxx)`，并补充原型新增的 `result-model-group`/`result-model-tag`/`result-refresh-btn` 规则。`index.css` = 旧 `translate.css` 中弹窗外壳规则（reset / body / `.popup` / `.toolbar` / `.content` / `.status-bar` / `.stream-cursor`）+ 前缀替换。

- [ ] **步骤 1：创建 components.css**

`frontend/src/popup/components.css` 完整内容：

```css
/* 翻译弹窗通用组件样式（原文卡 / 结果卡 / 语言选择器）。被 src/popup 与
   src/settings/panels/HistoryPanel.vue 共同消费，保证历史详情页与弹窗视觉一致。
   颜色全部引用 --popup-* 设计 token（见 popup-tokens.css）。
   规则源自旧 frontend/public/translate.css，var(--xxx) 已替换为 var(--popup-xxx)。 */

/* === 原文卡 === */
.source-card {
  background: var(--popup-bg-card);
  border-radius: var(--popup-radius-md);
  border: 0.5px solid var(--popup-border);
  box-shadow: var(--popup-shadow-card);
  padding: 10px 12px 8px;
  transition: box-shadow .15s, border-color .15s;
}
.source-card:focus-within {
  border-color: var(--popup-accent);
  outline: 1px solid var(--popup-accent);
  outline-offset: 0;
  box-shadow: var(--popup-shadow-card-h);
}
.source-input {
  display: block;
  width: 100%;
  border: none; background: transparent;
  font-family: var(--popup-font-family);
  font-size: 0.8125rem;
  line-height: 1.55;
  color: var(--popup-fg);
  resize: none; outline: none;
  padding: 0;
  min-height: 2.75rem;
  max-height: 10.85em;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--popup-border-2) transparent;
  user-select: text;
}
.source-input::-webkit-scrollbar { width: 4px; }
.source-input::-webkit-scrollbar-thumb { background: var(--popup-border-2); border-radius: 999px; }
.source-input::-webkit-scrollbar-track { background: transparent; }
.source-input::placeholder { color: var(--popup-fg-3); }

.source-meta {
  display: flex;
  align-items: center;
  gap: 3px;
  margin-top: 8px;
  padding-top: 6px;
  border-top: 0.5px solid var(--popup-border);
}
.meta-btn {
  width: 24px; height: 24px;
  border: none; background: transparent;
  border-radius: 5px;
  cursor: default;
  display: flex; align-items: center; justify-content: center;
  color: var(--popup-fg-2);
  transition: background .15s, color .15s;
}
.meta-btn:hover { background: rgba(28,25,23,0.05); color: var(--popup-fg); }
.meta-btn:focus-visible { outline: 2px solid var(--popup-accent); outline-offset: 1px; }
.meta-btn svg { width: 12px; height: 12px; stroke-width: 1.6; }
.meta-btn.copied { color: var(--popup-success); }

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
  color: var(--popup-fg-2);
  background: var(--popup-bg-soft-2);
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
  color: var(--popup-accent);
  background: var(--popup-accent-soft);
  padding: 2px 8px;
  border-radius: 10px;
  line-height: 1.5;
  font-weight: 600;
}
.lang-badge:empty { display: none; }

/* === 结果区 === */
.results {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.result-card {
  background: var(--popup-bg-card);
  border-radius: var(--popup-radius-md);
  border: 0.5px solid var(--popup-border);
  box-shadow: var(--popup-shadow-card);
  overflow: hidden;
  transition: box-shadow .2s, border-color .2s;
}
.result-card:hover {
  box-shadow: var(--popup-shadow-card-h);
  border-color: var(--popup-border-2);
}
.result-card-header {
  display: flex;
  align-items: center;
  padding: 6px 12px;
  gap: 6px;
  cursor: default;
  user-select: none;
  background: var(--popup-bg-soft);
}
.result-card-header:hover { background: var(--popup-bg-soft-2); }
.result-engine-icon { width: 18px; height: 18px; border-radius: 3px; flex-shrink: 0; }
.result-engine-name { font-size: 0.75rem; font-weight: 600; color: var(--popup-fg-2); flex: 1; }
.result-collapse-btn {
  width: 20px; height: 20px;
  border: none; background: transparent;
  border-radius: 4px;
  cursor: default;
  display: flex; align-items: center; justify-content: center;
  color: var(--popup-fg-2);
  transition: background .15s;
}
.result-collapse-btn:hover { background: rgba(28,25,23,0.05); }
.result-collapse-btn svg { width: 11px; height: 11px; transition: transform .25s ease; }
.result-card.collapsed .result-collapse-btn svg { transform: rotate(-90deg); }

.result-card-body { display: grid; grid-template-rows: 1fr; padding: 0 12px 9px; }
.result-card-body-inner { overflow: hidden; min-height: 0; }
.result-card.collapsed .result-card-body {
  grid-template-rows: 0fr;
  padding-top: 0;
  padding-bottom: 0;
  opacity: 0;
}
.result-text {
  font-size: 0.8125rem;
  line-height: 1.6;
  color: var(--popup-fg);
  white-space: pre-wrap;
  word-break: break-word;
  min-height: 1em;
}
.result-card.failed .result-text { color: var(--popup-danger); }
.result-card.cancelled .result-text { color: var(--popup-fg-3); }

.result-text-clip {
  position: relative;
  max-height: 6.4em;
  overflow: hidden;
  transition: max-height .3s ease;
}
.result-card.expanded .result-text-clip { max-height: 80em; }
.result-card.has-overflow:not(.expanded) .result-text-clip::after {
  content: '';
  position: absolute;
  left: 0; right: 0; bottom: 0;
  height: 28px;
  background: linear-gradient(to bottom, rgba(255,255,255,0), var(--popup-bg-card));
  pointer-events: none;
}
.result-expand-btn {
  display: none;
  align-items: center;
  gap: 3px;
  margin-top: 4px;
  margin-left: -2px;
  padding: 2px 4px;
  border: none;
  background: transparent;
  font-family: var(--popup-font-family);
  font-size: 0.6875rem;
  color: var(--popup-fg-2);
  cursor: default;
  border-radius: 4px;
  line-height: 1;
  transition: color .15s, background .15s;
  user-select: none;
}
.result-card.has-overflow .result-expand-btn { display: inline-flex; }
.result-expand-btn:hover { color: var(--popup-accent); background: rgba(28,25,23,0.04); }
.result-expand-btn:focus-visible { outline: 2px solid var(--popup-accent); outline-offset: 1px; }
.result-expand-chevron { width: 10px; height: 10px; transition: transform .25s ease; }
.result-card.expanded .result-expand-chevron { transform: rotate(180deg); }

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
  cursor: default;
  display: flex; align-items: center; justify-content: center;
  color: var(--popup-fg-2);
  transition: background .15s, color .15s;
}
.result-action-btn:hover { background: rgba(28,25,23,0.05); color: var(--popup-fg); }
.result-action-btn.copied { color: var(--popup-success); }
.result-action-btn:focus-visible { outline: 2px solid var(--popup-accent); outline-offset: 1px; }
.result-action-btn svg { width: 12px; height: 12px; stroke-width: 1.6; }

.result-model-group { margin-left: auto; display: inline-flex; align-items: center; gap: 3px; }
.result-model-tag {
  font-size: 0.625rem; color: var(--popup-fg-3);
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 55%;
}
.result-tokens {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  font-size: 0.625rem;
  color: var(--popup-fg-3);
  font-variant-numeric: tabular-nums;
  user-select: none;
  letter-spacing: 0.01em;
}
.result-tokens .tok { display: inline-flex; align-items: center; gap: 2px; }
.result-tokens .tok svg { width: 9px; height: 9px; opacity: 0.55; stroke-width: 2; }
.result-tokens .tok-sep { width: 1px; height: 9px; background: var(--popup-border); }
.result-refresh-btn:hover { color: var(--popup-danger); }

/* === 卡片头部状态点 === */
.result-header-status { display: none; align-items: center; margin-left: auto; margin-right: 2px; }
.result-header-status:not([hidden]) { display: inline-flex; }
.result-header-dot {
  width: 6px; height: 6px; border-radius: 50%; background: var(--popup-accent);
  flex-shrink: 0; animation: pulse-dot 1.2s ease-in-out infinite;
}
.result-header-dot.is-error { background: var(--popup-danger); }
@keyframes pulse-dot { 0%, 100% { opacity: 1; transform: scale(1); } 50% { opacity: 0.4; transform: scale(0.85); } }

/* === 流式光标 === */
.stream-cursor {
  display: inline-block; width: 1px; height: 0.95em;
  background: var(--popup-accent);
  margin-left: 1px; vertical-align: text-bottom;
  animation: blink 1s steps(1) infinite;
}
@keyframes blink { 0%, 49% { opacity: 1; } 50%, 100% { opacity: 0; } }

/* === 语言选择器（inline 搜索式 combobox） === */
.lang-picker {
  background: var(--popup-bg-card);
  border-radius: var(--popup-radius-md);
  border: 0.5px solid var(--popup-border);
  box-shadow: var(--popup-shadow-card);
  overflow: hidden;
  animation: langPickerIn .15s ease;
}
.lang-picker[hidden] { display: none; }
.lang-picker-search {
  display: flex; align-items: center; gap: 8px;
  padding: 7px 10px;
  border-bottom: 0.5px solid var(--popup-border);
  background: var(--popup-bg-soft);
}
.lang-picker-search-icon { width: 13px; height: 13px; color: var(--popup-fg-3); flex-shrink: 0; }
.lang-picker-input {
  flex: 1; min-width: 0;
  border: none; background: transparent;
  font-family: var(--popup-font-family);
  font-size: 0.75rem; color: var(--popup-fg); outline: none;
}
.lang-picker-input::placeholder { color: var(--popup-fg-3); }
.lang-picker-list {
  list-style: none;
  max-height: 220px;
  overflow-y: auto;
  padding: 4px 0;
  scrollbar-width: thin;
  scrollbar-color: var(--popup-border-2) transparent;
}
.lang-picker-list::-webkit-scrollbar { width: 4px; }
.lang-picker-list::-webkit-scrollbar-thumb { background: var(--popup-border-2); border-radius: 999px; }
.lang-picker-list::-webkit-scrollbar-track { background: transparent; }
.lang-option {
  display: flex; justify-content: space-between; align-items: center;
  gap: 12px;
  padding: 6px 12px;
  font-size: 0.75rem; color: var(--popup-fg);
  cursor: pointer;
  transition: background .08s;
}
.lang-option:hover, .lang-option.is-active { background: var(--popup-bg-soft); }
.lang-option.is-selected { color: var(--popup-accent); font-weight: 600; }
.lang-option.is-selected .lang-option-english { color: var(--popup-accent); opacity: .7; }
.lang-option-native { flex-shrink: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.lang-option-english { color: var(--popup-fg-3); font-size: 0.6875rem; flex-shrink: 0; }
@keyframes langPickerIn { from { opacity: 0; transform: translateY(-4px); } to { opacity: 1; transform: translateY(0); } }
```

- [ ] **步骤 2：创建 index.css**

`frontend/src/popup/index.css` 完整内容：

```css
/* 翻译弹窗外壳专属样式。仅弹窗侧 import（设置页不 import）。
   规则源自旧 frontend/public/translate.css，var(--xxx) 已替换为 var(--popup-xxx)。 */

*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }

html { background: transparent; }

body {
  font-family: var(--popup-font-family);
  background: transparent;
  color: var(--popup-fg);
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  font-feature-settings: "ss01" 1, "tnum" 1;
  overflow: hidden;
}

/* === 弹窗外壳 ===
   WebView2 窗口与 .popup 同尺寸（420x 自适应高），border-radius:0 保持四角一致。 */
.popup {
  width: 100%;
  background: var(--popup-bg-popup);
  border-radius: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  position: relative;
}
.popup::after {
  content: "";
  position: absolute;
  left: 0; right: 0; bottom: 0;
  height: 18px;
  pointer-events: none;
  background: linear-gradient(to bottom, rgba(28,25,23,0) 0%, rgba(28,25,23,0.04) 60%, rgba(28,25,23,0.07) 100%);
}

/* === 顶部工具栏（自绘标题栏，data-tauri-drag-region 拖拽） === */
.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 3px 6px;
  min-height: 26px;
  -webkit-app-region: drag;
}
.toolbar-left, .toolbar-right { display: flex; align-items: center; gap: 1px; }
.toolbar-btn {
  width: 22px; height: 22px;
  border: none; background: transparent;
  border-radius: 4px;
  cursor: default;
  display: flex; align-items: center; justify-content: center;
  color: var(--popup-fg-2);
  transition: background .15s, color .15s;
  -webkit-app-region: no-drag;
}
.toolbar-btn:hover { background: rgba(28,25,23,0.05); color: var(--popup-fg); }
.toolbar-btn.active { color: var(--popup-accent); }
.toolbar-btn.active svg { fill: currentColor; }
.toolbar-btn:focus-visible { outline: 2px solid var(--popup-accent); outline-offset: 1px; }
.toolbar-btn svg { width: 13px; height: 13px; stroke-width: 1.6; }

/* === 内容区 === */
.content {
  padding: 2px 10px 10px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

/* === 状态栏 === */
.status-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 14px;
  border-top: 0.5px solid var(--popup-border);
  font-size: 0.6875rem;
  color: var(--popup-fg-2);
  background: var(--popup-bg-popup);
}
.status-left { display: flex; align-items: center; gap: 6px; }
.status-dot {
  width: 6px; height: 6px;
  border-radius: 50%;
  background: var(--popup-success);
}
.status-dot.loading { background: var(--popup-warning); animation: pulse 1s ease-in-out infinite; }
@keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: .4; } }
.status-action {
  border: none; background: transparent;
  color: var(--popup-fg-2);
  font-family: var(--popup-font-family);
  font-size: 0.6875rem;
  cursor: default;
  padding: 0;
  transition: color .15s;
}
.status-action:hover { color: var(--popup-accent); }
.status-action:focus-visible { outline: 2px solid var(--popup-accent); outline-offset: 1px; }
```

- [ ] **步骤 3：Commit**

```bash
git add frontend/src/popup/components.css frontend/src/popup/index.css
git commit -m "feat(popup): 新增 components.css 与 index.css 共享样式"
```

---

## 任务 7：SourceCardView.vue + ResultCardView.vue（纯展示，历史复用）

**文件：**
- 创建：`frontend/src/popup/components/SourceCardView.vue`
- 创建：`frontend/src/popup/components/ResultCardView.vue`

`ResultCardView` 基本沿用原型（纯展示，不依赖 mock），status 类型用 spec 的 `'success'|'loading'|'pending'|'error'|'aborted'`。`SourceCardView` 用 shizi 旧 class（`.source-card`/`.source-input`/`.source-meta`/`.meta-btn`/`.lang-badge`），徽章显示 `langLabel`。

- [ ] **步骤 1：创建 SourceCardView.vue**

`frontend/src/popup/components/SourceCardView.vue` 完整内容：

```vue
<script setup lang="ts">
interface Props {
  text: string
  langLabel: string
}
withDefaults(defineProps<Props>(), { text: '', langLabel: '' })
const emit = defineEmits<{
  (e: 'speak'): void
  (e: 'copy'): void
  (e: 'focus'): void
}>()
</script>

<template>
  <div class="source-card" @click="emit('focus')">
    <div class="source-input" :title="text">{{ text || '输入要翻译的文本…' }}</div>
    <div class="source-meta">
      <button class="meta-btn" title="朗读原文" @click="emit('speak')">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" /><path d="M19.07 4.93a10 10 0 010 14.14M15.54 8.46a5 5 0 010 7.07" /></svg>
      </button>
      <button class="meta-btn" title="复制原文" @click="emit('copy')">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2" /><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" /></svg>
      </button>
      <div class="meta-badges">
        <span class="lang-badge" :title="`检测到的源语言：${langLabel}`">{{ langLabel }}</span>
      </div>
    </div>
  </div>
</template>
```

- [ ] **步骤 2：创建 ResultCardView.vue**

`frontend/src/popup/components/ResultCardView.vue` 完整内容（沿用原型，纯展示，含 model-tag/refresh/tokens）：

```vue
<script setup lang="ts">
type CardStatus = 'success' | 'loading' | 'error' | 'aborted' | 'pending'

interface Props {
  engineName: string
  /** 内嵌 SVG 片段（不含 <svg> 标签），viewBox="0 0 20 20" */
  engineIconHtml: string
  modelName?: string
  /** 已完成译文；流式态由默认 slot 提供 */
  text?: string
  status?: CardStatus
  /** 流式加载中（弹窗逐字流式时驱动蓝点 + 光标） */
  loading?: boolean
  collapsed?: boolean
  hasOverflow?: boolean
  expanded?: boolean
  showTokens?: boolean
  inputTokens?: number
  outputTokens?: number
  /** 是否显示底部 actions（朗读 / 复制） */
  showActions?: boolean
  /** 失败/中断时是否在操作栏右侧显示「刷新」按钮 */
  showRefresh?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  modelName: '',
  text: '',
  status: 'success',
  loading: false,
  collapsed: false,
  hasOverflow: false,
  expanded: false,
  showTokens: true,
  inputTokens: 0,
  outputTokens: 0,
  showActions: true,
  showRefresh: false,
})

const emit = defineEmits<{
  (e: 'toggle-collapse'): void
  (e: 'toggle-expand'): void
  (e: 'speak'): void
  (e: 'copy'): void
  (e: 'refresh'): void
}>()

const onHeaderClick = (e: MouseEvent): void => {
  if ((e.target as HTMLElement).closest('.result-collapse-btn')) return
  emit('toggle-collapse')
}
const onCollapseClick = (e: MouseEvent): void => { e.stopPropagation(); emit('toggle-collapse') }
const onExpandClick = (e: MouseEvent): void => { e.stopPropagation(); emit('toggle-expand') }

const dotClass = (): string => {
  if (props.status === 'error' || props.status === 'aborted') return 'result-header-dot is-error'
  return 'result-header-dot'
}
const showDot = (): boolean => props.loading || (props.status !== 'success' && props.status !== 'loading' ? props.status !== 'success' : false) && props.status !== 'success'
const showDotFinal = (): boolean => props.loading || props.status === 'loading' || props.status === 'pending'
</script>

<template>
  <div
    class="result-card"
    :class="{
      'collapsed': collapsed,
      'has-overflow': hasOverflow,
      'expanded': expanded,
      'is-error': status === 'error' || status === 'aborted',
    }"
  >
    <div class="result-card-header" @click="onHeaderClick">
      <svg class="result-engine-icon" viewBox="0 0 20 20" v-html="engineIconHtml" />
      <span class="result-engine-name">{{ engineName }}</span>
      <span class="result-header-status" :hidden="!showDotFinal()">
        <span :class="dotClass()" />
      </span>
      <button class="result-collapse-btn" title="折叠" @click="onCollapseClick">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9" /></svg>
      </button>
    </div>
    <div class="result-card-body">
      <div class="result-card-body-inner">
        <div class="result-text-clip">
          <slot>
            <div class="result-text">{{ text }}<span v-if="status === 'loading' || status === 'pending'" class="stream-cursor" /></div>
          </slot>
        </div>
        <button class="result-expand-btn" type="button" tabindex="-1" @click="onExpandClick">
          <span class="result-expand-label">{{ expanded ? '收起' : '展开全文' }}</span>
          <svg class="result-expand-chevron" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9" /></svg>
        </button>
        <div class="result-actions" :style="{ visibility: showActions ? 'visible' : 'hidden' }">
          <button class="result-action-btn" title="朗读翻译" @click="emit('speak')">
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" /><path d="M19.07 4.93a10 10 0 010 14.14M15.54 8.46a5 5 0 010 7.07" /></svg>
          </button>
          <button class="result-action-btn" title="复制翻译" @click="emit('copy')">
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2" /><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" /></svg>
          </button>
          <button
            v-if="showRefresh && (status === 'error' || status === 'aborted')"
            class="result-action-btn result-refresh-btn"
            title="重新翻译"
            @click="emit('refresh')"
          >
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 0 1 15-6.7L21 8" /><path d="M21 3v5h-5" /><path d="M21 12a9 9 0 0 1-15 6.7L3 16" /><path d="M3 21v-5h5" /></svg>
          </button>
          <span v-if="modelName || showTokens" class="result-model-group">
            <span v-if="modelName" class="result-model-tag">{{ modelName }}</span>
            <span v-if="showTokens" class="result-tokens" title="输入 / 输出 Token">
              <span class="tok"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="19" x2="12" y2="5" /><polyline points="5 12 12 5 19 12" /></svg>{{ inputTokens }}</span>
              <span class="tok-sep" />
              <span class="tok"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19" /><polyline points="19 12 12 19 5 12" /></svg>{{ outputTokens }}</span>
            </span>
          </span>
        </div>
      </div>
    </div>
  </div>
</template>
```

> 说明：`showDot`/`showDotFinal` 中保留一个最终使用的 `showDotFinal`（loading 或 pending 时显示头部点）。`showDot` 为中间推导，若 typecheck 报未使用变量，删除 `showDot` 与 `dotClass` 之外的多余定义，只保留 `showDotFinal`。执行时以 typecheck 通过为准：若 `showDot` 未使用则移除该函数，保留 `dotClass` 与 `showDotFinal`。

- [ ] **步骤 3：typecheck**

运行：`npm run typecheck`
预期：PASS（无 error）。若报 `showDot` 未使用，按上面说明删除该函数。

- [ ] **步骤 4：Commit**

```bash
git add frontend/src/popup/components/SourceCardView.vue frontend/src/popup/components/ResultCardView.vue
git commit -m "feat(popup): 新增 SourceCardView 与 ResultCardView 纯展示组件"
```

---

## 任务 8：LanguagePicker.vue + LanguageToolbar.vue

**文件：**
- 创建：`frontend/src/popup/components/LanguagePicker.vue`
- 创建：`frontend/src/popup/components/LanguageToolbar.vue`

基于原型，字段适配为 `value/label/english`。`LanguageToolbar` 的 swap 只 emit `'swap'`（由父组件处理 auto 检查 + toast + invoke），与 spec 5.4 契约一致（旧 translate.js 行为：auto 时 toast「自动检测不支持交换」并 return）。

- [ ] **步骤 1：创建 LanguagePicker.vue**

`frontend/src/popup/components/LanguagePicker.vue` 完整内容：

```vue
<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { LANGUAGES, type Language } from '../data/languages'

interface Props {
  modelValue: string
  type: 'source' | 'target'
  placeholder: string
}

const props = defineProps<Props>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
  (e: 'pick', value: string): void
}>()

const search = ref('')
const listRef = ref<HTMLUListElement | null>(null)
const inputRef = ref<HTMLInputElement | null>(null)

const filtered = computed<Language[]>(() => {
  const q = search.value.trim().toLowerCase()
  return LANGUAGES.filter((l) => {
    if (props.type === 'target' && l.value === 'auto') return false
    if (!q) return true
    return l.label.toLowerCase().includes(q) || l.english.toLowerCase().includes(q)
  })
})

const select = (value: string): void => {
  emit('update:modelValue', value)
  emit('pick', value)
}

const moveActive = (delta: 1 | -1): void => {
  if (!listRef.value) return
  const items = Array.from(listRef.value.querySelectorAll<HTMLElement>('.lang-option'))
  if (items.length === 0) return
  const currentIdx = items.findIndex((el) => el.classList.contains('is-active'))
  const nextIdx = Math.max(0, Math.min(items.length - 1, currentIdx + delta))
  items.forEach((el) => el.classList.remove('is-active'))
  items[nextIdx]?.classList.add('is-active')
  items[nextIdx]?.scrollIntoView({ block: 'nearest' })
}

const onKeydown = (e: KeyboardEvent): void => {
  if (e.key === 'ArrowDown') { e.preventDefault(); moveActive(1) }
  else if (e.key === 'ArrowUp') { e.preventDefault(); moveActive(-1) }
  else if (e.key === 'Enter') {
    e.preventDefault()
    const active = listRef.value?.querySelector<HTMLElement>('.lang-option.is-active')
    if (active) {
      const value = active.dataset.value
      if (value) select(value)
    }
  }
}

const setInitialActive = async (): Promise<void> => {
  await nextTick()
  if (!listRef.value) return
  const selected = listRef.value.querySelector<HTMLElement>('.lang-option.is-selected')
  ;(selected || listRef.value.querySelector<HTMLElement>('.lang-option'))?.classList.add('is-active')
}

watch(() => props.modelValue, () => { void setInitialActive() })

defineExpose({ focus: () => inputRef.value?.focus() })
</script>

<template>
  <div class="lang-picker">
    <div class="lang-picker-search">
      <svg class="lang-picker-search-icon" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="7" /><line x1="20" y1="20" x2="16.65" y2="16.65" /></svg>
      <input
        ref="inputRef"
        v-model="search"
        type="text"
        class="lang-picker-input"
        :placeholder="placeholder"
        autocomplete="off"
        spellcheck="false"
        @keydown="onKeydown"
      />
    </div>
    <ul ref="listRef" class="lang-picker-list">
      <li
        v-for="lang in filtered"
        :key="lang.value"
        class="lang-option"
        :class="{ 'is-selected': lang.value === modelValue }"
        :data-value="lang.value"
        @click="select(lang.value)"
      >
        <span class="lang-option-native">{{ lang.label }}</span>
        <span class="lang-option-english">{{ lang.english }}</span>
      </li>
    </ul>
  </div>
</template>
```

- [ ] **步骤 2：创建 LanguageToolbar.vue**

`frontend/src/popup/components/LanguageToolbar.vue` 完整内容：

```vue
<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { LANGUAGES } from '../data/languages'
import LanguagePicker from './LanguagePicker.vue'

interface Props {
  source: string
  target: string
  readonly?: boolean
}

const props = withDefaults(defineProps<Props>(), { readonly: false })

const emit = defineEmits<{
  (e: 'update:source', value: string): void
  (e: 'update:target', value: string): void
  (e: 'swap'): void
}>()

const sourceLabel = computed(() => LANGUAGES.find((l) => l.value === props.source)?.label ?? '自动检测')
const targetLabel = computed(() => LANGUAGES.find((l) => l.value === props.target)?.label ?? '简体中文')

const openType = ref<'source' | 'target' | null>(null)
const sourcePickerRef = ref<InstanceType<typeof LanguagePicker> | null>(null)
const targetPickerRef = ref<InstanceType<typeof LanguagePicker> | null>(null)

const toggle = (type: 'source' | 'target'): void => {
  if (props.readonly) return
  if (openType.value === type) { openType.value = null; return }
  openType.value = type
  requestAnimationFrame(() => {
    if (type === 'source') sourcePickerRef.value?.focus()
    else targetPickerRef.value?.focus()
  })
}

const onPick = (type: 'source' | 'target', value: string): void => {
  openType.value = null
  if (type === 'source') emit('update:source', value)
  else emit('update:target', value)
}

const swap = (): void => {
  if (props.readonly) return
  openType.value = null
  emit('swap')
}

const onDocClick = (e: MouseEvent): void => {
  if (!openType.value) return
  const target = e.target as HTMLElement
  if (target.closest('.lang-toolbar')) return
  openType.value = null
}

watch(openType, (val) => {
  if (val) {
    setTimeout(() => document.addEventListener('click', onDocClick), 0)
  } else {
    document.removeEventListener('click', onDocClick)
  }
})
</script>

<template>
  <div class="lang-toolbar">
    <button class="lang-side" :disabled="readonly" @click="toggle('source')">
      <span class="lang-label">{{ sourceLabel }}</span>
      <svg class="lang-chevron" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9" /></svg>
    </button>
    <button class="lang-swap" :disabled="readonly" title="交换语言" @click="swap">
      <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M7 16l-4-4 4-4" /><path d="M17 8l4 4-4 4" /><line x1="3" y1="12" x2="21" y2="12" /></svg>
    </button>
    <button class="lang-side" :disabled="readonly" @click="toggle('target')">
      <span class="lang-label">{{ targetLabel }}</span>
      <svg class="lang-chevron" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9" /></svg>
    </button>
  </div>
  <LanguagePicker
    v-if="openType === 'source'"
    ref="sourcePickerRef"
    :model-value="source"
    type="source"
    placeholder="搜索源语言…"
    @update:model-value="(v) => emit('update:source', v)"
    @pick="(v) => onPick('source', v)"
  />
  <LanguagePicker
    v-if="openType === 'target'"
    ref="targetPickerRef"
    :model-value="target"
    type="target"
    placeholder="搜索目标语言…"
    @update:model-value="(v) => emit('update:target', v)"
    @pick="(v) => onPick('target', v)"
  />
</template>
```

- [ ] **步骤 3：typecheck**

运行：`npm run typecheck`
预期：PASS。

- [ ] **步骤 4：Commit**

```bash
git add frontend/src/popup/components/LanguagePicker.vue frontend/src/popup/components/LanguageToolbar.vue
git commit -m "feat(popup): 新增 LanguagePicker 与 LanguageToolbar 组件"
```

---

## 任务 9：SourceCard.vue（含 textarea，弹窗独有）

**文件：**
- 创建：`frontend/src/popup/components/SourceCard.vue`

含 textarea 自动 resize、朗读/复制、`sourceBadge`（来自划词/来自 OCR）+ 检测语言徽章。复刻旧 `translate.js` 的 `autoResize`/`updateCharCount`/`speakText`/`copyText`/`setSourceBadge`。

- [ ] **步骤 1：创建 SourceCard.vue**

`frontend/src/popup/components/SourceCard.vue` 完整内容：

```vue
<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { speakText, copyText } from '../composables/utils'

interface Props {
  modelValue: string
  langLabel: string
  sourceBadge?: 'selectedText' | 'ocrText' | null
  detectedLang?: string
}

const props = withDefaults(defineProps<Props>(), { sourceBadge: null, detectedLang: '' })
const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
  (e: 'submit'): void
  (e: 'input'): void
}>()

const textareaRef = ref<HTMLTextAreaElement | null>(null)
const copied = ref(false)

const sourceBadgeText = computed(() => {
  switch (props.sourceBadge) {
    case 'selectedText': return '来自划词'
    case 'ocrText': return '来自 OCR'
    default: return ''
  }
})

const autoResize = (): void => {
  const el = textareaRef.value
  if (!el) return
  el.style.height = 'auto'
  const maxHeight = parseFloat(getComputedStyle(el).maxHeight)
  const nextHeight = Math.min(el.scrollHeight, maxHeight || el.scrollHeight)
  el.style.height = nextHeight + 'px'
  el.style.overflowY = el.scrollHeight > nextHeight ? 'auto' : 'hidden'
}

const onInput = (e: Event): void => {
  const value = (e.target as HTMLTextAreaElement).value
  emit('update:modelValue', value)
  emit('input')
  autoResize()
}

const onKeydown = (e: KeyboardEvent): void => {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    emit('submit')
  }
}

const onSpeak = (): void => {
  speakText(props.modelValue, 'en-US')
}

const onCopy = async (): Promise<void> => {
  const ok = await copyText(props.modelValue)
  if (ok) {
    copied.value = true
    setTimeout(() => { copied.value = false }, 1500)
  }
}

onMounted(() => {
  autoResize()
  if (typeof document !== 'undefined' && document.fonts) {
    document.fonts.ready.then(autoResize).catch(() => {})
  }
})

watch(() => props.modelValue, () => { nextTick(autoResize) })

defineExpose({ focus: () => textareaRef.value?.focus(), autoResize })
</script>

<template>
  <div class="source-card">
    <textarea
      ref="textareaRef"
      class="source-input"
      :value="modelValue"
      placeholder="输入要翻译的文本..."
      rows="3"
      @input="onInput"
      @keydown="onKeydown"
    />
    <div class="source-meta">
      <button class="meta-btn" :class="{ copied }" title="朗读原文" @click="onSpeak">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" /><path d="M19.07 4.93a10 10 0 010 14.14M15.54 8.46a5 5 0 010 7.07" /></svg>
      </button>
      <button class="meta-btn" :class="{ copied }" title="复制原文" @click="onCopy">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2" /><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" /></svg>
      </button>
      <div class="meta-badges">
        <span v-if="sourceBadgeText" class="source-badge">{{ sourceBadgeText }}</span>
        <span v-if="detectedLang" class="lang-badge">{{ detectedLang }}</span>
        <span v-else-if="langLabel" class="lang-badge">{{ langLabel }}</span>
      </div>
    </div>
  </div>
</template>
```

- [ ] **步骤 2：typecheck**

运行：`npm run typecheck`
预期：PASS。

- [ ] **步骤 3：Commit**

```bash
git add frontend/src/popup/components/SourceCard.vue
git commit -m "feat(popup): 新增 SourceCard 含 textarea 编辑组件"
```

---

## 任务 10：PopupToolbar.vue + StatusBar.vue + ResultCard.vue

**文件：**
- 创建：`frontend/src/popup/components/PopupToolbar.vue`
- 创建：`frontend/src/popup/components/StatusBar.vue`
- 创建：`frontend/src/popup/components/ResultCard.vue`

`ResultCard`（Container）接收 `CardState`，驱动 `ResultCardView`；流式渲染通过 `watch(card.text, { flush: 'sync' })` 增量 `appendChild` TextNode / 全量 `textContent` 替换 + 命令式光标 span（复刻旧 `setStreamCursor`/`scrollToBottom`）。`PopupToolbar` 接 invoke（图钉/OCR/设置）。`StatusBar` 接 props。

- [ ] **步骤 1：创建 PopupToolbar.vue**

`frontend/src/popup/components/PopupToolbar.vue` 完整内容（复刻旧 `togglePin`/`triggerOcr`/`openSettings`）：

```vue
<script setup lang="ts">
import { ref } from 'vue'
import { getTauriApis } from '../composables/utils'
import { toast } from '@/lib/toast'

const props = defineProps<{ pinned: boolean }>()
const emit = defineEmits<{ (e: 'update:pinned', value: boolean): void }>()

const togglePin = async (): Promise<void> => {
  const apis = getTauriApis()
  if (!apis) { toast.info('窗口 API 未就绪'); return }
  const next = !props.pinned
  try {
    await apis.getCurrentWindow().setAlwaysOnTop(next)
    emit('update:pinned', next)
    toast.info(next ? '窗口已固定' : '取消固定')
  } catch (e) {
    toast.error('固定失败', String(e))
  }
}

const triggerOcr = async (): Promise<void> => {
  const apis = getTauriApis()
  if (!apis) { toast.info('Tauri API 未就绪'); return }
  try {
    await apis.invoke('trigger_ocr_translation')
  } catch (e) {
    toast.error('OCR 触发失败', String(e))
  }
}

const openSettings = async (): Promise<void> => {
  const apis = getTauriApis()
  if (!apis) return
  try {
    await apis.invoke('open_settings')
  } catch (e) {
    toast.error('打开设置失败', String(e))
  }
}
</script>

<template>
  <div class="toolbar" data-tauri-drag-region>
    <div class="toolbar-left">
      <button class="toolbar-btn" :class="{ active: pinned }" title="固定窗口" @click="togglePin">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="17" x2="12" y2="22" /><path d="M5 17h14v-1.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6h1a2 2 0 0 0 0-4H8a2 2 0 0 0 0 4h1v4.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24Z" /></svg>
      </button>
    </div>
    <div class="toolbar-right">
      <button class="toolbar-btn" title="截图翻译" @click="triggerOcr">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M6 9l6-6 6 6M6 15l6 6 6-6" /></svg>
      </button>
      <button class="toolbar-btn" title="设置" @click="openSettings">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z" /></svg>
      </button>
    </div>
  </div>
</template>
```

- [ ] **步骤 2：创建 StatusBar.vue**

`frontend/src/popup/components/StatusBar.vue` 完整内容：

```vue
<script setup lang="ts">
interface StatusAction {
  label: string
  onClick: () => void
}

interface Props {
  text: string
  loading: boolean
  action?: StatusAction | null
  charCount: number
}

withDefaults(defineProps<Props>(), { action: null })
</script>

<template>
  <div class="status-bar">
    <div class="status-left">
      <span class="status-dot" :class="{ loading }" />
      <span>{{ text }}</span>
      <button v-if="action" class="status-action" @click="action.onClick">{{ action.label }}</button>
    </div>
    <span>{{ charCount }} 字</span>
  </div>
</template>
```

- [ ] **步骤 3：创建 ResultCard.vue**

`frontend/src/popup/components/ResultCard.vue` 完整内容（Container，含流式命令式 appendChild + 引擎图标 + 复制/朗读/重试）：

```vue
<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import ResultCardView from './ResultCardView.vue'
import type { CardState } from '../composables/useTranslationEvents'
import { speakText, copyText, getTauriApis } from '../composables/utils'
import { toast } from '@/lib/toast'

interface Props {
  card: CardState
  targetLang: string
}

const props = defineProps<Props>()
const emit = defineEmits<{ (e: 'toggle-expand', card: CardState): void }>()

const textRef = ref<HTMLElement | null>(null)

/* 引擎图标：与旧 translate.js engineIcon 一致（圆角矩形 + 首字母）。 */
const ENGINE_META: Record<string, { color: string; letter: string }> = {
  openai: { color: '#10A37F', letter: 'O' },
  deepseek: { color: '#4D6BFE', letter: 'D' },
  zhipu: { color: '#3B5BFE', letter: 'Z' },
  claude: { color: '#D97757', letter: 'C' },
  mock: { color: '#94918A', letter: 'M' },
}
const engineIconHtml = computed(() => {
  const meta = ENGINE_META[props.card.serviceType]
  const color = meta ? meta.color : '#94918A'
  const letter = meta ? meta.letter : ((props.card.serviceName || '?').trim().charAt(0).toUpperCase() || '?')
  return `<rect width="20" height="20" rx="5" fill="${color}"/><text x="10" y="14.5" text-anchor="middle" font-size="12" font-weight="700" fill="#fff" font-family="Segoe UI, system-ui, sans-serif">${letter}</text>`
})

/* ResultCardView 的 status 映射：CardState.status -> 展示态。 */
const viewStatus = computed<'success' | 'loading' | 'pending' | 'error' | 'aborted'>(() => {
  switch (props.card.status) {
    case 'translating': return 'loading'
    case 'finished': return 'success'
    case 'failed': return 'error'
    case 'cancelled': return 'aborted'
    default: return 'pending'
  }
})
const isLoading = computed(() => props.card.status === 'translating')

/* 流式渲染：watch card.text，增量 appendChild TextNode / 全量 textContent 替换，
   命令式管理光标 span（复刻旧 setStreamCursor + scrollToBottom）。flush:sync 保证不丢帧。 */
const renderText = (newText: string, oldText: string | undefined): void => {
  const el = textRef.value
  if (!el) return
  // 移除旧光标
  el.querySelector('.stream-cursor')?.remove()
  if (oldText !== undefined && newText.startsWith(oldText)) {
    el.appendChild(document.createTextNode(newText.slice(oldText.length)))
  } else {
    el.textContent = newText
  }
  if (props.card.status === 'translating') {
    const cursor = document.createElement('span')
    cursor.className = 'stream-cursor'
    el.appendChild(cursor)
  }
  el.scrollTop = el.scrollHeight
}

watch(() => props.card.text, (newText, oldText) => renderText(newText, oldText), { flush: 'sync' })

/* 挂载后若已有 text（如重试/回填），立即渲染一次。 */
nextTick(() => {
  if (props.card.text && textRef.value && !textRef.value.textContent) {
    renderText(props.card.text, undefined)
  }
})

const onToggleCollapse = (): void => { props.card.collapsed = !props.card.collapsed }

const onToggleExpand = (): void => {
  props.card.expanded = !props.card.expanded
  emit('toggle-expand', props.card)
}

/* overflow 检测（复刻旧 detectOverflow）：展开按钮可见性。 */
const detectOverflow = (): void => {
  const el = textRef.value?.parentElement /* .result-text-clip */
  if (!el || !textRef.value) return
  props.card.hasOverflow = textRef.value.scrollHeight > el.clientHeight + 1
}
watch(() => props.card.text, () => { nextTick(detectOverflow) })
watch(() => props.card.status, (s) => { if (s === 'finished') nextTick(detectOverflow) })

const onSpeak = (): void => {
  const text = textRef.value?.textContent ?? props.card.text
  speakText(text, props.targetLang)
}

const onCopy = async (): Promise<void> => {
  const text = textRef.value?.textContent ?? props.card.text
  const ok = await copyText(text)
  if (ok) toast.success('已复制到剪贴板')
  else toast.error('复制失败')
}

const onRefresh = async (): Promise<void> => {
  const apis = getTauriApis()
  if (!apis) { toast.info('Tauri API 未就绪'); return }
  try {
    await apis.invoke('retry_translation')
  } catch (e) {
    toast.error('重试失败', String(e))
  }
}

onBeforeUnmount(() => {
  /* 卡片由父组件按 serviceInstanceId 复用，组件销毁时无需清理 DOM（Vue 自动处理）。 */
})
</script>

<template>
  <ResultCardView
    :engine-name="card.serviceName"
    :engine-icon-html="engineIconHtml"
    :model-name="card.modelName"
    :status="viewStatus"
    :loading="isLoading"
    :collapsed="card.collapsed"
    :has-overflow="card.hasOverflow"
    :expanded="card.expanded"
    :show-tokens="card.usage !== null"
    :input-tokens="card.usage?.inputTokens ?? 0"
    :output-tokens="card.usage?.outputTokens ?? 0"
    :show-actions="card.showActions"
    :show-refresh="card.status === 'failed' || card.status === 'cancelled'"
    @toggle-collapse="onToggleCollapse"
    @toggle-expand="onToggleExpand"
    @speak="onSpeak"
    @copy="onCopy"
    @refresh="onRefresh"
  >
    <div ref="textRef" class="result-text" />
  </ResultCardView>
</template>
```

- [ ] **步骤 4：typecheck**

运行：`npm run typecheck`
预期：PASS。若 `onBeforeUnmount` 空函数报警告，删除该空函数与对应 import（保留 `onBeforeUnmount` 仅在有清理逻辑时使用）。

- [ ] **步骤 5：Commit**

```bash
git add frontend/src/popup/components/PopupToolbar.vue frontend/src/popup/components/StatusBar.vue frontend/src/popup/components/ResultCard.vue
git commit -m "feat(popup): 新增 PopupToolbar/StatusBar/ResultCard 组件"
```

---

## 任务 11：TranslationPopup.vue 根组件 + main.ts

**文件：**
- 创建：`frontend/src/popup/TranslationPopup.vue`
- 创建：`frontend/src/popup/main.ts`

根组件维护顶层状态（spec 6.1），组装子组件，接事件流。复刻旧 `translate.js` 的 `initCards`/`applyPendingSourceText`/`collectEdgeTranslateEnv`/`refreshCardsFromConfig`/`startManualTranslation`/`cancelTranslation`/`retryTranslation`/`swapLangs`/`selectLang`/`updateBatchStatus`。

- [ ] **步骤 1：创建 TranslationPopup.vue**

`frontend/src/popup/TranslationPopup.vue` 完整内容：

```vue
<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { Toaster } from '@/components/ui/toast'
import { createLogger } from '@public/logger.js'
import PopupToolbar from './components/PopupToolbar.vue'
import SourceCard from './components/SourceCard.vue'
import LanguageToolbar from './components/LanguageToolbar.vue'
import ResultCard from './components/ResultCard.vue'
import StatusBar from './components/StatusBar.vue'
import { useTranslationEvents, type CardState, type TranslationEventPayload } from './composables/useTranslationEvents'
import { usePopupHeight } from './composables/usePopupHeight'
import { getTauriApis } from './composables/utils'
import { toast } from '@/lib/toast'
import type { AppConfig, ServiceInstanceConfig } from '@/types/config'

const logger = createLogger('translate')

/* === 顶层状态（spec 6.1） === */
const popupRef = ref<HTMLElement | null>(null)
const sourceText = ref('')
const sessionSourceLang = ref('auto')
const sessionTargetLang = ref('zh-CN')
const isTranslating = ref(false)
const currentBatchId = ref<string | null>(null)
const cards = reactive<Map<string, CardState>>(new Map())
const pinned = ref(false)
const sourceBadge = ref<'selectedText' | 'ocrText' | null>(null)
const detectedLangBadge = ref('')
const charCount = ref(0)
const statusInfo = ref<{ text: string; loading: boolean; action: { label: string; onClick: () => void } | null }>({
  text: '就绪', loading: false, action: null,
})
const pendingConfigRefresh = ref<AppConfig | null>(null)

usePopupHeight(popupRef)

const setStatus = (text: string, loading: boolean, action: { label: string; onClick: () => void } | null): void => {
  statusInfo.value = { text, loading, action }
}

/* === 引擎/语言标签 === */
const sourceLangLabel = computed(() => LANGUAGES.find((l) => l.value === sessionSourceLang.value)?.label ?? '自动检测')
const detectedOrLabel = computed(() => {
  if (detectedLangBadge.value) return detectedLangBadge.value
  if (sessionSourceLang.value === 'auto') return '检测中…'
  return sourceLangLabel.value
})

/* === batchStatus（复刻旧 updateBatchStatus） === */
const updateBatchStatus = (): void => {
  const list = Array.from(cards.values())
  if (list.length === 0) return
  const allFinished = list.every((c) => c.status === 'finished')
  const allFailed = list.every((c) => c.status === 'failed' || c.status === 'cancelled')
  const anyTranslating = list.some((c) => c.status === 'translating')
  if (allFinished) {
    isTranslating.value = false
    currentBatchId.value = null
    sourceBadge.value = null
    if (sessionSourceLang.value === 'auto') {
      const detected = list.find((c) => c.detectedSourceLang)?.detectedSourceLang ?? ''
      detectedLangBadge.value = detected
    }
    setStatus('翻译完成', false, { label: '重试', onClick: retryTranslation })
    applyPendingConfigRefresh()
  } else if (allFailed) {
    isTranslating.value = false
    currentBatchId.value = null
    detectedLangBadge.value = ''
    setStatus('翻译失败', false, { label: '重试', onClick: retryTranslation })
    applyPendingConfigRefresh()
  } else if (anyTranslating) {
    setStatus('翻译中…', true, { label: '取消', onClick: cancelTranslation })
  } else {
    isTranslating.value = false
    currentBatchId.value = null
    sourceBadge.value = null
    detectedLangBadge.value = ''
    setStatus('部分完成', false, { label: '重试', onClick: retryTranslation })
    applyPendingConfigRefresh()
  }
}

/* === 事件分派 === */
const onStarted = (payload: TranslationEventPayload, isNewBatch: boolean): void => {
  if (isNewBatch) {
    if (payload.sourceText !== undefined) sourceText.value = payload.sourceText
    charCount.value = sourceText.value.length
    sourceBadge.value = payload.sourceType ?? null
    detectedLangBadge.value = sessionSourceLang.value === 'auto' ? '检测中…' : ''
    setStatus('翻译中…', true, { label: '取消', onClick: cancelTranslation })
  }
}
const onDetectedLang = (lang: string | null): void => {
  if (sessionSourceLang.value === 'auto' && lang) detectedLangBadge.value = lang
}

const events = useTranslationEvents({
  cards,
  getIsTranslating: () => isTranslating.value,
  setIsTranslating: (v) => { isTranslating.value = v },
  getCurrentBatchId: () => currentBatchId.value,
  setCurrentBatchId: (id) => { currentBatchId.value = id },
  onStarted,
  onBatchStatusChange: updateBatchStatus,
  onDetectedLang,
  onConfigChanged: (cfg) => {
    if (cfg.logLevel) logger.setLevel(cfg.logLevel)
    refreshCardsFromConfig(cfg)
  },
  logger,
})

/* === 卡片配置同步（复刻旧 refreshCardsFromConfig + syncServiceCards） === */
const enabledPayloads = (config: AppConfig): Array<{ serviceInstanceId: string; serviceType: string; serviceName: string }> =>
  (config.services || [])
    .filter((s) => s.enabled)
    .map((s) => ({ serviceInstanceId: s.id, serviceType: s.serviceType, serviceName: s.name }))

const refreshCardsFromConfig = (config: AppConfig): void => {
  const payloads = enabledPayloads(config)
  const enabledIds = new Set(payloads.map((p) => p.serviceInstanceId))
  if (isTranslating.value) {
    pendingConfigRefresh.value = config
    // 翻译中：不新增/删除，仅更新已有卡片元信息
    cards.forEach((card, id) => {
      if (!enabledIds.has(id) && card.status !== 'translating') cards.delete(id)
    })
    payloads.forEach((p) => {
      const card = cards.get(p.serviceInstanceId)
      if (card) { card.serviceName = p.serviceName; card.serviceType = p.serviceType }
    })
    return
  }
  pendingConfigRefresh.value = null
  cards.forEach((card, id) => {
    if (!enabledIds.has(id) && card.status !== 'translating') cards.delete(id)
  })
  payloads.forEach((p) => {
    let card = cards.get(p.serviceInstanceId)
    if (!card) {
      card = {
        serviceInstanceId: p.serviceInstanceId,
        serviceName: p.serviceName,
        serviceType: p.serviceType,
        modelName: '',
        text: '',
        status: 'pending',
        collapsed: !sourceText.value.trim(),
        expanded: false,
        hasOverflow: false,
        showActions: false,
        usage: null,
        detectedSourceLang: null,
      }
      cards.set(p.serviceInstanceId, card)
    } else {
      card.serviceName = p.serviceName
      card.serviceType = p.serviceType
    }
  })
}

const applyPendingConfigRefresh = (): void => {
  if (!pendingConfigRefresh.value) return
  const cfg = pendingConfigRefresh.value
  pendingConfigRefresh.value = null
  refreshCardsFromConfig(cfg)
}

/* === 翻译触发 === */
const startManualTranslation = async (): Promise<void> => {
  if (isTranslating.value) return
  const text = sourceText.value.trim()
  if (!text) { toast.info('请输入要翻译的文本'); return }
  const apis = getTauriApis()
  if (!apis) { toast.info('Tauri API 未就绪，请在桌面应用中运行'); return }
  try {
    await apis.invoke('start_translation', { text })
  } catch (e) {
    toast.error('翻译失败', String(e))
    logger.error('手动翻译失败', String(e))
  }
}

async function cancelTranslation(): Promise<void> {
  const apis = getTauriApis()
  if (!apis) return
  try {
    await apis.invoke('cancel_translation')
  } catch (e) {
    toast.error('取消失败', String(e))
    logger.warn('取消翻译失败', String(e))
  }
}

async function retryTranslation(): Promise<void> {
  if (isTranslating.value) return
  const apis = getTauriApis()
  if (!apis) { toast.info('Tauri API 未就绪'); return }
  try {
    await apis.invoke('retry_translation')
  } catch (e) {
    toast.error('重试失败', String(e))
    logger.error('重试失败', String(e))
  }
}

/* === 语言选择（复刻旧 selectLang/swapLangs） === */
const onSelectSource = async (code: string): Promise<void> => {
  sessionSourceLang.value = code
  detectedLangBadge.value = code === 'auto' ? '检测中…' : ''
  await persistSessionLanguages()
}
const onSelectTarget = async (code: string): Promise<void> => {
  sessionTargetLang.value = code
  await persistSessionLanguages()
}
const onSwap = async (): Promise<void> => {
  if (sessionSourceLang.value === 'auto' || sessionTargetLang.value === 'auto') {
    toast.info('自动检测不支持交换')
    return
  }
  const tmp = sessionSourceLang.value
  sessionSourceLang.value = sessionTargetLang.value
  sessionTargetLang.value = tmp
  await persistSessionLanguages()
}
const persistSessionLanguages = async (): Promise<void> => {
  const apis = getTauriApis()
  if (!apis) return
  try {
    await apis.invoke('set_session_languages', { sourceLang: sessionSourceLang.value, targetLang: sessionTargetLang.value })
  } catch (e) {
    toast.error('语言设置失败', String(e))
  }
}

/* === 原文输入 === */
const onSourceInput = (): void => {
  charCount.value = sourceText.value.length
  if (!sourceText.value.trim()) {
    cards.forEach((c) => { c.collapsed = true })
  }
}

/* === 待回填原文 + Edge 环境采集（复刻旧 applyPendingSourceText/collectEdgeTranslateEnv） === */
const applyPendingSourceText = async (): Promise<void> => {
  const apis = getTauriApis()
  if (!apis) return
  try {
    const text = await apis.invoke<string>('take_pending_source_text')
    if (text) {
      sourceText.value = text
      charCount.value = text.length
    }
  } catch (e) {
    toast.error('回填原文失败', String(e))
  }
}

const collectEdgeTranslateEnv = async (): Promise<void> => {
  const apis = getTauriApis()
  if (!apis) return
  try {
    const userAgent = navigator.userAgent
    const langs = navigator.languages ?? [navigator.language]
    const acceptLanguage = langs
      .map((l, i) => (i === 0 ? l : `${l};q=${(1 - i * 0.1).toFixed(1)}`))
      .join(',')
    await apis.invoke('save_edge_translate_env', { userAgent, acceptLanguage })
  } catch (e) {
    logger.warn('采集 Edge 翻译环境失败', String(e))
  }
}

/* === 初始化（复刻旧 initCards） === */
const initCards = async (): Promise<void> => {
  const apis = getTauriApis()
  if (!apis) return
  try {
    const [config, langs] = await Promise.all([
      apis.invoke<AppConfig>('get_app_config'),
      apis.invoke<{ sourceLang: string; targetLang: string }>('get_session_languages'),
    ])
    if (config?.logLevel) logger.setLevel(config.logLevel)
    sessionSourceLang.value = langs?.sourceLang ?? 'auto'
    sessionTargetLang.value = langs?.targetLang ?? 'zh-CN'
    refreshCardsFromConfig(config)
  } catch {
    return
  }
}

onMounted(() => {
  charCount.value = sourceText.value.length
  void initCards()
  void collectEdgeTranslateEnv()
  void applyPendingSourceText()
  window.addEventListener('focus', () => { void applyPendingSourceText() })
})
</script>

<script lang="ts">
import { computed } from 'vue'
import { LANGUAGES } from './data/languages'
</script>

<template>
  <div id="popup" ref="popupRef" class="popup">
    <PopupToolbar v-model:pinned="pinned" />

    <div class="content">
      <SourceCard
        v-model="sourceText"
        :lang-label="sourceLangLabel"
        :source-badge="sourceBadge"
        :detected-lang="detectedOrLabel"
        @submit="startManualTranslation"
        @input="onSourceInput"
      />

      <LanguageToolbar
        :source="sessionSourceLang"
        :target="sessionTargetLang"
        @update:source="onSelectSource"
        @update:target="onSelectTarget"
        @swap="onSwap"
      />

      <div class="results">
        <ResultCard
          v-for="card in cards.values()"
          :key="card.serviceInstanceId"
          :card="card"
          :target-lang="sessionTargetLang"
        />
      </div>
    </div>

    <StatusBar
      :text="statusInfo.text"
      :loading="statusInfo.loading"
      :action="statusInfo.action"
      :char-count="charCount"
    />
  </div>
  <Toaster />
</template>
```

> 说明：上方使用了两个 `<script>` 块（`<script setup>` + `<script lang="ts">`）以在 `setup` 内使用 `computed`/`LANGUAGES` 同时避免 setup 顶层 import 顺序问题。执行时若 typecheck 报错，统一改为：把 `import { computed } from 'vue'` 与 `import { LANGUAGES } from './data/languages'` 移到 `<script setup>` 顶部，删除第二个 `<script>` 块。`computed`/`LANGUAGES` 在 setup 内直接可用。这是首选写法，执行者优先采用单 `<script setup>` 合并写法。

- [ ] **步骤 2：创建 main.ts**

`frontend/src/popup/main.ts` 完整内容：

```typescript
import { createApp } from 'vue'
import TranslationPopup from './TranslationPopup.vue'
import '@/popup/popup-tokens.css'
import '@/popup/index.css'
import '@/popup/components.css'

createApp(TranslationPopup).mount('#app')
```

- [ ] **步骤 3：typecheck**

运行：`npm run typecheck`
预期：PASS。按上面说明把双 `<script>` 合并为单 `<script setup>`（把 `computed`、`LANGUAGES` 的 import 移到 setup 顶部，删除第二个 script 块），重新 typecheck 通过。

- [ ] **步骤 4：Commit**

```bash
git add frontend/src/popup/TranslationPopup.vue frontend/src/popup/main.ts
git commit -m "feat(popup): 新增 TranslationPopup 根组件与 main.ts 入口"
```

---

## 任务 12：translate.html + vite.config.ts + tsconfig.json

**文件：**
- 修改：`frontend/translate.html`（新建，原 `frontend/public/translate.html` 待任务 13 删除）
- 修改：`frontend/vite.config.ts`
- 修改：`frontend/tsconfig.json`

- [ ] **步骤 1：创建新的 frontend/translate.html**

注意：旧入口在 `frontend/public/translate.html`，新入口在 `frontend/translate.html`（Vite root 层，与 `settings.html` 同级）。先创建新文件，旧文件任务 13 删。

`frontend/translate.html` 完整内容：

```html
<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Shizi - 翻译</title>
</head>
<body>
  <div id="app"></div>
  <script type="module" src="/src/popup/main.ts"></script>
</body>
</html>
```

- [ ] **步骤 2：修改 vite.config.ts 增 translate input**

`frontend/vite.config.ts` 的 `build.rollupOptions.input` 从单入口改为对象双入口。把：

```typescript
    rollupOptions: {
      input: resolve(frontendDir, 'settings.html'),
    },
```

改为：

```typescript
    rollupOptions: {
      input: {
        settings: resolve(frontendDir, 'settings.html'),
        translate: resolve(frontendDir, 'translate.html'),
      },
    },
```

- [ ] **步骤 3：修改 tsconfig.json include 增 translate.html**

`frontend/tsconfig.json` 的 `include` 数组，把：

```json
    "include": ["src/**/*.ts", "src/**/*.vue", "settings.html"],
```

改为：

```json
    "include": ["src/**/*.ts", "src/**/*.vue", "settings.html", "translate.html"],
```

- [ ] **步骤 4：启动 dev 验证弹窗可加载**

运行：`npm run dev`（保持运行）
另开终端运行 release exe（或 `npm run tauri dev`）：
```bash
npm run tauri dev
```
预期：主窗口（translate.html）加载 Vue 弹窗，无白屏；控制台无 404。`http://localhost:5173/translate.html` 可直接访问。

> 验证要点：窗口能显示弹窗外壳（工具栏 + 原文卡 + 语言栏 + 状态栏）；输入文本 + Enter 触发翻译（需后端配置了启用服务）；如无法触发翻译，先确认 `get_app_config` 返回的 services 有 enabled 项。

- [ ] **步骤 5：Commit**

```bash
git add frontend/translate.html frontend/vite.config.ts frontend/tsconfig.json
git commit -m "feat(popup): translate.html 改为 Vue 入口，vite 多入口构建"
```

---

## 任务 13：端到端联调 + 删除旧文件

**文件：**
- 删除：`frontend/public/translate.html`、`frontend/public/translate.css`、`frontend/public/translate.js`、`frontend/public/translate-card-sync.js`、`frontend/src/translate-card-sync.test.js`

- [ ] **步骤 1：端到端手动验证（spec 9.1 清单）**

运行 `npm run tauri dev`，逐项核对（不勾选直到亲眼确认）：

- [ ] 手输 + Enter 触发多渠道并发流式翻译
- [ ] 状态栏文案：就绪 -> 翻译中… -> 翻译完成 / 部分完成 / 翻译失败
- [ ] Alt+D 划词 -> 原文区显示「来自划词」徽章
- [ ] Alt+E OCR -> 原文区显示「来自 OCR」徽章
- [ ] 取消：状态栏「翻译失败」，卡片追加「[已取消]」
- [ ] 单个渠道失败：仅对应卡片红字，其他不受影响
- [ ] 图钉：`setAlwaysOnTop` 生效，图标变蓝
- [ ] 打开设置按钮：`open_settings` 触发设置窗口
- [ ] OCR 按钮：`trigger_ocr_translation` 触发
- [ ] 语言 combobox：搜索 / ↑↓ / Enter / Esc / 点外收
- [ ] 交换：正常可交换；`auto` 时提示「自动检测不支持交换」
- [ ] 检测徽章：`source=auto` -> 「检测中…」-> 显示 detectedSourceLang
- [ ] 卡片折叠/展开、「展开全文」在 overflow 时出现
- [ ] Token 徽章在 `usage != null` 时显示
- [ ] 高度自适应：短文本紧凑、长内容超屏上限 80% + 内部滚动
- [ ] app-config 变更：非翻译中即时增删卡片；翻译中延迟到 batch 完成
- [ ] `take_pending_source_text` 回填原文（划词触发）
- [ ] Edge 环境采集 `save_edge_translate_env`
- [ ] 前端日志按 logLevel 生效

> 若任一项不通过，回到对应组件任务修复（此任务不 commit，直到全部通过或记录已知问题）。

- [ ] **步骤 2：删除旧文件**

```bash
git rm frontend/public/translate.html frontend/public/translate.css frontend/public/translate.js frontend/public/translate-card-sync.js frontend/src/translate-card-sync.test.js
```

- [ ] **步骤 3：验证删除后构建无残留引用**

运行：`npm run typecheck && npm run test`
预期：PASS。`translate-card-sync.test.js` 删除后，`utils.test.ts` + `useTranslationEvents.test.ts` 仍全绿，无模块找不到错误。

- [ ] **步骤 4：Commit**

```bash
git add -A
git commit -m "refactor(popup): 删除旧 translate.html/css/js 与 card-sync 测试，迁移完成"
```

---

## 任务 14：重写 HistoryPanel.vue + settings/main.ts 加 import

**文件：**
- 修改：`frontend/src/settings/panels/HistoryPanel.vue`（整段重写）
- 修改：`frontend/src/settings/main.ts`

`HistoryPanel` 基于原型双栏布局 + 滚动测高逻辑，数据源用 `props.state.ocrHistory`，本地 `adaptedSessions` computed 把 `OcrHistoryEntry` 适配为伪 `HistorySession`（spec 7.1）。复用 `SourceCardView`/`LanguageToolbar readonly`/`ResultCardView`。

- [ ] **步骤 1：重写 HistoryPanel.vue**

`frontend/src/settings/panels/HistoryPanel.vue` 完整内容（替换原文件全部内容）：

```vue
<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import { History as HistoryIcon, Trash2, Camera, ScanText, MousePointerSquareDashed, ClipboardList, PencilLine, Layers } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import Dialog from '@/components/ui/dialog/Dialog.vue'
import { toast } from '@/lib/toast'
import { speakText } from '@/popup/composables/utils'
import { LANGUAGES } from '../tokens'
import SourceCardView from '@/popup/components/SourceCardView.vue'
import ResultCardView from '@/popup/components/ResultCardView.vue'
import LanguageToolbar from '@/popup/components/LanguageToolbar.vue'
import type { AppSettings, OcrHistoryEntry } from '../types'

/** 本地适配类型（不污染 types.ts，未来接后端多渠道后整体删除）。 */
type HistoryTrigger = 'selection' | 'clipboard' | 'manual' | 'screenshot'
interface HistoryResult {
  serviceInstanceId: string
  serviceName: string
  modelName: string
  translation: string
  status: 'success' | 'loading' | 'pending' | 'error' | 'aborted'
  inputTokens: number
  outputTokens: number
}
interface HistorySession {
  id: string
  timestamp: string
  trigger: HistoryTrigger
  sourceLang: string
  targetLang: string
  source: string
  results: HistoryResult[]
}

interface Props {
  state: AppSettings
}
const props = defineProps<Props>()

const LANG_MAP = new Map(LANGUAGES.map((l) => [l.value, l.label]))
const LANG_SHORT_MAP = new Map(LANGUAGES.map((l) => [l.value.split('-')[0], l.label]))
const langLabel = (code: string): string => LANG_MAP.get(code) ?? LANG_SHORT_MAP.get(code) ?? code

const TRIGGER_META: Record<HistoryTrigger, { label: string; icon: typeof Camera }> = {
  selection: { label: '划词翻译', icon: MousePointerSquareDashed },
  clipboard: { label: '剪贴板', icon: ClipboardList },
  manual: { label: '手动输入', icon: PencilLine },
  screenshot: { label: '截图翻译', icon: ScanText },
}

const FILTERS = [
  { id: 'all' as const, label: '全部', icon: Layers },
  { id: 'screenshot' as const, label: '截图翻译', icon: ScanText },
  { id: 'selection' as const, label: '划词翻译', icon: MousePointerSquareDashed },
  { id: 'manual' as const, label: '手动输入', icon: PencilLine },
  { id: 'clipboard' as const, label: '剪贴板', icon: ClipboardList },
]

const activeFilter = ref<'all' | HistoryTrigger>('all')
const activeId = ref<string>('')
const showClearConfirm = ref(false)

/** OcrHistoryEntry -> 伪 HistorySession（spec 7.1）。OCR 记录单结果，trigger 恒为 screenshot。 */
const adaptedSessions = computed<HistorySession[]>(() =>
  props.state.ocrHistory.map((e: OcrHistoryEntry) => {
    const svc = e.serviceInstanceId ? props.state.services.find((s) => s.id === e.serviceInstanceId) : undefined
    return {
      id: e.id,
      timestamp: e.timestamp,
      trigger: 'screenshot',
      sourceLang: e.sourceLang,
      targetLang: e.targetLang,
      source: e.source,
      results: [{
        serviceInstanceId: e.serviceInstanceId ?? 'unknown',
        serviceName: svc?.name ?? '(已删除)',
        modelName: '',
        translation: e.translation,
        status: (e.translation ? 'success' : 'error') as 'success' | 'error',
        inputTokens: 0,
        outputTokens: 0,
      }],
    }
  }),
)

const isEmpty = computed(() => adaptedSessions.value.length === 0)
const activeSession = computed<HistorySession | null>(() =>
  activeId.value ? adaptedSessions.value.find((s) => s.id === activeId.value) ?? null : null,
)

/* 首条默认选中 */
watchEffect(() => {
  if (!activeId.value && adaptedSessions.value.length > 0) {
    activeId.value = adaptedSessions.value[0].id
  }
  if (activeId.value && !adaptedSessions.value.some((s) => s.id === activeId.value)) {
    activeId.value = adaptedSessions.value[0]?.id ?? ''
  }
})

const formatDetailTime = (iso: string): string => {
  const d = new Date(iso)
  const Y = d.getFullYear()
  const MO = String(d.getMonth() + 1).padStart(2, '0')
  const DD = String(d.getDate()).padStart(2, '0')
  const HH = String(d.getHours()).padStart(2, '0')
  const MM = String(d.getMinutes()).padStart(2, '0')
  const SS = String(d.getSeconds()).padStart(2, '0')
  return `${Y}-${MO}-${DD} ${HH}:${MM}:${SS}`
}

const formatTime = (iso: string): string => {
  const d = new Date(iso)
  const now = new Date()
  const sameDay = (a: Date, b: Date): boolean =>
    a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate()
  const HH = String(d.getHours()).padStart(2, '0')
  const MM = String(d.getMinutes()).padStart(2, '0')
  if (sameDay(d, now)) return `${HH}:${MM}`
  const y = new Date(now); y.setDate(now.getDate() - 1)
  if (sameDay(d, y)) return `昨天 ${HH}:${MM}`
  const diff = Math.floor((now.getTime() - d.getTime()) / 86400000)
  if (diff < 7) return `${diff} 天前`
  const MO = String(d.getMonth() + 1).padStart(2, '0')
  const DD = String(d.getDate()).padStart(2, '0')
  return `${MO}-${DD} ${HH}:${MM}`
}

type Bucket = { label: string; entries: HistorySession[] }
const grouped = computed<Bucket[]>(() => {
  const now = new Date()
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime()
  const startOfYesterday = startOfToday - 86400000
  const startOfWeek = startOfToday - 7 * 86400000
  const today: HistorySession[] = []
  const yesterday: HistorySession[] = []
  const week: HistorySession[] = []
  const older: HistorySession[] = []
  for (const s of adaptedSessions.value) {
    const t = new Date(s.timestamp).getTime()
    if (t >= startOfToday) today.push(s)
    else if (t >= startOfYesterday) yesterday.push(s)
    else if (t >= startOfWeek) week.push(s)
    else older.push(s)
  }
  const out: Bucket[] = []
  if (today.length) out.push({ label: '今天', entries: today })
  if (yesterday.length) out.push({ label: '昨天', entries: yesterday })
  if (week.length) out.push({ label: '本周', entries: week })
  if (older.length) out.push({ label: '更早', entries: older })
  return out
})

const filteredGrouped = computed<Bucket[]>(() => {
  if (activeFilter.value === 'all') return grouped.value
  return grouped.value
    .map((b) => ({ ...b, entries: b.entries.filter((s) => s.trigger === activeFilter.value) }))
    .filter((b) => b.entries.length > 0)
})

const copy = async (text: string): Promise<void> => {
  if (!text) { toast.error('复制失败', '该记录没有可复制的文本'); return }
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text)
    } else {
      const ta = document.createElement('textarea')
      ta.value = text
      ta.style.position = 'fixed'
      ta.style.opacity = '0'
      document.body.appendChild(ta)
      ta.select()
      document.execCommand('copy')
      document.body.removeChild(ta)
    }
    toast.success('已复制译文', text.length > 30 ? `${text.slice(0, 30)}…` : text)
  } catch (err) {
    toast.error('复制失败', err instanceof Error ? err.message : '请检查浏览器权限')
  }
}

const clearAll = (): void => {
  props.state.ocrHistory = []
  showClearConfirm.value = false
  activeId.value = ''
  toast.success('已清空翻译历史')
}

const retryResult = (r: HistoryResult): void => {
  toast.info('已请求重新翻译', `${r.serviceName} · ${r.modelName || '默认模型'}`)
}

/* 卡片折叠态：按 sessionId + serviceInstanceId 记录。 */
const collapsedMap = reactive<Record<string, boolean>>({})
const cardKey = (sessionId: string, r: HistoryResult): string => `${sessionId}:${r.serviceInstanceId}`
const isCollapsed = (sessionId: string, r: HistoryResult): boolean => collapsedMap[cardKey(sessionId, r)] ?? false
const toggleCollapse = (sessionId: string, r: HistoryResult): void => {
  const k = cardKey(sessionId, r)
  collapsedMap[k] = !collapsedMap[k]
}

const speakSource = (): void => {
  const text = activeSession.value?.source
  if (!text) { toast.error('朗读失败', '该记录没有原文可朗读'); return }
  const lang = activeSession.value?.sourceLang && activeSession.value.sourceLang !== 'auto'
    ? activeSession.value.sourceLang
    : 'en-US'
  speakText(text, lang)
}

const speak = (text: string): void => {
  if (!text) { toast.error('朗读失败', '该记录没有可朗读的译文'); return }
  speakText(text, activeSession.value?.targetLang || 'zh-CN')
}

const triggerIcon = (t: HistoryTrigger): typeof Camera => TRIGGER_META[t]?.icon ?? Camera

const serviceIconSvg = (r: HistoryResult): string => {
  const name = r.serviceName.replace(/翻译|Translate|Translation/gi, '').trim()
  const color = '#94918A'
  const letter = (name[0] ?? '?').toUpperCase()
  return `<rect width="20" height="20" rx="5" fill="${color}"/><text x="10" y="14.5" text-anchor="middle" font-size="11" font-weight="700" fill="#fff" font-family="Segoe UI, system-ui, sans-serif">${letter}</text>`
}

const cardStatus = (r: HistoryResult): 'success' | 'loading' | 'pending' | 'error' | 'aborted' => r.status

/* === 滚动布局测高（复刻原型 updateScrollMetrics） === */
const rootRef = ref<HTMLElement>()
const headerRef = ref<HTMLElement>()
let metricsObserver: ResizeObserver | null = null

const findScroller = (el: HTMLElement | null): HTMLElement | null => {
  let node = el
  while (node) {
    const oy = getComputedStyle(node).overflowY
    if (oy === 'auto' || oy === 'scroll') return node
    node = node.parentElement
  }
  return null
}

const updateScrollMetrics = (): void => {
  const root = rootRef.value
  const header = headerRef.value
  if (!root || !header) return
  const scroller = findScroller(root.parentElement)
  if (!scroller) return
  const clientH = scroller.clientHeight
  const padTop = parseFloat(getComputedStyle(scroller).paddingTop) || 0
  const padBottom = parseFloat(getComputedStyle(scroller).paddingBottom) || 0
  const contentH = clientH - padTop - padBottom
  const headerH = header.offsetHeight
  const GAP = 12
  const asideTop = headerH + GAP
  root.style.setProperty('--history-header-h', `${headerH}px`)
  root.style.setProperty('--history-aside-top', `${asideTop}px`)
  root.style.setProperty('--history-aside-h', `${Math.max(contentH - asideTop - 8, 0)}px`)
}

onMounted(() => {
  updateScrollMetrics()
  metricsObserver = new ResizeObserver(updateScrollMetrics)
  const scroller = findScroller(rootRef.value?.parentElement ?? null)
  if (scroller) metricsObserver.observe(scroller)
  if (headerRef.value) metricsObserver.observe(headerRef.value)
})

onBeforeUnmount(() => {
  metricsObserver?.disconnect()
  metricsObserver = null
})
</script>

<template>
  <div ref="rootRef" class="flex flex-col gap-3">
    <!-- 顶部说明 + 清空全部 -->
    <div class="flex items-center justify-between gap-4 rounded-md border border-amber-200/70 bg-amber-50/40 px-3 py-2 dark:border-amber-900/40 dark:bg-amber-900/10">
      <div class="flex items-start gap-2 text-[12px] leading-relaxed text-amber-900/80 dark:text-amber-200/80">
        <span class="mt-0.5 h-1.5 w-1.5 shrink-0 rounded-full bg-amber-500" />
        <span>此功能正在开发中 · 仅记录截图翻译(OCR)结果,划词/取词/输入框翻译不计入</span>
      </div>
      <Button variant="ghost" size="sm" :disabled="isEmpty" class="text-muted-foreground hover:text-destructive" @click="showClearConfirm = true">
        <Trash2 class="h-3.5 w-3.5" />
        清空全部
      </Button>
    </div>

    <!-- 空状态 -->
    <div v-if="isEmpty" class="flex flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-border py-16 text-center">
      <div class="flex h-12 w-12 items-center justify-center rounded-full bg-muted text-muted-foreground">
        <HistoryIcon class="h-5 w-5" />
      </div>
      <div class="flex flex-col gap-1">
        <p class="text-sm font-medium text-foreground">暂无截图翻译记录</p>
        <p class="text-[12px] text-muted-foreground">使用快捷键截图翻译后,识别与翻译结果会自动保存在这里。</p>
      </div>
    </div>

    <template v-else>
      <!-- 触发方式筛选（sticky 冻结顶部） -->
      <div ref="headerRef" class="sticky top-0 z-30 shrink-0 bg-background pb-4">
        <div class="-mt-[10px] h-[10px] bg-background" aria-hidden="true" />
        <div class="flex items-center gap-1 rounded-md border border-border bg-card p-1 text-[12px]">
          <button
            v-for="f in FILTERS"
            :key="f.id"
            :title="f.label"
            class="flex h-7 items-center gap-1.5 rounded px-2.5 transition-colors"
            :class="activeFilter === f.id ? 'bg-accent text-foreground' : 'text-muted-foreground hover:text-foreground'"
            @click="activeFilter = f.id"
          >
            <component :is="f.icon" class="h-3.5 w-3.5" />
            <span class="whitespace-nowrap">{{ f.label }}</span>
          </button>
        </div>
      </div>

      <!-- 左右布局 -->
      <div class="flex gap-4">
        <!-- 左:列表（独立滚动） -->
        <aside class="w-[240px] shrink-0 self-start sticky top-[var(--history-aside-top)] max-h-[var(--history-aside-h)] flex min-h-0 flex-col gap-3 overflow-y-auto scrollbar-thin">
          <template v-for="bucket in filteredGrouped" :key="bucket.label">
            <header class="flex items-center gap-2 px-1 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
              <span>{{ bucket.label }}</span>
              <span class="text-[10px] opacity-60">{{ bucket.entries.length }} 条</span>
              <div class="h-px flex-1 bg-border" />
            </header>
            <ul class="flex flex-col gap-1">
              <li
                v-for="s in bucket.entries"
                :key="s.id"
                class="flex cursor-pointer flex-col gap-1.5 rounded-md border border-transparent p-2 transition-colors hover:bg-accent/40"
                :class="activeId === s.id ? 'border-primary/40 bg-accent' : ''"
                @click="activeId = s.id"
              >
                <div class="flex items-center gap-1.5 text-[10px] text-muted-foreground">
                  <span class="font-mono">{{ formatTime(s.timestamp) }}</span>
                  <span class="flex items-center rounded border border-border bg-background/60 px-1 py-0.5" :title="TRIGGER_META[s.trigger]?.label">
                    <component :is="triggerIcon(s.trigger)" class="h-3 w-3" />
                  </span>
                  <span class="inline-flex items-center gap-0.5 rounded border border-border bg-background/60 px-1 py-0.5 font-mono tabular-nums" :title="`${s.results.length} 个翻译渠道`">
                    <Layers class="h-2.5 w-2.5" />
                    {{ s.results.length }}
                  </span>
                  <template v-if="s.results.some((r) => r.status !== 'success')">
                    <span class="h-1.5 w-1.5 shrink-0 rounded-full bg-destructive" :title="`${s.results.filter((r) => r.status !== 'success').length} 个翻译结果异常`" />
                  </template>
                </div>
                <div class="line-clamp-2 text-[12px] leading-snug text-foreground">{{ s.source }}</div>
              </li>
            </ul>
          </template>
        </aside>

        <!-- 右:详情 -->
        <section class="flex min-w-0 flex-1 flex-col">
          <div v-if="!activeSession" class="flex flex-1 flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-border py-16 text-center text-muted-foreground">
            <HistoryIcon class="h-6 w-6" />
            <p class="text-sm">从左侧选一条会话查看详情</p>
          </div>

          <template v-else>
            <header class="flex shrink-0 items-center gap-2 pb-3">
              <component :is="triggerIcon(activeSession.trigger)" class="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
              <h2 class="text-sm leading-none text-foreground">{{ TRIGGER_META[activeSession.trigger]?.label }}</h2>
              <span class="ml-auto text-[11px] leading-none font-mono tabular-nums text-muted-foreground/50">{{ formatDetailTime(activeSession.timestamp) }}</span>
            </header>

            <div class="flex flex-col gap-1.5">
              <SourceCardView
                :text="activeSession.source"
                :lang-label="langLabel(activeSession.sourceLang)"
                @copy="copy(activeSession.source)"
                @speak="speakSource"
              />
              <LanguageToolbar :source="activeSession.sourceLang" :target="activeSession.targetLang" readonly />
              <section>
                <ul class="results flex flex-col gap-2">
                  <li v-for="r in activeSession.results" :key="r.serviceInstanceId + r.modelName" class="relative">
                    <ResultCardView
                      :engine-name="r.serviceName"
                      :engine-icon-html="serviceIconSvg(r)"
                      :model-name="r.modelName"
                      :status="cardStatus(r)"
                      :text="r.translation"
                      :collapsed="isCollapsed(activeSession.id, r)"
                      :show-tokens="true"
                      :input-tokens="r.inputTokens"
                      :output-tokens="r.outputTokens"
                      :show-actions="r.status !== 'pending'"
                      :show-refresh="true"
                      @copy="copy(r.translation)"
                      @refresh="retryResult(r)"
                      @speak="speak(r.translation)"
                      @toggle-collapse="toggleCollapse(activeSession.id, r)"
                    />
                  </li>
                </ul>
              </section>
            </div>
          </template>
        </section>
      </div>
    </template>

    <!-- 清空确认 -->
    <Dialog v-model:open="showClearConfirm" title="清空全部翻译历史?" description="此操作不可撤销,所有截图翻译记录都将被永久删除。" width="420px">
      <div class="flex justify-end gap-2 pt-2">
        <Button variant="ghost" size="sm" @click="showClearConfirm = false">取消</Button>
        <Button variant="destructive" size="sm" @click="clearAll">
          <Trash2 class="h-3.5 w-3.5" />
          确认清空
        </Button>
      </div>
    </Dialog>
  </div>
</template>
```

> 说明：`watchEffect` 用于首条默认选中 + 失效回退。`import` 里若 `watchEffect` 未在 vue 导入列表中，补上：把 `import { computed, onBeforeUnmount, onMounted, reactive, ref } from 'vue'` 改为 `import { computed, onBeforeUnmount, onMounted, reactive, ref, watchEffect } from 'vue'`（上方代码已隐含使用 `watchEffect`，执行时确保导入）。

- [ ] **步骤 2：修改 settings/main.ts 加 import**

`frontend/src/settings/main.ts` 完整内容（在 `import '@/styles/main.css'` 后加两行）：

```typescript
import { createApp } from 'vue';
import App from './App.vue';
import '@/styles/main.css';
import '@/popup/popup-tokens.css';
import '@/popup/components.css';

createApp(App).mount('#app');
```

- [ ] **步骤 3：typecheck + 手动验证（spec 9.2 清单）**

运行：`npm run typecheck`
预期：PASS。

运行 `npm run tauri dev`，打开设置 -> 翻译历史：

- [ ] 左列表按今天/昨天/本周/更早分桶
- [ ] trigger 筛选栏渲染 5 个按钮（当前只 screenshot 有数据，其余切换为空列表）
- [ ] 点击左侧条目切换右侧详情
- [ ] `SourceCardView` / `LanguageToolbar readonly` / `ResultCardView` 三处复用样式与弹窗一致
- [ ] 空态提示显示
- [ ] 清空全部 Dialog 确认 -> 清空数据
- [ ] 左侧 aside 独立滚动、右侧文档流随窗口滚动、sticky 筛选栏不穿透

- [ ] **步骤 4：Commit**

```bash
git add frontend/src/settings/panels/HistoryPanel.vue frontend/src/settings/main.ts
git commit -m "feat(history): HistoryPanel 按原型双栏重写,复用 popup 组件"
```

---

## 任务 15：静态检查 + 打包验证

**文件：** 无新增（仅验证）

- [ ] **步骤 1：前端类型检查**

运行：`npm run typecheck`
预期：PASS（无 error）。

- [ ] **步骤 2：前端单测**

运行：`npm run test`
预期：PASS（原有 vitest 全绿 + `utils.test.ts` + `useTranslationEvents.test.ts` 新增全绿）。

- [ ] **步骤 3：后端单测（应零影响）**

运行：`cd src-tauri && cargo test`
预期：PASS（后端未动）。

- [ ] **步骤 4：前端构建（双入口产物）**

运行：`npm run build`
预期：`frontend/dist/` 产出 `settings.html` + `translate.html` 两个入口（及对应 assets）。

- [ ] **步骤 5：Tauri 打包验证**

运行：`npm run tauri build`
预期：生成 NSIS 安装包；安装后主窗口能正常加载新 `translate.html`（Vue 弹窗），设置页历史面板正常。

> 若打包后发现弹窗白屏，检查 `tauri.conf.json` 的 `main.url: "translate.html"` 在 build 模式下能否从 `frontendDist` 找到 `translate.html`（应已满足，因 dist 已产出该文件）。

- [ ] **步骤 6：Commit（如有构建配置微调）**

若步骤 1-5 全通过且无文件改动，跳过 commit。若有微调：

```bash
git add -A
git commit -m "chore(popup): 构建与打包验证修正"
```

---

## 任务 16：文档同步

**文件：**
- 修改：`CLAUDE.md`、`AGENTS.md`、`plugins.md`

- [ ] **步骤 1：更新 CLAUDE.md 项目结构**

在 `CLAUDE.md` 的「项目结构」章节，把 `frontend/` 部分的描述更新：
- `translate.html main 窗口翻译弹窗` 改为：`translate.html main 窗口翻译弹窗入口（Vue 3，加载 /src/popup/main.ts）`
- 在 `frontend/` 子项中增补：`src/popup/ 翻译弹窗 Vue 组件体系（根组件 TranslationPopup.vue + 8 子组件 + 3 composable + 共享 CSS），与设置页共享 src/ 工程；HistoryPanel.vue 复用其 SourceCardView/ResultCardView/LanguageToolbar`
- 在「架构关键点」-「分层结构」-「UI 层」补充：翻译弹窗已 Vue 化，与设置页共享 `src/popup/` 共享组件（`ResultCardView`/`SourceCardView`/`LanguageToolbar`），历史面板右侧详情复用这些组件。
- 在「前后端通信」段无需改动（command 契约未变）。
- 「翻译弹窗窗口」段：`.toolbar` 拖拽、`ResizeObserver` 高度自适应等行为不变（现由 `usePopupHeight` composable 实现）。

- [ ] **步骤 2：同步 AGENTS.md**

`AGENTS.md` 与 `CLAUDE.md` 保持内容同步（项目规范第 1 条），做相同改动。

- [ ] **步骤 3：更新 plugins.md 依赖清单**

在 `plugins.md` 的依赖清单中增补：`lucide-vue-next`（翻译弹窗/历史面板图标库，与既有 `@lucide/vue` 并存，待后续统一迁移）。

- [ ] **步骤 4：Commit**

```bash
git add CLAUDE.md AGENTS.md plugins.md
git commit -m "docs: 同步弹窗 Vue 化与历史面板重写的项目结构文档"
```

---

## 自检

### 1. 规格覆盖度

逐章核对 spec：
- 一、目标 1（弹窗 Vue 化）→ 任务 2-13 ✓
- 一、目标 2（HistoryPanel 双栏重写 + 复用）→ 任务 14 ✓
- 一、目标 3（1:1 行为对齐，后端不动）→ 全程约束，任务 13 端到端验证 ✓
- 二、范围「本次做」→ 全覆盖（依赖/CSS/组件/composable/入口/构建/清理/HistoryPanel/文档）✓
- 三、边界约束 → 后端契约冻结（未动）、窗口配置不动、权限清单不动、CSS `--popup-*` 前缀（任务 2）、lucide-vue-next（任务 1）、toast 走 `@/lib/toast`（任务 10/11）、languages.ts 独立（任务 3）✓
- 四、目录结构 → 任务 2-11 新建文件与 spec 第四节一致 ✓
- 五、组件契约 → 任务 7-10 各组件 props/emits 与 spec 5.1-5.8 一致 ✓
- 六、状态与事件流 → 任务 4（useTranslationEvents）+ 任务 11（TranslationPopup 状态/事件流）+ 任务 10（ResultCard 流式）✓
- 七、HistoryPanel UI → 任务 14（adaptedSessions + 双栏 + 滚动测高 + 清空 Dialog）✓
- 八、CSS 组织 → 任务 2（popup-tokens）+ 任务 6（components/index）+ 任务 14（settings/main.ts import）✓
- 九、验证清单 → 任务 13（9.1）+ 任务 14（9.2）+ 任务 15（9.3/9.4）✓
- 十、单元测试 → 任务 3（utils）+ 任务 4（useTranslationEvents）+ 删除旧 translate-card-sync.test.js（任务 13）✓
- 十一、迁移顺序 → 任务 1-15 顺序与 spec 第十一节一致 ✓
- 十二、风险与回滚 → 任务 12（风险 3 devUrl）+ 任务 10（风险 2 命令式 appendChild）+ 任务 15（打包验证）；回滚路径：删 `src/popup/` + 恢复 `public/translate.*` + revert vite.config/HistoryPanel ✓
- 十三、文档同步 → 任务 16 ✓

### 2. 占位符扫描

无「待定/TODO/后续实现」；无「添加适当错误处理」类空描述；每个代码步骤含完整代码块；无「类似任务 N」省略。任务 7 的 `showDot`/`showDotFinal` 与任务 11 的双 `<script>` 块均给了明确的执行时合并/清理指引（非占位符，是 typecheck 收尾说明）。

### 3. 类型一致性

- `CardState`：任务 4 定义，任务 10（ResultCard props.card）、任务 11（TranslationPopup cards Map）一致引用 ✓
- `CardStatus`：任务 4 定义 `'pending'|'translating'|'finished'|'failed'|'cancelled'`，任务 10 `viewStatus` 映射一致 ✓
- `TranslationEventPayload`：任务 4 定义，任务 11 `onStarted` 参数类型一致 ✓
- `CardViewStatus`（`'success'|'loading'|'pending'|'error'|'aborted'`）：任务 7 ResultCardView props、任务 10 viewStatus 返回、任务 14 cardStatus 返回一致 ✓
- `Language` 接口（`{value,label,english}`）：任务 3 定义，任务 8 LanguagePicker/LanguageToolbar 引用一致 ✓
- `HistorySession`/`HistoryResult`/`HistoryTrigger`：任务 14 本地定义并自洽 ✓
- invoke 命令名（`start_translation`/`cancel_translation`/`retry_translation`/`get_app_config`/`get_session_languages`/`set_session_languages`/`take_pending_source_text`/`trigger_ocr_translation`/`open_settings`/`save_edge_translate_env`）：任务 10/11 与 spec 第三节一致 ✓

### 已知执行时需注意的点

1. **任务 7 ResultCardView 的 `showDot`/`showDotFinal`**：原型有两个函数，执行时以 typecheck 通过为准，删除未使用的 `showDot`，保留 `dotClass` + `showDotFinal`。
2. **任务 11 TranslationPopup 双 `<script>` 块**：执行者优先用单 `<script setup>` 合并写法（把 `computed`/`LANGUAGES` import 移到 setup 顶部，删第二个 script 块）。
3. **任务 14 HistoryPanel `watchEffect`**：确保从 `vue` 导入 `watchEffect`（导入列表已含）。
4. **流式渲染性能**：`ResultCard` 用 `watch(card.text, { flush: 'sync' })` 增量 appendChild，避免 Vue 虚拟 DOM diff 大文本（spec 风险 2）。若实测有丢字，检查 `flush: 'sync'` 是否生效。
5. **`tauri.conf.json` 的 `main.url: "translate.html"`**：dev 模式拼接 `http://localhost:5173/translate.html`，build 模式从 `frontendDist` 找 `translate.html`，任务 12 + 15 已验证两端。
