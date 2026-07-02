# 设置页 Vue 3 重构 设计规格

> **关联里程碑**：前端体验优化（Tauri UI 路线）—— 第一个任务。
> **范围**：仅重构设置页（`settings.html`）。翻译页暂保持现状（后续迁移），overlay 永久保持纯静态不迁。
> **原型图**：由用户提供 open design 高保真原型图，UI 细节（布局/间距/窗口尺寸定稿）在原型图到位后对齐。本 spec 先定集成架构与交互行为，UI 视觉细节标注为"待原型图定稿"。

## 1. 目标与架构边界

### 1.1 目标

插入一个新里程碑"前端体验优化（Tauri UI 路线）"：基于 Tauri UI + Rust 先做体验优化，暂不切 Slint。第一步用 Vue 3 + Vite + Tailwind CSS v4 + shadcn-vue + Iconify 重构当前设置页。

动机：实测 Tauri UI（WebView）效果并未差到必须切 Slint，先在当前路线下用现代化前端框架升级 UI；后续根据实际体验再决定是否切 Slint。

### 1.2 架构边界

- **引入 Vite 构建步骤**：项目从"无构建"变为"有构建"。
- **仅设置页纳入 Vue 框架**；`translate.html` / `overlay.html` 作为纯静态资源放 `frontend/public/`，由 Vite 原样拷贝到产物根，不进构建、不进框架。
- **后端零改动**：`WebviewUrl::App("xxx.html")` 在 dev（`devUrl`）与 prod（`frontendDist`/`dist`）下都命中服务根路径，三个窗口 url 不变。后端 command 仍为 `get_app_config` / `save_app_config`，`AppConfig` shape 不动。
- **配置数据流不变**：设置页通过 `window.__TAURI__.core.invoke` 调用上述两个 command，与现状一致。

### 1.3 三页定位

| 页面 | 当前 | 重构后 | 后续 |
|---|---|---|---|
| `settings.html` | 纯静态 HTML/JS/CSS | Vue 3 + Vite + shadcn-vue | — |
| `translate.html` | 纯静态 | 纯静态（`public/`） | 后续迁移进 Vite |
| `overlay.html` | 纯静态 | 纯静态（`public/`） | **永久不迁**（性能敏感的 canvas 蒙版） |

### 1.4 迁移隔离与回滚

Vite 工程在 `frontend/` 内原地替换旧 `settings.*`；回退靠 `git revert`，不在仓库留 legacy 文件。设置页崩了不影响翻译页/OCR —— 它们在 `public/` 独立加载，应用不至于完全不可用。

## 2. 构建集成与目录结构

### 2.1 dev/build 接入

- **项目根 `package.json`**：新增 `"dev": "vite"` / `"build": "vite build"` 脚本，新增 Vite + Vue + Tailwind v4 + shadcn-vue + Iconify 相关依赖。
- **`tauri.conf.json`**：
  - `frontendDist`：`../frontend` → `../frontend/dist`
  - 新增 `devUrl: "http://localhost:5173"`
  - 新增 `beforeDevCommand: "npm run dev"`
  - 新增 `beforeBuildCommand: "npm run build"`
- **dev**：Tauri 拉起 Vite dev server，主窗口加载 `http://localhost:5173/settings.html`，HMR 热更新。translate/overlay 在 dev server 下由 Vite `public/` 静态服务，路径与 prod 一致。
- **build**：`vite build` 产出到 `frontend/dist/`，Tauri 打包该目录；产物根平铺 `settings.html` / `translate.html` / `overlay.html`。

### 2.2 Tauri API 调用

保留 `withGlobalTauri: true`，三个页面统一走 `window.__TAURI__.core.invoke`。设置页包薄封装 `src/lib/tauri.ts`，**不引 `@tauri-apps/api`**，避免与未迁移页面割裂。后续翻译页迁移时再统一评估是否切 ESM 包。

### 2.3 目录结构

Vite 工程在 `frontend/` 内原地替换：

