# 翻译弹窗启动无闪 + 动态高度与卡片收缩 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 冷启动翻译弹窗无闪显示（`visible=false` → Vue 就绪并 `setSize` 后再 `show`），并统一结果卡 `collapsed` 状态机与动态高度（空闲/等首包收缩，首可见正文或失败再展开）。

**架构：** 后端把 `main` 创建为不可见、去掉 setup 末尾无条件 `show`；前端在 `initCards` + 首帧布局 + 至少一次 `setSize`（推荐双 rAF）后调用 `getCurrentWindow().show()`，约 2s 超时强制 show。卡片折叠规则集中在 `useTranslationEvents` 的 `dispatch`：`started` 不展开，首非空 delta / failed / finished 需展示时展开；`collapseUserOverride` 保证本 batch 内用户手动折叠优先。高度继续由 `usePopupHeight` 的 ResizeObserver 驱动，并暴露「首次 setSize 完成」信号供 ready 门闩使用。

**技术栈：** Tauri 2（窗口 visible / show / setSize / capabilities）+ Vue 3.5 SFC + TypeScript + vitest 3；前端继续 `withGlobalTauri` + `window.__TAURI__`，不强制引入 `@tauri-apps/api` 包路径。

**关联文档：** spec 见 [docs/superpowers/specs/2026-07-10-popup-startup-height-collapse-design.md](../specs/2026-07-10-popup-startup-height-collapse-design.md)

**交付范围（P0，默认全部落地）：**

- 冷启动 `visible=false` + ready 后 show + 2s 超时
- 卡片收缩状态机（含 failed / finished / 多服务独立 / 新 batch 收回）
- 用户手动折叠本 batch 内优先（`collapseUserOverride`）
- show 前至少一次真实 DOM `setSize`
- 单测覆盖状态机与 ready 门闩
- `AGENTS.md` / `CLAUDE.md` 同步

**可选（P1，成本低可同 PR，不阻塞）：** `translate.html` 静态壳、占位 height 微调。

**重要约束：**

1. **不改**翻译协议 / `translation:event` 字段名 / session 格式。
2. **不改**冷启动产品默认（必须尽快完整翻译弹窗，非仅托盘、非默认设置页）。
3. Ready **不含**历史加载、OCR、模型探测等业务全量。
4. 二次唤起（`show_popup`）不重走冷启动 ready 门闩。
5. 宽度保持 420（与 `.popup` 一致）；高度以真实 DOM 为准，不用魔法数作为 show 后最终值。

---

## 文件结构

### 新建

| 文件 | 职责 |
|------|------|
| `frontend/src/popup/composables/mainWindowReady.ts` | 冷启动 show 一次门闩：ready 路径与 timeout race，`hasShown` 防重入 |
| `frontend/src/popup/composables/mainWindowReady.test.ts` | 门闩单测：先 ready 再 show、超时强制 show、二次 no-op |

### 修改

| 文件 | 改动 |
|------|------|
| `frontend/src/popup/composables/useTranslationEvents.ts` | `CardState` 增 `collapseUserOverride`；`started` 保持/设收缩；delta/failed/finished 条件展开；新 batch 清 override |
| `frontend/src/popup/composables/useTranslationEvents.test.ts` | 增补 collapsed 状态机用例 |
| `frontend/src/popup/composables/usePopupHeight.ts` | 对外返回 `whenFirstSized` Promise + 可选 `adjustNow()`；首次成功 setSize resolve |
| `frontend/src/popup/composables/utils.ts` | `TauriApis.getCurrentWindow` 增 `show` / `setFocus` |
| `frontend/src/popup/components/ResultCard.vue` | 用户 toggle 折叠时置 `collapseUserOverride = true` |
| `frontend/src/popup/TranslationPopup.vue` | 空闲新建卡默认 `collapsed: true`；集成 ready 流水线（initCards → setSize → 双 rAF → show） |
| `src-tauri/tauri.conf.json` | `main` 增 `"visible": false`；可选调低占位 `height` |
| `src-tauri/src/lib.rs` | 去掉 setup 末尾对 main 的无条件 `show` + `set_focus` |
| `src-tauri/capabilities/default.json` | 补 `core:window:allow-show`、`core:window:allow-set-focus`（前端 IPC show 需要） |
| `AGENTS.md` / `CLAUDE.md` | 启动可见性、ready show、卡片收缩规则 |

