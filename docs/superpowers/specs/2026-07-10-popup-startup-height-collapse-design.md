# 翻译弹窗启动无闪 + 动态高度与卡片收缩 设计规格

- **状态**：已确认（待实现）
- **作者**：xdj（与 AI 协作）
- **日期**：2026-07-10
- **关联**：
  - 启动展示翻译弹窗：`docs/superpowers/specs/2026-07-06-startup-translation-popup-live-services-design.md`
  - 弹窗 Vue 化：`docs/superpowers/specs/2026-07-10-popup-vue-migration-history-ui-design.md`
  - 现状代码：`src-tauri/src/lib.rs`、`src-tauri/tauri.conf.json`、`frontend/src/popup/composables/usePopupHeight.ts`、`frontend/src/popup/composables/useTranslationEvents.ts`、`frontend/src/popup/TranslationPopup.vue`

## 一、问题与目标

### 1.1 现状问题

冷启动打开翻译弹窗时，用户可见：

1. 窗口先以较大/默认形态出现，再收缩到接近弹窗尺寸；
2. 先出现全白（或空）WebView，再渲染完整 UI。

根因叠加：

| 现象 | 根因 |
|------|------|
| 尺寸跳变 | `tauri.conf.json` 固定 `420×480`，Vue 挂载后 `usePopupHeight` 再按真实 DOM `setSize`；启用服务数 N 变化时高度也变 |
| 白屏/空窗 | setup 末尾立即 `window.show()`，此时 `translate.html` 仅有空 `#app`，Vue 尚未首帧 |
| 翻译中高度再跳 | 当前 `started` 即 `collapsed = false`，空 body 先撑高，流式正文再改高 |

### 1.2 产品目标（已确认）

**冷启动默认行为：尽快出现翻译弹窗**（不是仅托盘、不是默认设置页）。

成功标准：

1. 用户要么看不到窗，要么一看到就是 **当前状态下的正确尺寸 + 完整可交互壳**；
2. 禁止可见的「大窗 → 小窗 → 白屏 → 内容」过程；
3. 弹窗高度随启用服务数 N 与卡片收缩/展开变化，但 **无意义的空白跳变** 要去掉；
4. 卡片收缩规则与高度策略一致，同一次交付落地。

### 1.3 非目标

- 不把冷启动默认改成仅托盘或设置页（可作为后续可选配置，本规格不实现）。
- 不重做卡片视觉设计、不改翻译协议/事件字段名（除非实现必需的前端状态逻辑）。
- 不要求业务数据（历史、探测模型等）全部就绪后再 show。
- 不追求「零毫秒、绝对无任何像素变化」的原生级保证；允许 show 后因流式正文产生的 **有意义长高**。

## 二、已确认的交互规则

### 2.1 卡片 `collapsed` 状态机

> 术语：`collapsed` = 结果卡正文折叠（仅 header）；与「展开全文」的 `expanded` 无关，本规格不改 `expanded` 语义。

| 阶段 | `collapsed` | 说明 |
|------|-------------|------|
| 打开弹窗、未在翻译、无有效译文展示需求 | `true` | 空闲紧凑；高度 ≈ 壳 + N × 收缩卡 |
| 已 `started` / 翻译中，正文仍空且未失败 | `true` | header 用 loading 点表达等待；**窗高尽量不变** |
| 出现首段 **可见正文**，或 **failed**，或 **finished 且需展示结果** | `false`（该卡） | 有内容/错误才占正文高度 |
| 翻译完成（成功） | 保持展开 | 不自动再收回 |
| 用户清空原文（无 trim 文本） | 各卡恢复 `true` | 与现有清空逻辑一致方向 |
| 新 batch | 参与卡先全部收回，再各自等首包展开 | 避免沿用上批展开态 |

**「第一个响应」定义（实现契约）：**

- 触发自动展开的条件（满足任一即可，按卡独立）：
  1. `status === 'translating'`（或流式中）且 `text` 从空变为非空（首条有效 delta / 等价正文）；
  2. `status === 'failed'`（即使无正文，也必须让错误可见 → 自动展开该卡）；
  3. `status === 'finished'` 且需要展示结果（含仅 finished、无中间 delta 的 provider）。
- **不**因单独的 `started` 展开。

**用户手动覆盖：**

- 用户点击 header / 折叠按钮后的 `collapsed` 以用户为准；
- 至少在本 batch 内，自动逻辑不得在无新状态跃迁时强行改回用户选择；
- 新 batch 重置时允许系统重新施加默认收缩，再按首包规则展开。

**多服务：**

- 每张卡独立状态机；A 出字只展开 A，不连坐 B。

### 2.2 高度策略

| 场景 | 策略 |
|------|------|
| 冷启动 show 前 | 高度必须已等于「当前 DOM：空闲 + N 张收缩卡」的实测高度 |
| 空闲 / 配置变更增删启用服务（非翻译中） | 同步卡片集合后 `setSize` 一次到新稳定高度 |
| 翻译已开始、仍全员收缩 | **尽量不改窗高** |
| 某卡首包展开 / 失败展开 | **允许长高**（有意义） |
| 流式变长 / 展开全文 | 继续由 `ResizeObserver` 跟随；超过屏高 80% 由内容区滚动（现有 maxHeight 行为保留） |

