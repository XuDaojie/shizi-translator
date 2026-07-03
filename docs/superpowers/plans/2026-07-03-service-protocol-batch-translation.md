# 服务协议抽象与多结果翻译实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 让启用的服务实例按设置页列表顺序同时翻译，并在弹窗中渲染多个独立结果卡。

**架构：** 新版配置直接以 `services[]` 为事实来源，不兼容旧单 provider 配置。前端负责维护 `ServiceInstance.protocol` 和服务元数据，后端用 `provider_for_service` 把服务实例映射到现有 LLM provider，翻译入口按启用服务创建一个批次。

**技术栈：** Vue 3、Vitest、Tauri 2、Rust、serde、tokio、reqwest。

---

## 文件结构

- 修改：`frontend/src/types/config.ts`
  定义新版 `AppConfig`、`ServiceInstanceConfig`、`ServiceProtocolId`，删除旧 `provider/openaiCompatible/claude` 配置形状。
- 修改：`frontend/src/settings/types.ts`
  给设置页 `ServiceInstance` 增加 `protocol`，给 `ServiceMeta` 增加 `protocols` 和 `officialEndpoint`。
- 修改：`frontend/src/lib/config.ts`
  把设置页状态直接投影成新版 `AppConfig.services[]`，校验启用服务的 key、endpoint、model、protocol。
- 修改：`frontend/src/lib/config.test.ts`
  覆盖新版投影和校验。
- 修改：`frontend/src/settings/tokens.ts`
  为内置服务补 `openai_chat` / `claude_messages` 协议元数据，DeepSeek / 智谱 AI 默认端点和模型落到元数据里。
- 修改：`frontend/src/settings/stores/settings.ts`
  首启默认只 seed DeepSeek / 智谱 AI，默认关闭；删除 `defaultServiceInstanceId` 旧逻辑。
- 创建：`frontend/src/settings/stores/settings.test.ts`
  验证首启服务列表和协议默认值。
- 创建：`frontend/src/settings/service-validation.ts`
  放置开启服务前的纯校验函数，供 UI 和测试共用。
- 创建：`frontend/src/settings/service-validation.test.ts`
  覆盖服务开关校验。
- 修改：`frontend/src/settings/panels/ServicesPanel.vue`
  endpoint 始终展示；增加协议选择；开关启用前走纯校验函数。
- 修改：`src-tauri/src/core/config/types.rs`
  新增 Rust 侧 `ServiceInstanceConfig`，`AppConfig` 改为 `services[]` 驱动。
- 修改：`src-tauri/src/core/config/mod.rs`
  导出 `ServiceInstanceConfig`。
- 创建：`src-tauri/src/core/llm/protocol.rs`
  新增 `provider_for_service` 工厂函数。
- 修改：`src-tauri/src/core/llm/mod.rs`
  导出协议工厂。
- 修改：`src-tauri/src/core/translation/types.rs`
  事件增加服务信息，`TranslationRequest` 携带 `TranslationServiceMeta`。
- 修改：`src-tauri/src/core/translation/service.rs`
  `Delta/Finished/Cancelled` 事件透传 request 中的服务信息。
- 创建：`src-tauri/src/core/translation/batch.rs`
  生成批次请求，过滤启用服务并保持数组顺序。
- 修改：`src-tauri/src/core/translation/mod.rs`
  导出批次 helper。
- 修改：`src-tauri/src/ui/web_popup.rs`
  启动翻译时按启用服务创建批次、逐服务发事件、失败互不影响。
- 修改：`src-tauri/src/lib.rs`
  启动显隐逻辑基于新版 `AppConfig::is_configured()`。
- 修改：`frontend/public/translate.html`
  单卡片容器改为 `resultsList`。
- 修改：`frontend/public/translate.js`
  用 `Map<serviceInstanceId, card>` 管理多个结果卡。
- 修改：`frontend/public/translate.css`
  保留现有卡片样式，补多卡片失败/取消状态。
- 修改：`README.md`
  同步新版配置和多结果翻译能力。
- 修改：`docs/roadmap/progressive-development-plan.md`
  标记服务协议与多结果弹窗进度。
- 修改：`AGENTS.md`
  同步项目结构与架构关键点。
- 修改：`CLAUDE.md`
  与 `AGENTS.md` 保持一致。

## 任务 1：前端配置类型与投影

**文件：**
- 修改：`frontend/src/types/config.ts`
- 修改：`frontend/src/settings/types.ts`
- 修改：`frontend/src/lib/config.ts`
- 修改：`frontend/src/lib/config.test.ts`

- [ ] **步骤 1：编写失败的测试**

把 `frontend/src/lib/config.test.ts` 中旧 provider 投影用例替换为新版服务数组用例：

```ts
import { describe, it, expect } from 'vitest';
import { projectToAppConfig, validateConfig } from './config';
import type { AppConfig } from '@/types/config';
import type { AppSettings, ServiceInstance } from '@/settings/types';

const makeInstance = (over: Partial<ServiceInstance>): ServiceInstance => ({
  id: 'inst-1',
  type: 'deepseek',
  name: 'DeepSeek',
  enabled: false,
  protocol: 'openai_chat',
  apiKey: '',
  model: 'deepseek-chat',
  endpoint: 'https://api.deepseek.com',
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

const makeState = (services: ServiceInstance[]): AppSettings => ({
  general: {
    launchAtLogin: false,
    startMinimized: false,
    showTrayIcon: true,
    closeAction: 'minimize',
    theme: 'light',
    language: 'zh-CN',
    updateChannel: 'stable',
    autoCheckUpdate: true,
    popupPrecreate: true,
    overlayPrecreate: false,
  },
  translation: {
    defaultSourceLang: 'auto',
    defaultTargetLang: '中文',
    autoCopy: true,
    restoreClipboard: true,
    autoPaste: false,
    showPhonetic: true,
    showAlternatives: true,
    autoDetect: true,
    wordLookupDelay: 300,
    historyLimit: 500,
  },
  shortcut: { bindings: [] },
  services,
  customServiceTypes: [],
  advanced: { logLevel: 'info', betaLookup: false, betaVoice: false, collectUsage: true },
  ocrHistory: [],
});

describe('projectToAppConfig', () => {
  it('保留 services 数组顺序并投影为后端配置', () => {
    const state = makeState([
      makeInstance({
        id: 'deepseek-1',
        type: 'deepseek',
        name: 'DeepSeek',
        enabled: true,
        apiKey: 'sk-ds',
        endpoint: 'https://api.deepseek.com',
        model: 'deepseek-chat',
      }),
      makeInstance({
        id: 'zhipu-1',
        type: 'zhipu',
        name: '智谱 AI',
        enabled: false,
        apiKey: 'sk-zp',
        endpoint: 'https://open.bigmodel.cn/api/paas/v4',
        model: 'glm-4-flash',
      }),
    ]);

    const config = projectToAppConfig(state);

    expect(config.targetLang).toBe('中文');
    expect(config.popupPrecreate).toBe(true);
    expect(config.overlayPrecreate).toBe(false);
    expect(config.collectUsage).toBe(true);
    expect(config.services.map((s) => s.id)).toEqual(['deepseek-1', 'zhipu-1']);
    expect(config.services[0]).toMatchObject({
      serviceType: 'deepseek',
      name: 'DeepSeek',
      enabled: true,
      protocol: 'openai_chat',
      apiKey: 'sk-ds',
      endpoint: 'https://api.deepseek.com',
      model: 'deepseek-chat',
      timeoutSeconds: 60,
    });
  });
});

describe('validateConfig', () => {
  const base: AppConfig = {
    targetLang: '中文',
    services: [],
    popupPrecreate: true,
    overlayPrecreate: true,
    collectUsage: true,
  };

  it('没有启用服务时允许保存', () => {
    expect(validateConfig(base)).toBeNull();
  });

  it('启用服务缺 API Key 时报错', () => {
    expect(validateConfig({
      ...base,
      services: [{
        id: 'deepseek-1',
        serviceType: 'deepseek',
        name: 'DeepSeek',
        enabled: true,
        protocol: 'openai_chat',
        apiKey: null,
        endpoint: 'https://api.deepseek.com',
        model: 'deepseek-chat',
        timeoutSeconds: 60,
      }],
    })).toBe('DeepSeek 请先填写 API Key');
  });

  it('启用服务 endpoint 必须是 http(s)', () => {
    expect(validateConfig({
      ...base,
      services: [{
        id: 'deepseek-1',
        serviceType: 'deepseek',
        name: 'DeepSeek',
        enabled: true,
        protocol: 'openai_chat',
        apiKey: 'sk-x',
        endpoint: 'ftp://api.deepseek.com',
        model: 'deepseek-chat',
        timeoutSeconds: 60,
      }],
    })).toBe('DeepSeek Endpoint 请输入有效的 http(s) 地址');
  });
});
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
npm run test -- frontend/src/lib/config.test.ts
```

