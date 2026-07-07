# 翻译弹窗 UI 打磨：卡片截断 / 输入框限高 / 上边框

> 日期：2026-07-07
> 状态：待实现
> 策略：对齐 OpenDesign 原型 commit `2e9355e`（卡片截断）、`d5d6335`（输入框高度），并修复上边框渲染

## 1. 背景与目标

OpenDesign 高保真原型（`C:\Users\xdj\IdeaProjects\LLM\OpenDesignProjects\shizi\translation-popup.html`）在两个 commit 后更新了翻译弹窗交互，shizi 当前实现未跟上，存在三处不一致：

1. **翻译结果卡片长内容不截断**：`.result-text` 无高度限制，长文本撑高卡片与窗口。
2. **输入原文无最大高度**：`.source-input` 只有 `min-height` + `overflow: hidden`，`autoResize` 无上限，内容一路撑开。
3. **输入框 focus 编辑态上边框更细**：上边蓝线比左右下三条更细。

本任务按原型对齐前两点，并定位修复第三点的渲染根因。**纯前端改动**（`translate.css` / `translate.js`），不碰后端、overlay、设置页。

## 2. 范围

### 改动

- `frontend/public/translate.css`
  - `.source-input`：加 `max-height`、`overflow-y: auto`、细滚动条样式。
  - `.source-card:focus-within`：描边从 `box-shadow 0 0 0 1px` 改 `outline`。
  - 新增 `.result-text-clip` / `.result-expand-btn` / 渐隐遮罩 / chevron 旋转样式。
- `frontend/public/translate.js`
  - `autoResize`：限高 + 动态切 `overflow-y`，补 `requestAnimationFrame` / `document.fonts.ready` 触发。
  - `getCard`：HTML 结构包 `.result-text-clip` 容器 + `.result-expand-btn` 按钮。
  - 新增 `detectOverflow` / `updateExpandButton` / `toggleExpand`；`finished` 后探测溢出；新 batch 重置展开状态。

### 不做（YAGNI）

- 不改 `translate.html`（卡片由 JS 动态创建，结构改动在 `getCard`）。
- 不碰后端、overlay.html、设置页。
- 不对齐原型其它小差异（`.source-card:hover` 阴影、`cursor: text`、点击卡片聚焦 textarea 等），只做本次三个问题。
- 不改 `.source-input` 的 `min-height`（保留当前 `2.75rem`，用户决策；只对齐 `max-height`）。
- 不改卡片截断行数阈值（用原型 `6.4em`）。

## 3. 问题 1：翻译卡片截断

### 3.1 HTML 结构（`getCard` innerHTML）

`.result-text` 外包一层 `.result-text-clip`，`.result-text` 之后、`.result-actions` 之前插入展开按钮：

```html
<div class="result-card-body">
  <div class="result-card-body-inner">
    <div class="result-text-clip">
      <div class="result-text"></div>
    </div>
    <button class="result-expand-btn" type="button" tabindex="-1">
      <span class="result-expand-label">展开全文</span>
      <svg class="result-expand-chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>
    </button>
    <div class="result-actions" style="visibility:hidden">…</div>
  </div>
</div>
```

### 3.2 CSS（新增，对齐原型 2e9355e）

