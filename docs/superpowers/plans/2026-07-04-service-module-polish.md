# 服务模块打磨：前后端配置同步与渠道开发中标记 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [x]`）语法来跟踪进度。

**目标：** 修复“启用多个渠道但翻译弹窗只显示一个卡片”的 bug，并为未对接渠道标记“开发中”状态，使前后端配置以 `config.json` 为事实来源保持一致。

**架构：** 后端 `provider_for_service` 用纯函数 `protocol_to_kind` 把协议 id（`openai_chat`/`claude_messages`/`mock`）映射到 provider 类型，未知协议报错；前端设置页挂载时调 `syncFromBackend` 拉取后端 `config.json`，后端空则推前端覆盖、后端非空则按 id 合并（后端核心字段覆盖、前端独有字段保留）；未对接渠道（`protocols.length === 0`）在添加 Dialog / 服务列表 / 详情页三处标 amber“开发中”并置灰启用开关；翻译弹窗 `ENGINE_META` 改按 `serviceType`（渠道 id）匹配彩色字母图标。

**技术栈：** Rust、serde、tokio、Tauri 2、Vue 3、Vitest、vue-tsc、纯静态 JS（translate.js）。

---

## 文件结构

- 修改：`src-tauri/src/core/llm/protocol.rs`
  新增 `ProviderKind` 枚举与 `protocol_to_kind` 纯函数；`provider_for_service` 改用 `openai_chat`/`claude_messages`/`mock` 匹配，未知协议返回 `Err`；内联单测覆盖四个分支。
- 修改：`src-tauri/src/core/config/types.rs`
  `DEFAULT_PROTOCOL` 改 `openai_chat`；`from_env` 默认 `service_type` 改 `openai`；`from_env` 与 `normalized` 里的 `"claude"` 字面量改 `"claude_messages"`；更新现有测试字面量并补 `from_env` 默认值断言。
- 修改：`frontend/src/settings/stores/settings.ts`
  新增导出纯函数 `mergeBackendIntoServices`（按 id 合并后端 services 到本地）与私有 `backendInstanceToLocal`；`useSettings()` 新增 `syncFromBackend()` 方法；import 补 `invokeGetAppConfig` 与类型；给 `defaultInstanceFor` 的 fallback 加注释。
- 修改：`frontend/src/settings/stores/settings.test.ts`
  扩展 `@/lib/tauri` mock 补 `invokeGetAppConfig`；新增 `mergeBackendIntoServices` 纯函数测试与 `syncFromBackend` 集成测试（后端空推覆盖、非空合并、降级）。
- 修改：`frontend/src/settings/SettingsPage.vue`
  `onMounted` 调 `settings.syncFromBackend()`。
- 修改：`frontend/src/settings/panels/ServicesPanel.vue`
  新增 `isDeveloping` helper；添加 Dialog 卡片加 amber“开发中”badge；服务列表启用开关 `disabled` + tooltip；详情页顶部 amber 横幅。
- 修改：`frontend/public/translate.js`
  `ENGINE_META` 改按渠道 id（`openai`/`deepseek`/`zhipu`/`claude`/`mock`）存颜色+字母；新增 `engineIcon` 函数，未匹配 fallback 取 `serviceName` 首字。
- 修改：`README.md`
  v0.2 段落补配置同步机制与开发中标记；环境变量 `SHIZI_LLM_PROVIDER` 值更新为 `openai_chat | claude_messages | mock`。
- 修改：`docs/roadmap/progressive-development-plan.md`
  当前完成状态补“服务模块打磨”条目。
- 修改：`AGENTS.md` 与 `CLAUDE.md`
  架构关键点补“前后端配置同步”“协议 id 统一”“开发中标记”说明，两文件保持一致。

---

## 任务 1：后端协议 id 对齐（protocol.rs）

**文件：**
- 修改：`src-tauri/src/core/llm/protocol.rs`

- [x] **步骤 1：编写失败的测试**

在 `src-tauri/src/core/llm/protocol.rs` 末尾追加内联测试模块。`protocol_to_kind` 与 `ProviderKind` 尚未定义，此时编译失败即为“测试失败”。

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn svc(protocol: &str) -> ServiceInstanceConfig {
        ServiceInstanceConfig {
            id: "test".to_string(),
            service_type: "openai".to_string(),
            name: "测试".to_string(),
            enabled: true,
            protocol: protocol.to_string(),
            api_key: Some("sk-test".to_string()),
            endpoint: "https://api.example.com".to_string(),
            model: "gpt-4o-mini".to_string(),
            timeout_seconds: 60,
        }
    }

    #[test]
    fn protocol_to_kind_openai_chat() {
        assert!(matches!(
            protocol_to_kind("openai_chat"),
            Ok(ProviderKind::OpenAiCompatible)
        ));
    }

    #[test]
    fn protocol_to_kind_claude_messages() {
        assert!(matches!(
            protocol_to_kind("claude_messages"),
            Ok(ProviderKind::Claude)
        ));
    }

    #[test]
    fn protocol_to_kind_mock() {
        assert!(matches!(
            protocol_to_kind("mock"),
            Ok(ProviderKind::Mock)
        ));
    }

    #[test]
    fn protocol_to_kind_unknown_returns_err() {
        // 旧字面量 openai-compatible 不再被识别，倒推前端覆盖后 config.json 不会有此值
        let err = protocol_to_kind("openai-compatible").unwrap_err();
        assert!(err.contains("openai-compatible"), "错误信息应包含协议名: {err}");
    }

    #[test]
    fn provider_for_service_claude_messages_ok() {
        // 修复前 claude_messages 走 default 返回 OpenAiCompatible，类型映射由 protocol_to_kind 把关
        let config = svc("claude_messages");
        assert!(provider_for_service(&config).is_ok());
    }

    #[test]
    fn provider_for_service_unknown_returns_err() {
        let config = svc("openai-compatible");
        assert!(provider_for_service(&config).is_err());
    }
}
```

- [x] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib protocol_to_kind`
预期：编译失败，报错 `cannot find function 'protocol_to_kind'` / `cannot find type 'ProviderKind'`。