预期：FAIL，TypeScript 报 `protocol`、新版 `AppConfig.services` 或 `projectToAppConfig` 返回形状不匹配。

- [ ] **步骤 3：编写最少实现代码**

`frontend/src/types/config.ts` 改为：

```ts
export type ServiceProtocolId = 'openai_chat' | 'claude_messages';

export interface ServiceInstanceConfig {
  id: string;
  serviceType: string;
  name: string;
  enabled: boolean;
  protocol: ServiceProtocolId;
  apiKey: string | null;
  endpoint: string;
  model: string;
  timeoutSeconds: number;
}

export interface AppConfig {
  targetLang: string;
  services: ServiceInstanceConfig[];
  popupPrecreate: boolean;
  overlayPrecreate: boolean;
  collectUsage: boolean;
}
```

`frontend/src/settings/types.ts` 中删除 `TranslationSettings.defaultServiceInstanceId`，并补协议类型：

```ts
import type { ServiceProtocolId } from '@/types/config'

export interface ServiceInstance {
  id: string
  type: ServiceId
  name: string
  enabled: boolean
  protocol: ServiceProtocolId
  apiKey: string
  model: string
  endpoint: string
  note: string
  pulledModels: string[]
  keyStatus: 'idle' | 'validating' | 'valid' | 'invalid'
  chainOfThought: 'off' | 'short' | 'medium' | 'long'
  systemPrompt: string
  translationPrompt: string
  reflectionPrompt: string
  reflectionEnabled: boolean
}

export interface TranslationSettings {
  defaultSourceLang: string
  defaultTargetLang: string
  autoCopy: boolean
  restoreClipboard: boolean
  autoPaste: boolean
  showPhonetic: boolean
  showAlternatives: boolean
  autoDetect: boolean
  wordLookupDelay: number
  historyLimit: number
}

export type ServiceProtocolMeta = {
  id: ServiceProtocolId
  label: string
  defaultEndpoint: string
  defaultModel: string
  editableEndpoint: boolean
  status: 'available' | 'planned'
}

export type ServiceMeta = {
  id: ServiceId
  name: string
  description: string
  builtin: boolean
  defaultModel?: string
  models?: string[]
  needsEndpoint?: boolean
  hasModelApi?: boolean
  iconifyId?: string
  category: 'llm' | 'ml'
  keyRequired: boolean
  protocols: ServiceProtocolMeta[]
  officialEndpoint?: string
}
```

`frontend/src/lib/config.ts` 改为直接投影服务数组：

```ts
import type { AppConfig, ServiceProtocolId } from '@/types/config';
import type { AppSettings } from '@/settings/types';

const AVAILABLE_PROTOCOLS: readonly ServiceProtocolId[] = ['openai_chat', 'claude_messages'];

export function validateConfig(config: AppConfig): string | null {
  for (const service of config.services.filter((s) => s.enabled)) {
    if (!AVAILABLE_PROTOCOLS.includes(service.protocol)) {
      return `${service.name} 当前协议不可用`;
    }
    if (!service.apiKey?.trim()) {
      return `${service.name} 请先填写 API Key`;
    }
    let url: URL;
    try {
      url = new URL(service.endpoint);
    } catch {
      return `${service.name} Endpoint 请输入有效的 http(s) 地址`;
    }
    if (url.protocol !== 'http:' && url.protocol !== 'https:') {
      return `${service.name} Endpoint 请输入有效的 http(s) 地址`;
    }
    if (!service.model.trim()) {
      return `${service.name} Model 不能为空`;
    }
    if (!Number.isInteger(service.timeoutSeconds)
      || service.timeoutSeconds < 1
      || service.timeoutSeconds > 600) {
      return `${service.name} Timeout 秒请输入 1-600 的整数`;
    }
  }
  return null;
}

export function projectToAppConfig(state: AppSettings): AppConfig {
  return {
    targetLang: state.translation.defaultTargetLang,
    services: state.services.map((service) => ({
      id: service.id,
      serviceType: service.type,
      name: service.name,
      enabled: service.enabled,
      protocol: service.protocol,
      apiKey: service.apiKey.trim() || null,
      endpoint: service.endpoint.trim(),
      model: service.model.trim(),
      timeoutSeconds: 60,
    })),
    popupPrecreate: state.general.popupPrecreate,
    overlayPrecreate: state.general.overlayPrecreate,
    collectUsage: state.advanced.collectUsage,
  };
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：

```bash
npm run test -- frontend/src/lib/config.test.ts
```

预期：PASS，`config.test.ts` 全部通过。

- [ ] **步骤 5：Commit**

```bash
git add frontend/src/types/config.ts frontend/src/settings/types.ts frontend/src/lib/config.ts frontend/src/lib/config.test.ts
git commit -m "feat(配置): 改用服务实例数组投影"
```

## 任务 2：服务元数据与首启默认列表

**文件：**
- 修改：`frontend/src/settings/tokens.ts`
- 修改：`frontend/src/settings/stores/settings.ts`
- 创建：`frontend/src/settings/stores/settings.test.ts`

- [ ] **步骤 1：编写失败的测试**

创建 `frontend/src/settings/stores/settings.test.ts`：

```ts
import { describe, expect, it } from 'vitest';
import { useSettings } from './settings';

