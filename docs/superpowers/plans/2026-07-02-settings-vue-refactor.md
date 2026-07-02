# 设置页 Vue 3 重构 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 用 Vue 3 + Vite + Tailwind v4 + shadcn-vue + Iconify 在 `frontend/` 内原地重构设置页，translate/overlay 平铺进 `public/` 保持纯静态，后端零改动。

**架构：** 引入 Vite 构建步骤。`settings.html` 作为 Vite 入口加载 Vue 应用；`translate.*` / `overlay.html` 搬进 `frontend/public/` 由 Vite 原样拷贝到产物根。`tauri.conf.json` 改 `frontendDist` 为 `../frontend/dist` 并加 `devUrl` / `beforeDevCommand` / `beforeBuildCommand`。三页统一走 `window.__TAURI__.core.invoke`，不引 `@tauri-apps/api`。

**技术栈：** Vite 7、Vue 3.5、TypeScript、Tailwind CSS v4、shadcn-vue（按需拷贝源码到 `src/components/ui/`）、Iconify (`@iconify/vue`)、vitest（纯函数单测）。

**关联 spec：** `docs/superpowers/specs/2026-07-02-settings-vue-refactor-design.md`

---

## 关于 UI 视觉细节的约定（先读这一节）

spec 已明确：**open design 高保真原型图尚未提供**，布局/间距/组件视觉/窗口尺寸定稿待原型图对齐。本计划因此分两类任务：

1. **可定稿任务（任务 1–8）**：构建集成、类型、校验逻辑、组件骨架、交互行为、文档同步 —— 这些与视觉无关或已由 spec 定死，**必须完整实现，不留占位**。
2. **待原型图定稿任务（任务 9）**：UI 视觉打磨（间距、配色、卡片层级、窗口尺寸定稿）。这一节**有意保留为带显式 `待原型图定稿` 标记的占位任务**，是 spec 授权的范围裁剪，不是计划缺陷。原型图到齐后在同一对话内补全为可执行步骤。

执行者在做完任务 1–8 后，应停在任务 9 之前，向用户确认原型图是否就绪：若就绪则补全任务 9 再收尾；若未就绪则按任务 8 的"骨架可交付态"提交并等待。

---

## 文件结构

### 创建

- `frontend/package.json` — Vite + Vue + Tailwind v4 + shadcn-vue + Iconify + vitest 依赖与脚本
- `frontend/vite.config.ts` — 多入口（仅 `settings.html`）、Tailwind v4 插件、`server.strictPort: true`、`server.port: 5173`、vitest 配置
- `frontend/tsconfig.json` — TS 严格配置，`@/*` → `src/*` 路径别名
- `frontend/tsconfig.node.json` — 给 `vite.config.ts` 用的 node 配置
- `frontend/components.json` — shadcn-vue CLI 配置（style: new-york、aliases 指向 `src/`）
- `frontend/.gitignore` — 本地补 `dist/`（`node_modules/` 已在根 `.gitignore`）
- `frontend/settings.html` — Vite 入口 HTML，`<script type="module" src="/src/settings/main.ts">`
- `frontend/src/styles/main.css` — `@import "tailwindcss";` + shadcn 主题 CSS 变量（`:root` / `.dark`）
- `frontend/src/lib/tauri.ts` — `window.__TAURI__.core.invoke` 薄封装：`invokeGetAppConfig()` / `invokeSaveAppConfig(config)`
- `frontend/src/types/config.ts` — `AppConfig` TS 类型，逐字段对齐后端 camelCase
- `frontend/src/lib/config.ts` — `readForm/writeForm/validateConfig` 纯函数（从 `settings.js` 平移）
- `frontend/src/settings/main.ts` — Vue 应用挂载入口
- `frontend/src/settings/App.vue` — 设置页根组件，编排 section + 持有加载/保存状态
- `frontend/src/settings/components/TargetLangSection.vue`
- `frontend/src/settings/components/ProviderSelect.vue`
- `frontend/src/settings/components/OpenAiSection.vue`
- `frontend/src/settings/components/ClaudeSection.vue`
- `frontend/src/settings/components/StrategySection.vue`
- `frontend/src/settings/components/SaveBar.vue`
- `frontend/src/settings/components/ApiKeyField.vue` — 密码框 + 显隐按钮（被两个 provider section 复用，DRY）
- `frontend/src/components/ui/*` — shadcn-vue 按需拷贝：button / input / label / select / switch / card
- `frontend/src/lib/config.test.ts` — `validateConfig` 单测

### 移动（git mv，内容不变）

- `frontend/translate.html` → `frontend/public/translate.html`
- `frontend/translate.js` → `frontend/public/translate.js`
- `frontend/translate.css` → `frontend/public/translate.css`
- `frontend/overlay.html` → `frontend/public/overlay.html`

### 删除（Vite 工程替换后旧文件不再需要）

- `frontend/settings.css` — 旧样式，被 Tailwind + shadcn 主题取代
- 旧 `frontend/settings.js` — 被 `src/settings/*` 取代（其逻辑平移到 `src/lib/config.ts`，已在 spec §2.4 列明）
- 旧 `frontend/settings.html` — 被新 Vite 入口 `settings.html` 覆盖（同路径写入，git 视为修改而非新增）

### 修改

- `package.json`（根） — 加 `"dev": "vite"` / `"build": "vite build"` 脚本；`devDependencies` 加 vite/vue/tailwind 等开发依赖（与 `frontend/package.json` 二选一，见任务 1 决策）
- `src-tauri/tauri.conf.json` — `frontendDist` → `../frontend/dist`；加 `devUrl` / `beforeDevCommand` / `beforeBuildCommand`；主窗口尺寸 560×640 + `minWidth/minHeight: 480`
- `.gitignore`（根） — 已有 `frontend/node_modules/`，补 `frontend/dist/`
- `README.md` — 开发命令区补 `npm install` / `npm run dev` / `npm run build`；当前能力区设置页技术栈
- `CLAUDE.md` / `AGENTS.md` — `frontend/` 描述更新；常用命令区同步
- `docs/roadmap/progressive-development-plan.md` — 插入新里程碑，设置页重构为第一个任务
- `plugins.md` — 新增 Vite/Vue/Tailwind/shadcn-vue/Iconify 依赖记录

---

## 任务 1：初始化 Vite 工程与依赖