### 不改（保持）

| 文件 | 说明 |
|------|------|
| `src-tauri/src/app/popup_window.rs` | 热窗 `show_popup` 仍定位 + show + focus |
| `frontend/src/popup/components/ResultCardView.vue` | header loading 点已在收缩态 header 内渲染（`showDotFinal`），一般无需改 |

---

## 任务 1：卡片 collapsed 状态机（TDD）

**文件：**
- 修改：`frontend/src/popup/composables/useTranslationEvents.ts`
- 测试：`frontend/src/popup/composables/useTranslationEvents.test.ts`

- [ ] **步骤 1：编写失败的 collapsed 用例**

在 `useTranslationEvents.test.ts` 末尾 `describe` 内新增（保持现有 `makeHarness`）：

```ts
describe('useTranslationEvents.collapsed 状态机', () => {
  it('started 后 collapsed 仍为 true（不因 started 展开）', () => {
    const h = makeHarness()
    h.dispatch({
      type: 'started',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      serviceName: 'A',
      serviceType: 'openai',
    })
    expect(h.cards.get('svc-a')!.collapsed).toBe(true)
    expect(h.cards.get('svc-a')!.status).toBe('translating')
  })

  it('首条非空 delta 后该卡 collapsed=false', () => {
    const h = makeHarness()
    h.dispatch({
      type: 'started',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      serviceName: 'A',
      serviceType: 'openai',
    })
    h.dispatch({
      type: 'delta',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      text: 'Hel',
    })
    expect(h.cards.get('svc-a')!.collapsed).toBe(false)
    expect(h.cards.get('svc-a')!.text).toBe('Hel')
  })

  it('空 delta 不展开', () => {
    const h = makeHarness()
    h.dispatch({
      type: 'started',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      serviceName: 'A',
      serviceType: 'openai',
    })
    h.dispatch({
      type: 'delta',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      text: '',
    })
    expect(h.cards.get('svc-a')!.collapsed).toBe(true)
  })

  it('failed 无正文也展开', () => {
    const h = makeHarness()
    h.dispatch({
      type: 'started',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      serviceName: 'A',
      serviceType: 'openai',
    })
    h.dispatch({
      type: 'failed',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      message: '网络错误',
    })
    expect(h.cards.get('svc-a')!.collapsed).toBe(false)
    expect(h.cards.get('svc-a')!.status).toBe('failed')
  })

  it('仅 finished（无中间 delta）展开', () => {
    const h = makeHarness()
    h.dispatch({
      type: 'started',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      serviceName: 'A',
      serviceType: 'openai',
    })
    h.dispatch({
      type: 'finished',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      fullText: '完整译文',
    })
    expect(h.cards.get('svc-a')!.collapsed).toBe(false)
    expect(h.cards.get('svc-a')!.text).toBe('完整译文')
  })

  it('多服务：A 出字只展开 A，B 仍收缩', () => {
    const h = makeHarness()
    h.dispatch({
      type: 'started',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      serviceName: 'A',
      serviceType: 'openai',
    })
    h.dispatch({
      type: 'started',
      sessionId: 'batch-1:svc-b',
      serviceInstanceId: 'svc-b',
      serviceName: 'B',
      serviceType: 'claude',
    })
    h.dispatch({
      type: 'delta',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      text: '仅 A',
    })
    expect(h.cards.get('svc-a')!.collapsed).toBe(false)
    expect(h.cards.get('svc-b')!.collapsed).toBe(true)
  })

  it('新 batch 先收回再各自等首包', () => {
    const h = makeHarness()
    h.dispatch({
      type: 'started',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      serviceName: 'A',
      serviceType: 'openai',
    })
    h.dispatch({
      type: 'delta',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      text: '旧',
    })
    expect(h.cards.get('svc-a')!.collapsed).toBe(false)
    h.dispatch({
      type: 'started',
      sessionId: 'batch-2:svc-a',
      serviceInstanceId: 'svc-a',
      serviceName: 'A',
      serviceType: 'openai',
    })
    expect(h.cards.get('svc-a')!.text).toBe('')
    expect(h.cards.get('svc-a')!.collapsed).toBe(true)
  })
})
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
npm run test -- frontend/src/popup/composables/useTranslationEvents.test.ts
```

预期：至少一个 collapsed 相关断言 FAIL（当前 `started` 会 `card.collapsed = false`）。