describe('settings defaults', () => {
  it('首启只展示 DeepSeek 和智谱 AI，且默认关闭', () => {
    const { state } = useSettings();

    expect(state.services.map((s) => s.type)).toEqual(['deepseek', 'zhipu']);
    expect(state.services.map((s) => s.enabled)).toEqual([false, false]);
    expect(state.services.map((s) => s.protocol)).toEqual(['openai_chat', 'openai_chat']);
    expect(state.services[0].endpoint).toBe('https://api.deepseek.com');
    expect(state.services[1].endpoint).toBe('https://open.bigmodel.cn/api/paas/v4');
    expect(state.services[1].model).toBe('glm-4-flash');
  });
});
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
npm run test -- frontend/src/settings/stores/settings.test.ts
```

预期：FAIL，当前默认服务仍是 OpenAI / Claude，且 `protocol` 字段不存在。

- [ ] **步骤 3：补服务协议元数据**

在 `frontend/src/settings/tokens.ts` 顶部增加共享协议常量：

```ts
const OPENAI_CHAT = (defaultEndpoint: string, defaultModel: string) => ({
  id: 'openai_chat' as const,
  label: 'OpenAI Chat',
  defaultEndpoint,
  defaultModel,
  editableEndpoint: true,
  status: 'available' as const,
})

const CLAUDE_MESSAGES = {
  id: 'claude_messages' as const,
  label: 'Claude Messages',
  defaultEndpoint: 'https://api.anthropic.com',
  defaultModel: 'claude-haiku-4-5',
  editableEndpoint: true,
  status: 'available' as const,
}
```

给相关 `BUILTIN_SERVICES` 条目补 `protocols`：

```ts
{
  id: 'deepseek',
  name: 'DeepSeek',
  description: '国产高性价比模型，长上下文表现优秀。',
  builtin: true,
  defaultModel: 'deepseek-chat',
  models: ['deepseek-chat', 'deepseek-reasoner'],
  hasModelApi: true,
  iconifyId: 'simple-icons:deepseek',
  category: 'llm',
  keyRequired: true,
  protocols: [OPENAI_CHAT('https://api.deepseek.com', 'deepseek-chat')],
},
{
  id: 'zhipu',
  name: '智谱 AI',
  description: 'GLM 系列，中文写作与编程能力稳定。',
  builtin: true,
  defaultModel: 'glm-4-flash',
  models: ['glm-4-plus', 'glm-4-air', 'glm-4-flash'],
  hasModelApi: true,
  category: 'llm',
  keyRequired: true,
  protocols: [OPENAI_CHAT('https://open.bigmodel.cn/api/paas/v4', 'glm-4-flash')],
},
{
  id: 'claude',
  name: 'Claude',
  description: 'Anthropic Claude 系列，长文与写作更自然。',
  builtin: true,
  defaultModel: 'claude-haiku-4-5',
  models: ['claude-haiku-4-5', 'claude-3-5-sonnet-latest', 'claude-3-5-haiku-latest'],
  hasModelApi: true,
  iconifyId: 'simple-icons:anthropic',
  category: 'llm',
  keyRequired: true,
  protocols: [CLAUDE_MESSAGES],
},
```

其他 OpenAI 兼容服务按表补默认端点；机器翻译服务补空 `protocols: []` 和 `officialEndpoint`。

- [ ] **步骤 4：修改默认实例生成**

在 `frontend/src/settings/stores/settings.ts` 中替换默认 endpoint / instance 逻辑：

```ts
const firstAvailableProtocol = (meta?: ServiceMeta) =>
  meta?.protocols.find((p) => p.status === 'available')

