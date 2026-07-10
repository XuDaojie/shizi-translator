# 翻译弹窗 Vue 化 + 翻译历史 UI 重写 设计规格

- **状态**：已实施
- **作者**：xdj（与 Claude 协作）
- **日期**：2026-07-10
- **关联原型**：`C:\Users\xdj\IdeaProjects\LLM\OpenDesignProjects\shizi`（http://localhost:5174/）

## 一、目标

1. 把翻译弹窗从原生 HTML（`frontend/public/translate.html` + `translate.css` + 819 行 `translate.js`）完整迁移到 Vue 3 单文件组件体系，与设置页共享 `frontend/src/` 工程。
2. 把设置页的翻译历史面板（`frontend/src/settings/panels/HistoryPanel.vue`）按原型的「左列表 + 右详情」双栏结构重写，其中右侧详情**直接复用**翻译弹窗的 Vue 组件（`ResultCardView` / `SourceCardView` / `LanguageToolbar`），确保 UI 与交互与弹窗一致。
3. 迁移过程严格 1:1 对齐旧行为，不引入功能变更；后端 Rust 侧完全不动。

## 二、范围

### 本次做

- **翻译弹窗完整 Vue 化**：新建 `frontend/src/popup/` 目录，包含根组件、8 个子组件、3 个 composable、共享 CSS。
- **翻译历史面板 UI 重写**：`HistoryPanel.vue` 按原型双栏布局，复用 `src/popup/components/*View.vue`。数据源仍为 `state.ocrHistory`，UI 层通过 `computed` 适配为伪 `HistorySession` 形状。
- **入口与构建配置**：`frontend/translate.html` 改为 Vue 入口；`vite.config.ts` 增加 `translate.html` 为 rollup 第二个 input。
- **依赖**：新增 `lucide-vue-next`。
- **清理**：删除 `frontend/public/translate.{html,css,js}` 与 `frontend/public/translate-card-sync.js`；删除 `frontend/src/translate-card-sync.test.js`。

### 本次不做

- 不动任何 Rust 后端代码、command 契约、事件契约、快捷键、OCR、DPI、托盘。
- 不改历史数据源类型（`OcrHistoryEntry` 保持不变），不接后端持久化。
- 不改设置页除 `HistoryPanel.vue` 外的任何面板。
- 不动 `overlay.html`（截图 overlay 保持纯静态）。
- 不动 `frontend/public/logger.js`（OCR overlay 仍依赖）。

## 三、边界与约束

1. **后端契约冻结**：`translation:event` 的 payload 形状不变；`start_translation` / `cancel_translation` / `retry_translation` / `get_app_config` / `save_app_config` / `get_session_languages` / `set_session_languages` / `take_pending_source_text` / `trigger_ocr_translation` / `open_settings` / `save_edge_translate_env` / `write_frontend_log` 全部照旧。
2. **窗口配置**：`tauri.conf.json` 中 `main.url` 保留为 `translate.html`（文件本身内容替换为 Vue 入口）。窗口尺寸、`decorations:false`、`transparent:true`、`resizable:false` 均不动。
3. **权限清单**：`capabilities/default.json` 现有 `core:window:allow-set-always-on-top` / `core:window:allow-set-size` 已满足，无需新增。
4. **CSS 复用**：弹窗与历史面板共享 `src/popup/popup-tokens.css` + `src/popup/components.css`；设置页入口需 `import '@/popup/popup-tokens.css'` 保证 `--popup-*` 变量可用。
5. **图标库**：新增 `lucide-vue-next`（历史面板需要 9 个图标）。弹窗内工具栏/服务图标仍用内嵌 SVG 保持体积与旧版一致。
6. **Toast**：统一走 `frontend/src/lib/toast.ts`；旧 translate.html 中自绘的 `.toast` div 一并删除。
7. **国际化**：LANGUAGES 数据表在弹窗侧新建 `src/popup/data/languages.ts`，内容与旧 `translate.js` 的 LANGUAGES 常量保持一致；设置侧 `src/settings/tokens.ts` 的 LANGUAGES 不动，两者独立维护（新增语言两处同步 —— 与旧代码约束一致）。

