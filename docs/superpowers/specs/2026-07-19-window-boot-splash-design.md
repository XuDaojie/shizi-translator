# 独立窗口冷启动 Logo Splash 设计规格

- **状态**：已确认（待实现）
- **作者**：xdj（与 AI 协作）
- **日期**：2026-07-19
- **规模**：M
- **关联**：
  - 弹窗启动无闪：`docs/superpowers/specs/2026-07-10-popup-startup-height-collapse-design.md`
  - 应用图标：`docs/superpowers/specs/2026-07-14-windows-app-icon-design.md`
  - 现状入口：`frontend/settings.html`、`frontend/ocr.html`、`frontend/translate.html`
  - 现状挂载：`frontend/src/settings/main.ts`、`frontend/src/ocr/main.ts`、`frontend/src/popup/main.ts`
  - 窗口创建：`src-tauri/src/app/window.rs`（设置/OCR 创建后 `present` 即 show）

## 一、问题与目标

### 1.1 现状问题

用户打开**设置页**（及同类独立 WebView 窗口）时，冷启动瞬间常见：

1. 窗口已 `show`，但页面几乎是空 HTML；
2. 在 CSS 模块与 Vue 挂载完成前，整窗呈**一片白**；
3. 加载完成后内容「突然出现」，缺乏品牌过渡。

根因：

| 现象 | 根因 |
|------|------|
| 白屏 | `settings` / `ocr` 等窗创建后立即 `show`，`#app` 仍为空；Tailwind 等模块 CSS 尚未生效 |
| 无品牌占位 | `settings.html` / `ocr.html` 无静态壳；仅 `translate.html` 有米色空壳，且无 Logo |
| 与 Cherry Studio 对比 | 对方冷启动中间有 Logo，再过渡到完整 UI |

### 1.2 产品目标（已确认）

参考 Cherry Studio：**冷启动时窗口中央显示应用 Logo**，页面就绪后过渡到完整 UI。

成功标准：

1. 用户在冷启动可见期看到的是**浅灰底 + 居中 Logo（轻微呼吸）**，而不是纯白空窗；
2. Vue 挂载并完成首帧绘制后，splash **淡出并移除**，露出可交互页面；
3. **再次打开**（关窗 = hide，进程内页仍存活）**不再出现** splash；
4. 覆盖所有带完整 UI 的独立入口页（见范围），行为一致。

### 1.3 已确认的产品选择

| 议题 | 选择 |
|------|------|
| 范围 | **C**：所有带 UI 的独立窗口（settings / ocr / main·translate） |
| 再次打开 | 保持现状：不重放 splash |
| 收起时机 | **A**：前端可交互就绪（mount + 双 rAF），**不等**配置/历史等后端数据 |
| 视觉 | **C**：浅灰底 + Logo 轻微呼吸动画 |
| 实现路线 | **方案 1**：各入口 HTML **内联静态 splash**（不依赖模块 CSS/JS） |

### 1.4 非目标

- 不设「最短展示时长」人为拖长 splash（已否决方案 C 的时长约束）。
- 不因 `syncFromBackend` / 历史拉取等异步数据延迟收起 splash。
- 不改为「先 hidden，就绪再 show」替代 Logo 过渡（方案 2 已否决；弹窗现有 ready gate 可保留，与本 splash 叠加不冲突）。
- 不做原生第二窗口 / 系统级 splash。
- 不做深色主题专用 splash（v1 固定浅灰）。
- **不改** `overlay.html`（框选层，非同类冷启动壳场景）。
- 不改 Rust 侧创建后立即 show 的策略（用前端静态壳盖住白屏即可）。

## 二、范围与入口

| 窗口 / 入口 | 标签 | HTML | 是否做 splash |
|-------------|------|------|----------------|
| 设置 | `settings` | `frontend/settings.html` | ✅ |
| 文字识别 | `ocr` | `frontend/ocr.html` | ✅ |
| 翻译弹窗 | `main` | `frontend/translate.html` | ✅（升级现有米色空壳） |
| OCR 框选 | overlay | `frontend/public/overlay.html` | ❌ |