- [x] **步骤 3：编写最少实现代码**

把 `src-tauri/src/core/llm/protocol.rs` 的全部内容替换为：

```rust
use std::sync::Arc;

use crate::core::{
    config::ServiceInstanceConfig,
    llm::{
        ClaudeConfig, ClaudeProvider, LlmProvider, MockLlmProvider, OpenAiCompatibleConfig,
        OpenAiCompatibleProvider,
    },
};

/// 协议 id 映射到的 provider 类型，供 `provider_for_service` 分发与单测断言。
pub enum ProviderKind {
    OpenAiCompatible,
    Claude,
    Mock,
}

/// 把协议 id 字符串映射到 `ProviderKind`。
///
/// 与前端 `frontend/src/types/config.ts` 的 `ServiceProtocolId` 保持一致：
/// - `"openai_chat"` → `OpenAiCompatible`
/// - `"claude_messages"` → `Claude`
/// - `"mock"` → `Mock`
/// - 其他 → 返回错误，不再静默走 OpenAI 兼容（修复 Claude 渠道被误当 OpenAI 的 bug）。
pub fn protocol_to_kind(protocol: &str) -> Result<ProviderKind, String> {
    match protocol {
        "openai_chat" => Ok(ProviderKind::OpenAiCompatible),
        "claude_messages" => Ok(ProviderKind::Claude),
        "mock" => Ok(ProviderKind::Mock),
        other => Err(format!("未支持的协议：{other}")),
    }
}

/// 根据 `ServiceInstanceConfig` 的 `protocol` 字段创建对应的 LLM provider。
///
/// 协议 id 由 [`protocol_to_kind`] 解析，未知协议返回 `Err`，避免静默误匹配。
pub fn provider_for_service(
    config: &ServiceInstanceConfig,
) -> Result<Arc<dyn LlmProvider>, String> {
    match protocol_to_kind(&config.protocol)? {
        ProviderKind::Mock => Ok(Arc::new(MockLlmProvider)),
        ProviderKind::Claude => Ok(Arc::new(ClaudeProvider::new(ClaudeConfig {
            api_key: config.api_key.clone(),
            base_url: config.endpoint.clone(),
            model: config.model.clone(),
            timeout_seconds: config.timeout_seconds as u64,
            enable_thinking: false, // ponytail: 默认关闭，用户可在配置扩展时打开
        }))),
        ProviderKind::OpenAiCompatible => Ok(Arc::new(OpenAiCompatibleProvider::new(
            OpenAiCompatibleConfig {
                api_key: config.api_key.clone(),
                base_url: config.endpoint.clone(),
                model: config.model.clone(),
                timeout_seconds: config.timeout_seconds as u64,
            },
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svc(protocol: &str) -> ServiceInstanceConfig {
        ServiceInstanceConfig {
            id: "test".to_string(),
            service_type: "openai".to_string(),
            name: "测试".to_string(),
            enabled: true,
            protocol: protocol.to_string(),
            api_key: Some("sk-test".to_string()),
            endpoint: "https://api.example.com".to_string(),
            model: "gpt-4o-mini".to_string(),
            timeout_seconds: 60,
        }
    }

    #[test]
    fn protocol_to_kind_openai_chat() {
        assert!(matches!(
            protocol_to_kind("openai_chat"),
            Ok(ProviderKind::OpenAiCompatible)
        ));
    }

    #[test]
    fn protocol_to_kind_claude_messages() {
        assert!(matches!(
            protocol_to_kind("claude_messages"),
            Ok(ProviderKind::Claude)
        ));
    }

    #[test]
    fn protocol_to_kind_mock() {
        assert!(matches!(
            protocol_to_kind("mock"),
            Ok(ProviderKind::Mock)
        ));
    }

    #[test]
    fn protocol_to_kind_unknown_returns_err() {
        let err = protocol_to_kind("openai-compatible").unwrap_err();
        assert!(err.contains("openai-compatible"), "错误信息应包含协议名: {err}");
    }

    #[test]
    fn provider_for_service_claude_messages_ok() {
        let config = svc("claude_messages");
        assert!(provider_for_service(&config).is_ok());
    }

    #[test]
    fn provider_for_service_unknown_returns_err() {
        let config = svc("openai-compatible");
        assert!(provider_for_service(&config).is_err());
    }
}
```

- [x] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib protocol_to_kind`
预期：4 个 `protocol_to_kind_*` + 2 个 `provider_for_service_*` 全部 PASS。

- [x] **步骤 5：运行后端全量构建**

运行：`cd src-tauri && cargo build`
预期：编译成功，无 warning。

- [x] **步骤 6：Commit**

```bash
git add src-tauri/src/core/llm/protocol.rs
git commit -m "fix(llm): provider_for_service 协议 id 对齐前端，未知协议报错"
```

---

## 任务 2：后端 config types 协议 id 对齐（types.rs）

**文件：**
- 修改：`src-tauri/src/core/config/types.rs`

- [x] **步骤 1：编写失败的测试**

在 `src-tauri/src/core/config/types.rs` 的 `#[cfg(test)] mod tests` 块内追加两个测试，并更新现有测试里的 `"openai-compatible"` 字面量为 `"openai_chat"`、`"claude"` 为 `"claude_messages"`（仅出现在 protocol 字段的字面量）。

追加的新测试：

