# 设置页高保真原型同步（导航动效 · 服务详情 · 主题色）

> 日期：2026-08-02  
> 状态：已确认设计，待实现  
> 规模：M  
> 原型源：`C:\Users\xdj\IdeaProjects\LLM\OpenDesignProjects\shizi\src`  
> 策略：视觉与交互对齐原型；产品业务字段嵌回，不回退；主题色仅前端预览持久化

## 1. 背景与目标

产品设置页已从早期原型搬迁，但后续 OpenDesign 又迭代了：

1. 左侧导航滑动高亮与过渡  
2. 服务 Tab、历史筛选分段的指示器动画  
3. 翻译服务详情页信息层级与分组  
4. 品牌强调色「橙色」为默认，并保留双色切换  

目标：在 **设置页前端** 同步上述视觉与交互，便于对照原型验收；不改变翻译/OCR 运行时协议与后端配置模型（主题色除外的业务字段保持现状）。

## 2. 已确认决策

| 项 | 决策 |
|----|------|
| 规模 | M：本对话确认后实现，不默认写 formal plan / 不跨对话 L 交接 |
| 服务详情同步策略 | **尽量整页照搬原型版式**；多协议、接入路径、真实 Key 校验、i18n、`openExternal` 等产品能力 **嵌回** 骨架，禁止功能回退 |
| 主题色默认 | **橙色**（与原型 `accent-orange` HSL 一致） |
| 主题色选项 | **橙色** / **蓝色**（文案不用「柿子橙 / 默认蓝」） |
| 主题色 UI | 通用 → 外观 → 「主题色」一行；**不**包在 DevOnly |
| 主题色持久化 | 仅前端 `general.accentTheme`；走现有 settings store；**不进** Rust `AppConfig` |
| 分类页切换 | 右侧内容 **无** 切换动画（与原型「点即到」一致）；动画仅限侧栏指示器 + 模块内 Tab/分段 |
| OCR 详情 | 本轮以 Tab 指示器动画为主；OCR 内容区不强制大改 |
| 浅色/深色主题 | 保持现网（DevOnly / wip），本轮不产品化 |

## 3. 范围

### 做

1. **左侧导航**（`SettingsSidebar.vue`）  
   - 滑动 pill 指示器（`settings-nav-indicator`）  
   - 选中态改为指示器 + 文字/图标颜色，去掉硬切整行 `bg-accent` 作为唯一高亮（与原型一致）  
   - `ResizeObserver` / `resize` / `watch(modelValue)` 更新位置  
   - 过渡缓动与 `prefers-reduced-motion`  
   - 保留 i18n 分类、底部保存状态与重试  

2. **服务 Tab 指示器**（`ServicesPanel.vue`）  
   - 「翻译服务 / 文字识别」共用一条滑动下划线（`module-tab-indicator`）  
   - 内容区瞬时切换  

3. **历史筛选分段指示器**（`HistoryPanel.vue`）  
   - 筛选栏内滑动 pill（`module-seg-indicator`）  
   - 列表瞬时切换；清空按钮等现网能力保留  

4. **翻译服务详情**（`ServicesPanel.vue` 右侧 translate 分支）  
   - Header：名称 / 重命名 / AI 角标 / 描述 / 可选 `detail` 段落 / 文档与 API Key 外链  
   - 微软等免 Key：三栏能力卡 + 状态绿条（对齐原型结构；文案走 i18n）  
   - 其余：单组 **基础配置**（接口地址 + 标题旁协议信息、API Key、默认模型）；多协议选择与接入路径嵌在该组内  
   - 折叠 **高级 · 提示词与推理**  
   - 危险操作、缺 Key 警告  

5. **主题色**  
   - `types`：`AccentTheme = 'orange' | 'blue'`，`GeneralSettings.accentTheme`，默认 `'orange'`  
   - `main.css`：默认 primary/ring/accent 走橙色；蓝色用 class（如 `accent-blue`）覆盖；深色模式对称定义  
   - store：`applyTheme` 同步 `dark` 与 accent class；watch `theme` + `accentTheme`  
   - `GeneralPanel`：外观组增加主题色 `SettingSelect`  
   - i18n：字段/描述/选项文案  