## 三、视觉规格

### 3.1 布局

- 全窗（或 body 视口）浅灰背景；
- 水平垂直居中应用 Logo；
- 无应用名文案（标题栏已有窗口标题）；
- 无进度条、无 loading 文案。

### 3.2 颜色与尺寸

| 项 | 值 |
|----|-----|
| 背景 | `#f4f5f7` |
| Logo 显示边长 | 约 `80×80` CSS px |
| Logo 圆角 | 与应用图标一致（圆角方形「文 / A」构图） |
| Logo 阴影 | 轻阴影，增强浮起感（实现时可微调，不挡辨识） |
| 图形来源 | 与 `src-tauri/icons/icon.svg` 同构图；**内联 SVG** 于 HTML，避免额外网络往返 |

### 3.3 呼吸动画

- 属性：`transform: scale` + `opacity`
- 幅度：`scale 1 ↔ 1.06`，`opacity 1 ↔ 0.88`
- 时长：约 `1.6s`，`ease-in-out`，无限循环
- 收起开始后：停止依赖循环动画的视觉焦点，以淡出为主

### 3.4 淡出

| 项 | 值 |
|----|-----|
| 属性 | `opacity: 0`（可辅以 `pointer-events: none`） |
| 时长 | 约 `200–250ms` |
| 结束后 | 从 DOM **移除** `#boot-splash`（或约定 id） |
| 兜底 | `transitionend` 未触发时，超时（如 400ms）强制 `remove` |

## 四、架构与实现契约

### 4.1 DOM 结构

每个目标 HTML 入口在 body 内采用：

```html
<body>
  <div id="boot-splash" class="boot-splash" aria-hidden="true">
    <!-- 内联 SVG Logo -->
  </div>
  <div id="app"><!-- 可保留极简占位；Vue mount 后接管 --></div>
  <script type="module" src="…"></script>
</body>
```

约束：

1. **splash 必须在 `#app` 之外**（或可独立控制），以便 mount 后仍能做淡出，而不是被 Vue 整树瞬间抹掉导致无过渡。
2. **样式与 SVG 必须内联在 HTML**（`<style>` + 内联 `<svg>`），不得依赖 Vite 模块 CSS 才能「看见」splash。
3. `aria-hidden="true"`：纯装饰，不进可访问性树。

### 4.2 共享 dismiss 逻辑

新增小模块（建议路径）：`frontend/src/shared/bootSplash.ts`

```ts
/** Vue mount 后调用：双 rAF → 淡出 → 移除 */
export function dismissBootSplash(options?: {
  rootId?: string // 默认 'boot-splash'
  hideClass?: string // 默认 'boot-splash--hide'
  fallbackRemoveMs?: number // 默认 400
}): Promise<void>
```

行为：

1. 查找 splash 节点；不存在则 no-op 成功返回（再次打开 / 热更新后已移除）。
2. `doubleRaf`（可复用 `frontend/src/popup/composables/mainWindowReady.ts` 的 `doubleRaf`，或在 shared 内实现同等逻辑，避免 popup→shared 反向依赖）。
3. 添加 hide class，触发 CSS transition。
4. `transitionend`（过滤 `opacity`）或超时后 `remove()`。
5. 幂等：多次调用安全。

各入口 `main.ts`：

```ts
createApp(…).mount('#app')
void dismissBootSplash()
```

设置 / 翻译入口若在 mount 前有 `initializeI18n` race：splash 在等待 i18n 期间**持续显示**（符合预期）；i18n 完成后 mount 再 dismiss。

### 4.3 与现有能力的关系

| 能力 | 关系 |
|------|------|
| `createMainWindowReadyGate` / 弹窗延迟 show | 可保留。窗口在 show 前用户看不到；show 时若仍短暂未 paint，splash 兜底。dismiss 仍在 mount+rAF 后执行。 |
| `translate.html` 米色 `.popup-boot-shell` | **替换/升级**为统一 splash 视觉；弹窗真实背景仍由 Vue 挂载后的 popup 样式决定。 |
| 设置窗 `SETTINGS_INITIAL_VISIBLE = false` 后 `present` | 不改 Rust；HTML 解析后即有 splash。 |