```rust
    #[test]
    fn from_env_default_protocol_is_openai_chat() {
        let config = AppConfig::from_env();
        assert_eq!(config.services[0].protocol, "openai_chat");
        assert_eq!(config.services[0].service_type, "openai");
    }

    #[test]
    fn normalized_claude_messages_uses_claude_base_url() {
        let svc = ServiceInstanceConfig {
            id: "test".to_string(),
            service_type: "claude".to_string(),
            name: "Claude".to_string(),
            enabled: true,
            protocol: "claude_messages".to_string(),
            api_key: None,
            endpoint: "".to_string(),
            model: "".to_string(),
            timeout_seconds: 0,
        }
        .normalized();
        assert_eq!(svc.endpoint, DEFAULT_CLAUDE_BASE_URL);
    }
```

现有测试字面量替换（逐处确认是 `protocol:` 字段赋值才改）：

- `normalized_fills_empty_service_fields`：`protocol: "openai-compatible"` → `protocol: "openai_chat"`。
- `is_configured_true_with_second_service`：`protocol: "openai-compatible"` → `protocol: "openai_chat"`。
- `is_configured_false_when_only_disabled_service_has_key`：`protocol: "openai-compatible"` → `protocol: "openai_chat"`。
- `deserializes_services_array`：`"protocol": "openai-compatible"` → `"protocol": "openai_chat"`。
- `service_instance_config_serializes_camel_case`：`protocol: "openai-compatible"` → `protocol: "openai_chat"`。

- [x] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib from_env_default_protocol_is_openai_chat`
预期：FAIL，`from_env_default_protocol_is_openai_chat` 断言失败（当前默认 protocol 仍是 `openai-compatible`），`normalized_claude_messages_uses_claude_base_url` 也 FAIL（当前 `"claude_messages"` 不匹配 `"claude"` 分支，走 default 用 `DEFAULT_BASE_URL`）。

- [x] **步骤 3：编写最少实现代码**

修改 `src-tauri/src/core/config/types.rs` 的三处：

1. 第 12 行 `DEFAULT_PROTOCOL` 常量：

```rust
const DEFAULT_PROTOCOL: &str = "openai_chat";
```

2. `impl ServiceInstanceConfig` 的 `normalized` 方法，endpoint 默认分支（约第 54 行）：

```rust
        if self.endpoint.trim().is_empty() {
            self.endpoint = match self.protocol.as_str() {
                "claude_messages" => DEFAULT_CLAUDE_BASE_URL.to_string(),
                _ => DEFAULT_BASE_URL.to_string(),
            };
        }
```

3. `impl AppConfig` 的 `from_env` 方法。把两处 `match protocol.as_str()` 里的 `"claude"` 改为 `"claude_messages"`，并把默认服务的 `service_type` 从 `"llm"` 改为 `"openai"`。改后片段：

```rust
        let (api_key, endpoint, model) = match protocol.as_str() {
            "claude_messages" => (
                env::var("SHIZI_CLAUDE_API_KEY").ok(),
                env::var("SHIZI_CLAUDE_BASE_URL")
                    .unwrap_or_else(|_| DEFAULT_CLAUDE_BASE_URL.to_string()),
                env::var("SHIZI_CLAUDE_MODEL")
                    .unwrap_or_else(|_| DEFAULT_CLAUDE_MODEL.to_string()),
            ),
            _ => (
                env::var("SHIZI_OPENAI_API_KEY").ok(),
                env::var("SHIZI_OPENAI_BASE_URL")
                    .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string()),
                env::var("SHIZI_OPENAI_MODEL")
                    .unwrap_or_else(|_| DEFAULT_MODEL.to_string()),
            ),
        };

        let name = match protocol.as_str() {
            "claude_messages" => "默认 Claude 服务".to_string(),
            "mock" => "Mock 服务".to_string(),
            _ => "默认服务".to_string(),
        };
```

以及默认 service 构造：

```rust
            services: vec![ServiceInstanceConfig {
                id: "default".to_string(),
                service_type: "openai".to_string(),
                name,
                enabled: true,
                protocol,
                api_key,
                endpoint,
                model,
                timeout_seconds: DEFAULT_TIMEOUT_SECONDS,
            }],
```

- [x] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test`
预期：全部 PASS（含新测试与更新字面量后的旧测试）。

- [x] **步骤 5：运行后端全量构建**

运行：`cd src-tauri && cargo build`
预期：编译成功。

- [x] **步骤 6：Commit**

```bash
git add src-tauri/src/core/config/types.rs
git commit -m "fix(config): from_env/normalized 协议 id 对齐为 openai_chat/claude_messages"
```

---

## 任务 3：前端 mergeBackendIntoServices 纯函数 + 测试

**文件：**
- 修改：`frontend/src/settings/stores/settings.ts`
- 修改：`frontend/src/settings/stores/settings.test.ts`

- [x] **步骤 1：编写失败的测试**

在 `frontend/src/settings/stores/settings.test.ts` 顶部 import 区追加（合并到已有的 `./settings` import 行）：

```ts
import { mergeBackendIntoServices, useSettings } from './settings';
import { DEFAULT_PROMPTS } from '../tokens';
import type { ServiceInstanceConfig } from '@/types/config';
import type { ServiceInstance } from '../types';
```

在文件末尾追加测试块：