- [ ] **步骤 3：实现最少状态机改动**

在 `useTranslationEvents.ts`：

1. `CardState` 增加字段（任务 2 会用到，本任务可先加默认值）：

```ts
export interface CardState {
  // ...existing fields...
  collapsed: boolean
  /** 用户在本 batch 内手动改过折叠；true 时自动规则不改 collapsed，新 batch 清除 */
  collapseUserOverride: boolean
  // ...
}
```

2. `ensureCard` 默认：

```ts
collapsed: true,
collapseUserOverride: false,
```

3. `resetCardForNewBatch` 增加：

```ts
card.collapsed = true
card.collapseUserOverride = false
```

4. `started` 分支：把 `card.collapsed = false` 改为：

```ts
if (!card.collapseUserOverride) {
  card.collapsed = true
}
```

（新 batch 已在 `resetCardForNewBatch` 清 override 并设 true；同 batch 第二服务新建卡也是 true。）

5. `delta` 分支：追加 text 后：

```ts
const wasEmpty = card.text.length === 0
card.text += payload.text ?? ''
if (
  !card.collapseUserOverride &&
  wasEmpty &&
  card.text.length > 0
) {
  card.collapsed = false
}
```

注意：当前代码是先 `card.text += ...`，需改为「追加前判断 wasEmpty」，或用追加前后长度对比：

```ts
const prevLen = card.text.length
card.text += payload.text ?? ''
if (!card.collapseUserOverride && prevLen === 0 && card.text.length > 0) {
  card.collapsed = false
}
```

6. `failed` 分支：在设 status/text 后：

```ts
if (!card.collapseUserOverride) {
  card.collapsed = false
}
```

7. `finished` 分支：在写入 fullText 后：

```ts
const needsShow =
  (card.text?.trim().length ?? 0) > 0 ||
  (payload.fullText?.trim().length ?? 0) > 0
// fullText 已赋给 card.text 后：
if (!card.collapseUserOverride && card.text.trim().length > 0) {
  card.collapsed = false
}
```

（「需展示结果」：有非空正文；空 finished 可保持收缩，与 spec「需展示结果」一致。）

- [ ] **步骤 4：运行测试验证通过**

```bash
npm run test -- frontend/src/popup/composables/useTranslationEvents.test.ts
```

预期：全部 PASS（含原有用例；原用例未断言 collapsed，不应被破坏）。

- [ ] **步骤 5：Commit**

```bash
git add frontend/src/popup/composables/useTranslationEvents.ts frontend/src/popup/composables/useTranslationEvents.test.ts
git commit -m "feat(popup): 结果卡 started 不展开，首包/失败/完成再展开"
```

---

## 任务 2：用户手动折叠 override（本 batch 优先）

**文件：**
- 修改：`frontend/src/popup/composables/useTranslationEvents.ts`（若任务 1 已加字段则主要改测试 + ResultCard）
- 修改：`frontend/src/popup/components/ResultCard.vue`
- 测试：`frontend/src/popup/composables/useTranslationEvents.test.ts`

- [ ] **步骤 1：编写失败的 override 用例**

```ts
it('用户 override 后首 delta 不自动展开', () => {
  const h = makeHarness()
  h.dispatch({
    type: 'started',
    sessionId: 'batch-1:svc-a',
    serviceInstanceId: 'svc-a',
    serviceName: 'A',
    serviceType: 'openai',
  })
  const card = h.cards.get('svc-a')!
  card.collapseUserOverride = true
  card.collapsed = true
  h.dispatch({
    type: 'delta',
    sessionId: 'batch-1:svc-a',
    serviceInstanceId: 'svc-a',
    text: 'Hello',
  })
  expect(card.collapsed).toBe(true)
  expect(card.text).toBe('Hello')
})

it('新 batch 清除 override 并恢复默认收缩', () => {
  const h = makeHarness()
  h.dispatch({
    type: 'started',
    sessionId: 'batch-1:svc-a',
    serviceInstanceId: 'svc-a',
    serviceName: 'A',
    serviceType: 'openai',
  })
  const card = h.cards.get('svc-a')!
  card.collapseUserOverride = true
  card.collapsed = true
  h.dispatch({
    type: 'started',
    sessionId: 'batch-2:svc-a',
    serviceInstanceId: 'svc-a',
    serviceName: 'A',
    serviceType: 'openai',
  })
  expect(card.collapseUserOverride).toBe(false)
  expect(card.collapsed).toBe(true)
})
```