## 四、目录结构

### 新增

```
frontend/
└── src/
    └── popup/
        ├── main.ts
        ├── TranslationPopup.vue
        ├── popup-tokens.css                 # --popup-* CSS 变量（从原型移植）
        ├── index.css                        # 外壳/工具栏/语言栏/状态栏/拖拽把手
        ├── components.css                   # source-card/result-card/lang-picker（历史面板复用）
        ├── components/
        │   ├── PopupToolbar.vue             # 图钉/OCR/设置（接 invoke，与原型 mock 版差异）
        │   ├── SourceCard.vue               # 含 textarea（弹窗编辑用）
        │   ├── SourceCardView.vue           # 纯展示（历史详情复用）
        │   ├── LanguageToolbar.vue          # 源/目标 + swap，支持 readonly
        │   ├── LanguagePicker.vue           # 内嵌搜索 combobox
        │   ├── ResultCard.vue               # Container：接 translation:event → 驱动 View
        │   ├── ResultCardView.vue           # 纯展示（历史详情复用）
        │   └── StatusBar.vue                # 状态点 + 文案 + 取消/重试
        ├── composables/
        │   ├── useTranslationEvents.ts      # listen('translation:event') + 分派到 cards
        │   ├── usePopupHeight.ts            # ResizeObserver + setSize（复刻 adjustHeight）
        │   └── utils.ts                     # speakText / copyText / batchIdFromSession
        └── data/
            └── languages.ts                 # LANGUAGES 数组（从旧 translate.js 提取）
```

### 修改

- `frontend/translate.html`：内容替换为 `<div id="app"></div>` + `<script type="module" src="/src/popup/main.ts"></script>`。
- `frontend/vite.config.ts`：`rollupOptions.input` 从单个 `settings.html` 扩为 `{ settings, translate }`。
- `frontend/src/settings/panels/HistoryPanel.vue`：整段重写（数据源仍为 `state.ocrHistory`）。
- `frontend/src/settings/main.ts`：加一行 `import '@/popup/popup-tokens.css'` 让设置页也能使用 `--popup-*` tokens。
- `package.json`：`dependencies` 增加 `lucide-vue-next`；`@lucide/vue` 保留（如果暂时都在用），或迁移完成后统一切换到 `lucide-vue-next`。**建议**：本次同时把项目内既有 `@lucide/vue` 引用改为 `lucide-vue-next`，只维护一个图标包（如影响面较大则单独另行处理，本次仅新增 `lucide-vue-next` 供新代码使用）。

### 删除

- `frontend/public/translate.html`
- `frontend/public/translate.css`
- `frontend/public/translate.js`
- `frontend/public/translate-card-sync.js`
- `frontend/src/translate-card-sync.test.js`

**保留**：`frontend/public/logger.js`、`frontend/public/overlay.html`（OCR overlay 仍是静态资源）。

## 五、组件契约

### 5.1 `ResultCardView.vue`（弹窗与历史双端复用）

```ts
interface Props {
  engineName: string
  engineIconHtml: string          // 内嵌 SVG 片段（不含 <svg> 标签），viewBox="0 0 20 20"
  modelName?: string
  text?: string                   // 已完成译文；流式态通过默认 slot 提供
  status: 'success' | 'loading' | 'pending' | 'error' | 'aborted'
  loading?: boolean               // 流式渲染时驱动头部蓝点 + 光标
  collapsed?: boolean
  hasOverflow?: boolean
  expanded?: boolean
  showTokens?: boolean
  inputTokens?: number
  outputTokens?: number
  showActions?: boolean
  showRefresh?: boolean           // 失败/中断时右侧的重新翻译按钮
}
emits: 'toggle-collapse' | 'toggle-expand' | 'speak' | 'copy' | 'refresh'
```

- 弹窗侧：由 `ResultCard.vue`（Container）通过 slot 注入流式 text（含 `.stream-cursor` 光标 span）。
- 历史侧：`text` 走 prop，不使用 slot。

### 5.2 `SourceCardView.vue`（纯展示，历史详情复用）

