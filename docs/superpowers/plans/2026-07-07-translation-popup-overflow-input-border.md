# 翻译弹窗 UI 打磨：卡片截断 / 输入框限高 / 上边框 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [x]`）语法来跟踪进度。

**目标：** 对齐 OpenDesign 原型（commit `2e9355e` / `d5d6335`），打磨翻译弹窗三处 UI 不一致——结果卡片长内容截断 + 展开全文、输入原文最大高度限高、focus 态上边框描边粗细一致。

**架构：** 纯前端改动，仅触达 `frontend/public/translate.css` 与 `frontend/public/translate.js` 两个静态文件（Tauri 直接当作静态资源加载，无 Vite 打包）。不碰后端、overlay、设置页、translate.html。三个问题互相独立，按 spec 顺序拆为三个原子 commit，最后一个 commit 同步 README。

**技术栈：** 原生 JS（无框架）+ CSS 变量 + Tauri 2 事件桥。`:root` font-size 16px；`.result-text` font-size 13px / line-height 1.6；`.source-input` font-size 13px / line-height 1.55。

**测试策略：** 本模块与 overlay 一致，**无自动化单测**（spec 第 7 节）。引入 jsdom 测 DOM 函数违反 YAGNI，项目无此基建。验证方式为 `npm run tauri dev` 手动验证清单（spec 第 7 节 8 项，按任务分配）。每个任务末尾设一个验证步骤，不写 TDD 红绿循环。

**验证清单（来自 spec 第 7 节）：**
1. 长文本翻译结果默认截断（约 4-5 行）+ 底部渐隐遮罩 + 出现「展开全文」按钮。
2. 点「展开全文」→ 完整显示，按钮变「收起」，chevron 旋转 180°；点「收起」回滚。
3. 展开按钮点击不触发卡片折叠（stopPropagation）。
4. 再次翻译（新 batch）→ 展开状态重置，旧卡片复用更新。
5. 输入超长原文 → 输入框到 `max-height` 10.85em 后内部滚动，窗口不再被撑高；滚动条为 4px 细条。
6. 短原文 → 输入框不滚动，无展开按钮。
7. focus 输入框 → 四边蓝描边粗细一致（上边不再更细）。
8. 取消 `border-radius` 圆角处 outline 无直角瑕疵。

---

## 文件结构

- 修改：`frontend/public/translate.css`
  - 任务 1：末尾追加 `.result-text-clip` / `.has-overflow` / `.expanded` / `.result-expand-btn` / chevron 样式。
  - 任务 2：`.source-input` 加 `max-height` / `overflow-y` / 滚动条样式。
  - 任务 3：`.source-card:focus-within` 描边从 `box-shadow` 改 `outline`。
- 修改：`frontend/public/translate.js`
  - 任务 1：新增 `detectOverflow` / `updateExpandButton` / `toggleExpand`；`getCard` innerHTML 包 `.result-text-clip` + 展开按钮 + 绑定；`finished` 后探测溢出；`started` 新 batch 重置展开态。
  - 任务 2：`autoResize` 限高 + 动态 `overflow-y`；初始化补 `requestAnimationFrame` / `document.fonts.ready` 触发。
- 修改：`README.md`
  - 任务 4：核心能力列表补充截断与限高两条。

---

## 任务 1：翻译结果卡片长内容截断 + 展开全文

**文件：**
- 修改：`frontend/public/translate.css`（末尾 L448 后追加）
- 修改：`frontend/public/translate.js`（`getCard` L151-183、新增函数 L220 后、`getCard` 绑定 L210 后、`finished` L337 后、`started` L294 后）

- [x] **步骤 1：在 translate.css 末尾追加截断 / 展开样式**

在 `frontend/public/translate.css` 文件末尾（当前最后一行是 L448 的空行）追加以下完整块：

```css
/* === 结果卡片长内容截断 + 展开全文 === */
.result-text-clip {
  position: relative;
  max-height: 6.4em;             /* 原型值；继承 :root 16px，约 4.9 行 */
  overflow: hidden;
  transition: max-height .3s ease;
}
.result-card.expanded .result-text-clip {
  max-height: 80em;
}
.result-card.has-overflow:not(.expanded) .result-text-clip::after {
  content: '';
  position: absolute;
  left: 0; right: 0; bottom: 0;
  height: 28px;
  background: linear-gradient(to bottom, rgba(255,255,255,0), var(--bg-card));
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
  font-family: var(--font-family);
  font-size: 0.6875rem;          /* 11px */
  color: var(--fg-2);
  cursor: default;
  border-radius: 4px;
  line-height: 1;
  transition: color .15s, background .15s;
  user-select: none;
}
.result-card.has-overflow .result-expand-btn { display: inline-flex; }
.result-expand-btn:hover {
  color: var(--accent);
  background: rgba(28,25,23,0.04);
}
.result-expand-btn:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 1px;
}
.result-expand-chevron {
  width: 10px;
  height: 10px;
  transition: transform .25s ease;
}
.result-card.expanded .result-expand-chevron {
  transform: rotate(180deg);
}
```