```ts
const makeLocal = (over: Partial<ServiceInstance>): ServiceInstance => ({
  id: 'local-1',
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

const makeBackend = (over: Partial<ServiceInstanceConfig>): ServiceInstanceConfig => ({
  id: 'b-1',
  serviceType: 'deepseek',
  name: 'DeepSeek',
  enabled: true,
  protocol: 'openai_chat',
  apiKey: 'sk-x',
  endpoint: 'https://api.deepseek.com',
  model: 'deepseek-chat',
  timeoutSeconds: 60,
  ...over,
});

describe('mergeBackendIntoServices', () => {
  it('后端核心字段覆盖前端同 id 实例，前端独有字段保留', () => {
    const local = [
      makeLocal({
        id: 'a',
        apiKey: 'old-key',
        enabled: false,
        endpoint: 'https://old',
        model: 'old-model',
        systemPrompt: '我的提示词',
        chainOfThought: 'long',
        note: '我的备注',
      }),
    ];
    const backend = [
      makeBackend({
        id: 'a',
        apiKey: 'new-key',
        enabled: true,
        endpoint: 'https://new',
        model: 'new-model',
        protocol: 'openai_chat',
      }),
    ];

    const result = mergeBackendIntoServices(local, backend);

    expect(result).toHaveLength(1);
    expect(result[0].apiKey).toBe('new-key');
    expect(result[0].enabled).toBe(true);
    expect(result[0].endpoint).toBe('https://new');
    expect(result[0].model).toBe('new-model');
    expect(result[0].systemPrompt).toBe('我的提示词');
    expect(result[0].chainOfThought).toBe('long');
    expect(result[0].note).toBe('我的备注');
  });

  it('后端多出的实例补进前端，独有字段用默认值', () => {
    const local: ServiceInstance[] = [];
    const backend = [
      makeBackend({ id: 'extra', name: '新服务', serviceType: 'claude', protocol: 'claude_messages' }),
    ];

    const result = mergeBackendIntoServices(local, backend);

    expect(result).toHaveLength(1);
    expect(result[0].id).toBe('extra');
    expect(result[0].systemPrompt).toBe(DEFAULT_PROMPTS.system);
    expect(result[0].translationPrompt).toBe(DEFAULT_PROMPTS.translation);
    expect(result[0].keyStatus).toBe('idle');
    expect(result[0].chainOfThought).toBe('off');
    expect(result[0].pulledModels).toEqual([]);
  });

  it('前端多出的实例被删除', () => {
    const local = [
      makeLocal({ id: 'local-only' }),
      makeLocal({ id: 'shared' }),
    ];
    const backend = [makeBackend({ id: 'shared' })];

    const result = mergeBackendIntoServices(local, backend);

    expect(result.map((s) => s.id)).toEqual(['shared']);
  });

  it('结果顺序按后端 services 顺序', () => {
    const local = [makeLocal({ id: 'a' }), makeLocal({ id: 'b' })];
    const backend = [makeBackend({ id: 'b' }), makeBackend({ id: 'a' })];

    const result = mergeBackendIntoServices(local, backend);

    expect(result.map((s) => s.id)).toEqual(['b', 'a']);
  });
});
```

- [x] **步骤 2：运行测试验证失败**

运行：`npx vitest run --config frontend/vite.config.ts frontend/src/settings/stores/settings.test.ts`
预期：FAIL，报错 `mergeBackendIntoServices is not a function` 或导入失败。

- [x] **步骤 3：编写最少实现代码**

在 `frontend/src/settings/stores/settings.ts` 顶部 import 区补类型（紧接现有 import）：

```ts
import type { AppConfig, ServiceInstanceConfig } from '@/types/config'
```

在 `seedInstances` 函数定义之前（`defaultInstanceFor` 之后）追加导出函数与私有 helper：

