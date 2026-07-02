# 设置页视觉打磨（一次搬运单页）实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 用 OpenDesign 原型整套重写 shizi 设置页，视觉对齐原型；已实现字段经 `projectToAppConfig` 投影接 `save_app_config` 真正生效，未实现字段本地持久化 + 「实现中」标签。

**架构：** 前端 `localStorage` 为唯一真相源（不调 `get_app_config`、不做老用户迁移）。保存时用纯函数 `projectToAppConfig(state, lastSavedProvider)` 把多实例前端状态压扁成后端单 provider `AppConfig`，经 `validateConfig` 校验后 `invokeSaveAppConfig`。原型 store/components/panels 整套搬入并做三处机械适配（Tailwind v3→v4 token、`lucide-vue-next`→`@lucide/vue`、`components/ui/` 替换）。

**技术栈：** Vue 3 + TS + Tailwind v4（CSS-first）+ reka-ui + `@lucide/vue` + `@iconify/vue` + Tauri 2（`window.__TAURI__.core.invoke`）。

**原型根目录（搬运源）：** `C:\Users\xdj\IdeaProjects\LLM\OpenDesignProjects\shizi\src`
**目标工程前端根：** `frontend/`

---

## 文件结构

### 新建（从原型搬入 + 适配）

| 路径 | 职责 | 适配 |
|---|---|---|
| `frontend/src/styles/main.css` | 旧文件**修改**：注入原型 HSL 调色板 + token + utilities/keyframes | 见任务 1 |
| `frontend/src/settings/types.ts` | 设置页数据模型（搬入 + 扩展 3 字段） | 见任务 2 |
| `frontend/src/settings/tokens.ts` | BUILTIN_SERVICES / LANGUAGES / DEFAULT_PROMPTS（原样搬入） | 仅图标 import 改写 |
| `frontend/src/settings/stores/settings.ts` | useSettings store（搬入 + seed 改造 + save 桥接改造） | 见任务 4 |
| `frontend/src/settings/SettingsPage.vue` / `SettingsLayout.vue` / `SettingsSidebar.vue` | 三件外壳（原样搬入） | 仅图标 import 改写 |
| `frontend/src/settings/components/*.vue` + `index.ts` + `types.ts` | 11 个原子组件（原样搬入） | 仅图标 import 改写 |
| `frontend/src/settings/panels/*.vue` | 6 个面板（搬入 + 改造） | 见任务 7 |
| `frontend/src/components/ui/{button,badge,input,switch,select,dialog,tooltip,toast}/` + `index.ts` | 原型 UI 原子整套替换现有生成原子 | 见任务 5 |
| `frontend/src/lib/toast.ts` | toast 响应式 store（原样搬入） | 无 |
| `frontend/src/lib/config.ts` | 旧文件**修改**：新增 `projectToAppConfig` | 见任务 3 |
| `frontend/src/lib/config.test.ts` | 旧文件**修改**：新增 `projectToAppConfig` 单测 | 见任务 3 |
| `frontend/src/settings/App.vue` | 旧文件**替换**：原型 App.vue 内容（TooltipProvider + Toaster） | 见任务 8 |
| `frontend/src/settings/main.ts` | 旧文件**修改**：保持挂载，CSS 已由 main.css 引入 | 见任务 8 |

### 删除（任务 9）

- `frontend/src/settings/components/{TargetLangSection,ProviderSelect,OpenAiSection,ClaudeSection,StrategySection,SaveBar,ApiKeyField}.vue`（旧 7 子组件）
- `frontend/src/components/ui/{button,card,input,label,select,switch}/`（旧生成原子，被原型整套替换；`card`/`label` 原型未用，旧引用仅来自旧设置页，删除旧页后无破坏面）

### 不动

- `frontend/src/lib/tauri.ts`（`invokeSaveAppConfig` / `isTauriReady` 复用；`invokeGetAppConfig` 保留但新页不调用）
- `frontend/src/lib/utils.ts`（`cn` 已就绪，原型沿用）
- `frontend/translate.html` / `frontend/overlay.html` 及其静态 JS/CSS（纯静态页，不碰 Vue）
- 后端任何文件（不新增 command）

---

## 搬运总规则（适用于所有「原样搬入」任务）

1. **图标 import 改写**：所有 `.vue` / `.ts` 文件中 `from 'lucide-vue-next'` → `from '@lucide/vue'`，命名导出保持不变（`Check` / `X` / `Settings2` / `Languages` / `Keyboard` / `Plug` / `Sliders` / `History` / `WandSparkles` / `Download` / `Upload` / `RotateCcw` / `FileText` / `Github` / `BookOpen` / `Sparkles` 等）。`@lucide/vue@1.23.0` 命名导出与 `lucide-vue-next` 一致，由 `vue-tsc` 兜底校验。
2. **路径别名**：原型用 `@/` 指向 `src/`，与 shizi `tsconfig.json` 的 `paths` 一致，无需改写。
3. **不搬**原型的 `style.css` / `tailwind.config.ts` / `postcss.config.js` / `router.example.ts`。
4. **CSS 类**：原型用 `bg-background` / `text-foreground` / `border-border` / `bg-card` / `bg-muted` / `text-muted-foreground` / `bg-primary` / `bg-accent` / `bg-destructive` / `bg-emerald-*` / `bg-amber-*` / `bg-sky-*` 等。其中 shadcn 语义色由 main.css `@theme inline` 桥接（任务 1 落地后可用）；`bg-emerald-*` / `bg-amber-*` / `bg-sky-*` 是 Tailwind v4 内置调色板，直接可用。`scrollbar-thin` / `toast-in` / `toast-out` / `api-key-progress` 由任务 1 补 utility/keyframes。

---

## 任务 1：main.css 配色 token 落地

**文件：**
- 修改：`frontend/src/styles/main.css`

把原型 `style.css` 的 HSL 调色板、token、utilities、keyframes 合并进 shizi 现有 `main.css`，**保留** shizi 现有 `@import` / `@custom-variant` / `@theme inline` 结构与 `@layer base`。现有注释「待原型图定稿后微调配色」即此次落地。

- [ ] **步骤 1：替换 `:root` 块的色值为原型 HSL 值**

把 `frontend/src/styles/main.css` 中 `:root { ... }`（第 13–47 行）整体替换为：