6. **测试**  
   - 主题 token / accent 默认与 class 切换（扩展现有 `theme-colors.test.ts` 或 store 单测）  
   - 必要时补充 types/defaults 相关断言  

### 不做（YAGNI）

- 后端 `AppConfig` 增加 accent / theme 字段  
- 翻译弹窗、托盘、OCR 窗口视觉全量同步  
- 设置分类之间的内容区 enter/leave 动画  
- 新增多于橙/蓝的主题色  
- 照搬原型后删除产品多协议 / 接入路径 / 真实校验  
- 重写 OCR 详情整页（除非阻塞 Tab 动画）  

## 4. 视觉与交互规格

### 4.1 侧栏指示器

- 容器：`nav ul.relative`  
- 指示器：`absolute inset-x-0 z-0 rounded-md bg-accent`，`top`/`height` 像素定位  
- 就绪前 `opacity-0`，就绪后 `settings-nav-indicator--ready`  
- 过渡：`top/height 280ms cubic-bezier(0.16, 1, 0.3, 1)`，`opacity 160ms`  
- 项：`relative z-[1]`；选中文字 `text-accent-foreground` / 图标 `text-primary`；非选中 `hover:bg-accent/40`（勿与指示器双重实心底叠死）  

### 4.2 服务 Tab 下划线

- `relative` tablist；指示器 `absolute -bottom-px h-0.5 rounded-full bg-primary`  
- `left`/`width` 跟踪当前 tab 按钮几何  
- 同套 280ms 缓动；`prefers-reduced-motion` 关闭  

### 4.3 历史分段 pill

- 筛选条 `relative`；指示器 `absolute z-0 rounded bg-accent`，跟踪选中 filter 按钮的 `left/top/width/height`  
- 按钮 `relative z-[1]`；选中仅靠指示器 + 前景色，不再单独 `bg-accent` 硬切（与原型一致）  
- 「清空历史」保持右侧，不参与指示器  

### 4.4 服务详情信息层级（翻译）

顺序固定：

```
header
  ├ name (+ rename, AI badge)
  ├ description
  ├ detail?（渠道长说明，有则显示）
  └ docs / apiKey 外链 chips
微软等免配置渠道
  ├ 三栏能力
  └ 状态条
需配置渠道
  └ SettingGroup「基础配置」bare
       ├ 协议（单协议文案 / 多协议 select）
       ├ 接入路径?（endpointPresets）
       ├ 接口地址
       ├ API Key?
       └ 默认模型
LLM / custom
  └ 折叠「高级 · 提示词与推理」
危险操作 · 删除
缺 Key 警告
```

「整页照搬」= 上述版式与组件外观；**字段集合以产品 `ServiceMeta` / 实例为准**。

### 4.5 主题色 CSS

浅色默认（橙色）：

```css
/* 写入 :root 主色，替换当前蓝系 primary */
--primary: 20 74% 48%;
--primary-foreground: 0 0% 100%;
--ring: 20 74% 48%;
--accent: 20 74% 95%;
--accent-foreground: 20 74% 38%;
/* sidebar-primary / sidebar-accent / chart-1 等同主色族对齐 */
```

蓝色覆盖（`html.accent-blue` 或 `html:not(...)` 策略二选一，实现时固定一种并写清）：

```css
html.accent-blue {
  --primary: 222 70% 48%;
  /* 对称覆盖 primary-foreground / ring / accent / sidebar-* */
}
```

深色：默认橙提亮（对齐原型 `.dark.accent-orange` 量级）；`accent-blue` 深色用现网蓝系暗色值。

应用：

```ts
root.classList.toggle('dark', resolved === 'dark')
root.classList.toggle('accent-blue', state.general.accentTheme === 'blue')
// 默认 orange：不挂 accent-blue 即可
```

## 5. 数据模型

