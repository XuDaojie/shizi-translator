# 翻译结果卡 Markdown 渲染

> 日期：2026-07-30  
> 状态：已实现  
> 规模：M  
> 策略：常用 Markdown 子集 + 完成后渲染；弹窗与历史共用；复制源文、朗读纯文本

## 1. 背景与目标

LLM 译文常带 `**粗体**`、列表、代码块等 Markdown。当前结果卡用纯文本 / `TextNode` 展示，标记原样露出，阅读体验差。

目标：在**翻译弹窗结果卡**与**设置页历史详情结果卡**中渲染常用 Markdown 子集，流式阶段保持纯文本，完成后切到安全 HTML。

## 2. 已确认决策

| 项 | 决策 |
|----|------|
| 语法范围 | 常用子集（非完整 GFM） |
| 流式 | 仅 `finished` 后渲染；`translating` 仍为纯文本 + 光标 |
| 复制 | 始终使用原始 Markdown 源文（`card.text`） |
| 朗读 | 去掉标记后的可见纯文本 |
| 范围 | 弹窗 + 历史详情一致 |
| 设置开关 | 不做（默认开启） |
| 原文卡 | 不渲染 Markdown |

## 3. 范围

### 做

- 新模块：`frontend/src/popup/composables/renderMarkdown.ts`（+ 单测）
- 依赖：`marked`、`dompurify`（及 `@types` 若需要）
- 改：`ResultCard.vue`（流式 / 完成态切换）、`ResultCardView.vue`（默认 `:text` 路径走 MD）
- 样式：`components.css` 中 `.result-md`（popup token）
- 链接点击：`http(s)` → `open_url`（经 `getTauriApis` / 既有封装），禁止 WebView 内跳转

### 不做（YAGNI）

- 表格、任务列表、删除线、脚注等 GFM 扩展
- 图片（外链风险 / 译文少见）
- 原始 HTML 透传
- 流式实时 Markdown
- 设置项开关
- 原文卡 Markdown
- 后端 / 协议 / 历史存储格式变更（仍存源字符串）

## 4. 语法白名单

**支持：**

- 标题：`#`～`###`（更深标题可降为 `h3` 或按 marked 默认后用 CSS 统一字号）
- 粗体 / 斜体 / 粗斜体
- 行内代码、围栏代码块（语言类名可保留，不做高亮）
- 有序 / 无序列表
- 链接（仅 `http:` / `https:`；其余协议消毒掉）
- 引用、水平线
- 软换行 / 硬换行（段落）

**禁止 / 剥掉：**

- `<script>`、事件属性、`javascript:` 等（DOMPurify）
- 任意 raw HTML 标签（配置 marked 不信任 HTML，或消毒后只留白名单标签）
- `img`、`iframe`、`object`、`form` 等

## 5. 架构

```
card.text (源 Markdown)
    │
    ├─ translating → ResultCard 命令式 TextNode + stream-cursor（现状）
    │
    ├─ finished    → renderMarkdownToHtml(text) → v-html.result-md
    │
    └─ History ResultCardView 默认 slot
                     text prop → 同上 HTML（status=success）
```

### 5.1 `renderMarkdown.ts`

```ts
/** Markdown 源 → 消毒后的 HTML（失败时转义为纯文本段落） */
export function renderMarkdownToHtml(source: string): string

/** 朗读用：去掉标记后的纯文本（保留换行语义） */
export function plainTextFromMarkdown(source: string): string
```

实现要点：

1. `marked.parse(source, { async: false, gfm: false 或仅开启安全子集 })`  
   - 若 gfm 默认含表格：用 renderer / 选项关闭不需要的扩展，或接受表格不样式化但消毒后无害——**优先关闭表格/任务列表**。
2. `DOMPurify.sanitize(html, { ALLOWED_TAGS, ALLOWED_ATTR })`  
   - tags 示例：`p, br, strong, em, b, i, code, pre, h1, h2, h3, ul, ol, li, a, blockquote, hr`  
   - attr 示例：`href, title, class`（`class` 仅代码块语言可选）  
   - 链接：`ALLOWED_URI_REGEXP` 限 `https?`