```css
:root {
  --background: 0 0% 100%;
  --foreground: 220 18% 18%;
  --card: 0 0% 100%;
  --card-foreground: 220 18% 18%;
  --primary: 222 70% 48%;
  --primary-foreground: 0 0% 100%;
  --secondary: 220 14% 96%;
  --secondary-foreground: 220 18% 22%;
  --muted: 220 14% 96%;
  --muted-foreground: 220 10% 46%;
  --accent: 222 60% 96%;
  --accent-foreground: 222 70% 38%;
  --destructive: 0 72% 52%;
  --destructive-foreground: 0 0% 100%;
  --border: 220 13% 91%;
  --input: 220 13% 91%;
  --ring: 222 70% 48%;
  --radius: 0.5rem;
  --popover: 0 0% 100%;
  --popover-foreground: 220 18% 18%;
  --chart-1: 222 70% 48%;
  --chart-2: 160 60% 40%;
  --chart-3: 38 80% 50%;
  --chart-4: 280 60% 55%;
  --chart-5: 0 72% 52%;
  --sidebar: 220 14% 96%;
  --sidebar-foreground: 220 18% 18%;
  --sidebar-primary: 222 70% 48%;
  --sidebar-primary-foreground: 0 0% 100%;
  --sidebar-accent: 222 60% 96%;
  --sidebar-accent-foreground: 222 70% 38%;
  --sidebar-border: 220 13% 91%;
  --sidebar-ring: 222 70% 48%;
  --sidebar-width: 240px;
  --content-max-width: 720px;
}
```

- [ ] **步骤 2：替换 `.dark` 块为原型深色值**

把 `.dark { ... }`（第 91–123 行）整体替换为：

```css
.dark {
  --background: 222 18% 10%;
  --foreground: 220 14% 95%;
  --card: 222 18% 12%;
  --card-foreground: 220 14% 95%;
  --popover: 222 18% 12%;
  --popover-foreground: 220 14% 95%;
  --primary: 222 80% 64%;
  --primary-foreground: 222 30% 10%;
  --secondary: 222 14% 18%;
  --secondary-foreground: 220 14% 95%;
  --muted: 222 14% 18%;
  --muted-foreground: 220 10% 64%;
  --accent: 222 24% 22%;
  --accent-foreground: 222 80% 80%;
  --destructive: 0 70% 56%;
  --destructive-foreground: 220 14% 95%;
  --border: 222 14% 20%;
  --input: 222 14% 20%;
  --ring: 222 80% 64%;
  --chart-1: 222 80% 64%;
  --chart-2: 160 60% 50%;
  --chart-3: 38 80% 60%;
  --chart-4: 280 60% 65%;
  --chart-5: 0 70% 56%;
  --sidebar: 222 18% 12%;
  --sidebar-foreground: 220 14% 95%;
  --sidebar-primary: 222 80% 64%;
  --sidebar-primary-foreground: 222 30% 10%;
  --sidebar-accent: 222 24% 22%;
  --sidebar-accent-foreground: 222 80% 80%;
  --sidebar-border: 222 14% 20%;
  --sidebar-ring: 222 80% 64%;
}
```

- [ ] **步骤 3：在文件末尾追加 utilities 与 keyframes**

在 `frontend/src/styles/main.css` 末尾（`@layer base` 块之后）追加：

```css
@layer utilities {
  .scrollbar-thin {
    scrollbar-width: thin;
    scrollbar-color: hsl(var(--border)) transparent;
  }
  .scrollbar-thin::-webkit-scrollbar {
    width: 6px;
    height: 6px;
  }
  .scrollbar-thin::-webkit-scrollbar-thumb {
    background: hsl(var(--border));
    border-radius: 3px;
  }
  .scrollbar-thin::-webkit-scrollbar-track {
    background: transparent;
  }
}

@keyframes api-key-progress {
  0%   { transform: translateX(0); }
  100% { transform: translateX(300%); }
}

@keyframes toast-slide-in {
  from { opacity: 0; transform: translateX(calc(100% + 16px)); }
  to   { opacity: 1; transform: translateX(0); }
}
@keyframes toast-slide-out {
  from { opacity: 1; transform: translateX(0); }
  to   { opacity: 0; transform: translateX(calc(100% + 16px)); }
}
.toast-in  { animation: toast-slide-in 220ms cubic-bezier(0.16, 1, 0.3, 1) both; }
.toast-out { animation: toast-slide-out 180ms cubic-bezier(0.4, 0, 1, 1) both; }
```

- [ ] **步骤 4：Commit**

```bash
git add frontend/src/styles/main.css
git commit -m "feat(frontend): 设置页配色 token 落地（原型 HSL 调色板 + scrollbar/toast 工具类）"
```

---

## 任务 2：搬入 settings types / tokens

**文件：**
- 创建：`frontend/src/settings/types.ts`
- 创建：`frontend/src/settings/tokens.ts`

- [ ] **步骤 1：搬入 types.ts 并扩展 3 个已实现字段**

把原型 `src/settings/types.ts` 原样复制到 `frontend/src/settings/types.ts`，然后做两处扩展（已实现字段归属，见 spec §5）：

1. `GeneralSettings` 接口新增两个字段（紧跟 `closeAction` 之后）：

```ts
  /** 主翻译弹窗是否预创建（后端已实现，接 save_app_config.popupPrecreate）。 */
  popupPrecreate: boolean
  /** 截图 OCR overlay 窗口是否预创建（后端已实现，接 save_app_config.overlayPrecreate）。 */
  overlayPrecreate: boolean
```

2. `AdvancedSettings` 接口新增一个字段：

```ts
  /** 是否收集匿名使用统计（后端已实现，接 save_app_config.collectUsage）。 */
  collectUsage: boolean
```

其余类型（`ServiceInstance` / `TranslationSettings` / `ShortcutBinding` / `OcrHistoryEntry` / `AppSettings` / `ServiceMeta` 等）原样保留。

- [ ] **步骤 2：搬入 tokens.ts（仅图标 import 改写）**

把原型 `src/settings/tokens.ts` 原样复制到 `frontend/src/settings/tokens.ts`，将第 1 行：

```ts
import { Plug, WandSparkles } from 'lucide-vue-next'
```

改为：

```ts
import { Plug, WandSparkles } from '@lucide/vue'
```