```ts
interface Props { text: string; langLabel: string }
emits: 'speak' | 'copy' | 'focus'
```

### 5.3 `SourceCard.vue`（弹窗独有，含 textarea）

```ts
interface Props { modelValue: string; langLabel: string; sourceBadge?: 'selectedText'|'ocrText'|null }
emits: 'update:modelValue' | 'submit'   // Enter 提交（Shift+Enter 换行）
```

- 自动 resize；朗读/复制按钮内嵌；徽章区显示 `来自划词`/`来自 OCR` + 检测到的语言。

### 5.4 `LanguageToolbar.vue`

```ts
interface Props { source: string; target: string; readonly?: boolean }
emits: 'update:source' | 'update:target' | 'swap'
```

- `readonly=true` 时关闭所有交互（历史详情场景）。
- 内嵌 `LanguagePicker.vue`（搜索 combobox），auto 状态下禁止 swap。

### 5.5 `LanguagePicker.vue`

```ts
interface Props { modelValue: string; type: 'source'|'target'; placeholder: string }
emits: 'update:modelValue' | 'pick'
defineExpose({ focus })
```

- 搜索输入 + ↑↓/Enter/Esc 键盘导航 + 点外收（由父组件处理）。

### 5.6 `PopupToolbar.vue`（弹窗独有）

- 图钉：`invoke → getCurrentWindow().setAlwaysOnTop(pinned)`
- OCR 按钮：`invoke('trigger_ocr_translation')`
- 设置按钮：`invoke('open_settings')`
- 全部包一层 `data-tauri-drag-region` 或不包（顶部 toolbar 整体作拖拽区）；行为与旧 `translate.html` 一致。

### 5.7 `StatusBar.vue`

```ts
interface Props {
  text: string
  loading: boolean
  action?: { label: string; onClick: () => void } | null
  charCount: number
}
```

### 5.8 `ResultCard.vue`（弹窗独有 Container）

职责：
- 由 `TranslationPopup.vue` 通过 `serviceInstanceId` 作 key 渲染。
- 接收 `CardState`（父组件维护的 reactive Map 中一条），驱动内部 `ResultCardView`。
- 处理复制（走 `toast`）、朗读（`speakText(text, sessionTargetLang)`）、重试（`invoke('retry_translation')`）。
- 流式态的 text slot 由本组件维护：接 `delta` 时 append TextNode，`finished` 时全量替换以校正。

## 六、状态与事件流

### 6.1 顶层状态（`TranslationPopup.vue`）

```ts
const sourceText = ref('')
const sessionSourceLang = ref('auto')
const sessionTargetLang = ref('zh-CN')
const isTranslating = ref(false)
const currentBatchId = ref<string|null>(null)
const cards = reactive<Map<string, CardState>>(new Map())
const pinned = ref(false)
const sourceBadge = ref<'selectedText'|'ocrText'|null>(null)
const detectedLangBadge = ref('')
const statusInfo = ref<{ text: string; loading: boolean; action: StatusAction|null }>({
  text: '就绪', loading: false, action: null,
})
const pendingConfigRefresh = ref<AppConfig|null>(null)

interface CardState {
  serviceInstanceId: string
  serviceName: string
  serviceType: string
  modelName: string
  text: string
  status: 'pending'|'translating'|'finished'|'failed'|'cancelled'
  collapsed: boolean
  expanded: boolean
  hasOverflow: boolean
  showDot: boolean
  showActions: boolean
  usage: { inputTokens: number; outputTokens: number } | null
  detectedSourceLang: string | null
}
```

- **不引入 Pinia**。状态直接在根组件用 `ref` / `reactive` 维护，通过 `provide/inject` 传给子组件（如需要）。
- Map 而非普通数组 —— 便于按 `serviceInstanceId` O(1) 查找，与旧 `resultCards` Map 一致。

### 6.2 三个 composable