3. 空串 → `''`
4. `plainTextFromMarkdown`：可对 HTML 做 `textContent` 提取（jsdom 不可用时：用临时逻辑——render 后再 strip tags 的轻量函数，或 marked lexer 拼文本）。**测试环境用 vitest + 简单 strip / 或 happy-dom 若项目已有。** 优先：render → 用正则/DOMParser（浏览器）提取；单测里对纯函数用固定用例断言。

### 5.2 `ResultCard.vue`

- `translating`：保持现有 `renderText` 增量 TextNode + 光标。
- 进入 `finished`：清空命令式 DOM，改用绑定 HTML（`v-html` 或切换显示层）。
  - 推荐结构：两个互斥层——`.result-text` 流式层 vs `.result-text.result-md` 完成层，避免混用命令式与 Vue 冲突。
- `failed` / `cancelled`：错误文案纯文本，不走 MD。
- 复制：`copyText(props.card.text)`（源文，不用 `textContent`）。
- 朗读：`speakText(plainTextFromMarkdown(props.card.text), targetLang)`。
- `finished` 与 text 变化后 `nextTick(detectOverflow)`。

### 5.3 `ResultCardView.vue`

- 默认 slot（历史）：`status === 'success' && text` 时  
  `<div class="result-text result-md" dir="auto" v-html="renderMarkdownToHtml(text)" />`  
  否则保持 `{{ text }}` 或错误样式。
- 弹窗通过 slot 自管正文时，不强制改 View 的 slot 行为。
- 测高：`querySelector('.result-text')` 仍可用；MD 渲染后 `scheduleMeasure` 已有 text/status watch。

### 5.4 链接点击

- 在结果正文容器上 `@click` 委托：若 `a[href]` 且协议为 http(s)，`preventDefault`，调用 `open_url`。
- 无 Tauri 时（纯浏览器 dev）可 `window.open(url, '_blank', 'noopener')` 降级。
- 弹窗与历史共用同一 handler（composables 内 `handleMarkdownLinkClick(e)`）。

### 5.5 样式 `.result-md`

- 取消 MD 容器上与块级冲突的 `white-space: pre-wrap`（流式层保留；`.result-md` 用 normal + 块级间距）。
- `p / ul / ol / pre / blockquote` 控制 margin，避免卡片过高。
- `code`：柔和底色；`pre code`：块级、可横向滚动、小字号 mono。
- `a`：accent 色；`h1–h3` 略大于正文，勿过大。
- 全部颜色用 `--popup-*`。

## 6. 错误与边界

| 场景 | 行为 |
|------|------|
| 无 MD 标记的纯文本 | 渲染为段落/纯文本，视觉接近现状 |
| 半截流式 `**` | 流式阶段不解析，不闪烁 |
| 恶意 HTML 注入 | 消毒剥离 |
| 解析异常 | 回退为转义纯文本 |
| 空译文 | 空节点，不报错 |

## 7. 测试

- `renderMarkdown.test.ts`：
  - 粗体 / 列表 / 代码块产出预期标签
  - `<script>` / `javascript:` 链接被去掉或中和
  - 空串
  - `plainTextFromMarkdown('**hi**')` ≈ `hi`
- 不强制组件 E2E；手动：弹窗流式→完成、历史详情、复制源文、朗读、链接、长文展开。

## 8. 验证命令

```bash
npm install   # 新依赖
npm run test
npm run typecheck
```

## 9. 文档收尾

- 本 spec 实现后将状态改为「已实现」。
- 若 `architecture-notes` 有结果卡展示说明，补一句「译文支持常用 Markdown 渲染」。
- 不单独改 README，除非现有功能列表已枚举结果卡能力。

## 10. 实现顺序

1. 加依赖 + `renderMarkdown.ts` + 测试（TDD）
2. CSS `.result-md`
3. `ResultCardView` 默认路径
4. `ResultCard` 流式/完成切换 + 复制/朗读/链接
5. typecheck / test / 架构笔记同步