其余内容（`BUILTIN_SERVICES` / `LANGUAGES` / `DEFAULT_PROMPTS` / `MOCK_PULLED_MODELS` 等）原样保留。

- [ ] **步骤 3：typecheck 验证**

运行：`npm run typecheck`
预期：PASS（types/tokens 无外部依赖错误；此时旧 App.vue 仍存在，不应引入新错误）

- [ ] **步骤 4：Commit**

```bash
git add frontend/src/settings/types.ts frontend/src/settings/tokens.ts
git commit -m "feat(frontend): 搬入设置页数据模型与渠道 token（扩展 popupPrecreate/overlayPrecreate/collectUsage）"
```

---

## 任务 3：projectToAppConfig 纯函数（TDD）

**文件：**
- 修改：`frontend/src/lib/config.ts`（新增函数）
- 测试：`frontend/src/lib/config.test.ts`（新增用例）

`projectToAppConfig` 把前端 `AppSettings` 投影成后端 `AppConfig`。映射规则（spec §4.3）：

- 默认实例 = `state.translation.defaultServiceInstanceId` 指向的 `ServiceInstance`。
- 走 OpenAI 兼容协议的 type 集合 `OPENAI_COMPATIBLE_TYPES = ['openai', 'custom', 'deepseek', 'zhipu', 'moonshot', 'siliconflow']`（这些渠道后端用 `openai-compatible` provider 接入）。
- 默认实例 type ∈ `OPENAI_COMPATIBLE_TYPES` → `provider='openai-compatible'`，`openaiCompatible` 段取实例值。
- 默认实例 type === `'claude'` → `provider='claude'`，`claude` 段取实例值，`enableThinking = chainOfThought !== 'off'`。
- 其他 type（DeepL / Google / 百度 / 有道 / 腾讯 / 火山 / 讯飞 / Gemini / 用户自定义）→ `provider = lastSavedProvider`（fallback），`unsupported=true`，`unsupportedName = instance.name`；`openaiCompatible` / `claude` 段用后端默认占位值。
- 默认实例不存在（id 为空或找不到）→ `provider = lastSavedProvider`，`unsupported=false`（无实例可提示），段用默认占位。
- 非活跃段始终用后端默认占位（`validateConfig` 只校验活跃 provider 段）。
- `timeoutSeconds` 固定 60（原型实例无此字段）。
- `targetLang ← state.translation.defaultTargetLang`；`popupPrecreate ← state.general.popupPrecreate`；`overlayPrecreate ← state.general.overlayPrecreate`；`collectUsage ← state.advanced.collectUsage`。

后端默认占位值（与 `src-tauri/src/core/config/types.rs` `from_env` 一致）：
- openaiCompatible：`{ apiKey: null, baseUrl: 'https://api.openai.com/v1', model: 'gpt-4o-mini', timeoutSeconds: 60 }`
- claude：`{ apiKey: null, baseUrl: 'https://api.anthropic.com', model: 'claude-haiku-4-5', timeoutSeconds: 60, enableThinking: false }`

- [ ] **步骤 1：编写失败测试**

在 `frontend/src/lib/config.test.ts` 顶部 import 区追加：

```ts
import { projectToAppConfig } from './config';
import type { AppSettings, ServiceInstance } from '@/settings/types';
```

在文件末尾追加测试用例：