高度 **不** 使用与真实 DOM 脱节的固定魔法数作为 show 后的最终值；`tauri.conf` 中的初始宽高仅作创建期占位，show 前必须以布局结果校正。

### 2.3 冷启动窗口生命周期

```
进程启动
  → 创建 main（visible=false，占位尺寸可先 420×估算）
  → 托盘 / 快捷键等照常
  → WebView 加载 translate.html → Vue 挂载
  → get_app_config → 按启用服务建 N 张收缩卡
  → 首帧布局 + usePopupHeight 完成至少一次 setSize
  → show + focus
用户看到：正确尺寸的完整空闲弹窗（收缩卡列表）
```

**Ready 定义（窄）：**

- Vue 根已挂载；
- 已根据配置（或明确降级）渲染卡片集合；
- 至少一次基于真实 DOM 的 `setSize` 完成；
- （推荐）再等一帧 paint（`requestAnimationFrame` 双 rAF 或等价）。

**Ready 不包含：**

- 翻译请求、OCR、模型列表探测、历史加载等。

**超时兜底：**

- 若超过约 2s 仍未 ready，强制 `show`，避免永远不可见；可打 warn 日志。

**二次唤起（快捷键 / 托盘）：**

- 窗口已存在时仅 `show` + 定位（现有 `show_popup`），不重走冷启动 ready 门闩；
- 若 hide 期间 DOM 仍在，应接近瞬时显示。

## 三、技术方案

### 3.1 后端 / 窗口配置

1. **`tauri.conf.json` `main` 窗口**
   - 增加 `"visible": false`（或等价：创建时不可见）。
   - 保留 `width: 420`、`transparent: true`、`decorations: false` 等；`height` 可改为更接近空态的占位值（实现时按空态实测微调），不作为最终权威高度。

2. **`src-tauri/src/lib.rs` setup**
   - 去掉（或条件化）启动末尾对 `main` 的无条件 `show` + `set_focus`。
   - 冷启动显式 show 改由前端 ready 路径触发（见 3.2）；若需 Rust 侧 command，可新增薄 command（如 `show_main_window`）或直接用前端 `getCurrentWindow().show()`（需 capabilities 已有 show 权限则用现有；若无则补 `core:window:allow-show` 等）。

3. **`popup_window::show_popup`**
   - 行为保持：定位 + show + focus，供划词/OCR/托盘唤起。
   - 不在此路径等待前端 ready（热窗假设）。

4. **设置窗 / overlay**
   - 继续预创建策略；不得抢 main 的 show 时机。无必要不在启动时 show 设置窗。

### 3.2 前端 ready → show

在 `TranslationPopup`（或 `usePopupHeight` 旁路 composable）中：

1. 标记 `hasShownOnce`（或模块级 flag），避免重复 show。
2. 流水线建议顺序：
   - `initCards()`（`get_app_config` + 语言）完成且卡片为收缩态；
   - `nextTick` + 一次 `adjust`/`setSize`；
   - 双 rAF（或 `document.fonts.ready` 与 rAF 组合，best-effort）；
   - `getCurrentWindow().show()` + `setFocus()`。
3. 超时定时器与上述路径 race，先到者 show，另一路径 no-op。
4. **dev 与 release**：逻辑相同；验收以 release 体感为准（dev + Vite 更慢属预期）。

### 3.3 可选增强：静态首屏壳（推荐，非阻塞）

为进一步压白屏与「无反馈等待」：

- 在 `translate.html` 或入口 CSS 中提供与弹窗同宽的透明/卡片色壳（不必像素级复刻全部控件）；
- Vue 挂载后替换为真实树。

若工期紧，可 **第一期只做 visible=false + ready show + 收缩状态机**；壳作为同规格内可选 Task，不阻塞主路径。

### 3.4 卡片状态机实现落点

**主要修改：**

- `useTranslationEvents.ts`
  - `started`：**不要** `collapsed = false`；新 batch / 该卡进入 translating 时设 `collapsed = true`（或保持 true），清空 text 等逻辑保留。
  - `delta`：若该卡此前 text 为空且本次追加后非空 → `collapsed = false`（尊重手动标记，见下）。
  - `failed`：`collapsed = false`。
  - `finished`：若需展示结果且未手动锁折 → `collapsed = false`。
- `TranslationPopup.vue` `refreshCardsFromConfig` / `initCards`
  - 新建空闲卡：`collapsed: true`（无原文或空闲规则与 2.1 一致）。
  - 非翻译中同步配置时，新建卡默认收缩。
- **手动覆盖**：为 `CardState` 增加可选字段如 `collapseUserOverride: boolean | null`，或在用户 toggle 时置位；自动规则仅在 `!userOverride` 或新 batch 清 override 时写入。

**Header 等待可见性：**

- 收缩且 translating 时必须显示 loading 点（`ResultCard` / `ResultCardView` 已有 loading 路径，确保收缩态仍渲染 status 点）。