```
frontend/
├── settings.html              # 入口：<script type="module" src="/src/settings/main.ts">
├── package.json               # Vite + Vue + Tailwind + shadcn-vue 依赖
├── vite.config.ts             # 多入口 + Tailwind 插件 + server.strictPort
├── tsconfig.json
├── components.json            # shadcn-vue CLI 配置
├── src/
│   ├── settings/
│   │   ├── main.ts            # Vue 应用挂载入口
│   │   ├── App.vue            # 设置页根组件
│   │   └── components/        # 业务组件（见 §3.2）
│   ├── components/ui/         # shadcn-vue 拷贝进来的组件源码
│   ├── lib/
│   │   ├── tauri.ts           # window.__TAURI__.core.invoke 薄封装
│   │   └── config.ts          # readForm/writeForm/validateConfig
│   ├── types/
│   │   └── config.ts          # AppConfig TS 类型（对齐后端）
│   └── styles/
│       └── main.css           # @import "tailwindcss" + shadcn 主题变量
├── public/
│   ├── translate.html         # 翻译页（纯静态，后续迁）
│   ├── translate.js
│   ├── translate.css
│   ├── overlay.html           # overlay（纯静态，永久不迁）
│   └── overlay.*（若存在独立 js/css）
└── dist/                      # 构建产物（.gitignore）
```

要点：
- Vite 入口直接用 `settings.html`（不建 `index.html`）；`build.rollupOptions.input` 指向 `settings.html`。
- translate/overlay 平铺进 `public/` 根（**非子目录**），保证 `WebviewUrl::App("translate.html")` 仍命中服务根路径。
- overlay 的 JS/CSS：实现时核对是否内联在 html；若有独立文件一并搬进 `public/`。
- `frontend/node_modules/` 与 `frontend/dist/` 都不提交（`.gitignore`）。

### 2.4 配置类型与校验

- 前端 `src/types/config.ts` 手工定义 `AppConfig` 接口，逐字段对齐后端 `AppConfig` 的 camelCase shape：
  ```ts
  interface AppConfig {
    provider: string;            // 'openai-compatible' | 'claude' | 'mock'
    targetLang: string;
    openaiCompatible: { apiKey: string | null; baseUrl: string; model: string; timeoutSeconds: number };
    claude: { apiKey: string | null; baseUrl: string; model: string; timeoutSeconds: number; enableThinking: boolean };
    popupPrecreate: boolean;
    overlayPrecreate: boolean;
    collectUsage: boolean;
  }
  ```
- `src/lib/config.ts` 平移现有 `settings.js` 的 `readForm/writeForm/validateConfig` 逻辑：
  - `validateConfig`：mock 跳过；其余校验 baseUrl 是有效 http(s) URL、model 非空、timeoutSeconds 为 1-600 整数。
- 后端 `normalized()` 兜底逻辑不变。

### 2.5 窗口尺寸

基线 560×640，`resizable: true`，新增 `minWidth: 480, minHeight: 480`。均标注"待原型图定稿"，原型图到位后微调。

## 3. 设置页组件与交互行为

### 3.1 信息架构（基于现有字段，待原型图定稿）

- 顶部标题区：Shizi + "设置"。
- 目标语言：Input。
- Provider 选择：Select（openai-compatible / claude / mock）。
- OpenAI Compatible 段：apiKey（密码框 + 显隐按钮）/ baseUrl / model / timeout 秒。
- Claude 段：同上 + enableThinking（Switch）。
- 窗口策略段：popupPrecreate / overlayPrecreate / collectUsage（三个 Switch）。
- 保存按钮 + 状态提示文本（内联，不引 Toast）。
- API Key 明文警告文案保留。

### 3.2 组件拆分（Vue SFC）

- `App.vue`：根组件，编排各 section，持有加载/保存状态。
- `TargetLangSection.vue`
- `ProviderSelect.vue`：含切换显隐两个 provider 段的逻辑。
- `OpenAiSection.vue`
- `ClaudeSection.vue`
- `StrategySection.vue`
- `SaveBar.vue`：保存按钮 + 状态文本。
- shadcn-vue 组件按需（源码落 `src/components/ui/`）：Input / Select / Switch / Label / Button / Card。密码显隐用 Button + Input 组合，不引额外组件。
- Iconify：provider 图标、密码显隐眼睛图标等小处。

### 3.3 交互行为（全部平移现状，行为不变）