```ts
const defaultOpenai = 'https://api.openai.com/v1';
const defaultClaude = 'https://api.anthropic.com';

const makeInstance = (over: Partial<ServiceInstance>): ServiceInstance => ({
  id: 'inst-1',
  type: 'openai',
  name: 'OpenAI',
  enabled: true,
  apiKey: '',
  model: 'gpt-4o-mini',
  endpoint: '',
  note: '',
  pulledModels: [],
  keyStatus: 'idle',
  chainOfThought: 'off',
  systemPrompt: '',
  translationPrompt: '',
  reflectionPrompt: '',
  reflectionEnabled: false,
  ...over,
});

const makeState = (services: ServiceInstance[], defaultId: string): AppSettings => ({
  general: {
    launchAtLogin: false, startMinimized: false, showTrayIcon: true,
    closeAction: 'minimize', theme: 'light', language: 'zh-CN',
    updateChannel: 'stable', autoCheckUpdate: true,
    popupPrecreate: true, overlayPrecreate: false,
  },
  translation: {
    defaultSourceLang: 'auto', defaultTargetLang: '中文',
    defaultServiceInstanceId: defaultId,
    autoCopy: true, restoreClipboard: true, autoPaste: false,
    showPhonetic: true, showAlternatives: true, autoDetect: true,
    wordLookupDelay: 300, historyLimit: 500,
  },
  shortcut: { bindings: [] },
  services,
  customServiceTypes: [],
  advanced: { logLevel: 'info', betaLookup: false, betaVoice: false, collectUsage: true },
  ocrHistory: [],
});

describe('projectToAppConfig', () => {
  it('默认实例为 openai → provider=openai-compatible，字段取实例值', () => {
    const s = makeState(
      [makeInstance({ id: 'i1', type: 'openai', apiKey: 'sk-x', endpoint: 'https://api.openai.com/v1', model: 'gpt-4o' })],
      'i1',
    );
    const { config, unsupported } = projectToAppConfig(s, 'openai-compatible');
    expect(unsupported).toBe(false);
    expect(config.provider).toBe('openai-compatible');
    expect(config.openaiCompatible.apiKey).toBe('sk-x');
    expect(config.openaiCompatible.baseUrl).toBe('https://api.openai.com/v1');
    expect(config.openaiCompatible.model).toBe('gpt-4o');
    expect(config.openaiCompatible.timeoutSeconds).toBe(60);
  });

  it('默认实例为 claude → provider=claude，enableThinking 按 chainOfThought 映射', () => {
    const s = makeState(
      [makeInstance({ id: 'i1', type: 'claude', apiKey: 'sk-ant', endpoint: defaultClaude, model: 'claude-haiku-4-5', chainOfThought: 'medium' })],
      'i1',
    );
    const { config } = projectToAppConfig(s, 'openai-compatible');
    expect(config.provider).toBe('claude');
    expect(config.claude.apiKey).toBe('sk-ant');
    expect(config.claude.enableThinking).toBe(true);
  });

  it('默认实例 chainOfThought=off → enableThinking=false', () => {
    const s = makeState(
      [makeInstance({ id: 'i1', type: 'claude', chainOfThought: 'off' })],
      'i1',
    );
    const { config } = projectToAppConfig(s, 'openai-compatible');
    expect(config.claude.enableThinking).toBe(false);
  });

  it('默认实例为 deepseek（openai 兼容集）→ provider=openai-compatible', () => {
    const s = makeState(
      [makeInstance({ id: 'i1', type: 'deepseek', apiKey: 'sk-ds', endpoint: 'https://api.deepseek.com/v1', model: 'deepseek-chat' })],
      'i1',
    );
    const { config, unsupported } = projectToAppConfig(s, 'openai-compatible');
    expect(unsupported).toBe(false);
    expect(config.provider).toBe('openai-compatible');
    expect(config.openaiCompatible.model).toBe('deepseek-chat');
  });

  it('默认实例为 deepl（非支持类型）→ fallback 到 lastSavedProvider 且 unsupported=true', () => {
    const s = makeState(
      [makeInstance({ id: 'i1', type: 'deepl', name: 'DeepL', apiKey: 'k' })],
      'i1',
    );
    const { config, unsupported, unsupportedName } = projectToAppConfig(s, 'openai-compatible');
    expect(unsupported).toBe(true);
    expect(unsupportedName).toBe('DeepL');
    expect(config.provider).toBe('openai-compatible');
    // fallback 时活跃段用后端默认占位，不取实例值
    expect(config.openaiCompatible.apiKey).toBeNull();
    expect(config.openaiCompatible.baseUrl).toBe(defaultOpenai);
    expect(config.openaiCompatible.model).toBe('gpt-4o-mini');
  });

  it('fallback 时 lastSavedProvider=claude → provider=claude，claude 段用默认占位', () => {
    const s = makeState(
      [makeInstance({ id: 'i1', type: 'deepl', name: 'DeepL' })],
      'i1',
    );
    const { config } = projectToAppConfig(s, 'claude');
    expect(config.provider).toBe('claude');
    expect(config.claude.baseUrl).toBe(defaultClaude);
    expect(config.claude.model).toBe('claude-haiku-4-5');
  });

  it('默认实例不存在（id 空）→ 安全降级，provider=lastSavedProvider，unsupported=false', () => {
    const s = makeState([], '');
    const { config, unsupported } = projectToAppConfig(s, 'openai-compatible');
    expect(unsupported).toBe(false);
    expect(config.provider).toBe('openai-compatible');
    expect(config.openaiCompatible.baseUrl).toBe(defaultOpenai);
  });

  it('已实现字段 targetLang/popupPrecreate/overlayPrecreate/collectUsage 透传', () => {
    const s = makeState(
      [makeInstance({ id: 'i1', type: 'openai' })],
      'i1',
    );
    s.translation.defaultTargetLang = 'English';
    s.general.popupPrecreate = false;
    s.general.overlayPrecreate = true;
    s.advanced.collectUsage = false;
    const { config } = projectToAppConfig(s, 'openai-compatible');
    expect(config.targetLang).toBe('English');
    expect(config.popupPrecreate).toBe(false);
    expect(config.overlayPrecreate).toBe(true);
    expect(config.collectUsage).toBe(false);
  });
});
```

- [ ] **步骤 2：运行测试验证失败**

运行：`npm run test -- src/lib/config.test.ts`
预期：FAIL，报错 `projectToAppConfig is not a function` 或导入失败。

- [ ] **步骤 3：实现 projectToAppConfig**

在 `frontend/src/lib/config.ts` 末尾追加：

```ts
import type { AppSettings, ServiceInstance } from '@/settings/types';

/** 走 OpenAI 兼容协议、后端能用 openai-compatible provider 接入的渠道 type 集合。 */
const OPENAI_COMPATIBLE_TYPES = ['openai', 'custom', 'deepseek', 'zhipu', 'moonshot', 'siliconflow'];

const DEFAULT_OPENAI = {
  apiKey: null as string | null,
  baseUrl: 'https://api.openai.com/v1',
  model: 'gpt-4o-mini',
  timeoutSeconds: 60,
};
const DEFAULT_CLAUDE = {
  apiKey: null as string | null,
  baseUrl: 'https://api.anthropic.com',
  model: 'claude-haiku-4-5',
  timeoutSeconds: 60,
  enableThinking: false,
};

export interface ProjectResult {
  config: AppConfig;
  /** 默认服务实例后端未接入（如 DeepL/Gemini/百度），需 toast 提示。 */
  unsupported: boolean;
  unsupportedName: string;
}

/**
 * 把前端多实例 AppSettings 投影成后端单 provider AppConfig。
 * lastSavedProvider：上次成功保存的 provider，用于非支持类型 fallback（首次 openai-compatible）。
 */
export function projectToAppConfig(state: AppSettings, lastSavedProvider: Provider): ProjectResult {
  const id = state.translation.defaultServiceInstanceId;
  const inst: ServiceInstance | undefined = id ? state.services.find((s) => s.id === id) : undefined;

  let provider: Provider;
  let unsupported = false;
  let unsupportedName = '';

  const openaiCompatible = { ...DEFAULT_OPENAI };
  const claude = { ...DEFAULT_CLAUDE };

  if (inst && OPENAI_COMPATIBLE_TYPES.includes(inst.type)) {
    provider = 'openai-compatible';
    openaiCompatible.apiKey = inst.apiKey || null;
    openaiCompatible.baseUrl = inst.endpoint || DEFAULT_OPENAI.baseUrl;
    openaiCompatible.model = inst.model || DEFAULT_OPENAI.model;
  } else if (inst && inst.type === 'claude') {
    provider = 'claude';
    claude.apiKey = inst.apiKey || null;
    claude.baseUrl = inst.endpoint || DEFAULT_CLAUDE.baseUrl;
    claude.model = inst.model || DEFAULT_CLAUDE.model;
    claude.enableThinking = inst.chainOfThought !== 'off';
  } else {
    provider = lastSavedProvider;
    if (inst) {
      unsupported = true;
      unsupportedName = inst.name;
    }
  }

  const config: AppConfig = {
    provider,
    targetLang: state.translation.defaultTargetLang,
    openaiCompatible,
    claude,
    popupPrecreate: state.general.popupPrecreate,
    overlayPrecreate: state.general.overlayPrecreate,
    collectUsage: state.advanced.collectUsage,
  };
  return { config, unsupported, unsupportedName };
}
```