- [x] **步骤 2：在 translate.js 新增三个截断/展开函数**

在 `frontend/public/translate.js` 的 `getCard` 函数结束后（当前 L220 `}` 之后，L222 `/* === 流式光标 === */` 之前）插入以下完整块。函数接收 DOM 元素（`cardEl`），不依赖 ref 对象结构，便于在 `getCard` 内部 `ref` 创建之前绑定事件：

```js
/* === 结果卡片截断 / 展开 === */
function detectOverflow(cardEl) {
  const clip = cardEl.querySelector('.result-text-clip');
  const text = cardEl.querySelector('.result-text');
  if (!clip || !text) return false;
  return text.scrollHeight > clip.clientHeight + 1;
}

function updateExpandButton(cardEl) {
  const label = cardEl.querySelector('.result-expand-label');
  if (detectOverflow(cardEl)) {
    cardEl.classList.add('has-overflow');
  } else {
    cardEl.classList.remove('has-overflow', 'expanded');
    if (label) label.textContent = '展开全文';
  }
}

function toggleExpand(cardEl) {
  const label = cardEl.querySelector('.result-expand-label');
  const expanded = cardEl.classList.toggle('expanded');
  if (label) label.textContent = expanded ? '收起' : '展开全文';
  adjustHeight();
}
```

- [x] **步骤 3：改 getCard innerHTML，包 .result-text-clip 容器 + 插入展开按钮**

在 `frontend/public/translate.js` 的 `getCard` 函数内，把当前 L151-183 的 `card.innerHTML = [...]` 数组**整体替换**为下面这段。变更点：`<div class="result-text"></div>` 外包一层 `.result-text-clip`，并在 `.result-text` 之后、`.result-actions` 之前插入 `.result-expand-btn` 按钮（`tabindex="-1"` 不参与 Tab 焦点）：

```js
  card.innerHTML = [
    '<div class="result-card-header">',
    '  <svg class="result-engine-icon" viewBox="0 0 20 20"></svg>',
    '  <span class="result-engine-name">' + displayName + '</span>',
    '  <button class="result-collapse-btn" title="折叠">',
    '    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>',
    '  </button>',
    '</div>',
    '<div class="result-card-body">',
    '  <div class="result-card-body-inner">',
    '    <div class="result-text-clip">',
    '      <div class="result-text"></div>',
    '    </div>',
    '    <button class="result-expand-btn" type="button" tabindex="-1">',
    '      <span class="result-expand-label">展开全文</span>',
    '      <svg class="result-expand-chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>',
    '    </button>',
    '    <div class="result-actions" style="visibility:hidden">',
    '      <button class="result-action-btn speak-btn" title="朗读翻译">',
    '        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><path d="M19.07 4.93a10 10 0 010 14.14M15.54 8.46a5 5 0 010 7.07"/></svg>',
    '      </button>',
    '      <button class="result-action-btn copy-btn" title="复制翻译">',
    '        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/></svg>',
    '      </button>',
    '      <span class="result-tokens" style="display:none">',
    '        <span class="tok tok-input">',
    '          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="19" x2="12" y2="5"/><polyline points="5 12 12 5 19 12"/></svg>',
    '          <span class="tok-value">0</span>',
    '        </span>',
    '        <span class="tok-sep"></span>',
    '        <span class="tok tok-output">',
    '          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19"/><polyline points="19 12 12 19 5 12"/></svg>',
    '          <span class="tok-value">0</span>',
    '        </span>',
    '      </span>',
    '    </div>',
    '  </div>',
    '</div>',
  ].join('\n');
```

- [x] **步骤 4：在 getCard 内绑定展开按钮点击事件**

在 `frontend/public/translate.js` 的 `getCard` 函数内，找到 `speakBtn` 绑定（当前 L212-213）：

```js
  const speakBtn = card.querySelector('.speak-btn');
  speakBtn.addEventListener('click', () => speakText(text.textContent, 'zh-CN'));
```