**决策（先定）：** 依赖装在根 `package.json` 还是 `frontend/package.json`？Tauri 的 `beforeDevCommand` / `beforeBuildCommand` 默认在仓库根执行。为避免两套 `package.json` 混乱，**依赖统一装在根 `package.json`**，`frontend/` 下不单独建 `package.json`。`vite.config.ts` / `tsconfig.json` / `components.json` 仍放 `frontend/`，构建产物在 `frontend/dist/`。spec §2.1 写的"`frontend/package.json`"按此决策调整为根 `package.json`。

**文件：**
- 修改：`package.json`
- 创建：`frontend/vite.config.ts`
- 创建：`frontend/tsconfig.json`
- 创建：`frontend/tsconfig.node.json`
- 创建：`frontend/.gitignore`
- 修改：`.gitignore`

- [ ] **步骤 1：在根 `package.json` 加脚本与依赖**

写入根 `package.json`：

```json
{
  "name": "shizi",
  "private": true,
  "scripts": {
    "tauri": "tauri",
    "dev": "vite --config frontend/vite.config.ts",
    "build": "vite build --config frontend/vite.config.ts",
    "typecheck": "vue-tsc --noEmit -p frontend/tsconfig.json",
    "test": "vitest run --config frontend/vite.config.ts",
    "test:watch": "vitest --config frontend/vite.config.ts"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2",
    "@types/node": "^22",
    "@vitejs/plugin-vue": "^5",
    "tailwindcss": "^4",
    "@tailwindcss/vite": "^4",
    "typescript": "^5.6",
    "vite": "^7",
    "vitest": "^3",
    "vue-tsc": "^2"
  },
  "dependencies": {
    "@iconify/vue": "^4",
    "vue": "^3.5"
  }
}
```

> shadcn-vue 组件源码在任务 5 用 `npx shadcn-vue@latest add` 拷贝，拷贝动作会自动补 `class-variance-authority` / `clsx` / `tailwind-merge` / `reka-ui` 等依赖到根 `package.json`，本步不预装。

- [ ] **步骤 2：创建 `frontend/vite.config.ts`**

```ts
import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import tailwindcss from '@tailwindcss/vite';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const frontendDir = fileURLToPath(new URL('./', import.meta.url));

export default defineConfig({
  root: frontendDir,
  plugins: [vue(), tailwindcss()],
  resolve: {
    alias: {
      '@': resolve(frontendDir, 'src'),
    },
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    rollupOptions: {
      input: resolve(frontendDir, 'settings.html'),
    },
  },
  server: {
    port: 5173,
    strictPort: true,
  },
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts'],
  },
});
```

> 注意：`root` 设为 `frontend/`，使 `settings.html` 与 `public/` 都相对 `frontend/` 解析。`server.strictPort: true` 对应 spec §5 风险 1 的缓解。

- [ ] **步骤 3：创建 `frontend/tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "jsx": "preserve",
    "esModuleInterop": true,
    "skipLibCheck": true,
    "noEmit": true,
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "types": ["vite/client"],
    "baseUrl": ".",
    "paths": {
      "@/*": ["src/*"]
    }
  },
  "include": ["src/**/*.ts", "src/**/*.vue", "settings.html"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

- [ ] **步骤 4：创建 `frontend/tsconfig.node.json`**

```json
{
  "compilerOptions": {
    "composite": true,
    "module": "ESNext",
    "moduleResolution": "bundler",
    "skipLibCheck": true,
    "noEmit": true
  },
  "include": ["vite.config.ts"]
}
```

- [ ] **步骤 5：创建 `frontend/.gitignore` 并更新根 `.gitignore`**

`frontend/.gitignore`：

```
dist/
```

根 `.gitignore` 在 `frontend/node_modules/` 行后补一行：

```
frontend/dist/
```

- [ ] **步骤 6：安装依赖**

运行：`npm install`
预期：依赖装好，`frontend/node_modules/` 生成；无报错。

- [ ] **步骤 7：验证空工程能起 dev server（暂无入口 HTML，预期失败可接受）**

运行：`npm run dev`
预期：Vite 启动报"找不到入口 settings.html"或类似 —— 这是预期的，证明配置已加载。Ctrl+C 退出。下一步建入口后即正常。

- [ ] **步骤 8：Commit**

```bash
git add package.json package-lock.json frontend/vite.config.ts frontend/tsconfig.json frontend/tsconfig.node.json frontend/.gitignore .gitignore
git commit -m "chore(frontend): 初始化 Vite + Vue + Tailwind v4 工程骨架"
```

---

## 任务 2：移动 translate/overlay 到 public/

**文件：**
- 移动：`frontend/translate.html` → `frontend/public/translate.html`
- 移动：`frontend/translate.js` → `frontend/public/translate.js`
- 移动：`frontend/translate.css` → `frontend/public/translate.css`
- 移动：`frontend/overlay.html` → `frontend/public/overlay.html`

- [ ] **步骤 1：git mv 四个文件**

```bash
mkdir -p frontend/public
git mv frontend/translate.html frontend/public/translate.html
git mv frontend/translate.js frontend/public/translate.js
git mv frontend/translate.css frontend/public/translate.css
git mv frontend/overlay.html frontend/public/overlay.html
```

> overlay.html 是内联 `<style>`+`<script>` 的单文件，无附属 js/css 需移动（已核对）。translate.html 引用 `translate.css` / `translate.js` 均为同目录相对名，平铺到 public 根后路径不变。

- [ ] **步骤 2：确认内容未变（git mv 保留历史，不应有 diff）**

运行：`git status`
预期：四个文件显示为 renamed，无内容修改。

- [ ] **步骤 3：语法检查 translate.js 未被破坏**

运行：`node --check frontend/public/translate.js`
预期：无输出（通过）。

- [ ] **步骤 4：Commit**

```bash
git add -A frontend/public
git commit -m "refactor(frontend): translate/overlay 平铺进 public/ 保持纯静态"
```

---

## 任务 3：AppConfig TS 类型（对齐后端）

**文件：**
- 创建：`frontend/src/types/config.ts`

- [ ] **步骤 1：编写类型文件**

```ts
// 与后端 src-tauri/src/core/config/types.rs 的 AppConfig 对齐。
// 后端用 #[serde(rename_all = "camelCase")]，故前端字段全部 camelCase。
// 任何一方增删字段，必须同步本文件与 spec §2.4、README 配置说明。