注意：`config.ts` 顶部已有 `import type { AppConfig } from '@/types/config';`，需补 `Provider` 导入。把第 1 行改为：

```ts
import type { AppConfig, Provider } from '@/types/config';
```

- [ ] **步骤 4：运行测试验证通过**

运行：`npm run test -- src/lib/config.test.ts`
预期：PASS（全部用例，含原 `validateConfig` 用例与新增 `projectToAppConfig` 用例）。

- [ ] **步骤 5：typecheck 验证**

运行：`npm run typecheck`
预期：PASS

- [ ] **步骤 6：Commit**

```bash
git add frontend/src/lib/config.ts frontend/src/lib/config.test.ts
git commit -m "feat(frontend): 新增 projectToAppConfig 投影函数及单测"
```

---

## 任务 4：搬入 store 并改造 seed / save

**文件：**
- 创建：`frontend/src/settings/stores/settings.ts`
- 创建：`frontend/src/lib/toast.ts`

- [ ] **步骤 1：搬入 lib/toast.ts（原样）**

把原型 `src/lib/toast.ts` 原样复制到 `frontend/src/lib/toast.ts`，无改写。

- [ ] **步骤 2：搬入 stores/settings.ts 并做 5 处改造**

把原型 `src/settings/stores/settings.ts` 原样复制到 `frontend/src/settings/stores/settings.ts`，然后做以下改造：

**改造 A — import 增补桥接依赖。** 在文件顶部 import 区（`import { BUILTIN_SERVICES, buildServices, DEFAULT_PROMPTS } from '../tokens'` 之后）追加：

```ts
import { projectToAppConfig } from '@/lib/config'
import { validateConfig } from '@/lib/config'
import { invokeSaveAppConfig, isTauriReady } from '@/lib/tauri'
import { toast } from '@/lib/toast'
import type { AppConfig, Provider } from '@/types/config'
```

**改造 B — seedInstances 只 seed openai + claude 两个实例（spec §4.1）。** 把 `seedInstances` 函数替换为：

```ts
const OPENAI_DEFAULT_ENDPOINT = 'https://api.openai.com/v1'
const CLAUDE_DEFAULT_ENDPOINT = 'https://api.anthropic.com'

/**
 * 首次启动 seed：只生成 openai 与 claude 各一个空实例，baseUrl/model 与后端 from_env 默认一致。
 * 其余 13 种渠道由用户在「服务」面板手动新建（addService）。
 */
const seedInstances = (): ServiceInstance[] => [
  defaultInstanceFor('openai', 'OpenAI'),
  defaultInstanceFor('claude', 'Claude'),
]
```

**改造 C — defaultInstanceFor 给 openai/claude 默认 endpoint。** 把 `defaultInstanceFor` 中 `endpoint: meta?.needsEndpoint ? '' : '',` 这一行替换为按 type 注入默认 endpoint：

```ts
const defaultEndpointFor = (type: ServiceId): string => {
  if (type === 'openai') return 'https://api.openai.com/v1'
  if (type === 'claude') return 'https://api.anthropic.com'
  return ''
}

const defaultInstanceFor = (type: ServiceId, name: string): ServiceInstance => {
  const meta = BUILTIN_SERVICES.find((s) => s.id === type)
  return {
    id: newInstanceId(),
    type,
    name,
    enabled: true,
    apiKey: '',
    model: meta?.defaultModel ?? '',
    endpoint: defaultEndpointFor(type),
    note: '',
    pulledModels: [],
    keyStatus: 'idle',
    chainOfThought: 'off',
    systemPrompt: DEFAULT_PROMPTS.system,
    translationPrompt: DEFAULT_PROMPTS.translation,
    reflectionPrompt: DEFAULT_PROMPTS.reflection,
    reflectionEnabled: false,
  }
}
```

**改造 D — buildDefaults 补 3 个已实现字段默认值 + 快捷键默认值改 Alt+T/Alt+O。** 在 `buildDefaults` 返回对象中：

- `general` 块追加 `popupPrecreate: true,` 与 `overlayPrecreate: true,`（紧跟 `autoCheckUpdate: true,` 之后）。
- `advanced` 块追加 `collectUsage: true,`（紧跟 `betaVoice: false,` 之后）。
- `shortcut.bindings` 中 `translate-selection` 的 `keys` 改为 `'Alt+T'`；`translate-screenshot` 的 `keys` 改为 `'Alt+O'` 并**删除**其 `error` 字段（Alt+O 不与系统冲突）。其余 4 条 binding 保持原样。

**改造 E — 新增 lastSavedProvider 模块变量，save() 改造为 async 桥接流程。** 在 `const dirty = reactive({ value: false })` 之前新增：

```ts
/** 内存持有上次成功保存的 provider，供非支持类型实例 fallback（spec §4.3）。首次为 openai-compatible。 */
let lastSavedProvider: Provider = 'openai-compatible'
```

把 `useSettings()` 返回对象中的 `save(): void { ... }` 替换为 async 版本（spec §4.4）：

```ts
  async save(): Promise<void> {
    const { config, unsupported, unsupportedName } = projectToAppConfig(state, lastSavedProvider)
    const err = validateConfig(config)
    if (err) {
      toast.error('保存失败', err)
      return
    }
    if (isTauriReady()) {
      try {
        await invokeSaveAppConfig(config)
        lastSavedProvider = config.provider
        Object.assign(baseline, JSON.parse(JSON.stringify(state)))
        dirty.value = false
        if (unsupported) {
          toast.info('已本地保存', `默认服务「${unsupportedName}」暂未接入后端，仅本地保存`)
        } else {
          toast.success('配置已保存')
        }
      } catch (e) {
        toast.error('保存失败', String(e))
      }
    } else {
      Object.assign(baseline, JSON.parse(JSON.stringify(state)))
      dirty.value = false
      toast.info('Tauri 未就绪，仅本地保存')
    }
  },
```

注意：`SettingsLayout.vue` 中 `@click="save"` 调用无需 await（Vue 事件可直接触发 async 函数），保持原型原样。

- [ ] **步骤 3：typecheck 验证**