- [ ] **步骤 2：运行确认 FAIL 或确认实现已覆盖**

```bash
npm run test -- frontend/src/popup/composables/useTranslationEvents.test.ts
```

若任务 1 已实现 `collapseUserOverride` 检查，本步应 PASS；若 FAIL 则补齐 `!card.collapseUserOverride` 守卫。

- [ ] **步骤 3：ResultCard 用户 toggle 写 override**

`ResultCard.vue` 中：

```ts
const onToggleCollapse = (): void => {
  props.card.collapsed = !props.card.collapsed
  props.card.collapseUserOverride = true
}
```

- [ ] **步骤 4：TranslationPopup 新建空闲卡默认字段**

`refreshCardsFromConfig` 新建卡时：

```ts
collapsed: true, // 空闲默认收缩；有原文且需展示历史结果时再按业务调（本规格：无 trim 原文 → 收缩）
collapseUserOverride: false,
```

原 `collapsed: !sourceText.value.trim()` 改为始终 `true` 更符合「空闲紧凑」；若有原文但尚未翻译、卡仍 pending，规格要求收缩，故用 `true`。

清空原文路径已有 `cards.forEach((c) => { c.collapsed = true })`，可顺带 `c.collapseUserOverride = false`（可选，清空视为系统重置）。

- [ ] **步骤 5：跑测试 + Commit**

```bash
npm run test -- frontend/src/popup/composables/useTranslationEvents.test.ts
```

```bash
git add frontend/src/popup/composables/useTranslationEvents.ts frontend/src/popup/composables/useTranslationEvents.test.ts frontend/src/popup/components/ResultCard.vue frontend/src/popup/TranslationPopup.vue
git commit -m "feat(popup): 本 batch 内尊重用户手动折叠"
```

---

## 任务 3：Tauri 权限 + 窗口类型扩展

**文件：**
- 修改：`src-tauri/capabilities/default.json`
- 修改：`frontend/src/popup/composables/utils.ts`

- [ ] **步骤 1：补 capabilities**

`default.json` 的 `permissions` 数组在现有 window 权限旁增加：

```json
"core:window:allow-show",
"core:window:allow-set-focus"
```

完整相关片段示例：

```json
"core:window:allow-set-always-on-top",
"core:window:allow-set-size",
"core:window:allow-hide",
"core:window:allow-show",
"core:window:allow-set-focus",
```

说明：Rust 直接 `window.show()` 不走 IPC；前端 `getCurrentWindow().show()` 需要上述权限。注释见 `src-tauri/src/ui/overlay.rs`。

- [ ] **步骤 2：扩展 `TauriApis`**

`utils.ts`：

```ts
export interface TauriApis {
  invoke: <T = unknown>(cmd: string, args?: Record<string, unknown>) => Promise<T>
  listen: <T = unknown>(event: string, handler: (event: { payload: T }) => void) => Promise<UnlistenFn>
  getCurrentWindow: () => {
    setAlwaysOnTop: (top: boolean) => Promise<void>
    setSize: (size: LogicalSize) => Promise<void>
    show: () => Promise<void>
    setFocus: () => Promise<void>
  }
}
```

`getTauriApis` 无需改探测逻辑（仍检查 `getCurrentWindow` 存在即可；缺方法时调用方 catch）。

- [ ] **步骤 3：Commit**

```bash
git add src-tauri/capabilities/default.json frontend/src/popup/composables/utils.ts
git commit -m "chore(tauri): 授权前端 show/setFocus 并扩展 TauriApis 类型"
```

---

## 任务 4：后端冷启动不可见 + 去掉 setup show

**文件：**
- 修改：`src-tauri/tauri.conf.json`
- 修改：`src-tauri/src/lib.rs`

- [ ] **步骤 1：main 窗口 `visible: false`**

`tauri.conf.json` 的 `main` 窗口对象：

```json
{
  "label": "main",
  "url": "translate.html",
  "title": "Shizi - 翻译助手",
  "width": 420,
  "height": 360,
  "visible": false,
  "resizable": false,
  "decorations": false,
  "transparent": true,
  "skipTaskbar": true,
  "center": true
}
```

`height: 360` 为更接近空闲壳的占位（非最终权威高度）；若后续实测更贴合可再调。**不要**依赖该值作为 show 后最终高度。