export type Provider = 'openai-compatible' | 'claude' | 'mock';

export interface OpenAiCompatibleConfig {
  apiKey: string | null;
  baseUrl: string;
  model: string;
  timeoutSeconds: number;
}

export interface ClaudeConfig {
  apiKey: string | null;
  baseUrl: string;
  model: string;
  timeoutSeconds: number;
  enableThinking: boolean;
}

export interface AppConfig {
  provider: Provider;
  targetLang: string;
  openaiCompatible: OpenAiCompatibleConfig;
  claude: ClaudeConfig;
  popupPrecreate: boolean;
  overlayPrecreate: boolean;
  collectUsage: boolean;
}
```

- [ ] **步骤 2：类型检查**

运行：`npm run typecheck`
预期：通过（此刻无引用，仅类型定义，不应报错）。

- [ ] **步骤 3：Commit**

```bash
git add frontend/src/types/config.ts
git commit -m "feat(frontend): 新增 AppConfig TS 类型对齐后端"
```

---

## 任务 4：validateConfig 纯函数 + TDD

> 这是行为不变性最关键的一环。`validateConfig` 是纯函数，可单测覆盖。逻辑从旧 `frontend/settings.js:74-97` 逐行平移。

**文件：**
- 创建：`frontend/src/lib/config.test.ts`
- 创建：`frontend/src/lib/config.ts`

- [ ] **步骤 1：编写失败测试**

`frontend/src/lib/config.test.ts`：

```ts
import { describe, it, expect } from 'vitest';
import { validateConfig } from './config';
import type { AppConfig } from '@/types/config';

const base: AppConfig = {
  provider: 'openai-compatible',
  targetLang: '中文',
  openaiCompatible: { apiKey: 'sk-x', baseUrl: 'https://api.openai.com/v1', model: 'gpt-4o-mini', timeoutSeconds: 60 },
  claude: { apiKey: null, baseUrl: 'https://api.anthropic.com', model: 'claude-haiku-4-5', timeoutSeconds: 60, enableThinking: false },
  popupPrecreate: true,
  overlayPrecreate: true,
  collectUsage: true,
};

describe('validateConfig', () => {
  it('mock provider 跳过校验', () => {
    expect(validateConfig({ ...base, provider: 'mock' })).toBeNull();
  });

  it('openai-compatible 有效配置返回 null', () => {
    expect(validateConfig(base)).toBeNull();
  });

  it('baseUrl 非 http(s) 报错', () => {
    expect(validateConfig({ ...base, openaiCompatible: { ...base.openaiCompatible, baseUrl: 'ftp://x' } }))
      .toBe('Base URL 请输入有效的 http(s) 地址');
  });

  it('baseUrl 非法 URL 报错', () => {
    expect(validateConfig({ ...base, openaiCompatible: { ...base.openaiCompatible, baseUrl: 'not a url' } }))
      .toBe('Base URL 请输入有效的 http(s) 地址');
  });

  it('model 为空报错', () => {
    expect(validateConfig({ ...base, openaiCompatible: { ...base.openaiCompatible, model: '' } }))
      .toBe('Model 不能为空');
  });

  it('timeoutSeconds 小于 1 报错', () => {
    expect(validateConfig({ ...base, openaiCompatible: { ...base.openaiCompatible, timeoutSeconds: 0 } }))
      .toBe('Timeout 秒请输入 1-600 的整数');
  });

  it('timeoutSeconds 大于 600 报错', () => {
    expect(validateConfig({ ...base, openaiCompatible: { ...base.openaiCompatible, timeoutSeconds: 601 } }))
      .toBe('Timeout 秒请输入 1-600 的整数');
  });

  it('timeoutSeconds 非整数报错', () => {
    expect(validateConfig({ ...base, openaiCompatible: { ...base.openaiCompatible, timeoutSeconds: 1.5 } }))
      .toBe('Timeout 秒请输入 1-600 的整数');
  });

  it('claude provider 校验 claude 段', () => {
    const c = { ...base, provider: 'claude' as const, claude: { ...base.claude, model: '' } };
    expect(validateConfig(c)).toBe('Model 不能为空');
  });

  it('claude provider 有效配置返回 null', () => {
    const c = { ...base, provider: 'claude' as const, claude: { ...base.claude, apiKey: 'sk-ant-x' } };
    expect(validateConfig(c)).toBeNull();
  });
});
```

- [ ] **步骤 2：运行测试验证失败**

运行：`npm test`
预期：FAIL，报错 `Cannot find module './config'` 或 `validateConfig is not a function`。

- [ ] **步骤 3：编写实现**

`frontend/src/lib/config.ts`：

```ts
import type { AppConfig } from '@/types/config';

/**
 * 校验配置，返回错误文案；无错返回 null。
 * 行为与旧 frontend/settings.js 的 validateConfig 完全一致（逐行平移）。
 */