运行：`npm run typecheck`
预期：可能因尚未搬入 components/panels/ui 而对 store 自身无影响（store 不依赖它们）；store 应通过类型检查。若报错仅来自旧 App.vue 引用的已删除符号，记下留待任务 9 处理。

- [ ] **步骤 4：运行现有单测确保未破坏**

运行：`npm run test`
预期：PASS（`config.test.ts` 全过；store 无单测）

- [ ] **步骤 5：Commit**

```bash
git add frontend/src/lib/toast.ts frontend/src/settings/stores/settings.ts
git commit -m "feat(frontend): 搬入设置页 store（seed 精简为 openai/claude + save 桥接 save_app_config）"
```

---

## 任务 5：搬入 UI 原子 + toast 组件

**文件：**
- 创建：`frontend/src/components/ui/{button,badge,input,switch,select,dialog,tooltip,toast}/`（含各 `index.ts` + `.vue`）
- 创建：`frontend/src/components/ui/index.ts`
- 删除：`frontend/src/components/ui/{button,card,input,label,select,switch}/`（旧生成原子，替换）

- [ ] **步骤 1：删除旧 UI 原子目录**

```bash
rm -rf frontend/src/components/ui/button frontend/src/components/ui/card frontend/src/components/ui/input frontend/src/components/ui/label frontend/src/components/ui/select frontend/src/components/ui/switch
```

（旧 `frontend/src/components/ui/` 下仅这 6 个目录，全删；原型整套重新搬入。）

- [ ] **步骤 2：搬入原型 UI 原子整套**

把原型 `src/components/ui/` 下的 8 个目录（`button` / `badge` / `input` / `switch` / `select` / `dialog` / `tooltip` / `toast`）原样复制到 `frontend/src/components/ui/`。把原型 `src/components/ui/index.ts` 原样复制到 `frontend/src/components/ui/index.ts`。

这些文件内部**不**引用 `lucide-vue-next`（已核对：仅 `badge`/`button` 用 `cva`，`toast`/`dialog`/`select`/`tooltip` 用 `reka-ui`），故无需图标改写。

- [ ] **步骤 3：核对 toast 组件引用的 toast store 路径**

打开 `frontend/src/components/ui/toast/` 下各 `.vue`，确认其 import `useToasts` / `toast` 的来源路径。原型 toast 组件从 `@/lib/toast` 引用（任务 4 已搬入）。若实际路径不同，改写为 `@/lib/toast`。

- [ ] **步骤 4：typecheck 验证**

运行：`npm run typecheck`
预期：旧 `App.vue` 引用的旧 UI 原子（如 `Button`）现在指向新原型原子，签名兼容（`Button` 仍有 `variant` / `size`）。可能仍有旧设置页子组件引用错误，留待任务 8/9。新 UI 原子自身应无类型错误。

- [ ] **步骤 5：Commit**

```bash
git add frontend/src/components/ui
git commit -m "feat(frontend): 搬入原型 UI 原子（button/badge/input/switch/select/dialog/tooltip/toast）"
```

---

## 任务 6：搬入 settings 外壳与原子组件

**文件：**
- 创建：`frontend/src/settings/SettingsPage.vue` / `SettingsLayout.vue` / `SettingsSidebar.vue`
- 创建：`frontend/src/settings/components/*.vue` + `index.ts` + `types.ts`

- [ ] **步骤 1：搬入三件外壳**

把原型 `src/settings/SettingsPage.vue` / `SettingsLayout.vue` / `SettingsSidebar.vue` 原样复制到 `frontend/src/settings/`。`SettingsLayout.vue` 与 `SettingsSidebar.vue` 中 `from 'lucide-vue-next'` → `from '@lucide/vue'`。

- [ ] **步骤 2：搬入 11 个原子组件 + index + types**

把原型 `src/settings/components/` 整个目录原样复制到 `frontend/src/settings/components/`（含 `SettingRow.vue` / `SettingGroup.vue` / `SettingSelect.vue` / `SettingSwitch.vue` / `SettingInput.vue` / `SettingTextarea.vue` / `ApiKeyInput.vue` / `ShortcutRecorder.vue` / `ServiceIcon.vue` / `ModelCombobox.vue` / `ChannelCombobox.vue` / `index.ts` / `types.ts`）。

对目录下所有 `.vue` / `.ts` 文件，将 `from 'lucide-vue-next'` → `from '@lucide/vue'`。

- [ ] **步骤 3：SettingRow 的 wip 文案统一为「实现中」（spec §6）**

打开 `frontend/src/settings/components/SettingRow.vue`，把：

```ts
const statusLabel: Record<NonNullable<Props['status']>, string> = {
  wip: '开发中',
  planned: '规划中',
}
```

改为：

```ts
const statusLabel: Record<NonNullable<Props['status']>, string> = {
  wip: '实现中',
  planned: '规划中',
}
```

同步把 `:title` 提示中「该功能尚未开发完成,留作后续迭代」保留不动（语义仍准确）。

- [ ] **步骤 4：SettingsSidebar 历史分类徽标文案同步**

打开 `frontend/src/settings/SettingsSidebar.vue`，把 `badgeLabel` 中 `if (kind === 'wip') return '开发中'` 改为 `if (kind === 'wip') return '实现中'`，与 SettingRow 统一（spec §6：视觉同款 amber Badge，文案统一「实现中」）。

- [ ] **步骤 5：typecheck 验证**

运行：`npm run typecheck`
预期：外壳与组件自身通过；`SettingsPage` 引用的 6 个 panels 尚未搬入，会报缺失模块错误——属预期，任务 7 补齐。

- [ ] **步骤 6：Commit**

```bash
git add frontend/src/settings/SettingsPage.vue frontend/src/settings/SettingsLayout.vue frontend/src/settings/SettingsSidebar.vue frontend/src/settings/components
git commit -m "feat(frontend): 搬入设置页外壳与原子组件（wip 文案统一为「实现中」）"
```

---

## 任务 7：搬入 6 个面板并改造

**文件：**
- 创建：`frontend/src/settings/panels/{General,Translate,Shortcut,Services,History,Advanced}Panel.vue`

先原样搬入全部 6 个面板并改图标 import，再对 3 个面板做已实现字段接入与标签改造。

- [ ] **步骤 1：搬入 6 个面板 + 图标改写**