- [ ] **步骤 2：删除 setup 末尾无条件 show**

`lib.rs` setup 中删除：

```rust
if let Some(window) = app.get_webview_window("main") {
    let _ = window.show();
    let _ = window.set_focus();
}
```

保留 `ensure_popup_window` / tray / shortcuts / overlay 预创建。热路径 `popup_window::show_popup` **不要**改。

- [ ] **步骤 3：编译检查**

```bash
cd src-tauri && cargo check
```

预期：通过（仅删代码/改 conf）。

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/tauri.conf.json src-tauri/src/lib.rs
git commit -m "fix(app): 冷启动 main 不可见并移除 setup 立即 show"
```

---

## 任务 5：usePopupHeight 暴露首次 setSize 完成信号

**文件：**
- 修改：`frontend/src/popup/composables/usePopupHeight.ts`
- 修改：`frontend/src/popup/TranslationPopup.vue`（调用签名变更，本任务先改 composable；集成可在任务 7）

- [ ] **步骤 1：改返回类型与首次 resolve**

将 `usePopupHeight` 改为返回：

```ts
export interface UsePopupHeightReturn {
  /** 至少完成一次基于真实 offsetHeight 的 setSize（或无 Tauri 时 resolve） */
  whenFirstSized: Promise<void>
  /** 立即测高并 setSize（绕过仅 height 未变的短路时仍更新 lastHeight） */
  adjustNow: () => Promise<void>
}

export function usePopupHeight(popupRef: Ref<HTMLElement | null>): UsePopupHeightReturn {
  let resizeRaf: number | null = null
  let lastHeight = 0
  let observer: ResizeObserver | null = null
  let firstSizedResolved = false
  let resolveFirstSized: () => void = () => {}
  const whenFirstSized = new Promise<void>((resolve) => {
    resolveFirstSized = resolve
  })

  const applySize = async (h: number): Promise<void> => {
    const apis = getTauriApis()
    if (apis) {
      try {
        await apis.getCurrentWindow().setSize({ type: 'Logical', width: 420, height: h })
      } catch {
        /* best-effort */
      }
    }
    if (!firstSizedResolved) {
      firstSizedResolved = true
      resolveFirstSized()
    }
  }

  const measureAndApply = async (): Promise<void> => {
    const el = popupRef.value
    if (!el) {
      // 无 DOM 时：非 Tauri/测试环境直接放行，避免永远不 resolve
      if (!getTauriApis() && !firstSizedResolved) {
        firstSizedResolved = true
        resolveFirstSized()
      }
      return
    }
    const h = el.offsetHeight
    if (h === lastHeight && firstSizedResolved) return
    lastHeight = h
    await applySize(h)
  }

  const adjust = (): void => {
    if (resizeRaf !== null) cancelAnimationFrame(resizeRaf)
    resizeRaf = requestAnimationFrame(() => {
      void measureAndApply()
    })
  }

  const adjustNow = (): Promise<void> => measureAndApply()

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
    if (typeof document !== 'undefined' && document.fonts) {
      document.fonts.ready.then(adjust).catch(() => {})
    }
    // 无 Tauri 时 onMounted 也 resolve，避免测试挂死
    if (!getTauriApis()) {
      void measureAndApply()
    }
  })

  onBeforeUnmount(() => {
    if (resizeRaf !== null) cancelAnimationFrame(resizeRaf)
    observer?.disconnect()
    observer = null
  })

  return { whenFirstSized, adjustNow }
}
```

要点：

- 宽度固定 `420`（与现网一致，spec 明确）。
- 首次 setSize 无论 height 是否与 lastHeight 相同都要能 resolve（`adjustNow` 在 init 后强制一次）。
- 浏览器/单测无 Tauri 时不阻塞。

- [ ] **步骤 2：临时适配调用方**

`TranslationPopup.vue`：

```ts
const { whenFirstSized, adjustNow } = usePopupHeight(popupRef)
// whenFirstSized / adjustNow 在任务 7 接入 ready；此处先解构避免 unused 可用 void 或下划线前缀
```

若 lint 对未使用变量报错，可先写：

```ts
const popupHeight = usePopupHeight(popupRef)
// 任务 7 使用 popupHeight.whenFirstSized / popupHeight.adjustNow
void popupHeight
```

更干净：本任务与任务 7 合并提交也可；若分 commit，允许本 commit 仅改 composable 签名，下一任务立刻接入。

- [ ] **步骤 3：Commit**

```bash
git add frontend/src/popup/composables/usePopupHeight.ts frontend/src/popup/TranslationPopup.vue
git commit -m "refactor(popup): usePopupHeight 暴露首次 setSize 完成信号"
```

---

## 任务 6：mainWindowReady 门闩（TDD）

**文件：**
- 创建：`frontend/src/popup/composables/mainWindowReady.ts`
- 创建：`frontend/src/popup/composables/mainWindowReady.test.ts`

- [ ] **步骤 1：编写失败测试**

`mainWindowReady.test.ts`：

```ts
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createMainWindowReadyGate } from './mainWindowReady'