```ts
export type AccentTheme = 'orange' | 'blue'

export interface GeneralSettings {
  // ...existing
  /** 品牌强调色；仅前端，默认 orange */
  accentTheme: AccentTheme
}
```

- 默认值：`accentTheme: 'orange'`  
- 从持久化恢复：未知/缺省 → `'orange'`  
- **不** 写入 `projectToAppConfig` / 后端  

## 6. i18n（最低集合）

中文（其它语言包同步键）：

| key | 建议文案 |
|-----|----------|
| `settings.field.accentTheme` | 主题色 |
| `settings.description.accentTheme` | 设置页与主界面的品牌强调色。 |
| `settings.option.accentOrange` | 橙色 |
| `settings.option.accentBlue` | 蓝色 |

服务详情若新增/改写分组标题（如「基础配置」），复用或新增 `settings.group.*` 键，避免硬编码中文（与现网 i18n 规范一致）。

## 7. 实现注意

- 指示器首次 paint 前避免闪烁：`indicatorReady` 模式与原型一致  
- WebView2 / Windows：确认 `scrollbar-gutter` 等既有修复不受主题变量影响  
- 服务详情改版时保持 `keyStatusFor` / `onPullModels` / `onEndpointPresetChange` 等现网逻辑  
- 侧栏、Tab、历史指示器逻辑可复制原型，但 **i18n / 产品字段 / 命令封装** 不得改回原型 mock  
- 改 `main.css` 后跑 `theme-colors` 相关测试；设置 store 若有 defaults 测试一并更新  

## 8. 验收标准

1. 设置侧栏切换分类：高亮 pill 平滑滑动，无整行闪切  
2. 服务「翻译服务 ↔ 文字识别」：下划线滑动，内容即时切换  
3. 历史筛选 Tab：分段 pill 滑动，列表即时切换  
4. 打开 LLM 服务详情：版式接近原型单组基础配置 + 折叠高级；多协议/接入路径仍可用  
5. 打开微软翻译：三栏能力 + 绿条，无 Key 表单  
6. 默认主题色为橙色（primary 按钮、侧栏选中图标、Tab 下划线等）  
7. 外观 → 主题色 可切蓝色并即时生效；重启设置页后选择仍在（前端持久化）  
8. `prefers-reduced-motion: reduce` 下指示器无过渡动画  
9. 前端 typecheck / 相关 vitest 通过  

## 9. 风险与缓解

| 风险 | 缓解 |
|------|------|
| 整页照搬导致漏嵌产品字段 | 实现时对照现网 `ServicesPanel` 字段清单逐项勾选 |
| 默认改橙影响未测试深色对比 | 双模式肉眼验收 + token 测试断言关键 CSS 片段 |
| 指示器在 dev 热更新后错位 | `ResizeObserver` + tab/filter 变化 `nextTick` 更新 |

## 10. 主要文件

| 文件 | 变更 |
|------|------|
| `frontend/src/settings/SettingsSidebar.vue` | 滑动指示器 + 样式 |
| `frontend/src/settings/panels/ServicesPanel.vue` | Tab 指示器 + 翻译详情版式 |
| `frontend/src/settings/panels/HistoryPanel.vue` | 筛选分段指示器 |
| `frontend/src/settings/panels/GeneralPanel.vue` | 主题色行 |
| `frontend/src/settings/types.ts` | `AccentTheme` |
| `frontend/src/settings/stores/settings.ts` | 默认值、恢复、applyTheme |
| `frontend/src/styles/main.css` | 橙默认 + 蓝 class + 深色 |
| `frontend/src/i18n/locales/*` | 文案键 |
| `frontend/src/styles/theme-colors.test.ts` 等 | 断言 |

## 11. 实现节奏（M，无 formal plan 文件）

1. 主题色 token + types/store + GeneralPanel + 测试  
2. 侧栏指示器  
3. 服务 Tab + 历史分段指示器  
4. 翻译服务详情版式照搬并嵌回字段  
5. 手动/自动验证与文档状态更新  

提交建议按功能分组（2–4 项一组），Conventional Commits 中文描述。