把原型 `src/settings/panels/` 整个目录原样复制到 `frontend/src/settings/panels/`。对目录下所有 `.vue` 文件，将 `from 'lucide-vue-next'` → `from '@lucide/vue'`。

- [ ] **步骤 2：GeneralPanel 新增「窗口策略」分组（已实现 popupPrecreate/overlayPrecreate）**

打开 `frontend/src/settings/panels/GeneralPanel.vue`，在 `<SettingGroup title="更新"...>` 之前插入新分组：

```html
  <SettingGroup
    title="窗口策略"
    description="翻译弹窗与截图 overlay 的预创建策略，重启应用后生效。"
  >
    <SettingRow
      title="预创建翻译弹窗"
      description="应用启动时即创建翻译窗口，划词时响应更快。"
    >
      <SettingSwitch v-model="state.general.popupPrecreate" aria-label="预创建翻译弹窗" />
    </SettingRow>
    <SettingRow
      title="预创建截图 Overlay"
      description="应用启动时即创建截图 overlay 窗口，截图 OCR 时响应更快。"
    >
      <SettingSwitch v-model="state.general.overlayPrecreate" aria-label="预创建截图 Overlay" />
    </SettingRow>
  </SettingGroup>
```

- [ ] **步骤 3：AdvancedPanel 新增「使用统计」行（已实现 collectUsage）**

打开 `frontend/src/settings/panels/AdvancedPanel.vue`，在 `<SettingGroup title="实验性功能"...>` 之前插入：

```html
  <SettingGroup title="隐私" description="匿名使用统计帮助改进产品，不包含翻译内容与 API Key。">
    <SettingRow
      title="收集匿名使用统计"
      description="重启后生效。"
    >
      <SettingSwitch v-model="state.advanced.collectUsage" aria-label="收集匿名使用统计" />
    </SettingRow>
  </SettingGroup>
```

- [ ] **步骤 4：ShortcutPanel 改造（Alt+T/Alt+O 只读 + 全行「实现中」标签）**

spec §5/§6 对快捷键面板：后端硬编码 Alt+T（划词）/Alt+O（截图 OCR）只读展示当前绑定；后端不支持动态快捷键配置，整面板属「实现中」。本计划采用：所有快捷键行 `status="wip"`；Alt+T/Alt+O 两条 `ShortcutRecorder` 设为只读（`disabled`），其余可编辑。

打开 `frontend/src/settings/panels/ShortcutPanel.vue`，把 `<script setup>` 改为：

```ts
<script setup lang="ts">
import { SettingGroup, SettingRow, ShortcutRecorder } from '../components'
import type { AppSettings } from '../types'

defineProps<{
  state: AppSettings
}>()

/** 后端硬编码、不可配置的快捷键 id（只读展示真实绑定）。 */
const READONLY_IDS = new Set(['translate-selection', 'translate-screenshot'])
</script>
```

把模板中 `<SettingRow v-for="binding in state.shortcut.bindings" ...>` 改为（加 `status="wip"`，ShortcutRecorder 加 `:disabled`）：

```html
    <SettingRow
      v-for="binding in state.shortcut.bindings"
      :key="binding.id"
      :title="binding.label"
      :description="binding.description"
      status="wip"
    >
      <ShortcutRecorder
        :model-value="binding.keys"
        :error="binding.error"
        :disabled="READONLY_IDS.has(binding.id)"
        @update:model-value="(v) => {
          binding.keys = v
          if (v) binding.error = undefined
        }"
      />
    </SettingRow>
```

确认 `ShortcutRecorder.vue` 支持 `disabled` prop（原型组件通常已支持；若不支持，在任务 6 搬入的 `ShortcutRecorder.vue` 中给 `defineProps` 加 `disabled?: boolean` 并在根元素 `:disabled` / `aria-disabled` 上透传，输入keydown 监听里 `if (props.disabled) return`）。如需修改，一并改并在 commit 说明。

- [ ] **步骤 5：typecheck 验证**

运行：`npm run typecheck`
预期：PASS（此时 settings 整套已搬入；旧 App.vue 仍在，但它引用的旧 7 子组件还未删，可能仍报错——留待任务 8 替换 App.vue、任务 9 删旧子组件后清零）

- [ ] **步骤 6：Commit**

```bash
git add frontend/src/settings/panels
git commit -m "feat(frontend): 搬入 6 个设置面板（通用窗口策略/高级统计/快捷键只读+实现中标签）"
```

---

## 任务 8：替换 settings 入口 App.vue / main.ts

**文件：**
- 修改：`frontend/src/settings/App.vue`（内容替换为原型 App.vue）
- 修改：`frontend/src/settings/main.ts`

- [ ] **步骤 1：替换 App.vue**

把 `frontend/src/settings/App.vue` 整体内容替换为：

```vue
<script setup lang="ts">
import { TooltipProvider } from 'reka-ui'
import SettingsPage from '@/settings/SettingsPage.vue'
import { Toaster } from '@/components/ui/toast'
</script>

<template>
  <TooltipProvider :delay-duration="250" :skip-delay-duration="300">
    <SettingsPage />
  </TooltipProvider>
  <Toaster />
</template>
```

- [ ] **步骤 2：核对 main.ts**

`frontend/src/settings/main.ts` 保持现状：

```ts
import { createApp } from 'vue';
import App from './App.vue';
import '@/styles/main.css';

createApp(App).mount('#app');
```

无需改动（CSS 已由 main.css 引入，原型 `style.css` 不搬）。

- [ ] **步骤 3：typecheck 验证**

运行：`npm run typecheck`
预期：仍可能因旧 7 子组件文件存在但已无人引用而通过；若旧子组件内部互相引用报错，任务 9 删除后即清。

- [ ] **步骤 4：Commit**

```bash
git add frontend/src/settings/App.vue
git commit -m "feat(frontend): 设置页入口接入原型外壳（TooltipProvider + Toaster）"
```

---

## 任务 9：清理旧设置页子组件

**文件：**
- 删除：`frontend/src/settings/components/{TargetLangSection,ProviderSelect,OpenAiSection,ClaudeSection,StrategySection,SaveBar,ApiKeyField}.vue`

- [ ] **步骤 1：删除旧 7 子组件**