```ts
/**
 * 把后端 services 合并进本地 services 数组（按 id 匹配）。
 *
 * 合并规则（spec 2026-07-04 §5.1）：
 * - 后端核心字段（enabled/apiKey/endpoint/model/protocol）覆盖同 id 的本地实例
 * - 本地独有字段（prompts/keyStatus/chainOfThought/pulledModels/note）保留
 * - 后端多出的实例：补进本地，独有字段用默认值
 * - 本地多出的实例：删除（后端为源）
 * - 顺序按后端 services 顺序
 *
 * 该函数会 mutate local 中被匹配到的对象（让 Vue reactive 感知字段更新），
 * 返回新数组（顺序与 backend 一致）。
 */
export function mergeBackendIntoServices(
  local: ServiceInstance[],
  backend: ServiceInstanceConfig[],
): ServiceInstance[] {
  const localById = new Map(local.map((s) => [s.id, s]))
  const result: ServiceInstance[] = []
  for (const be of backend) {
    const existing = localById.get(be.id)
    if (existing) {
      existing.enabled = be.enabled
      existing.apiKey = be.apiKey ?? ''
      existing.endpoint = be.endpoint
      existing.model = be.model
      existing.protocol = be.protocol
      result.push(existing)
      localById.delete(be.id)
    } else {
      result.push(backendInstanceToLocal(be))
    }
  }
  return result
}

/** 后端 ServiceInstanceConfig → 前端 ServiceInstance，独有字段用默认值。 */
function backendInstanceToLocal(be: ServiceInstanceConfig): ServiceInstance {
  return {
    id: be.id,
    type: be.serviceType as ServiceId,
    name: be.name,
    enabled: be.enabled,
    protocol: be.protocol,
    apiKey: be.apiKey ?? '',
    model: be.model,
    endpoint: be.endpoint,
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

- [x] **步骤 4：运行测试验证通过**

运行：`npx vitest run --config frontend/vite.config.ts frontend/src/settings/stores/settings.test.ts`
预期：`mergeBackendIntoServices` 4 个用例 + 原 `settings defaults` 全部 PASS。

- [x] **步骤 5：类型检查**

运行：`npm run typecheck`
预期：无错误。

- [x] **步骤 6：Commit**

```bash
git add frontend/src/settings/stores/settings.ts frontend/src/settings/stores/settings.test.ts
git commit -m "feat(settings): 新增 mergeBackendIntoServices 按后端 services 合并"
```

---

## 任务 4：前端 syncFromBackend 方法 + 测试

**文件：**
- 修改：`frontend/src/settings/stores/settings.ts`
- 修改：`frontend/src/settings/stores/settings.test.ts`

- [x] **步骤 1：编写失败的测试**

在 `frontend/src/settings/stores/settings.test.ts` 顶部扩展 `@/lib/tauri` mock 并补 import。

把现有 mock：

```ts
vi.mock('@/lib/tauri', () => ({
  invokeSaveAppConfig: vi.fn(),
  isTauriReady: vi.fn(() => false),
}));
```

改为：

```ts
vi.mock('@/lib/tauri', () => ({
  invokeGetAppConfig: vi.fn(),
  invokeSaveAppConfig: vi.fn(),
  isTauriReady: vi.fn(() => false),
}));
```

在 import 区追加：

```ts
import { invokeGetAppConfig, invokeSaveAppConfig, isTauriReady } from '@/lib/tauri';
```

在 `beforeEach` 里追加 `useSettings().reset()` 以隔离各用例的模块级 state：

```ts
beforeEach(() => {
  fakeLocalStorage.clear();
  vi.clearAllMocks();
  useSettings().reset();
});
```

在文件末尾追加测试块：

```ts
describe('syncFromBackend', () => {
  it('Tauri 未就绪时静默降级，不调用任何 invoke', async () => {
    vi.mocked(isTauriReady).mockReturnValue(false);
    const settings = useSettings();
    await settings.syncFromBackend();
    expect(invokeGetAppConfig).not.toHaveBeenCalled();
    expect(invokeSaveAppConfig).not.toHaveBeenCalled();
  });

  it('后端 services 为空时推前端配置覆盖后端', async () => {
    vi.mocked(isTauriReady).mockReturnValue(true);
    vi.mocked(invokeGetAppConfig).mockResolvedValue({
      targetLang: '中文',
      services: [],
      popupPrecreate: true,
      overlayPrecreate: true,
      collectUsage: true,
      shortcuts: {},
    });

    const settings = useSettings();
    const expectedIds = settings.state.services.map((s) => s.id);
    await settings.syncFromBackend();

    expect(invokeSaveAppConfig).toHaveBeenCalledTimes(1);
    const saved = vi.mocked(invokeSaveAppConfig).mock.calls[0][0];
    expect(saved.services.map((s) => s.id)).toEqual(expectedIds);
  });

  it('invokeGetAppConfig 抛错时静默降级', async () => {
    vi.mocked(isTauriReady).mockReturnValue(true);
    vi.mocked(invokeGetAppConfig).mockRejectedValue(new Error('boom'));
    const settings = useSettings();
    await settings.syncFromBackend();
    expect(invokeSaveAppConfig).not.toHaveBeenCalled();
  });

  it('后端非空时按 id 合并到 state，不推覆盖', async () => {
    vi.mocked(isTauriReady).mockReturnValue(true);
    const settings = useSettings();
    const localId = settings.state.services[0].id;

    vi.mocked(invokeGetAppConfig).mockResolvedValue({
      targetLang: '中文',
      services: [
        {
          id: localId,
          serviceType: 'deepseek',
          name: 'DeepSeek',
          enabled: true,
          protocol: 'openai_chat',
          apiKey: 'backend-key',
          endpoint: 'https://api.deepseek.com',
          model: 'deepseek-chat',
          timeoutSeconds: 60,
        },
        {
          id: 'extra',
          serviceType: 'claude',
          name: 'Claude',
          enabled: false,
          protocol: 'claude_messages',
          apiKey: null,
          endpoint: 'https://api.anthropic.com',
          model: 'claude-haiku-4-5',
          timeoutSeconds: 60,
        },
      ],
      popupPrecreate: true,
      overlayPrecreate: true,
      collectUsage: true,
      shortcuts: {},
    });

    await settings.syncFromBackend();

    // 前端原有的 zhipu 实例（后端没有）被删除；后端 extra 被补进
    expect(settings.state.services.map((s) => s.id)).toEqual([localId, 'extra']);
    // 后端核心字段覆盖前端
    expect(settings.state.services[0].apiKey).toBe('backend-key');
    expect(settings.state.services[0].enabled).toBe(true);
    // 非空分支不推覆盖
    expect(invokeSaveAppConfig).not.toHaveBeenCalled();
  });
});
```

- [x] **步骤 2：运行测试验证失败**

运行：`npx vitest run --config frontend/vite.config.ts frontend/src/settings/stores/settings.test.ts`
预期：FAIL，报错 `settings.syncFromBackend is not a function`。

- [x] **步骤 3：编写最少实现代码**

在 `frontend/src/settings/stores/settings.ts` 顶部 import 区，把 `@/lib/tauri` 的 import 改为：

```ts
import { invokeGetAppConfig, invokeSaveAppConfig, isTauriReady } from '@/lib/tauri'
```

给 `defaultInstanceFor` 的 `protocol` fallback 加注释（约第 41 行）：

```ts
    // protocols 为空的渠道（gemini/deepl 等）走 'openai_chat' 占位；
    // 这类渠道在 ServicesPanel 启用开关被置灰，不会进入翻译批次，protocol 值不影响运行。
    protocol: protocol?.id ?? 'openai_chat',