在其**之后**（`resultsList.appendChild(card);` L215 之前）追加展开按钮绑定。`e.stopPropagation()` 防止点击展开按钮触发 header 的折叠切换（清单第 3 项）；`card` 是 getCard 内的 DOM 元素（L144 定义），直接传给 `toggleExpand`：

```js
  const expandBtn = card.querySelector('.result-expand-btn');
  expandBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    toggleExpand(card);
  });
```

- [x] **步骤 5：finished 事件后探测溢出**

在 `frontend/public/translate.js` 的 `renderTranslationEvent` 的 `finished` 分支（L323-342），找到 `card.actions.style.visibility = 'visible';`（当前 L337），在其**之后**、`card.status = 'finished';`（L338）之前插入一行。`card` 此处是 ref 对象（L325 `resultCards.get`），传 `card.el`（DOM 元素）给 `updateExpandButton`：

```js
      updateExpandButton(card.el);
```

插入后该段应为：

```js
      card.actions.style.visibility = 'visible';
      updateExpandButton(card.el);
      card.status = 'finished';
      scrollToBottom(card);
      updateBatchStatus();
```

- [x] **步骤 6：started 新 batch 重置展开状态**

在 `frontend/public/translate.js` 的 `renderTranslationEvent` 的 `started` 分支新 batch 重置块（L286-302），找到 `c.el.classList.remove('failed', 'cancelled');`（当前 L294），在其**之后**插入两行，清除截断/展开态并把按钮 label 回「展开全文」：

```js
          c.el.classList.remove('has-overflow', 'expanded');
          const expandLabel = c.el.querySelector('.result-expand-label');
          if (expandLabel) expandLabel.textContent = '展开全文';
```

插入后该段应为：

```js
        resultCards.forEach(function (c) {
          c.status = 'pending';
          c.text.textContent = '';
          c.text.style.color = '';
          c.actions.style.visibility = 'hidden';
          c.tokens.style.display = 'none';
          c.el.classList.remove('failed', 'cancelled');
          c.el.classList.remove('has-overflow', 'expanded');
          const expandLabel = c.el.querySelector('.result-expand-label');
          if (expandLabel) expandLabel.textContent = '展开全文';
        });
```

- [x] **步骤 7：手动验证（清单 1-4）**

运行：`npm run tauri dev`

在翻译弹窗输入一段足够长的文本（如 8-10 行中文）触发翻译，验证：

1. 翻译结果默认显示约 4-5 行，底部有白色渐隐遮罩，遮罩下方出现「展开全文」按钮 + 向下 chevron。
2. 点「展开全文」→ 完整内容显示，按钮变「收起」，chevron 旋转 180°（朝上）；点「收起」→ 回到截断态，按钮回「展开全文」。
3. 点「展开全文」/「收起」时，卡片**不**发生折叠（header 区域不收缩）——验证 `stopPropagation` 生效。
4. 再次输入新文本翻译（触发新 batch）→ 所有卡片展开状态重置为截断态，按钮回「展开全文」，卡片复用更新内容（不重建）。

预期：以上 4 项全部符合。流式 delta 期间长内容被硬截断、**无**渐隐遮罩、**无**展开按钮（finished 后才出现）——这是对齐原型的正确行为。

- [x] **步骤 8：Commit**

```bash
git add frontend/public/translate.css frontend/public/translate.js
git commit -m "feat(translation): 结果卡片长内容截断与展开全文"
```

---

## 任务 2：输入框最大高度限高

**文件：**
- 修改：`frontend/public/translate.css`（`.source-input` L129-142）
- 修改：`frontend/public/translate.js`（`autoResize` L41-44、初始化 L558）

- [x] **步骤 1：改 .source-input CSS，加 max-height + 滚动条**

在 `frontend/public/translate.css` 中，把当前 L129-142 的 `.source-input { ... }` 规则**整体替换**为下面这段。变更点：`overflow: hidden` → `overflow-y: auto`；新增 `max-height: 10.85em`（em 相对自身 13px = 141px，约 7 行）；新增 Firefox `scrollbar-width` / `scrollbar-color`；新增 webkit 4px 细滚动条伪元素：

```css
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
  max-height: 10.85em;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--border-2) transparent;
  user-select: text;
}
.source-input::-webkit-scrollbar { width: 4px; }
.source-input::-webkit-scrollbar-thumb {
  background: var(--border-2);
  border-radius: 999px;
}
.source-input::-webkit-scrollbar-track { background: transparent; }
```

- [x] **步骤 2：改造 autoResize，限高 + 动态 overflow-y**