### 4.4 再次打开

设置 / OCR：关窗 = hide，WebView 与 DOM 保留。splash 已在首次 dismiss 时移除 → 再 show **无 splash**。

冷启动定义：该 WebView **进程内首次加载该 HTML 文档**。刷新 dev 页面会再次出现 splash（可接受）。

## 五、文件改动清单（实现指引）

| 文件 | 动作 |
|------|------|
| `frontend/settings.html` | 内联 splash 样式 + SVG |
| `frontend/ocr.html` | 同上 |
| `frontend/translate.html` | 升级现有 boot 壳为统一 splash |
| `frontend/src/shared/bootSplash.ts` | 新增 dismiss API |
| `frontend/src/shared/bootSplash.test.ts` | 单元测：无节点 no-op、幂等、调用后移除（jsdom） |
| `frontend/src/settings/main.ts` | mount 后 `dismissBootSplash` |
| `frontend/src/ocr/main.ts` | 同上 |
| `frontend/src/popup/main.ts` | 同上 |
| `docs/agent/architecture-notes.md` 或 README（若有「启动/窗口」小节） | 实现收尾时按门禁同步一句说明（若文档已有对应段） |

不强制改 `src-tauri`。

## 六、测试与验收

### 6.1 自动化

- `bootSplash`：缺失节点 no-op；存在节点时添加 hide class 并最终从 document 移除；二次调用不抛错。
- 不强制对 HTML 内联做 e2e。

### 6.2 手工验收

1. **设置冷启动**：托盘/快捷键首次打开设置 → 见浅灰 + Logo 呼吸 → 淡出到设置 UI；无纯白长驻。
2. **设置再开**：关闭（hide）后再开 → 直接设置 UI，无 splash。
3. **OCR 冷启动 / 再开**：同上。
4. **翻译弹窗冷启动**：见统一 splash 后过渡到弹窗 UI；高度/ready 门闸行为不回退到可见白窗乱跳。
5. **overlay**：框选流程不受影响。

### 6.3 性能与失败

- splash 内联体积应保持较小（单份 SVG + 少量 CSS）；不引入外链字体专用于 splash。
- dismiss 失败（节点异常）不得阻塞业务：catch / best-effort。

## 七、风险与缓解

| 风险 | 缓解 |
|------|------|
| 三份 HTML 内联 SVG 重复 | 接受 v1 重复；注释标明同源 `icon.svg`；后续若痛再抽构建注入 |
| 弹窗透明/异形与 splash 背景冲突 | translate 的 splash 仅覆盖冷启动；mount 后由 popup 背景接管；手工验收弹窗形态 |
| 双 rAF 仍偶发未 paint | 可接受 best-effort；不引入最短展示时长 |
| dev HMR 后残留节点 | dismiss 幂等 + 可选查询已存在则跳过创建（HTML 静态，HMR 全刷则重新冷启动） |

## 八、实现顺序建议

1. 实现 `bootSplash.ts` + 单测（TDD）。
2. 改造 `settings.html` + `settings/main.ts`（主痛点）。
3. 同步 `ocr.html` + `ocr/main.ts`。
4. 升级 `translate.html` + `popup/main.ts`。
5. 手工验收三窗冷启动 / 再开；文档一句同步；提交。

## 九、决策记录

| 决策 | 结论 | 理由 |
|------|------|------|
| 静态 HTML splash vs 延迟 show | 静态 splash | 需要 Cherry 式 Logo 过渡，且与现有 translate boot 壳一致 |
| 收起是否等配置 | 否 | 配置慢时不应绑死品牌壳；可交互壳优先 |
| 是否含 overlay | 否 | 非同构 Vue 冷启动壳 |
| 视觉 | 浅灰 + 呼吸 | 用户从 A/B/C 中选定 C |
| 再次打开 | 不重放 | 用户明确要求 |