```

在 `useSettings()` 返回对象里，紧接 `save` 方法之后追加 `syncFromBackend`：

```ts
  /** 启动时从后端 config.json 同步：后端空则推前端覆盖，后端非空则按 id 合并。失败静默降级。 */
  async syncFromBackend(): Promise<void> {
    if (!isTauriReady()) return
    let backend: AppConfig
    try {
      backend = await invokeGetAppConfig()
    } catch {
      return
    }
    if (!backend.services || backend.services.length === 0) {
      // 后端空（旧格式残留 / 首次启动）→ 前端推后端覆盖
      try {
        await invokeSaveAppConfig(projectToAppConfig(state))
      } catch {
        // 忽略：下次启动再试
      }
      return
    }
    state.services = mergeBackendIntoServices(state.services, backend.services)
    Object.assign(baseline, JSON.parse(JSON.stringify(state)))
    dirty.value = false
  },
```

- [x] **步骤 4：运行测试验证通过**

运行：`npx vitest run --config frontend/vite.config.ts frontend/src/settings/stores/settings.test.ts`
预期：`syncFromBackend` 4 个用例 + 之前所有用例全部 PASS。

- [x] **步骤 5：类型检查**

运行：`npm run typecheck`
预期：无错误。

- [x] **步骤 6：Commit**

```bash
git add frontend/src/settings/stores/settings.ts frontend/src/settings/stores/settings.test.ts
git commit -m "feat(settings): 新增 syncFromBackend 启动时同步后端配置"
```

---

## 任务 5：SettingsPage 挂载时调 syncFromBackend

**文件：**
- 修改：`frontend/src/settings/SettingsPage.vue`

- [x] **步骤 1：修改组件**

把 `frontend/src/settings/SettingsPage.vue` 的 `<script setup lang="ts">` 块改为：

```vue
<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import SettingsLayout from './SettingsLayout.vue'
import GeneralPanel from './panels/GeneralPanel.vue'
import TranslatePanel from './panels/TranslatePanel.vue'
import ShortcutPanel from './panels/ShortcutPanel.vue'
import ServicesPanel from './panels/ServicesPanel.vue'
import AdvancedPanel from './panels/AdvancedPanel.vue'
import HistoryPanel from './panels/HistoryPanel.vue'
import { useSettings } from './stores/settings'

interface Props {
  initialCategory?: string
}

const props = withDefaults(defineProps<Props>(), {
  initialCategory: 'general',
})

const active = ref<string>(props.initialCategory)
watch(
  () => props.initialCategory,
  (value) => {
    if (value) active.value = value
  },
)

const onUpdateActive = (value: string): void => {
  active.value = value
  if (typeof window !== 'undefined') {
    const url = new URL(window.location.href)
    url.hash = value
    window.history.replaceState({}, '', url)
  }
}

const settings = useSettings()
onMounted(() => {
  void settings.syncFromBackend()
})
</script>
```

- [x] **步骤 2：类型检查**

运行：`npm run typecheck`
预期：无错误。

- [x] **步骤 3：Commit**

```bash
git add frontend/src/settings/SettingsPage.vue
git commit -m "feat(settings): SettingsPage 挂载时调 syncFromBackend"
```

---

## 任务 6：ServicesPanel 未对接渠道标记“开发中”

**文件：**
- 修改：`frontend/src/settings/panels/ServicesPanel.vue`

- [x] **步骤 1：新增 isDeveloping helper**

在 `frontend/src/settings/panels/ServicesPanel.vue` 的 `<script setup>` 内，紧接 `activeService` 的 `computed` 定义之后追加：

```ts
/** 渠道 protocols 为空即视为“尚未对接”，在 UI 上标记开发中并置灰启用。 */
const isDeveloping = (type: ServiceId): boolean =>
  serviceById(type)?.protocols.length === 0
```

- [x] **步骤 2：添加 Dialog 卡片“开发中”badge**

在添加服务 Dialog 的卡片按钮里，`v-if="!svc.builtin"` 的“自定义”badge 之前追加 amber“开发中”badge。定位到模板内 `<span v-if="!svc.builtin"` 那一行，在它前面插入：

```html
                  <span
                    v-if="svc.protocols.length === 0"
                    class="absolute right-1.5 top-1.5 rounded bg-amber-100 px-1 py-0.5 text-[9px] font-normal text-amber-700 dark:bg-amber-900/40 dark:text-amber-300"
                    title="该渠道尚未对接，暂不可用"
                  >
                    开发中
                  </span>
```

注意：原“自定义”badge 用 `absolute right-1.5 top-1.5`，与新增 badge 同位置会重叠。把原“自定义”badge 的 class 改为非 absolute（靠左下）或调整。这里采用：新增“开发中”在右上，原“自定义”移到卡片底部右下。把原 `<span v-if="!svc.builtin" class="absolute right-1.5 top-1.5 ...">自定义</span>` 改为：

```html
                  <span
                    v-if="!svc.builtin"
                    class="absolute bottom-1.5 right-1.5 rounded bg-muted px-1 py-0.5 text-[9px] text-muted-foreground"
                  >
                    自定义
                  </span>
```

- [x] **步骤 3：服务列表启用开关 disabled + tooltip**

定位到服务列表里的 `<SettingSwitch`（约第 344 行），把它替换为带外层 tooltip 包裹的版本：

```html
                  <span
                    :title="isDeveloping(inst.type) ? '该渠道尚未对接，暂不可用' : undefined"
                    class="inline-flex"
                  >
                    <SettingSwitch
                      :model-value="inst.enabled"
                      :disabled="isDeveloping(inst.type)"
                      :aria-label="`${inst.enabled ? '停用' : '启用'} ${inst.name}`"
                      @update:model-value="() => handleToggle(inst)"
                    />
                  </span>