- **`useTranslationEvents(cards, ctx)`**：
  - `listen('translation:event', evt => dispatch(evt.payload))`
  - `dispatch` 对 `started/delta/finished/failed/cancelled` 各分支更新 cards Map 中对应 `serviceInstanceId` 的条目；`started` 若 batchId 变化则先重置所有卡片（复刻旧 `renderTranslationEvent`）。
  - `listen('app-config:changed', ...)`：更新 logLevel + 调用 `refreshCardsFromConfig(cfg)`（复刻旧 `syncServiceCards` 逻辑，非翻译中即时改，翻译中延迟到本次完成）。

- **`usePopupHeight(rootRef)`**：
  - `ResizeObserver` 观察 `.popup`；rAF 节流；`getCurrentWindow().setSize({ Logical, width:420, height })`。
  - 初始化设 `popup.style.maxHeight = screen.availHeight * 0.8 + 'px'`。

- **`utils.ts`**：`speakText(text, lang)` / `copyText(text)`（返回 boolean）/ `batchIdFromSession(sessionId)`。

### 6.3 事件流

```
用户 Enter/快捷键
  → invoke('start_translation', { text })
  → Rust 广播 translation:event: started
  → useTranslationEvents.dispatch:
      - batchId 变化 → reset cards
      - cards.get(serviceInstanceId).status = 'translating'
  → Vue 响应式重渲染 ResultCardView
  → translation:event: delta * N
  → cards[...].text += chunk（通过 slot 里的 <div ref="textRef"> 直接 appendChild TextNode 保持流式性能）
  → translation:event: finished
  → cards[...].status = 'finished', usage, detectedSourceLang
  → 全部完成 → statusInfo = { text:'翻译完成', action: retry }
```

### 6.4 关键 UI 状态转移

- **弹窗打开时**：`initCards()` 并发调 `get_app_config` + `get_session_languages` → 预建卡片、设置语言。同时 `collectEdgeTranslateEnv()`（采集 UA 供后端 Edge 翻译使用）+ `applyPendingSourceText()`（回填划词/OCR 触发的原文）。
- **翻译中收到 app-config:changed**：`pendingConfigRefresh = cfg`；本次 batch 完成后 `applyPendingConfigRefresh()`。
- **source=auto 时**：`detectedLangBadge` 先显示"检测中…"，收到首个 `finished.detectedSourceLang` 后替换。

## 七、HistoryPanel UI 重写

### 7.1 数据适配

**数据源不变**：仍为 `props.state.ocrHistory: OcrHistoryEntry[]`。

在 panel 内新增本地 `computed`：

```ts
const adaptedSessions = computed(() =>
  props.state.ocrHistory.map(e => ({
    id: e.id,
    timestamp: e.timestamp,
    trigger: 'screenshot' as const,
    sourceLang: e.sourceLang,
    targetLang: e.targetLang,
    source: e.source,
    results: [{
      serviceInstanceId: e.serviceInstanceId ?? 'unknown',
      serviceName: props.state.services.find(s => s.id === e.serviceInstanceId)?.name ?? '(已删除)',
      modelName: '',
      translation: e.translation,
      status: (e.translation ? 'success' : 'error') as 'success'|'error',
      inputTokens: 0,
      outputTokens: 0,
    }],
  }))
)
```

- 只在 panel 内做适配，不污染 `types.ts`。未来接后端后适配层可整体删除，UI 完全不用改。

### 7.2 布局与交互（对齐原型 HistoryPanel）

- **顶部 sticky 筛选栏**：4 个 trigger + `全部`，共 5 个按钮。当前所有数据实际都是 `screenshot`，但保留完整 UI 结构。
- **左侧列表** aside，宽 240px，sticky top，独立滚动。按今天/昨天/本周/更早分桶。
- **右侧详情** 走文档流，随窗口滚动。构成：
  1. header（trigger 图标 + 标签 + 详细时间）
  2. `<SourceCardView>` 显示原文
  3. `<LanguageToolbar readonly>` 显示源→目标
  4. `<ResultCardView>` × N 显示各渠道结果（当前实际总是 1 个）