```bash
rm frontend/src/settings/components/TargetLangSection.vue \
   frontend/src/settings/components/ProviderSelect.vue \
   frontend/src/settings/components/OpenAiSection.vue \
   frontend/src/settings/components/ClaudeSection.vue \
   frontend/src/settings/components/StrategySection.vue \
   frontend/src/settings/components/SaveBar.vue \
   frontend/src/settings/components/ApiKeyField.vue
```

注意：`frontend/src/settings/components/` 现已存放原型搬入的 11 个原子组件（任务 6），上述 7 个旧文件是旧设置页专属，删除后无其他引用（新 App.vue 不引用它们）。

- [ ] **步骤 2：全量 typecheck + 单测**

运行：`npm run typecheck`
预期：PASS（零错误）

运行：`npm run test`
预期：PASS（`config.test.ts` 全过）

- [ ] **步骤 3：构建前端**

运行：`npm run build`
预期：构建成功，产物输出到 `frontend/dist/`。

- [ ] **步骤 4：Commit**

```bash
git add -A frontend/src/settings
git commit -m "chore(frontend): 清理旧设置页 7 个子组件"
```

---

## 任务 10：验收（手动 + 回归）

- [ ] **步骤 1：启动 tauri dev**

运行：`npm run tauri dev`
预期：应用启动，主窗口加载新设置页，左侧 sidebar 6 个分类可见，「翻译历史」带「实现中」amber 徽标。

- [ ] **步骤 2：验证已实现字段保存生效**

在「通用 → 窗口策略」切换「预创建翻译弹窗」/「预创建截图 Overlay」；在「高级 → 隐私」切换「收集匿名使用统计」；在「翻译」改默认目标语言；在「服务」选默认实例为 openai，填 apiKey/baseUrl/model。点「保存」→ toast「配置已保存」。关闭应用后查看 `<app config dir>/config.json`，确认 `popupPrecreate` / `overlayPrecreate` / `collectUsage` / `targetLang` / `provider` / `openaiCompatible` 字段已写入。

重启应用，再次触发划词翻译（Alt+T）与截图 OCR（Alt+O），确认窗口策略与翻译链路按新配置工作。

- [ ] **步骤 3：验证未实现字段本地持久化 + 标签**

在「服务」新建一个 DeepL 实例并设为默认服务，填 key。点「保存」→ toast「已本地保存：默认服务「DeepL」暂未接入后端，仅本地保存」。刷新页面（或重启 dev server），确认 DeepL 实例仍在、字段保留。确认快捷键面板所有行带「实现中」amber 标签，Alt+T/Alt+O 两条只读不可编辑，其余可编辑。确认「翻译历史」面板内容展示且 sidebar 徽标为「实现中」。

- [ ] **步骤 4：回归现有单测与 typecheck**

运行：`npm run test && npm run typecheck && npm run build`
预期：全部 PASS。

- [ ] **步骤 5：Commit 验收记录（可选）**

若验收过程有微小修复，按修复内容提交；无改动则跳过。

---

## 自检

**1. 规格覆盖度**（对照 spec 章节）：
- §2 删除范围 → 任务 9 删旧 7 子组件 + 任务 5 删旧 ui 6 目录 ✓
- §2 新增范围 → 任务 1–8 覆盖 settings/、components/ui/、lib/config.ts、lib/toast.ts ✓
- §3.1 Tailwind v3→v4 → 任务 1（main.css token）✓
- §3.2 图标包改写 → 搬运总规则 + 各任务图标改写步骤 ✓
- §3.3 UI 原子替换 → 任务 5 ✓
- §4.1 store 原样搬入 + localStorage 唯一真相源 + seed openai/claude → 任务 4 改造 B/C/D ✓
- §4.2/§4.3 projectToAppConfig → 任务 3 ✓
- §4.4 save 流程 → 任务 4 改造 E ✓
- §5 字段实现状态 → 通用窗口策略（任务 7 步骤 2）、翻译 defaultTargetLang（原型 TranslatePanel 已绑定 store）、快捷键 Alt+T/Alt+O 只读（任务 7 步骤 4）、服务 key/baseUrl/model（原型 ServicesPanel + projectToAppConfig）、高级 collectUsage（任务 7 步骤 3）✓
- §6 「实现中」标签 → SettingRow wip 文案统一（任务 6 步骤 3）、sidebar 徽标（任务 6 步骤 4）、快捷键行 status=wip（任务 7 步骤 4）、历史面板 sidebar wip（原型自带）✓
- §7 测试 → projectToAppConfig 单测（任务 3）+ validateConfig 保留 + typecheck + tauri dev 手动（任务 10）✓
- §8 风险 → Tailwind token 保留 HSL（任务 1 用 hsl(var(--x)) 桥接，不混 oklch）；两份配置漂移在保存时同步写两边（任务 4 save）✓

**2. 占位符扫描**：无「待定/TODO/类似任务 N」；每个代码步骤均含完整代码或精确搬运规则 + 验证命令。

**3. 类型一致性**：
- `projectToAppConfig(state, lastSavedProvider): { config, unsupported, unsupportedName }` 在任务 3 定义，任务 4 save 按此签名解构使用 ✓
- `AppSettings.general.popupPrecreate/overlayPrecreate`、`AppSettings.advanced.collectUsage` 在任务 2 类型扩展，任务 4 buildDefaults 给默认值，任务 7 面板绑定，任务 3 测试构造 ✓
- `Provider` 类型来自 `@/types/config`，任务 3 与任务 4 均从此导入 ✓
- `ServiceInstance` 字段（id/type/name/apiKey/endpoint/model/chainOfThought）在任务 3 测试与 projectToAppConfig 实现中一致 ✓

**遗留决策（计划阶段细化，执行时如审查者异议可回退）**：
1. `OPENAI_COMPATIBLE_TYPES` 集合含 openai/custom/deepseek/zhipu/moonshot/siliconflow（走 openai 兼容协议）；Gemini 走自有协议归 fallback（与 spec §4.3 表「其他」举例含 Gemini 一致）。
2. spec §5 与 §6 对「Alt+T/Alt+O 两条是否标标签」表述略有差异：本计划采用「全行 status=wip + 两条只读」，即整面板「实现中」，因为后端不支持动态快捷键配置。
3. SettingRow / sidebar 的 wip 文案统一为「实现中」（spec §6 顶部明确文案「实现中」），覆盖原型默认「开发中」。