describe('createMainWindowReadyGate', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })
  afterEach(() => {
    vi.useRealTimers()
  })

  it('ready 路径只 show 一次', async () => {
    const show = vi.fn(async () => {})
    const warn = vi.fn()
    const gate = createMainWindowReadyGate({
      timeoutMs: 2000,
      show,
      onTimeoutWarn: warn,
    })
    await gate.notifyReady()
    await gate.notifyReady()
    expect(show).toHaveBeenCalledTimes(1)
    expect(warn).not.toHaveBeenCalled()
  })

  it('超时强制 show 并 warn', async () => {
    const show = vi.fn(async () => {})
    const warn = vi.fn()
    createMainWindowReadyGate({
      timeoutMs: 2000,
      show,
      onTimeoutWarn: warn,
    })
    await vi.advanceTimersByTimeAsync(2000)
    expect(show).toHaveBeenCalledTimes(1)
    expect(warn).toHaveBeenCalled()
  })

  it('ready 已 show 后超时 no-op', async () => {
    const show = vi.fn(async () => {})
    const warn = vi.fn()
    const gate = createMainWindowReadyGate({
      timeoutMs: 2000,
      show,
      onTimeoutWarn: warn,
    })
    await gate.notifyReady()
    await vi.advanceTimersByTimeAsync(2000)
    expect(show).toHaveBeenCalledTimes(1)
    expect(warn).not.toHaveBeenCalled()
  })

  it('超时后 ready 不再二次 show', async () => {
    const show = vi.fn(async () => {})
    const warn = vi.fn()
    const gate = createMainWindowReadyGate({
      timeoutMs: 2000,
      show,
      onTimeoutWarn: warn,
    })
    await vi.advanceTimersByTimeAsync(2000)
    await gate.notifyReady()
    expect(show).toHaveBeenCalledTimes(1)
  })
})
```

- [ ] **步骤 2：运行验证 FAIL**

```bash
npm run test -- frontend/src/popup/composables/mainWindowReady.test.ts
```

预期：模块不存在或 `createMainWindowReadyGate` undefined → FAIL。

- [ ] **步骤 3：实现 `mainWindowReady.ts`**

```ts
export interface MainWindowReadyOptions {
  timeoutMs: number
  show: () => Promise<void>
  onTimeoutWarn: (message: string) => void
}

export interface MainWindowReadyGate {
  /** UI 就绪后调用；与超时 race，先到者 show，其后 no-op */
  notifyReady: () => Promise<void>
  /** 是否已 show（含超时路径） */
  hasShown: () => boolean
  /** 取消超时定时器（组件卸载） */
  dispose: () => void
}

export function createMainWindowReadyGate(
  opts: MainWindowReadyOptions,
): MainWindowReadyGate {
  let shown = false
  let timer: ReturnType<typeof setTimeout> | null = null

  const doShow = async (fromTimeout: boolean): Promise<void> => {
    if (shown) return
    shown = true
    if (timer !== null) {
      clearTimeout(timer)
      timer = null
    }
    if (fromTimeout) {
      opts.onTimeoutWarn(
        `翻译弹窗 ready 超时（${opts.timeoutMs}ms），强制 show`,
      )
    }
    try {
      await opts.show()
    } catch {
      /* best-effort：show 失败不抛到调用方，避免阻塞 */
    }
  }

  timer = setTimeout(() => {
    void doShow(true)
  }, opts.timeoutMs)

  return {
    notifyReady: () => doShow(false),
    hasShown: () => shown,
    dispose: () => {
      if (timer !== null) {
        clearTimeout(timer)
        timer = null
      }
    },
  }
}