- **顶部清空全部按钮**：保留 amber 提示条 + `showClearConfirm` Dialog（清空即 `state.ocrHistory = []`）。
- **图标**：新增 lucide `History`, `Trash2`, `Copy`, `Camera`, `ScanText`, `MousePointerSquareDashed`, `ClipboardList`, `PencilLine`, `Layers`。
- **滚动测高逻辑**：完整复刻原型的 `updateScrollMetrics` + ResizeObserver（左侧 aside 独立滚动 + 右侧文档流滚动 + sticky 头部）。

## 八、CSS 组织

三个共享 CSS 文件放 `src/popup/`：

- **`popup-tokens.css`**：只含 `:root { --popup-* }` CSS 变量。被弹窗与设置页共同 import。
- **`index.css`**：弹窗外壳、`.popup`、`.popup-toolbar`、`.lang-toolbar`、`.status-bar`、`.stream-cursor` 光标动画、`.toast`（如有）。**仅弹窗侧 import。**
- **`components.css`**：`.source-card`、`.source-input`、`.source-meta`、`.result-card`（header/body/text-clip/expand-btn/actions/tokens）、`.lang-picker`。**两侧 import：**弹窗侧走 `TranslationPopup.vue` 里 `import '@/popup/components.css'`；设置侧走 `settings/main.ts` 里 `import '@/popup/components.css'`。

Vue SFC 内部**不写 `<style scoped>`**（避免 scoped hash 干扰复用），所有样式经全局 class 复用。

## 九、验证清单

### 9.1 端到端手动验证（1:1 行为对齐）

- [x] 手输 + Enter 触发多渠道并发流式翻译
- [x] 状态栏文案：就绪 → 翻译中… → 翻译完成 / 部分完成 / 翻译失败
- [x] Alt+D 划词 → `source-badge = 来自划词`
- [x] Alt+E OCR → `source-badge = 来自 OCR`
- [x] 取消：状态栏"翻译失败"，卡片追加"[已取消]"
- [x] 单个渠道失败：仅对应卡片红字，其他不受影响
- [x] 图钉：`setAlwaysOnTop` 生效，图标变蓝
- [x] 打开设置按钮：`open_settings` command 触发
- [x] OCR 按钮：`trigger_ocr_translation` command 触发
- [x] 语言 combobox：搜索 / ↑↓ / Enter / Esc / 点外收
- [x] 交换：正常状态可交换；`auto` 时提示不支持
- [x] 检测徽章：`source=auto` → "检测中…" → 显示 `detectedSourceLang`
- [x] 卡片折叠/展开、"展开全文"按钮在 overflow 时出现
- [x] Token 徽章在 `usage != null` 时显示
- [x] 高度自适应：短文本紧凑、长内容超屏时上限 80% + 内部滚动
- [x] app-config 变更事件：非翻译中即时新增/删除/更新卡片；翻译中延迟到 batch 完成
- [x] `take_pending_source_text` 回填原文（划词触发场景）
- [x] Edge 环境采集 `save_edge_translate_env`
- [x] 前端日志按 logLevel 生效（`app-config:changed` 联动）

### 9.2 历史面板验证

- [x] 左列表按今天/昨天/本周/更早分桶
- [x] trigger 筛选栏渲染 5 个按钮（当前只 screenshot 有数据）
- [x] 点击左侧条目切换右侧详情
- [x] `SourceCardView` / `LanguageToolbar readonly` / `ResultCardView` 三处复用样式与弹窗完全一致
- [x] 空态提示 + 快捷键 kbd 显示（复刻现有）
- [x] 清空全部 Dialog 确认 → 清空数据

### 9.3 静态检查

- [x] `npm run typecheck` 通过（新增 popup 组件类型无 error）
- [x] `npm run test` 通过（原有 vitest 全绿 + `useTranslationEvents` 新增单测）
- [x] `cd src-tauri && cargo test` 通过（后端未动，应零影响）

### 9.4 打包验证

- [x] `npm run build` 产出 `dist/settings.html` + `dist/translate.html` 两个入口
- [ ] `npm run tauri build` 生成 NSIS 安装包，主窗口能正常加载新 `translate.html`（执行时跳过，待用户 tauri build 确认）

## 十、单元测试