```

- [x] **步骤 4：详情页顶部 amber 横幅**

在详情页 `<header>...</header>` 闭合标签之后、第一个 `<SettingGroup title="接入点" bare>` 之前，追加：

```html
      <div
        v-if="activeService.protocols.length === 0"
        class="flex items-start gap-2 rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800 dark:border-amber-900 dark:bg-amber-950/40 dark:text-amber-200"
      >
        <CircleAlert class="mt-0.5 h-3.5 w-3.5 shrink-0" />
        <span>该渠道尚未对接，暂不可用。</span>
      </div>
```

- [x] **步骤 5：类型检查**

运行：`npm run typecheck`
预期：无错误。

- [x] **步骤 6：Commit**

```bash
git add frontend/src/settings/panels/ServicesPanel.vue
git commit -m "feat(services): 未对接渠道标记开发中并置灰启用"
```

---

## 任务 7：翻译弹窗图标按渠道 id 匹配

**文件：**
- 修改：`frontend/public/translate.js`

- [x] **步骤 1：替换 ENGINE_META 并新增 engineIcon**

在 `frontend/public/translate.js` 里，把现有 `ENGINE_META` 对象（约第 83–96 行）整段替换为：

```js
/* === 引擎图标/名映射 === */
// 按 payload.serviceType（渠道 id）匹配；未匹配 fallback 取 serviceName 首字，灰底。
const ENGINE_META = {
  openai: { color: '#10A37F', letter: 'O' },
  deepseek: { color: '#4D6BFE', letter: 'D' },
  zhipu: { color: '#3B5BFE', letter: 'Z' },
  claude: { color: '#D97757', letter: 'C' },
  mock: { color: '#94918A', letter: 'M' },
};

function engineIcon(serviceType, serviceName) {
  const meta = ENGINE_META[serviceType];
  const color = meta ? meta.color : '#94918A';
  const letter = meta
    ? meta.letter
    : ((serviceName || '?').trim().charAt(0).toUpperCase() || '?');
  return (
    '<rect width="20" height="20" rx="5" fill="' + color + '"/>' +
    '<text x="10" y="14.5" text-anchor="middle" font-size="12" font-weight="700" fill="#fff" ' +
    'font-family="Segoe UI, system-ui, sans-serif">' + letter + '</text>'
  );
}
```

- [x] **步骤 2：getCard 改用 engineIcon**

定位到 `getCard` 函数里设置图标的片段（约第 164–169 行）：

```js
  const meta = ENGINE_META[payload.serviceType];
  if (meta) {
    card.querySelector('.result-engine-icon').innerHTML = meta.icon;
  } else {
    card.querySelector('.result-engine-icon').innerHTML = ENGINE_META['mock'].icon;
  }
```

替换为：

```js
  card.querySelector('.result-engine-icon').innerHTML = engineIcon(
    payload.serviceType,
    payload.serviceName,
  );
```

- [x] **步骤 3：语法检查**

运行：`node --check frontend/public/translate.js`
预期：无输出（语法正确）。

- [x] **步骤 4：前端全量测试与构建**

运行：`npm run test && npm run build`
预期：vitest 全部 PASS；vite 构建成功（translate.js 是纯静态资源，不参与构建，但确认 settings 页构建无副作用）。

- [x] **步骤 5：Commit**

```bash
git add frontend/public/translate.js
git commit -m "fix(translate): 弹窗图标按渠道 id 匹配，补彩色字母图标"
```

---

## 任务 8：文档同步

**文件：**
- 修改：`README.md`
- 修改：`docs/roadmap/progressive-development-plan.md`
- 修改：`AGENTS.md`
- 修改：`CLAUDE.md`

- [x] **步骤 1：更新 README v0.2 段落**

在 `README.md` 的“### 服务协议与多结果翻译（v0.2）”段落（约第 58–63 行），把三条要点替换/扩充为：

```markdown
### 服务协议与多结果翻译（v0.2）