const defaultInstanceFor = (type: ServiceId, name: string, enabled = false): ServiceInstance => {
  const meta = BUILTIN_SERVICES.find((s) => s.id === type)
  const protocol = firstAvailableProtocol(meta)
  return {
    id: newInstanceId(),
    type,
    name,
    enabled,
    protocol: protocol?.id ?? 'openai_chat',
    apiKey: '',
    model: protocol?.defaultModel ?? meta?.defaultModel ?? '',
    endpoint: protocol?.defaultEndpoint ?? '',
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

const seedInstances = (): ServiceInstance[] =>
  ['deepseek', 'zhipu']
    .map((id) => BUILTIN_SERVICES.find((s) => s.id === id))
    .filter((m): m is ServiceMeta => !!m)
    .map((svc) => defaultInstanceFor(svc.id, svc.name, false))
```

删除 `defaultServiceInstanceId` 的默认值、迁移分支和删除服务时的维护代码。`save()` 中调用改为：

```ts
const config = projectToAppConfig(state)
const err = validateConfig(config)
```

- [ ] **步骤 5：运行测试验证通过**

运行：

```bash
npm run test -- frontend/src/settings/stores/settings.test.ts frontend/src/lib/config.test.ts
```

预期：PASS。

- [ ] **步骤 6：Commit**

```bash
git add frontend/src/settings/tokens.ts frontend/src/settings/stores/settings.ts frontend/src/settings/stores/settings.test.ts frontend/src/lib/config.test.ts
git commit -m "feat(设置): 默认展示 DeepSeek 与智谱服务"
```

## 任务 3：服务启用校验与编辑页协议 UI

**文件：**
- 创建：`frontend/src/settings/service-validation.ts`
- 创建：`frontend/src/settings/service-validation.test.ts`
- 修改：`frontend/src/settings/panels/ServicesPanel.vue`

- [ ] **步骤 1：编写失败的测试**

创建 `frontend/src/settings/service-validation.test.ts`：

```ts
import { describe, expect, it } from 'vitest';
import { validateServiceForEnable } from './service-validation';
import type { ServiceInstance, ServiceMeta } from './types';

const meta: ServiceMeta = {
  id: 'deepseek',
  name: 'DeepSeek',
  description: '',
  builtin: true,
  category: 'llm',
  keyRequired: true,
  protocols: [{
    id: 'openai_chat',
    label: 'OpenAI Chat',
    defaultEndpoint: 'https://api.deepseek.com',
    defaultModel: 'deepseek-chat',
    editableEndpoint: true,
    status: 'available',
  }],
};

const inst = (over: Partial<ServiceInstance>): ServiceInstance => ({
  id: 'deepseek-1',
  type: 'deepseek',
  name: 'DeepSeek',
  enabled: false,
  protocol: 'openai_chat',
  apiKey: 'sk-x',
  endpoint: 'https://api.deepseek.com',
  model: 'deepseek-chat',
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

describe('validateServiceForEnable', () => {
  it('配置完整时允许开启', () => {
    expect(validateServiceForEnable(inst({}), meta)).toBeNull();
  });

  it('缺 API Key 时阻止开启', () => {
    expect(validateServiceForEnable(inst({ apiKey: '' }), meta)).toBe('请先填写 API Key');
  });

  it('endpoint 非 http(s) 时阻止开启', () => {
    expect(validateServiceForEnable(inst({ endpoint: 'ftp://x' }), meta)).toBe('Endpoint 请输入有效的 http(s) 地址');
  });

  it('model 为空时阻止开启', () => {
    expect(validateServiceForEnable(inst({ model: '' }), meta)).toBe('Model 不能为空');
  });

  it('协议不在可用列表时阻止开启', () => {
    expect(validateServiceForEnable(inst({ protocol: 'claude_messages' }), meta)).toBe('当前协议不可用');
  });
});
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
npm run test -- frontend/src/settings/service-validation.test.ts
```

预期：FAIL，模块不存在。

- [ ] **步骤 3：实现纯校验函数**

创建 `frontend/src/settings/service-validation.ts`：

```ts
import type { ServiceInstance, ServiceMeta } from './types'

const isHttpUrl = (value: string): boolean => {
  try {
    const url = new URL(value)
    return url.protocol === 'http:' || url.protocol === 'https:'
  } catch {
    return false
  }
}

export function validateServiceForEnable(
  instance: ServiceInstance,
  meta: ServiceMeta | undefined,
): string | null {
  const protocol = meta?.protocols.find((p) => p.id === instance.protocol && p.status === 'available')
  if (!protocol) return '当前协议不可用'
  if (meta?.keyRequired !== false && !instance.apiKey.trim()) return '请先填写 API Key'
  if (!isHttpUrl(instance.endpoint.trim())) return 'Endpoint 请输入有效的 http(s) 地址'
  if (!instance.model.trim()) return 'Model 不能为空'
  return null
}
```

- [ ] **步骤 4：接入 ServicesPanel**

在 `frontend/src/settings/panels/ServicesPanel.vue` 中导入：

```ts
import { validateServiceForEnable } from '../service-validation'
```

增加可用协议选项和开关 handler：

```ts
const protocolOptions = computed(() =>
  activeService.value?.protocols.map((p) => ({
    label: p.status === 'available' ? p.label : `${p.label}（不可用）`,
    value: p.id,
  })) ?? [],
)

const onToggleService = (inst: ServiceInstance, enabled: boolean): void => {
  if (!enabled) {
    inst.enabled = false
    return
  }
  const error = validateServiceForEnable(inst, serviceById(inst.type))
  if (error) {
    toast.error('无法启用服务', error)
    return
  }
  inst.enabled = true
}
```

把左侧开关改为：

```vue
<SettingSwitch
  :model-value="inst.enabled"
  :aria-label="`${inst.enabled ? '停用' : '启用'} ${inst.name}`"
  @update:model-value="(v) => onToggleService(inst, v)"
/>
```

把 endpoint 区块改为始终显示，并增加协议选择：

```vue
<SettingGroup title="协议与接入点" bare>
  <SettingRow title="服务协议" description="当前版本可调用的协议。" vertical>
    <SettingSelect
      :model-value="activeInstance.protocol"
      :options="protocolOptions"
      @update:model-value="(v) => (activeInstance!.protocol = v as typeof activeInstance.protocol)"
    />
  </SettingRow>
  <SettingRow
    title="API Endpoint"
    description="当前协议的 base URL，后端会按协议拼接请求路径。"
    vertical
  >
    <SettingInput
      v-model="activeInstance.endpoint"
      placeholder="https://api.deepseek.com"
    />
  </SettingRow>
  <SettingRow
    v-if="activeService.officialEndpoint"
    title="官方原生端点"
    description="该服务商官方翻译 API 地址，当前版本仅展示。"
    vertical
  >
    <code class="break-all rounded bg-muted px-2 py-1 text-xs text-muted-foreground">
      {{ activeService.officialEndpoint }}
    </code>
  </SettingRow>
</SettingGroup>
```

- [ ] **步骤 5：运行测试与类型检查**

运行：

```bash
npm run test -- frontend/src/settings/service-validation.test.ts
npm run typecheck
```

预期：两条命令均 PASS。

- [ ] **步骤 6：Commit**

```bash
git add frontend/src/settings/service-validation.ts frontend/src/settings/service-validation.test.ts frontend/src/settings/panels/ServicesPanel.vue
git commit -m "feat(设置): 启用服务前校验协议配置"
```

## 任务 4：Rust 新版配置模型

**文件：**
- 修改：`src-tauri/src/core/config/types.rs`
- 修改：`src-tauri/src/core/config/mod.rs`
- 修改：`src-tauri/src/lib.rs`

- [ ] **步骤 1：编写失败的测试**

在 `src-tauri/src/core/config/types.rs` 的 `tests` 模块中加入：

```rust
#[test]
fn app_config_deserializes_services_array() {
    let json = r#"{
        "targetLang": "中文",
        "services": [{
            "id": "deepseek-1",
            "serviceType": "deepseek",
            "name": "DeepSeek",
            "enabled": true,
            "protocol": "openai_chat",
            "apiKey": " sk-x ",
            "endpoint": " https://api.deepseek.com/ ",
            "model": " deepseek-chat ",
            "timeoutSeconds": 60
        }],
        "popupPrecreate": true,
        "overlayPrecreate": false,
        "collectUsage": true
    }"#;

    let config = serde_json::from_str::<AppConfig>(json)
        .expect("新版配置应可反序列化")
        .normalized();

    assert_eq!(config.target_lang, "中文");
    assert_eq!(config.services.len(), 1);
    assert_eq!(config.services[0].api_key.as_deref(), Some("sk-x"));
    assert_eq!(config.services[0].endpoint, "https://api.deepseek.com/");
    assert_eq!(config.services[0].model, "deepseek-chat");
    assert!(config.is_configured());
}

#[test]
fn app_config_is_not_configured_without_enabled_service() {
    let config = AppConfig::from_env();
    assert!(!config.is_configured());
}

#[test]
fn service_instance_config_normalized_trims_empty_values() {
    let service = ServiceInstanceConfig {
        id: " id-1 ".to_string(),
        service_type: " deepseek ".to_string(),
        name: " DeepSeek ".to_string(),
        enabled: true,
        protocol: " openai_chat ".to_string(),
        api_key: Some("   ".to_string()),
        endpoint: " https://api.deepseek.com ".to_string(),
        model: " deepseek-chat ".to_string(),
        timeout_seconds: 0,
    }.normalized();

    assert_eq!(service.id, "id-1");
    assert_eq!(service.service_type, "deepseek");
    assert_eq!(service.name, "DeepSeek");
    assert_eq!(service.protocol, "openai_chat");
    assert!(service.api_key.is_none());
    assert_eq!(service.endpoint, "https://api.deepseek.com");
    assert_eq!(service.model, "deepseek-chat");
    assert_eq!(service.timeout_seconds, 60);
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
cd src-tauri && cargo test core::config::types
```

预期：FAIL，`ServiceInstanceConfig` 或 `AppConfig.services` 不存在。

- [ ] **步骤 3：实现新版配置类型**

在 `src-tauri/src/core/config/types.rs` 中用新版结构替换旧 provider 字段：

```rust
use std::env;

use serde::{Deserialize, Serialize};

const DEFAULT_TARGET_LANG: &str = "中文";
const DEFAULT_TIMEOUT_SECONDS: u64 = 60;

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub target_lang: String,
    #[serde(default)]
    pub services: Vec<ServiceInstanceConfig>,
    #[serde(default = "default_true")]
    pub popup_precreate: bool,
    #[serde(default = "default_true")]
    pub overlay_precreate: bool,
    #[serde(default = "default_true")]
    pub collect_usage: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInstanceConfig {
    pub id: String,
    pub service_type: String,
    pub name: String,
    pub enabled: bool,
    pub protocol: String,
    pub api_key: Option<String>,
    pub endpoint: String,
    pub model: String,
    pub timeout_seconds: u64,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            target_lang: env::var("SHIZI_TARGET_LANG")
                .unwrap_or_else(|_| DEFAULT_TARGET_LANG.to_string()),
            services: Vec::new(),
            popup_precreate: true,
            overlay_precreate: true,
            collect_usage: env::var("SHIZI_COLLECT_USAGE")
                .map(|value| value.eq_ignore_ascii_case("true"))
                .unwrap_or(true),
        }
        .normalized()
    }

    pub fn normalized(mut self) -> Self {
        self.target_lang = normalize_string(self.target_lang, DEFAULT_TARGET_LANG);
        self.services = self
            .services
            .into_iter()
            .map(ServiceInstanceConfig::normalized)
            .collect();
        self
    }

    pub fn enabled_services(&self) -> impl Iterator<Item = &ServiceInstanceConfig> {
        self.services.iter().filter(|service| service.enabled)
    }

    pub fn is_configured(&self) -> bool {
        self.enabled_services().any(ServiceInstanceConfig::is_configured)
    }
}

impl ServiceInstanceConfig {
    pub fn normalized(mut self) -> Self {
        self.id = normalize_string(self.id, "service");
        self.service_type = normalize_string(self.service_type, "custom");
        self.name = normalize_string(self.name, &self.service_type);
        self.protocol = normalize_string(self.protocol, "openai_chat");
        self.api_key = self.api_key.and_then(non_empty_string);
        self.endpoint = non_empty_string(self.endpoint).unwrap_or_default();
        self.model = non_empty_string(self.model).unwrap_or_default();
        if self.timeout_seconds == 0 {
            self.timeout_seconds = DEFAULT_TIMEOUT_SECONDS;
        }
        self
    }

    pub fn is_configured(&self) -> bool {
        self.api_key.is_some() && !self.endpoint.is_empty() && !self.model.is_empty()
    }
}

fn normalize_string(value: String, default_value: &str) -> String {
    non_empty_string(value).unwrap_or_else(|| default_value.to_string())
}

fn non_empty_string(value: String) -> Option<String> {
    let value = value.trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}
```

`src-tauri/src/core/config/mod.rs` 改为：

```rust
pub mod store;
pub mod types;

pub use store::ConfigStore;
pub use types::{AppConfig, ServiceInstanceConfig};
```

`src-tauri/src/lib.rs` 只需要保留 `config.is_configured()` 调用，注释改成新版含义：

```rust
// 按是否存在已启用且配置完整的服务决定主窗口显隐
```

- [ ] **步骤 4：运行测试验证通过**

运行：

```bash
cd src-tauri && cargo test core::config::types
```

预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/core/config/types.rs src-tauri/src/core/config/mod.rs src-tauri/src/lib.rs
git commit -m "feat(配置): 后端改用服务实例配置"
```

## 任务 5：协议 adapter 工厂

**文件：**
- 创建：`src-tauri/src/core/llm/protocol.rs`
- 修改：`src-tauri/src/core/llm/mod.rs`

- [ ] **步骤 1：编写失败的测试**

创建 `src-tauri/src/core/llm/protocol.rs`，先放测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn service(protocol: &str) -> ServiceInstanceConfig {
        ServiceInstanceConfig {
            id: "svc-1".to_string(),
            service_type: "deepseek".to_string(),
            name: "DeepSeek".to_string(),
            enabled: true,
            protocol: protocol.to_string(),
            api_key: Some("sk-x".to_string()),
            endpoint: "https://api.deepseek.com".to_string(),
            model: "deepseek-chat".to_string(),
            timeout_seconds: 60,
        }
    }

    #[test]
    fn provider_for_service_accepts_openai_chat() {
        assert!(provider_for_service(&service("openai_chat")).is_ok());
    }

    #[test]
    fn provider_for_service_accepts_claude_messages() {
        let mut svc = service("claude_messages");
        svc.service_type = "claude".to_string();
        svc.endpoint = "https://api.anthropic.com".to_string();
        svc.model = "claude-haiku-4-5".to_string();
        assert!(provider_for_service(&svc).is_ok());
    }

    #[test]
    fn provider_for_service_rejects_unknown_protocol() {
        let err = provider_for_service(&service("deepl_translate"))
            .expect_err("未知协议应报错");
        assert!(err.contains("当前协议暂未接入"));
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
cd src-tauri && cargo test core::llm::protocol
```

预期：FAIL，模块未导出或 `provider_for_service` 不存在。

- [ ] **步骤 3：实现 adapter 工厂**

`src-tauri/src/core/llm/protocol.rs`：

```rust
use std::sync::Arc;

use crate::core::{
    config::ServiceInstanceConfig,
    llm::{
        ClaudeConfig, ClaudeProvider, LlmProvider, OpenAiCompatibleConfig,
        OpenAiCompatibleProvider,
    },
};

pub fn provider_for_service(
    service: &ServiceInstanceConfig,
) -> Result<Arc<dyn LlmProvider>, String> {
    match service.protocol.as_str() {
        "openai_chat" => Ok(Arc::new(OpenAiCompatibleProvider::new(OpenAiCompatibleConfig {
            api_key: service.api_key.clone(),
            base_url: service.endpoint.clone(),
            model: service.model.clone(),
            timeout_seconds: service.timeout_seconds,
        }))),
        "claude_messages" => Ok(Arc::new(ClaudeProvider::new(ClaudeConfig {
            api_key: service.api_key.clone(),
            base_url: service.endpoint.clone(),
            model: service.model.clone(),
            timeout_seconds: service.timeout_seconds,
            enable_thinking: false,
        }))),
        other => Err(format!("当前协议暂未接入：{other}")),
    }
}
```

`src-tauri/src/core/llm/mod.rs`：

```rust
pub mod claude;
pub mod mock;
pub mod openai_compatible;
pub mod protocol;
pub mod provider;

pub use claude::{ClaudeConfig, ClaudeProvider};
pub use mock::MockLlmProvider;
pub use openai_compatible::{OpenAiCompatibleConfig, OpenAiCompatibleProvider};
pub use protocol::provider_for_service;
pub use provider::{LlmError, LlmProvider, LlmStreamEvent};
```

- [ ] **步骤 4：运行测试验证通过**

运行：

```bash
cd src-tauri && cargo test core::llm::protocol
```

预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/core/llm/protocol.rs src-tauri/src/core/llm/mod.rs
git commit -m "feat(协议): 按服务实例创建 provider"
```

## 任务 6：翻译事件携带服务信息

**文件：**
- 修改：`src-tauri/src/core/translation/types.rs`
- 修改：`src-tauri/src/core/translation/service.rs`

- [ ] **步骤 1：编写失败的测试**

在 `src-tauri/src/core/translation/types.rs` 测试模块中新增：

```rust
fn service_meta() -> TranslationServiceMeta {
    TranslationServiceMeta {
        service_instance_id: "deepseek-1".to_string(),
        service_name: "DeepSeek".to_string(),
        service_type: "deepseek".to_string(),
        protocol: "openai_chat".to_string(),
    }
}

#[test]
fn delta_event_serializes_service_fields_flattened() {
    let event = TranslationEvent::Delta {
        session_id: TranslationSessionId("batch-1:deepseek-1".to_string()),
        service: service_meta(),
        text: "你好".to_string(),
    };

    let payload = serde_json::to_value(event).expect("事件应可序列化");

    assert_eq!(payload["type"], "delta");
    assert_eq!(payload["sessionId"], "batch-1:deepseek-1");
    assert_eq!(payload["serviceInstanceId"], "deepseek-1");
    assert_eq!(payload["serviceName"], "DeepSeek");
    assert_eq!(payload["serviceType"], "deepseek");
    assert_eq!(payload["protocol"], "openai_chat");
}
```

在 `src-tauri/src/core/translation/service.rs` 的 `request()` helper 中加入 `service: service_meta()`，并把事件匹配分支改成带 `..`：

```rust
TranslationEvent::Delta { .. } => "delta",
TranslationEvent::Finished { .. } => "finished",
TranslationEvent::Failed { .. } => "failed",
TranslationEvent::Cancelled { .. } => "cancelled",
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
cd src-tauri && cargo test core::translation
```

预期：FAIL，`TranslationServiceMeta` 或事件字段不存在。

- [ ] **步骤 3：实现事件结构**

在 `src-tauri/src/core/translation/types.rs` 添加：

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationServiceMeta {
    pub service_instance_id: String,
    pub service_name: String,
    pub service_type: String,
    pub protocol: String,
}
```

`TranslationRequest` 改为：

```rust
pub struct TranslationRequest {
    pub session_id: TranslationSessionId,
    pub input: TranslationInput,
    pub target_lang: String,
    pub service: TranslationServiceMeta,
}
```

`TranslationEvent` 改为每个变体 flatten 服务字段：

```rust
pub enum TranslationEvent {
    Started {
        session_id: TranslationSessionId,
        #[serde(flatten)]
        service: TranslationServiceMeta,
        source_text: String,
        source_type: String,
    },
    Delta {
        session_id: TranslationSessionId,
        #[serde(flatten)]
        service: TranslationServiceMeta,
        text: String,
    },
    Finished {
        session_id: TranslationSessionId,
        #[serde(flatten)]
        service: TranslationServiceMeta,
        full_text: String,
        usage: Option<TokenUsage>,
    },
    Failed {
        session_id: TranslationSessionId,
        #[serde(flatten)]
        service: TranslationServiceMeta,
        message: String,
        retryable: bool,
    },
    Cancelled {
        session_id: TranslationSessionId,
        #[serde(flatten)]
        service: TranslationServiceMeta,
    },
}
```

`src-tauri/src/core/translation/service.rs` 中事件创建改为：

```rust
emit(TranslationEvent::Delta {
    session_id: delta_session_id.clone(),
    service: request.service.clone(),
    text,
});
```

`Finished` 和 `Cancelled` 同样使用 `request.service.clone()` 或移动 `request.service`。

- [ ] **步骤 4：运行测试验证通过**

运行：

```bash
cd src-tauri && cargo test core::translation
```

预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/core/translation/types.rs src-tauri/src/core/translation/service.rs
git commit -m "feat(翻译): 事件携带服务实例信息"
```

## 任务 7：批次请求生成与后端并发编排

**文件：**
- 创建：`src-tauri/src/core/translation/batch.rs`
- 修改：`src-tauri/src/core/translation/mod.rs`
- 修改：`src-tauri/src/ui/web_popup.rs`

- [ ] **步骤 1：编写失败的测试**

创建 `src-tauri/src/core/translation/batch.rs`，先写测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::ServiceInstanceConfig;

    fn service(id: &str, enabled: bool) -> ServiceInstanceConfig {
        ServiceInstanceConfig {
            id: id.to_string(),
            service_type: "deepseek".to_string(),
            name: format!("svc-{id}"),
            enabled,
            protocol: "openai_chat".to_string(),
            api_key: Some("sk-x".to_string()),
            endpoint: "https://api.deepseek.com".to_string(),
            model: "deepseek-chat".to_string(),
            timeout_seconds: 60,
        }
    }

    #[test]
    fn build_batch_requests_keeps_enabled_service_order() {
        let input = TranslationInput::ManualText("hello".to_string());
        let requests = build_batch_requests(
            input,
            "中文".to_string(),
            &[service("a", true), service("b", false), service("c", true)],
            "batch-1",
        ).expect("应生成批次");

        assert_eq!(
            requests.iter().map(|r| r.session_id.0.as_str()).collect::<Vec<_>>(),
            vec!["batch-1:a", "batch-1:c"],
        );
        assert_eq!(
            requests.iter().map(|r| r.service.service_instance_id.as_str()).collect::<Vec<_>>(),
            vec!["a", "c"],
        );
    }

    #[test]
    fn build_batch_requests_rejects_empty_enabled_services() {
        let err = build_batch_requests(
            TranslationInput::ManualText("hello".to_string()),
            "中文".to_string(),
            &[service("a", false)],
            "batch-1",
        ).expect_err("无启用服务应报错");

        assert_eq!(err, "请先在服务列表启用至少一个已配置服务");
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
cd src-tauri && cargo test core::translation::batch
```

预期：FAIL，模块或函数不存在。

- [ ] **步骤 3：实现批次 helper**

`src-tauri/src/core/translation/batch.rs`：

```rust
use crate::core::{
    config::ServiceInstanceConfig,
    translation::{
        TranslationInput, TranslationRequest, TranslationServiceMeta, TranslationSessionId,
    },
};

pub fn build_batch_requests(
    input: TranslationInput,
    target_lang: String,
    services: &[ServiceInstanceConfig],
    batch_id: &str,
) -> Result<Vec<TranslationRequest>, String> {
    let requests = services
        .iter()
        .filter(|service| service.enabled)
        .map(|service| TranslationRequest {
            session_id: TranslationSessionId(format!("{batch_id}:{}", service.id)),
            input: input.clone(),
            target_lang: target_lang.clone(),
            service: TranslationServiceMeta {
                service_instance_id: service.id.clone(),
                service_name: service.name.clone(),
                service_type: service.service_type.clone(),
                protocol: service.protocol.clone(),
            },
        })
        .collect::<Vec<_>>();

    if requests.is_empty() {
        Err("请先在服务列表启用至少一个已配置服务".to_string())
    } else {
        Ok(requests)
    }
}
```

`src-tauri/src/core/translation/mod.rs`：

```rust
pub mod batch;
pub mod service;
pub mod types;

pub use batch::build_batch_requests;
pub use service::TranslationService;
pub use types::{
    TokenUsage, TranslationEvent, TranslationInput, TranslationRequest, TranslationServiceMeta,
    TranslationSessionId,
};
```

- [ ] **步骤 4：改造 `start_translation_from_input`**

在 `src-tauri/src/ui/web_popup.rs` 中删除单 provider 构造，导入：

```rust
use futures_util::future::join_all;

use crate::core::{
    llm::provider_for_service,
    translation::{build_batch_requests, TranslationEvent, TranslationInput, TranslationService},
};
```

核心流程改为：

```rust
let config = state.config_store.get().map_err(|error| error.to_string())?;
let batch_id = create_session_id()?;
let requests = build_batch_requests(
    input.clone(),
    config.target_lang.clone(),
    &config.services,
    &batch_id,
)?;
let services = config
    .services
    .iter()
    .filter(|service| service.enabled)
    .cloned()
    .collect::<Vec<_>>();

state.try_begin_translation()?;
let cancel_token = CancellationToken::new();
state.set_current_cancel_token(cancel_token.clone())?;
state.set_last_translation_input(input.clone())?;
cache_automatic_source_text_for_popup(&input, input.text(), state)?;

for request in &requests {
    emit_translation_event(
        &app,
        TranslationEvent::Started {
            session_id: request.session_id.clone(),
            service: request.service.clone(),
            source_text: request.source_text().to_string(),
            source_type: request.input.kind().to_string(),
        },
    ).map_err(|error| error.to_string())?;
}

let app_handle = app.clone();
let state_for_task = state.clone();
let collect_usage = config.collect_usage;

tauri::async_runtime::spawn(async move {
    let jobs = requests.into_iter().zip(services).map(|(request, service_config)| {
        let app_handle = app_handle.clone();
        let cancel = cancel_token.clone();
        async move {
            let failed_session_id = request.session_id.clone();
            let failed_service = request.service.clone();
            match provider_for_service(&service_config) {
                Ok(provider) => {
                    let translation_service = TranslationService::new(provider);
                    let result = translation_service
                        .translate_with(request, collect_usage, cancel, |event| {
                            let _ = emit_translation_event(&app_handle, event);
                        })
                        .await;
                    if let Err(error) = result {
                        let _ = emit_translation_event(
                            &app_handle,
                            TranslationEvent::Failed {
                                session_id: failed_session_id,
                                service: failed_service,
                                message: error.to_string(),
                                retryable: error.retryable(),
                            },
                        );
                    }
                }
                Err(message) => {
                    let _ = emit_translation_event(
                        &app_handle,
                        TranslationEvent::Failed {
                            session_id: failed_session_id,
                            service: failed_service,
                            message,
                            retryable: false,
                        },
                    );
                }
            }
        }
    });
    join_all(jobs).await;
    let _ = state_for_task.clear_current_cancel_token();
    let _ = state_for_task.finish_translation();
});

Ok(batch_id)
```

若 `emit Started` 中途失败，保留现有清理模式：`clear_current_cancel_token()` 和 `finish_translation()` 后返回错误。

- [ ] **步骤 5：运行后端测试**

运行：

```bash
cd src-tauri && cargo test core::translation::batch
cd src-tauri && cargo test ui::web_popup
```

预期：两条命令均 PASS。

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/core/translation/batch.rs src-tauri/src/core/translation/mod.rs src-tauri/src/ui/web_popup.rs
git commit -m "feat(翻译): 按启用服务创建翻译批次"
```

## 任务 8：翻译弹窗多结果卡

**文件：**
- 修改：`frontend/public/translate.html`
- 修改：`frontend/public/translate.js`
- 修改：`frontend/public/translate.css`

- [ ] **步骤 1：替换 HTML 结果容器**

在 `frontend/public/translate.html` 中把原单卡片内容替换为：

```html
<div class="results" id="resultsList"></div>
```

保留原 `.source-card`、语言栏、状态栏和 toast。

- [ ] **步骤 2：实现卡片创建 helper**

在 `frontend/public/translate.js` 中删除单卡片 DOM 常量，新增状态：

```js
const resultsList = document.getElementById('resultsList');

let isTranslating = false;
let currentBatchId = null;
let pinned = false;
const resultCards = new Map();
```

新增批次和卡片 helper：

```js
function batchIdFromSession(sessionId) {
  return typeof sessionId === 'string' ? sessionId.split(':')[0] : null;
}

function getCard(payload) {
  const id = payload.serviceInstanceId;
  if (resultCards.has(id)) return resultCards.get(id);

  const card = document.createElement('div');
  card.className = 'result-card';
  card.dataset.serviceInstanceId = id;
  card.innerHTML = `
    <div class="result-card-header">
      <svg class="result-engine-icon" viewBox="0 0 20 20">
        <rect width="20" height="20" rx="5" fill="#94918A"></rect>
        <text x="10" y="14.5" text-anchor="middle" font-size="11" font-weight="700" fill="#fff" font-family="Segoe UI, system-ui, sans-serif"></text>
      </svg>
      <span class="result-engine-name"></span>
      <button class="result-collapse-btn" title="折叠">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>
      </button>
    </div>
    <div class="result-card-body">
      <div class="result-card-body-inner">
        <div class="result-text"></div>
        <div class="result-actions" style="visibility:hidden">
          <button class="result-action-btn speak" title="朗读翻译"></button>
          <button class="result-action-btn copy" title="复制翻译"></button>
          <span class="result-tokens" title="输入 / 输出 Token" style="display:none">
            <span class="tok tok-input"><span class="tok-value">0</span></span>
            <span class="tok-sep"></span>
            <span class="tok tok-output"><span class="tok-value">0</span></span>
          </span>
        </div>
      </div>
    </div>`;

  card.querySelector('.result-engine-icon text').textContent =
    (payload.serviceName || payload.serviceType || 'T').slice(0, 1).toUpperCase();
  card.querySelector('.result-engine-name').textContent = payload.serviceName || '翻译';
  card.querySelector('.result-card-header').addEventListener('click', (event) => {
    if (event.target.closest('.result-collapse-btn')) return;
    card.classList.toggle('collapsed');
    adjustHeight();
  });
  card.querySelector('.result-collapse-btn').addEventListener('click', (event) => {
    event.stopPropagation();
    card.classList.toggle('collapsed');
    adjustHeight();
  });
  card.querySelector('.copy').addEventListener('click', () =>
    copyText(card.querySelector('.result-text').textContent, card.querySelector('.copy')));
  card.querySelector('.speak').addEventListener('click', () =>
    speakText(card.querySelector('.result-text').textContent, 'zh-CN'));

  resultsList.appendChild(card);
  const refs = {
    el: card,
    text: card.querySelector('.result-text'),
    actions: card.querySelector('.result-actions'),
    tokens: card.querySelector('.result-tokens'),
    inputTokens: card.querySelector('.tok-input .tok-value'),
    outputTokens: card.querySelector('.tok-output .tok-value'),
    status: 'loading',
  };
  resultCards.set(id, refs);
  return refs;
}
```

把按钮 SVG 从旧 HTML 复制进 `.speak` / `.copy` 按钮，避免空按钮。

- [ ] **步骤 3：改造事件渲染**

`renderTranslationEvent` 按服务卡更新：

```js
function shouldHandleBatchEvent(payload) {
  const batchId = batchIdFromSession(getSessionId(payload));
  return !currentBatchId || !batchId || batchId === currentBatchId;
}

function updateBatchStatus() {
  const cards = Array.from(resultCards.values());
  const loading = cards.some((card) => card.status === 'loading');
  const failed = cards.filter((card) => card.status === 'failed').length;
  const finished = cards.filter((card) => card.status === 'finished').length;
  isTranslating = loading;

  if (loading) {
    setStatus({ text: '翻译中…', loading: true, action: { label: '取消', onClick: cancelTranslation } });
  } else if (failed > 0 && finished > 0) {
    currentBatchId = null;
    setStatus({ text: '部分完成', loading: false, action: { label: '重试', onClick: retryTranslation } });
  } else if (failed > 0) {
    currentBatchId = null;
    setStatus({ text: '翻译失败', loading: false, action: { label: '重试', onClick: retryTranslation } });
  } else {
    currentBatchId = null;
    setStatus({ text: '翻译完成', loading: false, action: { label: '重试', onClick: retryTranslation } });
  }
}
```

`started` 分支：

```js
case 'started': {
  const batchId = batchIdFromSession(getSessionId(payload));
  if (!currentBatchId || currentBatchId !== batchId) {
    currentBatchId = batchId;
    resultCards.clear();
    resultsList.textContent = '';
  }
  sourceText.value = payload.sourceText ?? sourceText.value;
  autoResize();
  updateCharCount();
  setSourceBadge(payload.sourceType);
  const card = getCard(payload);
  card.text.textContent = '';
  card.text.style.color = '';
  card.actions.style.visibility = 'hidden';
  card.tokens.style.display = 'none';
  card.status = 'loading';
  setStreamCursor(card.text, true);
  updateBatchStatus();
  break;
}
```

`delta/finished/failed/cancelled` 分支都先 `if (!shouldHandleBatchEvent(payload)) return;`，再 `const card = getCard(payload)`，只更新该服务卡。

- [ ] **步骤 4：更新 cursor helper 和复制朗读**

把 `setStreamCursor` 改为接收目标文本容器：

```js
function setStreamCursor(container, visible) {
  const existing = container.querySelector('.stream-cursor');
  if (existing) existing.remove();
  if (visible) {
    const cursor = document.createElement('span');
    cursor.className = 'stream-cursor';
    container.appendChild(cursor);
  }
}
```

删除旧 `resultHeader/resultText/resultActions` 事件绑定；保留源文朗读、复制、语言按钮和状态栏按钮绑定。

- [ ] **步骤 5：补 CSS 状态**

在 `frontend/public/translate.css` 中增加：

```css
.result-card.failed {
  border-color: color-mix(in srgb, var(--danger) 35%, var(--border));
}
.result-card.failed .result-text {
  color: var(--danger);
}
.result-card.cancelled .result-text {
  color: var(--fg-3);
}
.results:empty::before {
  content: "";
  display: none;
}
```

- [ ] **步骤 6：构建验证**

运行：

```bash
npm run build
```

预期：PASS，Vite 构建不报错。

- [ ] **步骤 7：Commit**

```bash
git add frontend/public/translate.html frontend/public/translate.js frontend/public/translate.css
git commit -m "feat(弹窗): 渲染多服务翻译结果"
```

## 任务 9：全量验证与文档同步

**文件：**
- 修改：`README.md`
- 修改：`docs/roadmap/progressive-development-plan.md`
- 修改：`AGENTS.md`
- 修改：`CLAUDE.md`

- [ ] **步骤 1：同步 README**

在 `README.md` 的功能说明中写入当前能力：

```md
- 服务列表默认展示 DeepSeek 与智谱 AI，默认关闭；启用后按列表顺序参与翻译。
- 服务实例通过 `protocol` 选择调用协议；当前可用协议为 OpenAI Chat 与 Claude Messages。
- 翻译弹窗按启用服务渲染多个结果卡，单个服务失败不影响其他服务。
```

- [ ] **步骤 2：同步 roadmap**

在 `docs/roadmap/progressive-development-plan.md` 对应阶段勾选或补充：

```md
- [x] 服务实例按启用状态和列表顺序驱动翻译批次
- [x] 翻译弹窗支持多服务结果卡
- [x] 服务协议抽象接入 OpenAI Chat 与 Claude Messages
```

- [ ] **步骤 3：同步 AGENTS 与 CLAUDE**

在 `AGENTS.md` 与 `CLAUDE.md` 的项目结构和架构关键点中同步：

```md
- **服务协议配置**：后端配置以 `services[]` 为事实来源，每个服务实例包含 `protocol`、`endpoint`、`model`、`apiKey` 与启用状态；旧单 provider 配置不再作为运行路径。
- **批次翻译**：翻译入口过滤启用服务并保持列表顺序，为每个服务创建 `{batch_id}:{service_id}` session，事件携带 `serviceInstanceId/serviceName/serviceType/protocol`。
- **翻译弹窗**：弹窗按服务实例渲染多个结果卡，单服务失败只更新对应卡片。
```

确认两个文件对应段落内容一致。

- [ ] **步骤 4：全量验证**

运行：

```bash
npm run test
npm run typecheck
npm run build
cd src-tauri && cargo test
cd src-tauri && cargo build
```

预期：全部 exit 0。

- [ ] **步骤 5：手动验证**

运行桌面应用：

```bash
npm run tauri dev
```

手动检查：

1. 首次打开设置页只显示 DeepSeek / 智谱 AI，开关关闭。
2. DeepSeek 缺 Key 时点击开关不会开启，并提示“请先填写 API Key”。
3. 填写 DeepSeek Key、endpoint、model 后开启，翻译弹窗出现一个 DeepSeek 卡片。
4. 再开启智谱 AI，翻译弹窗出现 DeepSeek、智谱 AI 两张卡，顺序和设置页一致。
5. 拖拽调整服务顺序后再次翻译，结果卡顺序随服务列表变化。
6. 一个服务 Key 错误时，只有对应卡片失败，另一个服务继续输出。
7. 点击取消后，本批次所有仍在 loading 的卡片进入取消或停止状态。

- [ ] **步骤 6：Commit**

```bash
git add README.md docs/roadmap/progressive-development-plan.md AGENTS.md CLAUDE.md
git commit -m "docs(架构): 同步服务协议批次翻译说明"
```

## 自检

- 规格覆盖：默认 DeepSeek / 智谱 AI、服务协议、endpoint 可见、启用校验、后端批次、多结果弹窗、文档同步均有任务覆盖。
- 最新用户指令覆盖：不保留旧单 provider 兼容路径；计划中删除旧投影和迁移逻辑。
- Ponytail 取舍：不新增依赖；不做服务级重试、fallback、竞速、质量评分；只建一个 provider 工厂和一个批次 helper。
- 验证链路：每个代码任务都有测试或构建命令；收尾有全量验证和手动验证。