```css
.result-text-clip {
  position: relative;
  max-height: 6.4em;             /* 原型值；.result-text-clip 继承 :root 16px，约 4-5 行 */
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

> 注：原型注释写「4 行 @ 13px / 1.6」，但 `em` 相对 `.result-text-clip` 自身 font-size（继承 `:root` 16px），实际约 4.9 行。原型与 shizi `:root` 均为 16px，两边表现一致，按原型值 `6.4em` 即可，不刻意调整为整数行。

### 3.3 JS

- `detectOverflow(card)`：`text.scrollHeight > clip.clientHeight + 1`。
- `updateExpandButton(card)`：溢出则 `card.classList.add('has-overflow')`；否则移除 `has-overflow` 与 `expanded`，label 回「展开全文」。
- `toggleExpand(card)`：切 `expanded`，label 切「展开全文」/「收起」。
- `getCard`：展开按钮 `addEventListener('click', e => { e.stopPropagation(); toggleExpand(card); })`（与现有 `collapseBtn` 风格一致，不用 inline `onclick`）；`tabindex="-1"` 不参与 Tab 焦点。
- `finished` 事件后：调用 `updateExpandButton(card)`。
- `started` 新 batch 重置：对每张卡 `classList.remove('has-overflow', 'expanded')`，label 回「展开全文」。

## 4. 问题 2：输入框最大高度

### 4.1 CSS（`.source-input`，对齐原型 d5d6335）

- `min-height`：保留 `2.75rem`（用户决策）。
- 新增 `max-height: 10.85em`（约 7 行；em 相对自身 font-size 13px，= 141px）。
- `overflow: hidden` → `overflow-y: auto`。
- 新增 `scrollbar-width: thin; scrollbar-color: var(--border-2) transparent;`。
- 新增 `::-webkit-scrollbar { width: 4px; }` 与 `::-webkit-scrollbar-thumb { background: var(--border-2); border-radius: 999px; }`。

### 4.2 JS（`autoResize`）

```js
function autoResize() {
  sourceText.style.height = 'auto';
  const maxHeight = parseFloat(getComputedStyle(sourceText).maxHeight);
  const nextHeight = Math.min(sourceText.scrollHeight, maxHeight || sourceText.scrollHeight);
  sourceText.style.height = nextHeight + 'px';
  sourceText.style.overflowY = sourceText.scrollHeight > nextHeight ? 'auto' : 'hidden';
}
```

补触发时机（对齐原型）：

- `requestAnimationFrame(autoResize)`（初始化时）。
- `if (document.fonts) document.fonts.ready.then(autoResize);`（字体加载完再算一次，避免 scrollHeight 偏差）。

## 5. 问题 3：上边框 outline

### 5.1 根因

`.source-card` 与 `:focus-within` 的 CSS 写法与原型**完全一致**（`border: 0.5px` + `box-shadow: 0 0 0 1px var(--accent)`）。差异在容器：shizi 的 `.content`（[translate.css:106-114](frontend/public/translate.css#L106-L114)）为窗口高度上限时内容滚动，加了 `overflow-y: auto` 且 `padding: 0 10px 10px`（上 padding 0）；原型 `.content` 无 `overflow`。

`box-shadow` 会被祖先 `overflow != visible` 裁剪。因上 padding 为 0，`.source-card` 上边紧贴 `.content` 顶部，focus 态向上的 1px 描边被裁掉，只剩 0.5px border；左右下有 10px padding 衬托，1px 描边保留 → 上边显细。

shizi 的 `overflow-y: auto` 是必要的（窗口 `max-height` 80% 屏幕高，内容超长要能滚），不能去掉。

### 5.2 修复（方案 A）

`:focus-within` 去掉 `box-shadow` 的 `0 0 0 1px var(--accent)` 描边，改用 `outline`，保留 `var(--shadow-card-h)` 阴影：

```css
.source-card:focus-within {
  border-color: var(--accent);
  outline: 1px solid var(--accent);
  outline-offset: 0;
  box-shadow: var(--shadow-card-h);
}
```

`outline` 不被祖先 `overflow` 裁剪，四边一致；WebView2（Chromium 94+）`outline` 跟随 `border-radius`，圆角处不会变直角。视觉与原型 1px 蓝描边等价。

## 6. 交互细节

- **截断与折叠独立**：`.has-overflow` / `.expanded`（截断）与 `.collapsed`（空内容折叠，grid 动画 0fr）是独立状态，不冲突；折叠态 body 高度 0，clip 不可见。
- **流式过程中截断**：`delta` 逐字追加时，`.result-text-clip` 的 `overflow: hidden` 已生效，长内容被硬截断；此时 `has-overflow` 未加，**无渐隐遮罩、无展开按钮**（文字被硬切）。`finished` 后 `updateExpandButton` 才加 `has-overflow`，渐隐遮罩与展开按钮一同出现（对齐原型 `streamInto` 完成后探测的行为）。
- **新 batch 重置**：`started` 事件 isNewBatch 时清除所有卡片的 `has-overflow` / `expanded`，label 回「展开全文」，不保留上次展开态。
- **`scrollToBottom` 不改**：`.result-text` `overflow` 默认 visible，`scrollTop` 无效（shizi 既有行为，原型同此），截断时不自动滚到底部，与原型一致。

## 7. 测试

纯前端，无 vitest 单测（与 overlay 一致）。`tauri dev` 手动验证清单：

1. 长文本翻译结果默认截断（约 4-5 行）+ 底部渐隐遮罩 + 出现「展开全文」按钮。
2. 点「展开全文」→ 完整显示，按钮变「收起」，chevron 旋转 180°；点「收起」回滚。
3. 展开按钮点击不触发卡片折叠（stopPropagation）。
4. 再次翻译（新 batch）→ 展开状态重置，旧卡片复用更新。
5. 输入超长原文 → 输入框到 `max-height` 10.85em 后内部滚动，窗口不再被撑高；滚动条为 4px 细条。
6. 短原文 → 输入框不滚动，无展开按钮。
7. focus 输入框 → 四边蓝描边粗细一致（上边不再更细）。
8. 取消 `border-radius` 圆角处 outline 无直角瑕疵。

## 8. 文档同步（收尾硬门禁）

- spec：本文档。
- README.md：翻译弹窗能力更新（结果卡片长内容截断 + 展开全文、输入原文限高内部滚动）。
- CLAUDE.md / AGENTS.md：架构关键点无变化（窗口配置不变），无需改。

## 9. 风险

- **outline 跟随 border-radius**：WebView2 Evergreen（Chromium 94+）支持，风险低；手动验证清单第 8 项覆盖。
- **`max-height: 10.85em`**：em 相对 `.source-input` 自身 font-size 13px，= 141px，约 7 行；与原型一致。
- **截断行数非整数**：`6.4em` 相对 16px 约 4.9 行，与原型表现一致（两边 `:root` 均 16px），不刻意调整为整数行。
- **流式时只显示前约 4-5 行**：对齐原型，用户已确认以原型为准。