- **`useTranslationEvents.test.ts`**：给定一段 `started → delta*N → finished`（或 `failed`/`cancelled`）payload 序列，验证 cards Map 最终状态、batchId 切换重置、跨 batch 陈旧事件被丢弃。
- **`HistoryPanel adaptedSessions`**（可选）：给定 `ocrHistory` 数组，验证输出的 `adaptedSessions` 形状正确、`serviceName` 兜底 `(已删除)`、`status` 根据 `translation` 空/非空判定。

现有 `frontend/src/translate-card-sync.test.js` 因对应功能被 Vue 组件替代而删除；测试职能由 `useTranslationEvents.test.ts` 承接。

## 十一、迁移顺序

1. 安装 `lucide-vue-next`：`npm i lucide-vue-next`
2. 建 `src/popup/` 目录 + `popup-tokens.css` / `index.css` / `components.css`（先只 CSS 与空 Vue 骨架，可独立预览）
3. 移植 `data/languages.ts`（从旧 `translate.js` LANGUAGES 抽取）+ `composables/utils.ts`
4. 逐个建组件：`SourceCardView` → `SourceCard` → `LanguagePicker` → `LanguageToolbar` → `ResultCardView` → `StatusBar` → `PopupToolbar`
5. 建 composable：`useTranslationEvents.ts` → `usePopupHeight.ts`
6. 组装 `TranslationPopup.vue` + `main.ts`
7. 改 `translate.html`（Vue 入口）+ `vite.config.ts`（增 input）
8. `npm run tauri dev` 端到端联调所有验证清单
9. 删旧 `public/translate.*` 四件套 + `translate-card-sync.test.js`
10. 重写 `HistoryPanel.vue` + 设置侧 `main.ts` 增 import
11. `npm run typecheck && npm run test && cd src-tauri && cargo test`
12. `npm run build` + `npm run tauri build` 验证打包产物

## 十二、风险与回滚

- **风险 1**：新 Vue 弹窗在 Tauri 真实窗口中高度自适应与旧版不一致
  - 缓解：`usePopupHeight` 严格复刻旧 `adjustHeight` 的 rAF 节流 + 相同的 setSize 参数（logical 420×h）；打包前用 chrome-devtools-mcp 对照原型交叉验证
- **风险 2**：流式渲染性能回退（大量 delta 触发 Vue 虚拟 DOM diff）
  - 缓解：`.result-text` 内部走命令式 `appendChild(document.createTextNode(chunk))`，与旧代码一致；Vue 只管容器
- **风险 3**：`translate.html` 换成 Vite 入口后 dev 与 build 路径不一致
  - 缓解：vite dev 走 `http://localhost:5173/translate.html`；tauri.conf.json 已有 `devUrl: http://localhost:5173`，需确认 Tauri main 窗口拼接 URL 为 `${devUrl}/translate.html`
- **风险 4**：`lucide-vue-next` 与 `@lucide/vue` 并存导致包体积增大
  - 缓解：本次统一图标库（可选：迁移完成后单独一次 PR 把 `@lucide/vue` 全部替换为 `lucide-vue-next` 并删除 `@lucide/vue`）
- **回滚**：删除 `src/popup/` + 恢复 `public/translate.*` 四件套 + 恢复 `translate.html` + revert `vite.config.ts` 与 `HistoryPanel.vue` 即可整体回滚，后端零影响。

## 十三、文档同步

实施完成后需同步：
- `CLAUDE.md` / `AGENTS.md`：更新「项目结构」章节，说明弹窗改为 Vue 入口 + `src/popup/` 子目录 + 组件复用接缝
- `plugins.md`：如新增 lucide-vue-next 需在依赖清单同步
- README：如已提及"弹窗使用原生 HTML"需更新为"弹窗使用 Vue + shared components"

## 十四、后续可能的扩展（本次不做）

- 翻译历史后端持久化 —— 需扩 `types.ts` 为 `HistorySession` 多渠道结构 + Rust 侧新增 command
- 弹窗内 markdown/LaTeX 渲染 —— 目前 `.result-text` 是纯文本
- 弹窗内代码块高亮 —— 与上一条一起考虑