/** 双 rAF：等布局 + paint 一帧（best-effort） */
export function doubleRaf(): Promise<void> {
  return new Promise((resolve) => {
    if (typeof requestAnimationFrame !== 'function') {
      resolve()
      return
    }
    requestAnimationFrame(() => {
      requestAnimationFrame(() => resolve())
    })
  })
}
```

- [ ] **步骤 4：运行验证 PASS**

```bash
npm run test -- frontend/src/popup/composables/mainWindowReady.test.ts
```

- [ ] **步骤 5：Commit**

```bash
git add frontend/src/popup/composables/mainWindowReady.ts frontend/src/popup/composables/mainWindowReady.test.ts
git commit -m "feat(popup): 冷启动 show 一次门闩与 2s 超时"
```

---

## 任务 7：TranslationPopup 集成 ready → show 流水线

**文件：**
- 修改：`frontend/src/popup/TranslationPopup.vue`

- [ ] **步骤 1：import 与创建 gate**

在 script 顶部增加：

```ts
import { nextTick, onBeforeUnmount, onMounted, reactive, ref } from 'vue'
// ...
import { usePopupHeight } from './composables/usePopupHeight'
import {
  createMainWindowReadyGate,
  doubleRaf,
} from './composables/mainWindowReady'
```

在 `popupRef` / 状态定义后：

```ts
const popupHeight = usePopupHeight(popupRef)

const showMainWindow = async (): Promise<void> => {
  const apis = getTauriApis()
  if (!apis) return
  const win = apis.getCurrentWindow()
  await win.show()
  await win.setFocus()
}

const readyGate = createMainWindowReadyGate({
  timeoutMs: 2000,
  show: showMainWindow,
  onTimeoutWarn: (msg) => logger.warn(msg),
})

onBeforeUnmount(() => {
  events.unlisten()
  readyGate.dispose()
})
```

注意：现有 `onBeforeUnmount` 只有 `events.unlisten()`，合并 dispose。

- [ ] **步骤 2：实现 `runColdStartReady`**

```ts
const runColdStartReady = async (): Promise<void> => {
  try {
    await initCards()
    await nextTick()
    await popupHeight.adjustNow()
    await popupHeight.whenFirstSized
    await doubleRaf()
  } catch (e) {
    logger.warn('冷启动 ready 流水线异常，仍尝试 show', String(e))
  } finally {
    await readyGate.notifyReady()
  }
}
```

`initCards` 已是 async；保持其内部 try/catch，失败时仍走 finally show（与超时兜底一致）。

- [ ] **步骤 3：onMounted 调度**

```ts
onMounted(() => {
  charCount.value = sourceText.value.length
  void runColdStartReady()
  void collectEdgeTranslateEnv()
  void applyPendingSourceText()
  window.addEventListener('focus', () => {
    void applyPendingSourceText()
  })
})
```

**不要**再单独 `void initCards()`（已并入 `runColdStartReady`）。

- [ ] **步骤 4：确认空闲卡默认收缩**

`refreshCardsFromConfig` 新建卡：

```ts
collapsed: true,
collapseUserOverride: false,
```

- [ ] **步骤 5：类型检查**

```bash
npm run typecheck
npm run test
```

预期：typecheck 与 vitest 全绿。

- [ ] **步骤 6：Commit**

```bash
git add frontend/src/popup/TranslationPopup.vue
git commit -m "feat(popup): 冷启动 UI 就绪并 setSize 后再 show"
```

---

## 任务 8：文档同步（AGENTS.md / CLAUDE.md）

**文件：**
- 修改：`AGENTS.md`
- 修改：`CLAUDE.md`（内容与 AGENTS 对应段落保持同步）

- [ ] **步骤 1：更新「启动窗口与设置窗口」或「翻译弹窗窗口」段落**

在「启动窗口与设置窗口」相关 bullet 补充：

- `main` 在 `tauri.conf.json` 中 `visible: false`；setup **不**再立即 `show`。
- 冷启动由前端 `TranslationPopup`：`initCards` → 至少一次 `setSize` → 双 rAF → `show` + `setFocus`；约 2s 超时强制 show。
- 二次唤起仍走 `show_popup`（热窗，不等 ready）。

在「翻译弹窗」或卡片相关 bullet 补充：

- 结果卡默认 `collapsed`；`started` 不展开；首非空正文 / failed / finished 需展示时展开；用户手动折叠本 batch 内优先（`collapseUserOverride`）。
- 高度随 N 张收缩/展开卡由 `usePopupHeight` 实测 DOM，非 conf 常量。

权限 bullet 补充 `core:window:allow-show` / `allow-set-focus`。

两文件同步修改，避免漂移。

- [ ] **步骤 2：Commit**

```bash
git add AGENTS.md CLAUDE.md
git commit -m "docs: 同步弹窗冷启动无闪与卡片收缩规则"
```

---

## 任务 9（可选 P1）：静态首屏壳

**文件：**
- 修改：`frontend/translate.html`（及若需 `frontend/src/popup/index.css`）

**仅当工期允许时做；跳过不影响 P0 验收。**

- [ ] **步骤 1：在 `#app` 内放与弹窗同宽的轻量壳**