在 `frontend/public/translate.js` 中，把当前 L41-44 的 `autoResize` 函数**整体替换**为下面这段。变更点：读取 `getComputedStyle` 的 `maxHeight`；`Math.min(scrollHeight, maxHeight)` 限高；超限时 `overflow-y: auto`，否则 `hidden`（短文本不显示滚动条）：

```js
function autoResize() {
  sourceText.style.height = 'auto';
  const maxHeight = parseFloat(getComputedStyle(sourceText).maxHeight);
  const nextHeight = Math.min(sourceText.scrollHeight, maxHeight || sourceText.scrollHeight);
  sourceText.style.height = nextHeight + 'px';
  sourceText.style.overflowY = sourceText.scrollHeight > nextHeight ? 'auto' : 'hidden';
}
```

- [x] **步骤 3：初始化补 requestAnimationFrame + fonts.ready 触发**

在 `frontend/public/translate.js` 末尾的初始化区（当前 L555-560），把：

```js
initMaxHeight();
initCards();
autoResize();
updateCharCount();
applyPendingSourceText();
```

替换为（`autoResize()` 改 `requestAnimationFrame(autoResize)`，并补 `document.fonts.ready` 再算一次，避免字体加载导致 `scrollHeight` 偏差）：

```js
initMaxHeight();
initCards();
requestAnimationFrame(autoResize);
if (document.fonts) document.fonts.ready.then(autoResize);
updateCharCount();
applyPendingSourceText();
```

- [x] **步骤 4：手动验证（清单 5-6）**

运行：`npm run tauri dev`（若已在运行，直接在弹窗中验证）

1. 在输入框粘贴一段超长原文（超过 7 行）→ 输入框高度到 `max-height`（约 7 行 / 141px）后停止增长，超出部分在输入框内部滚动；弹窗窗口高度不再被撑高。滚动条为 4px 细条、圆角、`--border-2` 颜色。
2. 清空输入框，输入短文本（1-2 行）→ 输入框按 `min-height` / 内容自适应，无滚动条、无展开按钮。
3. 边界：输入恰好 6-7 行时，输入框刚好不滚动（`scrollHeight` 不超 `maxHeight`）。

预期：以上 3 项全部符合。

- [x] **步骤 5：Commit**

```bash
git add frontend/public/translate.css frontend/public/translate.js
git commit -m "feat(translation): 输入原文限高内部滚动"
```

---

## 任务 3：focus 态上边框 outline 修复

**文件：**
- 修改：`frontend/public/translate.css`（`.source-card:focus-within` L125-128）

**根因回顾：** `.content`（L106-114）的 `overflow-y: auto` + `padding: 0 10px 10px`（上 padding 0）裁剪了 `.source-card` 上边的 `box-shadow 0 0 0 1px` 描边，只剩 0.5px border；左右下有 10px padding 衬托保留 1px。`overflow-y: auto` 不能去掉（窗口 max-height 80% 屏幕高，内容超长要滚），故改用不被祖先 overflow 裁剪的 `outline`。

- [x] **步骤 1：改 .source-card:focus-within，box-shadow 描边改 outline**

在 `frontend/public/translate.css` 中，把当前 L125-128 的 `.source-card:focus-within { ... }` 规则**整体替换**为下面这段。变更点：`box-shadow` 去掉 `0 0 0 1px var(--accent)` 描边部分，只保留 `var(--shadow-card-h)` 阴影；新增 `outline: 1px solid var(--accent)` + `outline-offset: 0`。WebView2（Chromium 94+）的 `outline` 跟随 `border-radius`，圆角处不会变直角：

```css
.source-card:focus-within {
  border-color: var(--accent);
  outline: 1px solid var(--accent);
  outline-offset: 0;
  box-shadow: var(--shadow-card-h);
}
```

- [x] **步骤 2：手动验证（清单 7-8）**

运行：`npm run tauri dev`（若已在运行，直接在弹窗中验证）

1. 鼠标点击输入框使其获得 focus → 输入框四边蓝色描边粗细一致（上边不再比左右下细）。对比修复前后可放大屏幕观察上边 1px 描边是否完整。
2. 观察 `.source-card` 的圆角（`--radius-md` = 9px）处 → outline 贴合圆角弧线，无直角瑕疵。
3. blur（点击弹窗其它区域）→ 描边消失，回到默认 `0.5px var(--border)` border + `--shadow-card`。

预期：以上 3 项全部符合。

- [x] **步骤 3：Commit**

```bash
git add frontend/public/translate.css
git commit -m "fix(translation): focus 态上边框描边粗细一致"
```

---

## 任务 4：README 文档同步

**文件：**
- 修改：`README.md`（核心能力列表 L17 后）