- 服务列表默认展示 DeepSeek 与智谱 AI，默认关闭；启用后按列表顺序参与翻译。
- 服务实例通过 `protocol` 选择调用协议；协议 id 前后端统一为 `openai_chat` / `claude_messages` / `mock`，未知协议后端报错而非静默走 OpenAI 兼容。
- 前后端配置以 `config.json` 为事实来源：设置页挂载时从后端拉取，后端 `services` 为空则推前端覆盖（用于旧格式残留 / 首次启动），后端非空则按实例 id 合并（后端核心字段覆盖前端、前端独有字段如提示词保留）。
- 翻译弹窗按启用服务渲染多个结果卡，单个服务失败不影响其他服务；卡片图标按渠道 id（openai/deepseek/zhipu/claude/mock）区分。
- 未对接渠道（gemini/deepl/google/baidu/youdao/tencent/volcengine/iflytek/moonshot/siliconflow）在添加 Dialog 标“开发中”badge、服务列表启用开关置灰、详情页顶部横幅提示。
```

- [x] **步骤 2：更新 README 环境变量段落**

在 `README.md` 环境变量示例（约第 73 行），把：

```bash
SHIZI_LLM_PROVIDER=mock | openai-compatible | claude
```

改为：

```bash
SHIZI_LLM_PROVIDER=mock | openai_chat | claude_messages
```

- [x] **步骤 3：更新 roadmap 当前完成状态**

在 `docs/roadmap/progressive-development-plan.md` 的“已完成 / 基本完成”列表末尾追加：

```markdown
- 服务模块打磨（v0.2.1）：协议 id 前后端统一为 `openai_chat`/`claude_messages`/`mock`，未知协议报错；设置页挂载时与后端 `config.json` 双向同步；未对接渠道标记“开发中”并置灰启用；翻译弹窗卡片图标按渠道 id 区分。
```

- [x] **步骤 4：更新 AGENTS.md 与 CLAUDE.md 架构关键点**

在 `AGENTS.md` 与 `CLAUDE.md` 的“架构关键点”章节里，把“服务协议配置”与“批次翻译”两条之间追加一条“前后端配置同步”（两文件内容保持一致）：

```markdown
- **前后端配置同步**：`config.json` 的 `services[]` 是事实来源。设置页 `SettingsPage` 挂载时调 `settings.syncFromBackend()`：后端 `services` 为空（旧格式残留 / 首次启动）→ 前端 `projectToAppConfig` 推 `invokeSaveAppConfig` 覆盖后端；后端非空 → `mergeBackendIntoServices` 按 id 合并（后端 `enabled/apiKey/endpoint/model/protocol` 覆盖前端同 id 实例，前端 `prompts/keyStatus/chainOfThought/pulledModels/note` 保留；后端多出补进、前端多出删除）。协议 id 前后端统一为 `openai_chat`/`claude_messages`/`mock`，后端 `provider_for_service` 未知协议返回错误。未对接渠道（`ServiceMeta.protocols.length === 0`）在添加 Dialog / 服务列表 / 详情页三处标 amber“开发中”，启用开关 `disabled`。
```

- [x] **步骤 5：Commit**

```bash
git add README.md docs/roadmap/progressive-development-plan.md AGENTS.md CLAUDE.md
git commit -m "docs: 同步服务模块打磨（配置同步/协议 id/开发中标记）"
```

---

## 最终手动验证

编码与文档全部 commit 后，执行 spec §7 的手动验证（不产生新 commit，仅作为完成门禁）：

1. `cd src-tauri && cargo test` 全绿。
2. `npm run test` 全绿。
3. `npm run typecheck` 无错误。
4. `cd src-tauri && cargo build` 成功。
5. `npm run tauri dev` 启动后：
   - 启用 DeepSeek + 智谱 AI，点保存，划词翻译 → 弹窗出现两张卡片，图标不同（D / Z）。
   - 删除 `%APPDATA%/com.shizi.app/config.json` 模拟旧格式残留，重启设置页 → `config.json` 变为新格式（含 `services` 数组）。
   - 添加 Dialog 里 gemini/deepl 等显示“开发中”badge；添加后服务列表启用开关置灰，详情页顶部 amber 横幅。
   - 启用 Claude 渠道（填 Key），翻译不再因协议误匹配失败。

---

## 自检

**1. 规格覆盖度**

- spec §4.1（protocol.rs 协议 id 对齐 + 未知报错）→ 任务 1。
- spec §4.2（types.rs DEFAULT_PROTOCOL / from_env service_type / normalized / match 分支）→ 任务 2。
- spec §4.3–4.4（store.rs / batch.rs 不动）→ 计划未触及，符合范围。
- spec §5.1（syncFromBackend + mergeBackendIntoServices + defaultInstanceFor 注释）→ 任务 3、4。
- spec §5.2（SettingsPage onMounted）→ 任务 5。
- spec §5.3（ServicesPanel 三处开发中标记）→ 任务 6。
- spec §5.4（translate.js ENGINE_META 改 serviceType + 5 图标 + fallback）→ 任务 7。
- spec §7（验证命令 + 手动验证）→ 各任务步骤 + 最终手动验证。
- spec §8（文档同步）→ 任务 8。
- spec §2 范围外（旧格式字段迁移 / 未对接渠道原生协议 / overlay / 单卡重试）→ 计划未触及，符合 YAGNI。

**2. 占位符扫描**

无 TODO / 待定 / “类似任务 N” / “添加适当错误处理” 等占位。每个代码步骤均给出完整可粘贴代码与精确命令。

**3. 类型一致性**

- `ProviderKind` 枚举三个变体在任务 1 的 `protocol_to_kind` 与 `provider_for_service` 中一致使用。
- `mergeBackendIntoServices(local: ServiceInstance[], backend: ServiceInstanceConfig[])` 签名在任务 3 实现与测试、任务 4 `syncFromBackend` 调用处一致。
- `backendInstanceToLocal(be: ServiceInstanceConfig): ServiceInstance` 在任务 3 定义并被 `mergeBackendIntoServices` 调用，字段名与 `frontend/src/settings/types.ts` 的 `ServiceInstance` 一致。
- `syncFromBackend()` 在任务 4 定义、任务 5 `SettingsPage.vue` 调用，方法名一致。
- `isDeveloping(type: ServiceId)` 在任务 6 定义并在模板三处使用，与 `serviceById(type)?.protocols.length === 0` 推导一致。
- `engineIcon(serviceType, serviceName)` 在任务 7 定义并被 `getCard` 调用，参数名与 `payload.serviceType` / `payload.serviceName` 一致。
- 前端协议 id `openai_chat`/`claude_messages`（`frontend/src/types/config.ts` 的 `ServiceProtocolId`）与后端任务 1/2 的字面量完全一致。


---

## 补充变更（计划外）

**翻译弹窗卡片预建**（提交 `eaf6d94`）：

用户反馈翻译弹窗卡片只在收到 `started` 事件后才出现，体验不符合预期。改动：

- `frontend/public/translate.js` 新增 `initCards()`：弹窗加载时调 `invoke('get_app_config')` 获取启用服务列表，预建所有占位卡片（含引擎图标和名称）
- `started` 事件 `isNewBranch` 逻辑：不再 `resultCards.clear()` + `resultsList.innerHTML = ''`，改为 `forEach` 重置已有卡片到 `pending` 状态，`getCard` 原地复用

## 最终验证结果

| 检查项 | 结果 |
|--------|------|
| `cd src-tauri && cargo test` | 109 passed, 0 failed, 2 ignored |
| `npx vitest run` | 3 files, 19 passed |
| `npm run typecheck` | 零错误 |
| `cd src-tauri && cargo build` | 成功 |
| `npm run build` | 成功 |