1. **加载**：挂载时 `invoke('get_app_config')` 填表；失败显示错误文本。
2. **Provider 切换**：Select 变化时显隐对应 provider 段（mock 时两段都隐藏）。
3. **保存**：读表单 → `validateConfig` → 校验失败则状态文本报错且不提交 → `invoke('save_app_config', {config})` → 成功回填表单 → 对比 `popupPrecreate`/`overlayPrecreate` 是否变化：
   - 变化 → 提示"配置已保存，窗口策略切换需重启应用生效"。
   - 未变 → 提示"配置已保存，下一次翻译生效"。
4. **保存按钮**：提交期间 `disabled` + 文案"保存中..."，结束恢复。
5. **API Key**：密码框默认隐藏，显隐按钮切换 `type`（password ↔ text）。

### 3.4 YAGNI 边界

不引 Toast/通知系统、不加表单未保存离开拦截、不加自动保存、不引路由、不引状态管理库（Pinia 等）。配置类型双写（前端 TS + 后端 Rust），靠 spec/README 约束同步。

## 4. 验证策略

- **Rust 侧**：零改动，`cd src-tauri && cargo test` 与 `cargo build` 应继续全绿（回归基线）。
- **前端类型检查**：`vue-tsc`（或 `tsc --noEmit`）通过，确保 `AppConfig` 类型与调用对齐。
- **前端构建**：`npm run build`（`vite build`）成功产出 `frontend/dist/`，产物根包含 `settings.html` / `translate.html` / `overlay.html` 三个平铺 html。
- **translate/overlay 语法**：`node --check frontend/public/translate.js`（及 overlay 若有独立 js）通过；搬进 public 后内容不变。
- **手动验证（Windows）**：
  - `npm run tauri dev` 下设置页 HMR 可用、能加载/保存配置、provider 切换显隐正确、密码显隐、保存提示文案与旧版一致。
  - `Alt+T` 划词翻译、`Alt+O` 截图 OCR 行为不回归（验证 `public/` 下两页未被牵连）。
- **原型图对齐**：原型图到位后做一次 UI 对照验证，窗口尺寸据此定稿。

## 5. 风险与缓解

1. **Vite dev server 端口冲突**（5173 被占）→ Vite 自动递增端口会与 Tauri `devUrl` 写死 5173 失配。缓解：`vite.config.ts` 设 `server.strictPort: true`，让 Vite 失败而非静默换端口，便于及时发现。
2. **translate/overlay 相对路径在 public/ 下失效**→ 它们引用各自 js/css 用相对文件名，平铺在根不受影响；实现时核对 overlay 是否有内联脚本，逐一验证。
3. **`withGlobalTauri` 在 Vite dev server 下注入时机**→ `window.__TAURI__` 由 Tauri 注入，dev server 模式下 WebView 仍是 Tauri 容器，注入应正常；若设置页 `invoke` 时 `window.__TAURI__` 未就绪，需加就绪检测/重试。**标注为待验证项。**
4. **shadcn-vue + Tailwind v4 兼容**→ v4 适配仍在过渡，初始化时以 shadcn-vue 官方 v4 指引为准，遇组件源码 v3/v4 语法差异就地调整。
5. **前端 AppConfig 类型与后端漂移**→ 字段稳定，靠 spec/README 约束可接受；后续字段频繁变动再评估 `ts-rs` 自动生成。

## 6. 文档同步（协作规范第 2 条硬门禁）

- 本 spec：`docs/superpowers/specs/2026-07-02-settings-vue-refactor-design.md`。
- `README.md`：开发命令区补充 `npm install`（首次需装 Vite 依赖）、`npm run dev`/`build` 说明；"当前能力"区设置页技术栈更新为 Vue + shadcn-vue。
- `CLAUDE.md` / `AGENTS.md`：项目结构区 `frontend/` 描述更新（"无构建"→"Vite 工程，设置页 Vue，translate/overlay 纯静态 public"）；常用命令区同步；两文件保持同步。
- `docs/roadmap/progressive-development-plan.md`：插入新里程碑"前端体验优化（Tauri UI 路线）"，设置页重构为第一个任务。
- `plugins.md`：新增 Vite/Vue/Tailwind/shadcn-vue/Iconify 依赖记录。

## 7. 不在本次范围

翻译页迁移、overlay 任何改动、Slint、SecretStore、多 provider 管理增强、性能埋点 —— 均留给后续里程碑。