export function validateConfig(config: AppConfig): string | null {
  if (config.provider === 'mock') return null;
  const sections = config.provider === 'claude' ? [config.claude] : [config.openaiCompatible];
  for (const section of sections) {
    let url: URL;
    try {
      url = new URL(section.baseUrl);
    } catch {
      return 'Base URL 请输入有效的 http(s) 地址';
    }
    if (url.protocol !== 'http:' && url.protocol !== 'https:') {
      return 'Base URL 请输入有效的 http(s) 地址';
    }
    if (!section.model) {
      return 'Model 不能为空';
    }
    if (!Number.isInteger(section.timeoutSeconds)
        || section.timeoutSeconds < 1
        || section.timeoutSeconds > 600) {
      return 'Timeout 秒请输入 1-600 的整数';
    }
  }
  return null;
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`npm test`
预期：PASS，全部用例绿。

- [ ] **步骤 5：Commit**

```bash
git add frontend/src/lib/config.ts frontend/src/lib/config.test.ts
git commit -m "feat(frontend): 平移 validateConfig 纯函数并补单测"
```

---

## 任务 5：Tauri invoke 封装 + Tailwind/shadcn 主题样式

**文件：**
- 创建：`frontend/src/lib/tauri.ts`
- 创建：`frontend/src/styles/main.css`

- [ ] **步骤 1：编写 tauri 薄封装**

`frontend/src/lib/tauri.ts`：

```ts
import type { AppConfig } from '@/types/config';

// 不引 @tauri-apps/api；三页统一走 window.__TAURI__.core.invoke（withGlobalTauri: true）。
const tauriGlobal = (window as unknown as { __TAURI__?: { core: { invoke: <T>(cmd: string, args?: Record<string, unknown>) => Promise<T> } } }).__TAURI__;

function requireInvoke() {
  const invoke = tauriGlobal?.core?.invoke;
  if (!invoke) {
    throw new Error('Tauri API 未就绪');
  }
  return invoke;
}

export async function invokeGetAppConfig(): Promise<AppConfig> {
  return requireInvoke()<AppConfig>('get_app_config');
}

export async function invokeSaveAppConfig(config: AppConfig): Promise<AppConfig> {
  return requireInvoke()<AppConfig>('save_app_config', { config });
}

/** 供组件层判断是否就绪（用于挂载时给出"Tauri API 未就绪"提示）。 */
export function isTauriReady(): boolean {
  return Boolean(tauriGlobal?.core?.invoke);
}
```

- [ ] **步骤 2：编写 Tailwind v4 + shadcn 主题样式**

`frontend/src/styles/main.css`：

```css
@import "tailwindcss";

/* shadcn-vue 主题变量（new-york 风格基线，待原型图定稿后微调配色）。 */
:root {
  --background: 0 0% 100%;
  --foreground: 240 10% 3.9%;
  --card: 0 0% 100%;
  --card-foreground: 240 10% 3.9%;
  --primary: 240 5.9% 10%;
  --primary-foreground: 0 0% 98%;
  --secondary: 240 4.8% 95.9%;
  --secondary-foreground: 240 5.9% 10%;
  --muted: 240 4.8% 95.9%;
  --muted-foreground: 240 3.8% 46.1%;
  --accent: 240 4.8% 95.9%;
  --accent-foreground: 240 5.9% 10%;
  --destructive: 0 84.2% 60.2%;
  --destructive-foreground: 0 0% 98%;
  --border: 240 5.9% 90%;
  --input: 240 5.9% 90%;
  --ring: 240 5.9% 10%;
  --radius: 0.5rem;
}

@theme inline {
  --color-background: hsl(var(--background));
  --color-foreground: hsl(var(--foreground));
  --color-card: hsl(var(--card));
  --color-card-foreground: hsl(var(--card-foreground));
  --color-primary: hsl(var(--primary));
  --color-primary-foreground: hsl(var(--primary-foreground));
  --color-secondary: hsl(var(--secondary));
  --color-secondary-foreground: hsl(var(--secondary-foreground));
  --color-muted: hsl(var(--muted));
  --color-muted-foreground: hsl(var(--muted-foreground));
  --color-accent: hsl(var(--accent));
  --color-accent-foreground: hsl(var(--accent-foreground));
  --color-destructive: hsl(var(--destructive));
  --color-destructive-foreground: hsl(var(--destructive-foreground));
  --color-border: hsl(var(--border));
  --color-input: hsl(var(--input));
  --color-ring: hsl(var(--ring));
  --radius: var(--radius);
}

@layer base {
  * {
    border-color: hsl(var(--border));
  }
  body {
    background-color: hsl(var(--background));
    color: hsl(var(--foreground));
    font-family: system-ui, -apple-system, "Segoe UI", "Microsoft YaHei", sans-serif;
  }
}
```

- [ ] **步骤 3：创建 shadcn-vue 的 `components.json` 与 `utils.ts`**

`frontend/components.json`：

```json
{
  "$schema": "https://shadcn-vue.com/schema.json",
  "style": "new-york",
  "typescript": true,
  "tailwind": {
    "config": "",
    "css": "src/styles/main.css",
    "baseColor": "zinc",
    "cssVariables": true
  },
  "aliases": {
    "components": "@/components",
    "composables": "@/composables",
    "utils": "@/lib/utils",
    "ui": "@/components/ui",
    "lib": "@/lib"
  }
}
```

`frontend/src/lib/utils.ts`（shadcn-vue 组件依赖的 `cn` 工具）：

```ts
import { type ClassValue, clsx } from 'clsx';
import { twMerge } from 'tailwind-merge';

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
```

> `clsx` / `tailwind-merge` / `class-variance-authority` / `reka-ui` 会在任务 6 `npx shadcn-vue add` 时自动安装。本步先建 `utils.ts`，若 typecheck 报缺包，任务 6 装完即解决。

- [ ] **步骤 4：Commit**

```bash
git add frontend/src/lib/tauri.ts frontend/src/lib/utils.ts frontend/src/styles/main.css frontend/components.json
git commit -m "feat(frontend): Tauri invoke 封装 + Tailwind v4/shadcn 主题样式"
```

---

## 任务 6：拷贝 shadcn-vue 组件源码

**文件：**
- 创建：`frontend/src/components/ui/button/*`
- 创建：`frontend/src/components/ui/input/*`
- 创建：`frontend/src/components/ui/label/*`
- 创建：`frontend/src/components/ui/select/*`
- 创建：`frontend/src/components/ui/switch/*`
- 创建：`frontend/src/components/ui/card/*`

- [ ] **步骤 1：用 shadcn-vue CLI 拷贝组件**

在仓库根执行（`components.json` 在 `frontend/`，CLI 默认在 cwd 找，故先 cd）：

```bash
cd frontend
npx shadcn-vue@latest add button input label select switch card
cd ..
```

预期：在 `frontend/src/components/ui/` 下生成上述六个组件目录，并自动把 `class-variance-authority` / `clsx` / `tailwind-merge` / `reka-ui` 等依赖加进根 `package.json`。

- [ ] **步骤 2：再次安装自动补上的依赖**

运行：`npm install`
预期：装好新增依赖。

- [ ] **步骤 3：类型检查（确认拷贝的组件与 Tailwind v4 适配）**

运行：`npm run typecheck`
预期：通过。若个别组件因 v3/v4 语法差异报错，按 spec §5 风险 4「以 shadcn-vue 官方 v4 指引为准就地调整」修该组件文件。

- [ ] **步骤 4：Commit**

```bash
git add -A frontend/src/components/ui package.json package-lock.json
git commit -m "chore(frontend): 拷贝 shadcn-vue 组件源码(button/input/label/select/switch/card)"
```

---

## 任务 7：设置页 Vue 组件骨架与交互行为

> 本任务实现 spec §3 全部交互行为（加载/Provider 切换显隐/保存校验/保存提示文案/保存按钮 disabled/密码显隐），UI 视觉只用 Tailwind 基础类搭骨架，**不追求高保真**（留给任务 9）。行为必须与旧 `settings.js` 逐项对齐。

**文件：**
- 创建：`frontend/settings.html`（覆盖旧文件）
- 创建：`frontend/src/settings/main.ts`
- 创建：`frontend/src/settings/App.vue`
- 创建：`frontend/src/settings/components/TargetLangSection.vue`
- 创建：`frontend/src/settings/components/ProviderSelect.vue`
- 创建：`frontend/src/settings/components/ApiKeyField.vue`
- 创建：`frontend/src/settings/components/OpenAiSection.vue`
- 创建：`frontend/src/settings/components/ClaudeSection.vue`
- 创建：`frontend/src/settings/components/StrategySection.vue`
- 创建：`frontend/src/settings/components/SaveBar.vue`
- 删除：旧 `frontend/settings.js`、`frontend/settings.css`

- [ ] **步骤 1：写 Vite 入口 HTML（覆盖旧 settings.html）**

`frontend/settings.html`：

```html
<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Shizi - 设置</title>
</head>
<body>
  <div id="app"></div>
  <script type="module" src="/src/settings/main.ts"></script>
</body>
</html>
```

- [ ] **步骤 2：写 Vue 挂载入口**

`frontend/src/settings/main.ts`：

```ts
import { createApp } from 'vue';
import App from './App.vue';
import '@/styles/main.css';

createApp(App).mount('#app');
```

- [ ] **步骤 3：写 ApiKeyField（密码显隐，DRY 复用）**

`frontend/src/settings/components/ApiKeyField.vue`：

```vue
<script setup lang="ts">
import { ref, computed } from 'vue';
import { Icon } from '@iconify/vue';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Button } from '@/components/ui/button';

const props = defineProps<{
  modelValue: string;
  label: string;
  placeholder: string;
}>();
const emit = defineEmits<{ 'update:modelValue': [value: string] }>();

const visible = ref(false);
const inputType = computed(() => (visible.value ? 'text' : 'password'));

function onInput(e: Event) {
  emit('update:modelValue', (e.target as HTMLInputElement).value);
}
function toggle() {
  visible.value = !visible.value;
}
</script>

<template>
  <div class="space-y-1.5">
    <Label>{{ props.label }}</Label>
    <div class="relative">
      <Input
        :type="inputType"
        :placeholder="props.placeholder"
        :value="props.modelValue"
        class="pr-10"
        @input="onInput"
      />
      <Button
        type="button"
        variant="ghost"
        size="icon"
        class="absolute right-1 top-1 h-8 w-8"
        :aria-label="visible ? '隐藏 API Key' : '显示 API Key'"
        @click="toggle"
      >
        <Icon :icon="visible ? 'lucide:eye-off' : 'lucide:eye'" class="h-4 w-4" />
      </Button>
    </div>
    <p class="text-xs text-muted-foreground">API Key 本阶段会明文保存到本机配置文件，请只在可信设备上使用。</p>
  </div>
</template>
```

- [ ] **步骤 4：写 TargetLangSection**

`frontend/src/settings/components/TargetLangSection.vue`：

```vue
<script setup lang="ts">
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

const props = defineProps<{ modelValue: string }>();
const emit = defineEmits<{ 'update:modelValue': [value: string] }>();

function onInput(e: Event) {
  emit('update:modelValue', (e.target as HTMLInputElement).value);
}
</script>

<template>
  <div class="space-y-1.5">
    <Label>目标语言</Label>
    <Input :value="props.modelValue" placeholder="中文" @input="onInput" />
  </div>
</template>
```

- [ ] **步骤 5：写 ProviderSelect**

`frontend/src/settings/components/ProviderSelect.vue`：

```vue
<script setup lang="ts">
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import type { Provider } from '@/types/config';

const props = defineProps<{ modelValue: Provider }>();
const emit = defineEmits<{ 'update:modelValue': [value: Provider] }>();

function onChange(v: string) {
  emit('update:modelValue', v as Provider);
}
</script>

<template>
  <div class="space-y-1.5">
    <Label>Provider</Label>
    <Select :model-value="props.modelValue" @update:model-value="onChange">
      <SelectTrigger>
        <SelectValue placeholder="选择 Provider" />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="openai-compatible">OpenAI Compatible</SelectItem>
        <SelectItem value="claude">Claude</SelectItem>
        <SelectItem value="mock">Mock（调试用）</SelectItem>
      </SelectContent>
    </Select>
  </div>
</template>
```

- [ ] **步骤 6：写 OpenAiSection**

`frontend/src/settings/components/OpenAiSection.vue`：

```vue
<script setup lang="ts">
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import ApiKeyField from './ApiKeyField.vue';
import type { OpenAiCompatibleConfig } from '@/types/config';

const props = defineProps<{ modelValue: OpenAiCompatibleConfig }>();
const emit = defineEmits<{ 'update:modelValue': [value: OpenAiCompatibleConfig] }>();

function patch<K extends keyof OpenAiCompatibleConfig>(key: K, value: OpenAiCompatibleConfig[K]) {
  emit('update:modelValue', { ...props.modelValue, [key]: value });
}
function numPatch(e: Event) {
  patch('timeoutSeconds', Number((e.target as HTMLInputElement).value));
}
</script>

<template>
  <div class="space-y-3">
    <h3 class="text-sm font-medium">OpenAI Compatible</h3>
    <ApiKeyField
      :model-value="props.modelValue.apiKey ?? ''"
      label="API Key"
      placeholder="sk-..."
      @update:model-value="(v) => patch('apiKey', v.trim() || null)"
    />
    <div class="space-y-1.5">
      <Label>Base URL</Label>
      <Input :value="props.modelValue.baseUrl" placeholder="https://api.openai.com/v1" @input="(e: Event) => patch('baseUrl', (e.target as HTMLInputElement).value)" />
    </div>
    <div class="space-y-1.5">
      <Label>Model</Label>
      <Input :value="props.modelValue.model" placeholder="gpt-4o-mini" @input="(e: Event) => patch('model', (e.target as HTMLInputElement).value)" />
    </div>
    <div class="space-y-1.5">
      <Label>Timeout 秒</Label>
      <Input type="number" min="1" step="1" :value="props.modelValue.timeoutSeconds" placeholder="60" @input="numPatch" />
    </div>
  </div>
</template>
```

- [ ] **步骤 7：写 ClaudeSection**

`frontend/src/settings/components/ClaudeSection.vue`：

```vue
<script setup lang="ts">
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import ApiKeyField from './ApiKeyField.vue';
import type { ClaudeConfig } from '@/types/config';

const props = defineProps<{ modelValue: ClaudeConfig }>();
const emit = defineEmits<{ 'update:modelValue': [value: ClaudeConfig] }>();

function patch<K extends keyof ClaudeConfig>(key: K, value: ClaudeConfig[K]) {
  emit('update:modelValue', { ...props.modelValue, [key]: value });
}
function numPatch(e: Event) {
  patch('timeoutSeconds', Number((e.target as HTMLInputElement).value));
}
</script>

<template>
  <div class="space-y-3">
    <h3 class="text-sm font-medium">Claude</h3>
    <ApiKeyField
      :model-value="props.modelValue.apiKey ?? ''"
      label="API Key"
      placeholder="sk-ant-..."
      @update:model-value="(v) => patch('apiKey', v.trim() || null)"
    />
    <div class="space-y-1.5">
      <Label>Base URL</Label>
      <Input :value="props.modelValue.baseUrl" placeholder="https://api.anthropic.com" @input="(e: Event) => patch('baseUrl', (e.target as HTMLInputElement).value)" />
    </div>
    <div class="space-y-1.5">
      <Label>Model</Label>
      <Input :value="props.modelValue.model" placeholder="claude-haiku-4-5" @input="(e: Event) => patch('model', (e.target as HTMLInputElement).value)" />
    </div>
    <div class="space-y-1.5">
      <Label>Timeout 秒</Label>
      <Input type="number" min="1" step="1" :value="props.modelValue.timeoutSeconds" placeholder="60" @input="numPatch" />
    </div>
    <div class="flex items-center justify-between">
      <Label class="leading-tight">Enable Thinking<br /><span class="text-xs text-muted-foreground font-normal">仅对支持的模型生效，Haiku 需关闭</span></Label>
      <Switch :model-value="props.modelValue.enableThinking" @update:model-value="(v: boolean) => patch('enableThinking', v)" />
    </div>
  </div>
</template>
```

- [ ] **步骤 8：写 StrategySection**

`frontend/src/settings/components/StrategySection.vue`：

```vue
<script setup lang="ts">
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';

const props = defineProps<{
  popupPrecreate: boolean;
  overlayPrecreate: boolean;
  collectUsage: boolean;
}>();
const emit = defineEmits<{
  'update:popupPrecreate': [v: boolean];
  'update:overlayPrecreate': [v: boolean];
  'update:collectUsage': [v: boolean];
}>();
</script>

<template>
  <div class="space-y-3">
    <h3 class="text-sm font-medium">窗口策略</h3>
    <div class="flex items-center justify-between">
      <Label>预创建翻译弹窗</Label>
      <Switch :model-value="props.popupPrecreate" @update:model-value="(v: boolean) => emit('update:popupPrecreate', v)" />
    </div>
    <div class="flex items-center justify-between">
      <Label>预创建截图窗口</Label>
      <Switch :model-value="props.overlayPrecreate" @update:model-value="(v: boolean) => emit('update:overlayPrecreate', v)" />
    </div>
    <div class="flex items-center justify-between">
      <Label>采集 token 用量<span class="block text-xs text-muted-foreground font-normal">显示翻译 token 消耗</span></Label>
      <Switch :model-value="props.collectUsage" @update:model-value="(v: boolean) => emit('update:collectUsage', v)" />
    </div>
  </div>
</template>
```

- [ ] **步骤 9：写 SaveBar**

`frontend/src/settings/components/SaveBar.vue`：

```vue
<script setup lang="ts">
import { Button } from '@/components/ui/button';

const props = defineProps<{
  saving: boolean;
  status: string;
  isError: boolean;
}>();
const emit = defineEmits<{ save: [] }>();
</script>

<template>
  <div class="space-y-2">
    <Button :disabled="props.saving" @click="emit('save')">
      {{ props.saving ? '保存中...' : '保存配置' }}
    </Button>
    <p class="text-sm" :class="props.isError ? 'text-destructive' : 'text-muted-foreground'">
      {{ props.status }}
    </p>
  </div>
</template>
```

- [ ] **步骤 10：写 App.vue（编排 + 加载/保存状态机）**

`frontend/src/settings/App.vue`：

```vue
<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import type { AppConfig, Provider } from '@/types/config';
import { validateConfig } from '@/lib/config';
import { invokeGetAppConfig, invokeSaveAppConfig, isTauriReady } from '@/lib/tauri';
import TargetLangSection from './components/TargetLangSection.vue';
import ProviderSelect from './components/ProviderSelect.vue';
import OpenAiSection from './components/OpenAiSection.vue';
import ClaudeSection from './components/ClaudeSection.vue';
import StrategySection from './components/StrategySection.vue';
import SaveBar from './components/SaveBar.vue';

const config = ref<AppConfig | null>(null);
const status = ref('');
const isError = ref(false);
const saving = ref(false);

const showOpenAi = computed(() => config.value?.provider === 'openai-compatible');
const showClaude = computed(() => config.value?.provider === 'claude');

function setStatus(msg: string, err = false) {
  status.value = msg;
  isError.value = err;
}

async function load() {
  if (!isTauriReady()) {
    setStatus('Tauri API 未就绪，无法读取配置', true);
    return;
  }
  try {
    config.value = await invokeGetAppConfig();
    setStatus('');
  } catch (e) {
    setStatus(String(e), true);
  }
}

async function save() {
  if (!config.value) return;
  if (!isTauriReady()) {
    setStatus('Tauri API 未就绪，无法保存配置', true);
    return;
  }
  const err = validateConfig(config.value);
  if (err) {
    setStatus(err, true);
    return;
  }
  saving.value = true;
  setStatus('保存中...');
  const before = { popup: config.value.popupPrecreate, overlay: config.value.overlayPrecreate };
  try {
    const saved = await invokeSaveAppConfig(config.value);
    config.value = saved;
    const changed = saved.popupPrecreate !== before.popup || saved.overlayPrecreate !== before.overlay;
    setStatus(changed ? '配置已保存，窗口策略切换需重启应用生效' : '配置已保存，下一次翻译生效');
  } catch (e) {
    setStatus(String(e), true);
  } finally {
    saving.value = false;
  }
}

function setProvider(p: Provider) {
  if (config.value) config.value = { ...config.value, provider: p };
}

onMounted(load);
</script>

<template>
  <div v-if="config" class="mx-auto max-w-2xl space-y-6 p-6">
    <header class="space-y-1">
      <h1 class="text-2xl font-semibold">Shizi</h1>
      <p class="text-muted-foreground">设置</p>
    </header>

    <TargetLangSection v-model="config.targetLang" />

    <ProviderSelect :model-value="config.provider" @update:model-value="setProvider" />

    <OpenAiSection v-if="showOpenAi" v-model="config.openaiCompatible" />
    <ClaudeSection v-if="showClaude" v-model="config.claude" />

    <StrategySection
      v-model:popup-precreate="config.popupPrecreate"
      v-model:overlay-precreate="config.overlayPrecreate"
      v-model:collect-usage="config.collectUsage"
    />

    <SaveBar :saving="saving" :status="status" :is-error="isError" @save="save" />
  </div>
  <div v-else class="p-6 text-muted-foreground">
    {{ status || '加载中...' }}
  </div>
</template>
```

> 注意保存提示文案分支与旧 `settings.js:133-137` 对齐：对比的是「保存前 popup/overlay」与「保存后回填的 popup/overlay」，体现窗口策略是否变化。旧实现是保存后再读 DOM checkbox 比较，这里改用保存前快照 vs 保存后回填值，**语义等价且更直接**。

- [ ] **步骤 11：删除旧 settings.js / settings.css**

```bash
git rm frontend/settings.js frontend/settings.css
```

- [ ] **步骤 12：类型检查**

运行：`npm run typecheck`
预期：通过。若 `@input` 监听器类型报错，调整为 `(e: Event) => ...` 显式标注（模板内联类型注解需 vue-tsc 3.x+ 支持；若报错可把 numPatch 等处理函数提到 setup 内复用，不在模板写类型注解）。

- [ ] **步骤 13：构建验证**

运行：`npm run build`
预期：`vite build` 成功，`frontend/dist/` 下平铺 `settings.html` / `translate.html` / `overlay.html` 三个 html。

- [ ] **步骤 14：验证产物根含三页**

运行：`ls frontend/dist/`
预期：包含 `settings.html`、`translate.html`、`overlay.html`（及各自的 assets）。

- [ ] **步骤 15：Commit**

```bash
git add -A frontend
git commit -m "feat(frontend): 设置页 Vue 3 组件骨架与交互行为"
```

---

## 任务 8：Tauri 配置接入 + 回归验证

**文件：**
- 修改：`src-tauri/tauri.conf.json`

- [ ] **步骤 1：更新 tauri.conf.json**

把 `src-tauri/tauri.conf.json` 改为：

```json
{
  "productName": "Shizi",
  "version": "0.1.0",
  "identifier": "com.shizi.app",
  "build": {
    "frontendDist": "../frontend/dist",
    "devUrl": "http://localhost:5173",
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build"
  },
  "app": {
    "withGlobalTauri": true,
    "windows": [
      {
        "label": "main",
        "url": "settings.html",
        "title": "Shizi - 翻译助手",
        "width": 560,
        "height": 640,
        "minWidth": 480,
        "minHeight": 480,
        "resizable": true,
        "center": true
      }
    ]
  }
}
```

> 窗口尺寸 560×640 + min 480 对应 spec §2.5，标注「待原型图定稿」，任务 9 再微调。

- [ ] **步骤 2：Rust 回归基线**

运行：`cd src-tauri && cargo test`
预期：全绿（后端零改动，回归基线）。

- [ ] **步骤 3：Rust 构建**

运行：`cd src-tauri && cargo build`
预期：成功。

- [ ] **步骤 4：手动验证（Windows，记录结果）**

运行：`npm run tauri dev`，逐项核对并记录：
1. 设置页 HMR 可用（改 `App.vue` 文案看是否热更新）。
2. 挂载后能加载现有 config（字段填充正确）。
3. Provider 切换 → openai/claude 段显隐正确；mock 时两段都隐藏。
4. 密码显隐按钮切换 `type`。
5. 故意填非法 baseUrl → 保存报错文案「Base URL 请输入有效的 http(s) 地址」且不提交。
6. 合法保存 → 文案「配置已保存，下一次翻译生效」。
7. 改 popup/overlay 开关后保存 → 文案「配置已保存，窗口策略切换需重启应用生效」。
8. 保存中按钮 disabled + 文案「保存中...」。
9. `Alt+T` 划词翻译正常（验证 `public/translate.html` 未被牵连）。
10. `Alt+O` 截图 OCR 正常（验证 `public/overlay.html` 未被牵连）。

> spec §5 风险 3「withGlobalTauri 在 dev server 下注入时机」在本步验证。若 `window.__TAURI__` 未就绪导致 invoke 失败，在 `tauri.ts` 的 `invokeGetAppConfig`/`invokeSaveAppConfig` 加就绪重试（轮询 `isTauriReady()` 至多 N 次）。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/tauri.conf.json
git commit -m "build(tauri): 接入 Vite devUrl/build 命令并调整窗口尺寸"
```

---

## 任务 9：UI 视觉打磨（待原型图定稿）

> **此任务为 spec 授权的占位任务。** 原型图到齐前不执行。原型图到齐后，在本任务下补全为带代码的具体步骤（间距、配色、卡片层级、Iconify 图标选型、窗口尺寸定稿），再按 TDD/原子 commit 落地。执行者做完任务 8 后应停在此处，向用户确认原型图是否就绪。

- [ ] **步骤 0：确认原型图就绪**

向用户确认 open design 高保真原型图已提供。未就绪 → 跳过本任务，按任务 8 的可交付态收尾（任务 10 文档同步仍要执行，UI 视觉细节标注「待原型图」）。就绪 → 继续。

- [ ] **步骤 1（待原型图）：按原型图对齐布局/间距/卡片层级**

> 待原型图定稿后补全：具体 Tailwind 类、Card 包裹结构、各 section 间距数值。

- [ ] **步骤 2（待原型图）：配色与主题变量微调**

> 待原型图定稿后补全：`src/styles/main.css` 的 HSL 变量调整、是否引入 dark 模式。

- [ ] **步骤 3（待原型图）：Iconify 图标选型（顶部标题区/provider 图标等）**

> 待原型图定稿后补全：图标集与具体 icon name。

- [ ] **步骤 4（待原型图）：窗口尺寸定稿**

> 待原型图定稿后补全：`tauri.conf.json` 的 width/height/minWidth/minHeight 终值。

- [ ] **步骤 5（待原型图）：UI 对照验证**

> 待原型图定稿后补全：逐屏对照原型图的手动验证清单。

---

## 任务 10：文档同步（协作规范第 2 条硬门禁）

**文件：**
- 修改：`README.md`
- 修改：`CLAUDE.md`
- 修改：`AGENTS.md`
- 修改：`docs/roadmap/progressive-development-plan.md`
- 修改：`plugins.md`

- [ ] **步骤 1：更新 README 开发命令与当前能力**

在 `README.md` 开发命令区，把现有命令块补充为：

```bash
npm install               # 首次需装前端依赖（Vite/Vue/Tailwind/shadcn-vue）
npm run tauri dev         # 开发模式（拉起 Vite dev server + 后端）
npm run tauri build       # 生成 release 安装包
npm run dev               # 仅启动前端 Vite dev server（无 Tauri 容器，invoke 不可用）
npm run build             # 仅构建前端到 frontend/dist/
npm run typecheck         # vue-tsc 类型检查
npm run test              # vitest 单测
cd src-tauri && cargo build           # 仅构建后端 debug
cd src-tauri && cargo build --release # 仅构建后端 release
cd src-tauri && cargo test            # 后端单测
```

在"当前能力"区，设置页技术栈更新为「Vue 3 + Vite + Tailwind v4 + shadcn-vue + Iconify」；注明 translate/overlay 仍为纯静态。

- [ ] **步骤 2：同步 CLAUDE.md 与 AGENTS.md**

两文件的项目结构区 `frontend/` 描述从"静态前端（原生 HTML/JS/CSS，无构建步骤）"改为：

> `frontend/` Vite 工程：设置页 `settings.html` 为 Vue 3 + Tailwind v4 + shadcn-vue 入口；`translate.html` / `overlay.html` 平铺在 `frontend/public/` 保持纯静态（overlay 永久不迁）。构建产物 `frontend/dist/`。

常用命令区同步 README 的新增脚本。两文件保持内容一致（协作规范第 1 条）。

- [ ] **步骤 3：更新 roadmap**

在 `docs/roadmap/progressive-development-plan.md` 插入新里程碑"前端体验优化（Tauri UI 路线）"，设置页 Vue 重构记为第一个任务；若 UI 视觉细节未定稿，标注「待原型图」并记为进行中。

- [ ] **步骤 4：更新 plugins.md**

在 `plugins.md` 新增前端依赖记录：Vite 7、Vue 3.5、`@vitejs/plugin-vue`、Tailwind CSS v4 + `@tailwindcss/vite`、shadcn-vue（new-york，按需拷贝源码）、`@iconify/vue`、vitest、vue-tsc、TypeScript、reka-ui、class-variance-authority、clsx、tailwind-merge。

- [ ] **步骤 5：Commit**

```bash
git add README.md CLAUDE.md AGENTS.md docs/roadmap/progressive-development-plan.md plugins.md
git commit -m "docs: 同步设置页 Vue 重构的命令/结构/路线/插件记录"
```

---

## 自检

**1. 规格覆盖度：**
- §1 目标与边界 → 任务 1（Vite 接入）、任务 2（public 平铺）、任务 8（后端零改动回归）。✓
- §2.1 dev/build 接入 → 任务 1（vite.config）、任务 8（tauri.conf）。✓
- §2.2 Tauri API 调用（withGlobalTauri + 薄封装） → 任务 5 `tauri.ts`。✓
- §2.3 目录结构 → 任务 1–7 文件结构一致。✓
- §2.4 配置类型与校验 → 任务 3（类型）、任务 4（validateConfig + 单测）。✓
- §2.5 窗口尺寸 → 任务 8（560×640 + min 480，待原型图）。✓
- §3.1 信息架构 → 任务 7 各 section。✓
- §3.2 组件拆分 → 任务 7 七个 SFC + ApiKeyField 复用。✓
- §3.3 交互行为 5 项 → 任务 7 App.vue（load/save/切换/disabled）+ ApiKeyField（显隐）。✓
- §3.4 YAGNI 边界 → 计划未引 Toast/Pinia/路由，符合。✓
- §4 验证策略 → 任务 4（单测）、任务 7（typecheck/build）、任务 8（cargo test/build + 手动 10 项）。✓
- §5 风险 → 任务 1 strictPort（风险 1）、任务 2 平铺核对（风险 2）、任务 8 步骤 4 第 10 项 + tauri.ts 就绪检测（风险 3）、任务 6 v4 适配就地调整（风险 4）、任务 3 类型注释约束同步（风险 5）。✓
- §6 文档同步 → 任务 10。✓
- §7 不在范围 → 计划未触及翻译页迁移/overlay/Slint/SecretStore。✓
- UI 视觉细节 → 任务 9 占位（spec 授权）。✓

**2. 占位符扫描：** 任务 1–8、10 全部含完整代码与精确命令。任务 9 为 spec 明确授权的「待原型图」占位，已在计划开头专节说明，非缺陷。无其他"TODO/待定/类似任务N"模式。

**3. 类型一致性：** `AppConfig` / `Provider` / `OpenAiCompatibleConfig` / `ClaudeConfig`（任务 3 定义）在任务 4（validateConfig 入参）、任务 5（invoke 返回值）、任务 7（各 section props）中名称与字段一致。`invokeGetAppConfig` / `invokeSaveAppConfig` / `isTauriReady`（任务 5 定义）在任务 7 App.vue 引用一致。shadcn 组件导入路径 `@/components/ui/*` 在任务 6 生成、任务 7 引用一致。`validateConfig` 返回 `string | null` 在任务 4 定义、任务 7 使用 `if (err)` 判定一致。