例如 `translate.html`：

```html
<div id="app">
  <div class="popup-boot-shell" aria-hidden="true"></div>
</div>
```

CSS（可放 `index.css` 或 inline）：宽 420px、圆角、与 `--popup-bg-popup` 接近的背景、最小高度约一屏空闲壳；Vue `createApp(...).mount('#app')` 后整树替换，壳消失。

- [ ] **步骤 2：手动看一眼 dev 冷启动是否更少「空洞」**

- [ ] **步骤 3：Commit（若做了）**

```bash
git add frontend/translate.html frontend/src/popup/index.css
git commit -m "feat(popup): 可选静态首屏壳降低冷启动空窗感"
```

---

## 手动验收清单（实现后执行者勾选）

在 Windows 上优先 release 体感（`npm run tauri build` 或等价）；dev 允许更慢但不得永不 show。

| # | 场景 | 期望 |
|---|------|------|
| 1 | 冷启动 | 托盘出现后，弹窗首次出现即为完整 UI + 稳定尺寸；无大→小、无明显白屏 |
| 2 | 启用 1 / 3 / 5 服务分别冷启动 | 高度随 N 变，首次出现即正确 |
| 3 | 空闲 → 翻译至首包前 | 卡保持收缩；header loading 点可见 |
| 4 | 首包 delta | 对应卡展开，窗高合理增加 |
| 5 | 故意失败服务 | 无正文也可见错误（卡展开） |
| 6 | 手动折叠某卡后同 batch 再 delta | 不强制自动展开 |
| 7 | hide 后再 Alt+D | 热窗快速显示，无冷启动级闪动 |
| 8 | 模拟 init 极慢（可临时加大延时） | ≤2s 仍会 show |

自动化已覆盖：任务 1/2/6 的 vitest；任务 7 的 typecheck。

---

## 自检（对照 spec）

| Spec 需求 | 任务 |
|-----------|------|
| visible=false + 去 setup show | 任务 4 |
| Ready：挂载 + N 张收缩卡 + setSize + 双 rAF | 任务 5、7 |
| 2s 超时强制 show | 任务 6、7 |
| started 不展开；首正文/failed/finished 展开 | 任务 1 |
| 多服务独立；新 batch 收回 | 任务 1 |
| 用户手动本 batch 优先 | 任务 2 |
| show 前 setSize；高度跟 DOM | 任务 5、7 |
| capabilities show/focus | 任务 3 |
| 热窗 show_popup 不改 ready | 任务 4（不改 popup_window） |
| 单测状态机 | 任务 1、2 |
| 文档 AGENTS/CLAUDE | 任务 8 |
| P1 静态壳 | 任务 9 可选 |
| 不改协议/默认托盘/设置页 | 全程遵守 |

**占位符扫描：** 无 TODO/TBD 步骤；测试与实现均含具体代码。

**类型一致性：**

- `CardState.collapseUserOverride: boolean`
- `UsePopupHeightReturn.{ whenFirstSized, adjustNow }`
- `createMainWindowReadyGate` / `doubleRaf`
- `TauriApis.getCurrentWindow().show|setFocus`

---

## 风险提醒（执行时）

1. **永远不 show：** ready 流水线 catch 后必须 `notifyReady`；超时 gate 并行。
2. **权限遗漏：** 前端 show 失败 → 检查 capabilities 是否已加 `allow-show` / `allow-set-focus`。
3. **whenFirstSized 永不 resolve：** 无 DOM / 无 Tauri 时 measure 路径必须放行（任务 5）。
4. **旧测试：** 原 `started` 用例未断言 collapsed；若有 UI 依赖「started 即展开」的假设，以本 spec 为准改掉。