> 协作规范第 2 条：文档同步是收尾硬门禁。spec 第 8 节明确 README 需更新翻译弹窗能力，CLAUDE.md / AGENTS.md 无需改（窗口配置不变）。

- [x] **步骤 1：在 README 核心能力列表补充截断与限高两条**

在 `README.md` 中，找到 L17（「流式结果展示：...」一行），在其**之后**插入两条新能力。该处当前上下文为：

```
- 流式结果展示：Rust 后端通过 Tauri event 推送翻译状态和增量文本，前端实时渲染。
- 翻译取消与重试：流式翻译过程中可取消，失败或取消后可一键重试。
```

替换为（中间插入两条）：

```
- 流式结果展示：Rust 后端通过 Tauri event 推送翻译状态和增量文本，前端实时渲染。
- 结果卡片长内容截断：翻译结果超过约 4-5 行时自动截断，底部渐隐遮罩 + 「展开全文」按钮，点击展开/收起。
- 输入原文限高：输入框超过最大高度（约 7 行）后内部滚动，不再撑高弹窗。
- 翻译取消与重试：流式翻译过程中可取消，失败或取消后可一键重试。
```

- [x] **步骤 2：Commit**

```bash
git add README.md
git commit -m "docs(readme): 同步翻译弹窗截断与限高能力"
```

---

## 自检

**1. 规格覆盖度：** 逐章对照 spec：
- spec §3（问题 1 卡片截断）→ 任务 1 全部覆盖（HTML 结构 §3.1、CSS §3.2、JS §3.3、交互 §6）。✓
- spec §4（问题 2 输入框限高）→ 任务 2 全部覆盖（CSS §4.1、JS autoResize §4.2、触发时机）。✓
- spec §5（问题 3 上边框）→ 任务 3 全部覆盖（根因 §5.1、方案 A §5.2）。✓
- spec §6（交互细节）→ 任务 1 步骤 5-6（finished 后探测、新 batch 重置）+ 验证步骤 7（流式硬切无遮罩）覆盖；`.has-overflow`/`.expanded` 与 `.collapsed` 独立由 CSS 自然保证（折叠态 body 0fr，clip 不可见）。✓
- spec §7（测试）→ 验证清单 8 项分配到任务 1（1-4）、任务 2（5-6）、任务 3（7-8）。✓
- spec §8（文档同步）→ 任务 4。✓
- spec §9（风险）→ outline 跟随 border-radius 由任务 3 验证步骤 2 覆盖；`max-height` 数值与原型一致；截断行数非整数按原型值。✓
- 无遗漏。

**2. 占位符扫描：** 全文搜索「待定」「TODO」「类似任务」「补充细节」「添加适当的」——无。每个代码步骤含完整可替换代码块。✓

**3. 类型一致性：**
- `detectOverflow(cardEl)` / `updateExpandButton(cardEl)` / `toggleExpand(cardEl)` 三处签名一致，均接收 DOM 元素。✓
- `getCard` 内绑定 `toggleExpand(card)`（L144 的 `card` 是 DOM 元素）→ 匹配 `cardEl` 参数。✓
- `finished` 调 `updateExpandButton(card.el)`（L325 的 `card` 是 ref，`.el` 是 DOM）→ 匹配。✓
- `started` 重置用 `c.el.querySelector('.result-expand-label')`（`c` 是 ref，`.el` 是 DOM）→ 一致。✓
- CSS 类名 `.result-text-clip` / `.has-overflow` / `.expanded` / `.result-expand-btn` / `.result-expand-label` / `.result-expand-chevron` 在 CSS 与 JS 中拼写一致。✓
- `adjustHeight()` 在 `toggleExpand` 内调用，该函数已存在于 translate.js L489。✓

**4. 风险复核：**
- 任务 1 步骤 4 的 `expandBtn` 绑定位于 `ref` 创建（L217）之前，但闭包内只引用 `card`（DOM，L144 已定义）和 `toggleExpand`（函数声明，已提升），无时序问题。✓
- 任务 2 `parseFloat(getComputedStyle(sourceText).maxHeight)`：CSS 设 `10.85em`，computed style 返回像素值（如 `141.05px`），`parseFloat` 得 141.05，`||` 短路兜底防 NaN。✓
- 任务 3 `outline-offset: 0`：outline 紧贴 border 外侧，视觉与原 `box-shadow 0 0 0 1px` 等价但不被裁剪。✓

---

## 执行交接

计划已完成并保存到 `docs/superpowers/plans/2026-07-07-translation-popup-overflow-input-border.md`。