### 3.5 高度模块

- `usePopupHeight` 继续 ResizeObserver + rAF 节流 `setSize(width: 420, height: h)`。
- 与 ready 门闩集成：对外暴露「首次有效 setSize 完成」回调或 Promise，供 show 使用。
- show 之前的 setSize 必须发生；show 之后仅跟随真实内容变化。
- 宽度保持 420（与 `.popup` 一致）；若 body 留白/阴影策略有 padding，以现网 CSS 为准，本规格不改为 452，除非实现中发现与阴影裁切冲突再单列修复。

### 3.6 权限与 API

- 确认前端 `show` / `setFocus` / `setSize` 所需 capabilities；缺则补 `capabilities/default.json`。
- 继续 `withGlobalTauri` + `window.__TAURI__`，不强制引入 `@tauri-apps/api` 包路径（与弹窗现约定一致）。

## 四、范围

### 本次做

- 冷启动 main 不可见 → ready 后 show。
- 去掉启动路径无条件 show。
- 卡片收缩状态机（含失败展开、首包展开、新 batch 收回）。
- 用户手动折叠覆盖（最小可用：本 batch 内不被无意义覆盖）。
- 高度与 N 张收缩卡 / 展开联动，show 前校正。
- 超时强制 show。
- 单测：状态机纯逻辑（事件 → collapsed 变化）。
- 文档：`AGENTS.md` / `CLAUDE.md` 启动与折叠行为同步。

### 本次不做

- 「启动时显示弹窗」用户设置开关（可列后续）。
- 默认打开设置页或仅托盘。
- 重做 ResultCard 视觉。
- 改后端翻译协议、session 格式。
- 为设置窗单独做 hide-until-ready（非本迭代焦点；若顺手统一可记 follow-up）。

## 五、测试与验收

### 5.1 自动化

- 前端 vitest：给定事件序列，断言各卡 `collapsed`：
  - 空闲 init → true；
  - started → true；
  - 首 delta 非空 → false；
  - failed 无字 → false；
  - 仅 finished 有 fullText → false；
  - 多服务独立展开；
  - 新 batch 先收回。
- 若抽离 ready 判定纯函数，可测「条件未满足不 show / 满足可 show」（mock 窗 API）。

### 5.2 手动（Windows release 优先）

1. 冷启动：托盘出现后，弹窗首次出现即为完整 UI + 稳定尺寸，无大→小、无明显白屏。
2. 启用 1 / 3 / 5 个服务分别冷启动：高度随 N 变，且首次出现即正确。
3. 空闲卡均为收缩；发起翻译后至首包前仍收缩；首包后对应卡展开、窗高合理增加。
4. 故意失败的服务：无正文也可见错误（卡展开或错误可见）。
5. 用户手动折叠某卡后，同 batch 内自动逻辑不立刻强行展开（除非新的失败/新 batch 等约定例外——实现按 2.1）。
6. 关闭弹窗（hide）后再 Alt+D：热窗快速显示，无冷启动级闪动。
7. dev 模式允许更慢，但不允许永远不 show。

## 六、风险与缓解

| 风险 | 缓解 |
|------|------|
| ready 条件过宽导致“启动慢”体感 | Ready 仅 UI 壳 + N 卡布局；托盘先可见 |
| ready 过严永不 show | 2s 超时强制 show |
| 透明窗 + setSize 仍有 1 帧毛刺 | show 前 setSize + 双 rAF；可选静态壳 |
| 手动与自动折叠冲突 | userOverride + 新 batch 重置 |
| 无 delta 只 finished 的 provider | finished 分支也展开 |
| conf 占位高与真实差大 | show 前必校正；占位尽量接近空态 |

## 七、实现分期建议

| 期 | 内容 | 验收重点 |
|----|------|----------|
| P0 | visible=false、去 setup show、ready show、超时、收缩状态机、show 前 setSize | 无闪 + 折叠正确 |
| P1 | 手动 override 完善、静态壳、占位 height 微调 | 体感再压一档 |
| P2（后续规格） | 「启动时显示弹窗」配置项 | 产品可选 |

本规格默认交付 **P0**；P1 若成本低可同 PR，不强制。

## 八、文档同步（实现收尾）

- `AGENTS.md` / `CLAUDE.md`：启动时 main 初始不可见、前端 ready 后 show；卡片默认收缩与首包展开规则。
- 如有 roadmap 条目涉及弹窗启动体验，勾选更新。

## 九、决策记录

| 决策 | 选择 |
|------|------|
| 冷启动第一眼 | 尽快完整翻译弹窗 |
| 仅托盘 / 默认设置页 | 不做本规格默认 |
| 空闲卡 | 收缩 |
| started 且无正文 | 仍收缩 + header loading |
| 首可见正文 / 失败 | 该卡自动展开 |
| 用户手动折叠 | 本 batch 内优先尊重 |
| Ready 范围 | UI+布局，不含业务全量 |
| 高度权威 | 真实 DOM + setSize，非 conf 常量 |
