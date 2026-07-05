# 当前 UI 未实现能力打磨实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 只处理当前 UI 已经露出的未接通能力，优先接通成本低、用户能直接感知的能力，并把不值得现在实现的入口收口或删除。

**架构：** 后端配置继续以 `AppConfig.services[]` 为事实来源；新增的服务探测命令只负责 API Key 校验和模型列表拉取，不混入保存配置流程。翻译弹窗仍保持静态页，轻量读取 `get_app_config` 完成自动复制和 OCR 历史写入；大型能力如取词、语音输入、自动更新不在本计划内实现。

**技术栈：** Tauri 2、Rust、reqwest、arboard、enigo、Vue 3、Vitest、vue-tsc。

---

## 审计结果

当前 UI 已展示但没有真实运行路径的能力如下：

| 入口 | 现状 | 本计划处理 |
| --- | --- | --- |
| 服务 API Key 校验 | `ServicesPanel.vue` 使用随机 50/50 mock | 接通后端探测命令 |
| 模型列表拉取 | 使用 `MOCK_PULLED_MODELS` | 接通服务商 `/models` 类接口 |
| 系统提示词 / 翻译提示词 | 只存在前端字段 | 写入后端配置并参与 LLM 请求 |
| 思维链长度 | 只存在前端字段，Claude provider 只有布尔开关 | `off` 关闭，其他档位映射为 Claude thinking 开启；OpenAI 兼容先不发送额外字段 |
| 反思提示词 | UI 暴露但需要二次模型调用 | 建议先隐藏，除非用户明确要牺牲速度实现 |
| 自动复制结果 | 设置页有开关，弹窗未读取 | 翻译完成后复制主结果 |
| 翻译后恢复原剪贴板 | 划词复制当前总是恢复，设置开关不生效 | 配置化控制划词阶段是否恢复 |
| 翻译后自动粘贴 | UI 有开关，但原应用焦点已丢失 | 建议删除或置灰 |
| OCR 历史 | 设置页展示样本数据，真实 OCR 不写入 | 改为空默认，并在 OCR 翻译完成时写入真实历史 |
| 翻译弹窗收藏 / 书签 | 只切样式或提示“功能开发中” | 建议删除按钮 |
| 翻译弹窗源语言 / 目标语言 / 交换 | 只提示“功能开发中” | 建议改成只读显示默认语言 |
| 取词、音标、备选翻译、取词增强 | UI 有入口但没有取词/词典链路 | 建议隐藏，单独开规格再做 |
| 语音输入 | UI 有入口但没有录音、ASR、权限链路 | 建议隐藏，单独开规格再做 |
| 开机启动、更新、日志导出、项目主页、使用文档、分享 | UI 有入口但缺系统集成或目标链接 | 从本轮移出执行，仅保留可描述为“未启用”的 UI |

## 执行前确认

编码执行前先问用户以下问题，推荐答案写在前面：

1. 是否删除翻译弹窗的收藏、书签按钮，并把语言切换改成只读显示？推荐删除/只读，因为当前没有收藏库、术语库或单次语言覆盖链路。
2. 是否隐藏 `自动粘贴`、`取词翻译`、`显示音标`、`显示备选翻译`、`取词增强`、`语音输入`？推荐隐藏，等取词或语音功能有单独规格后再加回来。
3. 是否保留高级页的导入/导出配置？推荐保留并接通，因为无需新增依赖，收益明确。

## 文件结构

计划一：服务与翻译主链路接通。

- 修改：`src-tauri/src/core/config/types.rs`
  - 为后端配置补齐 UI 运行需要的字段：`defaultSourceLang`、`autoCopy`、`restoreClipboard`、服务级提示词、`chainOfThought`。
- 修改：`frontend/src/types/config.ts`
  - 与后端配置字段保持 camelCase 对齐。
- 修改：`frontend/src/lib/config.ts`
  - 将设置页状态投影到新增后端字段，并更新校验。
- 修改：`frontend/src/settings/stores/settings.ts`
  - 后端同步时保留并合并新增字段；OCR 历史默认改为空。
- 修改：`src-tauri/src/core/translation/types.rs`
  - 在 `TranslationRequest` 中携带提示词配置，集中渲染 system/user prompt。
- 修改：`src-tauri/src/core/translation/batch.rs`
  - 从 `ServiceInstanceConfig` 构造带提示词的批次请求。
- 修改：`src-tauri/src/core/llm/openai_compatible.rs`
  - 使用请求中的 system/user prompt。
- 修改：`src-tauri/src/core/llm/claude.rs`
  - 使用请求中的 system/user prompt，并接入 thinking 开关。
- 修改：`src-tauri/src/core/llm/protocol.rs`
  - 按服务配置传递 `chainOfThought` 到 Claude provider。
- 创建：`src-tauri/src/ui/service_probe.rs`
  - 提供 `validate_service_credential` 和 `list_service_models` 两个 Tauri commands。
- 修改：`src-tauri/src/ui/mod.rs`
  - 导出 `service_probe` 模块。
- 修改：`src-tauri/src/lib.rs`
  - 注册新增 Tauri commands。
- 修改：`frontend/src/lib/tauri.ts`
  - 增加服务探测命令封装。
- 修改：`frontend/src/settings/panels/ServicesPanel.vue`
  - 移除 mock 校验和 mock 拉模型，调用真实命令。
- 修改：`src-tauri/src/core/selection/mod.rs`
  - 让划词复制是否恢复剪贴板受配置控制。
- 修改：`src-tauri/src/app/shortcuts.rs`
  - 读取 `restoreClipboard` 后调用划词复制。
- 修改：`frontend/public/translate.js`
  - 读取 `autoCopy/defaultSourceLang/targetLang`；翻译完成后自动复制主结果；OCR 翻译完成时写入真实历史。

计划二：UI 诚实化收口。

- 修改：`frontend/public/translate.html`
  - 删除收藏、书签按钮；语言栏改为只读显示。
- 修改：`frontend/public/translate.js`
  - 删除对应事件监听，避免“功能开发中”提示。
- 修改：`frontend/public/translate.css`
  - 清理被删除按钮和禁用语言栏的样式。
- 修改：`frontend/src/settings/panels/TranslatePanel.vue`
  - 隐藏或置灰未实现的自动粘贴、音标、备选翻译、取词延迟。
- 修改：`frontend/src/settings/panels/ShortcutPanel.vue`
  - 隐藏或置灰 `word-lookup` 快捷键。
- 修改：`frontend/src/settings/panels/AdvancedPanel.vue`
  - 隐藏取词增强、语音输入；接通配置导入导出。

计划三：低成本设置页补齐。

- 创建：`frontend/src/settings/config-io.ts`
  - 提供导出配置、导入配置、剔除 API Key、保留现有 API Key 的纯函数。
- 修改：`frontend/src/settings/panels/AdvancedPanel.vue`
  - 导出按钮下载 JSON；导入按钮读取 JSON 并合并设置。
- 测试：`frontend/src/settings/config-io.test.ts`
  - 覆盖导出剔除密钥、导入保留本地密钥、非法 JSON 报错。

---

### 任务 1：补齐配置模型与前端投影

**文件：**
- 修改：`src-tauri/src/core/config/types.rs`
- 修改：`frontend/src/types/config.ts`
- 修改：`frontend/src/lib/config.ts`
- 修改：`frontend/src/lib/config.test.ts`
- 修改：`frontend/src/settings/stores/settings.ts`
- 修改：`frontend/src/settings/stores/settings.test.ts`

- [ ] **步骤 1：编写失败的 Rust 配置测试**

在 `src-tauri/src/core/config/types.rs` 的 `tests` 模块新增：

```rust
#[test]
fn normalized_fills_ui_runtime_defaults() {
    let mut config = AppConfig::from_env();
    config.default_source_lang = "".to_string();
    config.auto_copy = false;
    config.restore_clipboard = false;
    config.services[0].system_prompt = "  ".to_string();
    config.services[0].translation_prompt = "  ".to_string();
    config.services[0].chain_of_thought = "bad".to_string();

    let normalized = config.normalized();

    assert_eq!(normalized.default_source_lang, "auto");
    assert!(!normalized.auto_copy);
    assert!(!normalized.restore_clipboard);
    assert_eq!(normalized.services[0].system_prompt, "");
    assert_eq!(normalized.services[0].translation_prompt, "");
    assert_eq!(normalized.services[0].chain_of_thought, "off");
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
cd src-tauri && cargo test normalized_fills_ui_runtime_defaults
```

预期：FAIL，报错包含 `unknown field` 或字段不存在。

- [ ] **步骤 3：实现后端配置字段**

在 `ServiceInstanceConfig` 添加：

```rust
#[serde(default)]
pub system_prompt: String,
#[serde(default)]
pub translation_prompt: String,
#[serde(default)]
pub reflection_prompt: String,
#[serde(default)]
pub reflection_enabled: bool,
#[serde(default = "default_chain_of_thought")]
pub chain_of_thought: String,
```

在 `AppConfig` 添加：

```rust
#[serde(default = "default_source_lang")]
pub default_source_lang: String,
#[serde(default = "default_true")]
pub auto_copy: bool,
#[serde(default = "default_true")]
pub restore_clipboard: bool,
```

新增默认函数：

```rust
fn default_source_lang() -> String {
    "auto".to_string()
}

fn default_chain_of_thought() -> String {
    "off".to_string()
}

fn normalize_chain_of_thought(value: String) -> String {
    match value.trim() {
        "short" | "medium" | "long" => value.trim().to_string(),
        _ => "off".to_string(),
    }
}
```

在 `ServiceInstanceConfig::normalized` 中追加：

```rust
self.system_prompt = self.system_prompt.trim().to_string();
self.translation_prompt = self.translation_prompt.trim().to_string();
self.reflection_prompt = self.reflection_prompt.trim().to_string();
self.chain_of_thought = normalize_chain_of_thought(self.chain_of_thought);
```

在 `AppConfig::from_env` 初始化：

```rust
default_source_lang: default_source_lang(),
auto_copy: true,
restore_clipboard: true,
```

在 `AppConfig::normalized` 追加：

```rust
self.default_source_lang = normalize_string(self.default_source_lang, "auto");
```

- [ ] **步骤 4：运行 Rust 配置测试验证通过**

运行：

```bash
cd src-tauri && cargo test normalized_fills_ui_runtime_defaults
```

预期：PASS。

- [ ] **步骤 5：编写失败的前端投影测试**

在 `frontend/src/lib/config.test.ts` 的 `projectToAppConfig` describe 中新增：

```ts
it('投影翻译行为与提示词字段到后端配置', () => {
  const state = makeState([
    makeInstance({
      id: 'deepseek-1',
      enabled: true,
      apiKey: 'sk-ds',
      systemPrompt: ' 系统 ',
      translationPrompt: ' 翻译 {text} ',
      reflectionPrompt: ' 反思 ',
      reflectionEnabled: true,
      chainOfThought: 'medium',
    }),
  ]);
  state.translation.defaultSourceLang = 'en-US';
  state.translation.autoCopy = false;
  state.translation.restoreClipboard = false;

  const config = projectToAppConfig(state);

  expect(config.defaultSourceLang).toBe('en-US');
  expect(config.autoCopy).toBe(false);
  expect(config.restoreClipboard).toBe(false);
  expect(config.services[0]).toMatchObject({
    systemPrompt: '系统',
    translationPrompt: '翻译 {text}',
    reflectionPrompt: '反思',
    reflectionEnabled: true,
    chainOfThought: 'medium',
  });
});
```

- [ ] **步骤 6：运行前端测试验证失败**

运行：

```bash
npm run test -- frontend/src/lib/config.test.ts
```

预期：FAIL，新增字段不存在或断言失败。

- [ ] **步骤 7：实现前端类型与投影**

在 `frontend/src/types/config.ts` 扩展：

```ts
export type ChainOfThought = 'off' | 'short' | 'medium' | 'long';

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
  systemPrompt: string;
  translationPrompt: string;
  reflectionPrompt: string;
  reflectionEnabled: boolean;
  chainOfThought: ChainOfThought;
}

export interface AppConfig {
  targetLang: string;
  defaultSourceLang: string;
  autoCopy: boolean;
  restoreClipboard: boolean;
  services: ServiceInstanceConfig[];
  popupPrecreate: boolean;
  overlayPrecreate: boolean;
  collectUsage: boolean;
  shortcuts: Record<string, string>;
}
```

在 `projectToAppConfig` 添加：

```ts
defaultSourceLang: state.translation.defaultSourceLang,
autoCopy: state.translation.autoCopy,
restoreClipboard: state.translation.restoreClipboard,
```

并在 service 投影中添加：

```ts
systemPrompt: service.systemPrompt.trim(),
translationPrompt: service.translationPrompt.trim(),
reflectionPrompt: service.reflectionPrompt.trim(),
reflectionEnabled: service.reflectionEnabled,
chainOfThought: service.chainOfThought,
```

在 `mergeBackendIntoServices` 中把后端字段覆盖回前端：

```ts
systemPrompt: b.systemPrompt || existing.systemPrompt,
translationPrompt: b.translationPrompt || existing.translationPrompt,
reflectionPrompt: b.reflectionPrompt || existing.reflectionPrompt,
reflectionEnabled: b.reflectionEnabled,
chainOfThought: b.chainOfThought,
```

在 `syncFromBackend` 中同步：

```ts
state.translation.defaultSourceLang = backend.defaultSourceLang ?? state.translation.defaultSourceLang;
state.translation.autoCopy = backend.autoCopy ?? state.translation.autoCopy;
state.translation.restoreClipboard = backend.restoreClipboard ?? state.translation.restoreClipboard;
```

- [ ] **步骤 8：运行前端投影测试验证通过**

运行：

```bash
npm run test -- frontend/src/lib/config.test.ts
```

预期：PASS。

- [ ] **步骤 9：Commit**

```bash
git add src-tauri/src/core/config/types.rs frontend/src/types/config.ts frontend/src/lib/config.ts frontend/src/lib/config.test.ts frontend/src/settings/stores/settings.ts frontend/src/settings/stores/settings.test.ts
git commit -m "feat(config): 接通 UI 运行配置字段"
```

---

### 任务 2：让提示词与思维链进入 LLM 请求

**文件：**
- 修改：`src-tauri/src/core/translation/types.rs`
- 修改：`src-tauri/src/core/translation/batch.rs`
- 修改：`src-tauri/src/core/llm/openai_compatible.rs`
- 修改：`src-tauri/src/core/llm/claude.rs`
- 修改：`src-tauri/src/core/llm/protocol.rs`

- [ ] **步骤 1：编写失败的提示词渲染测试**

在 `src-tauri/src/core/translation/types.rs` 的 `tests` 模块新增：

```rust
#[test]
fn request_uses_custom_prompts_with_placeholders() {
    let request = TranslationRequest {
        session_id: TranslationSessionId("s1".to_string()),
        input: TranslationInput::ManualText("hello".to_string()),
        target_lang: "中文".to_string(),
        service: fake_service(),
        prompts: TranslationPromptConfig {
            source_lang: "English".to_string(),
            system_prompt: "sys".to_string(),
            translation_prompt: "from {source_lang} to {target_lang}: {text}".to_string(),
            chain_of_thought: "off".to_string(),
        },
    };

    assert_eq!(request.system_prompt(), "sys");
    assert_eq!(request.user_prompt(), "from English to 中文: hello");
}

#[test]
fn request_falls_back_to_default_prompts() {
    let request = TranslationRequest {
        session_id: TranslationSessionId("s1".to_string()),
        input: TranslationInput::ManualText("hello".to_string()),
        target_lang: "中文".to_string(),
        service: fake_service(),
        prompts: TranslationPromptConfig {
            source_lang: "auto".to_string(),
            system_prompt: "".to_string(),
            translation_prompt: "".to_string(),
            chain_of_thought: "off".to_string(),
        },
    };

    assert!(request.system_prompt().contains("专业翻译"));
    assert!(request.user_prompt().contains("中文"));
    assert!(request.user_prompt().contains("hello"));
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
cd src-tauri && cargo test request_uses_custom_prompts_with_placeholders request_falls_back_to_default_prompts
```

预期：FAIL，`TranslationPromptConfig` 或方法不存在。

- [ ] **步骤 3：实现提示词配置结构与渲染方法**

在 `TranslationRequest` 增加字段：

```rust
pub prompts: TranslationPromptConfig,
```

新增结构：

```rust
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationPromptConfig {
    pub source_lang: String,
    pub system_prompt: String,
    pub translation_prompt: String,
    pub chain_of_thought: String,
}
```

在 `impl TranslationRequest` 中新增：

```rust
pub fn system_prompt(&self) -> String {
    let prompt = self.prompts.system_prompt.trim();
    if prompt.is_empty() {
        "你是一个专业翻译引擎。只输出译文，不要解释。".to_string()
    } else {
        prompt.to_string()
    }
}

pub fn user_prompt(&self) -> String {
    let template = self.prompts.translation_prompt.trim();
    if template.is_empty() {
        return format!("请将以下文本翻译为{}：\n\n{}", self.target_lang, self.source_text());
    }

    template
        .replace("{source_lang}", &self.prompts.source_lang)
        .replace("{target_lang}", &self.target_lang)
        .replace("{text}", self.source_text())
}

pub fn thinking_enabled(&self) -> bool {
    self.prompts.chain_of_thought != "off"
}
```

- [ ] **步骤 4：批次请求填入提示词配置**

在 `build_batch_requests` 构造 `TranslationRequest` 时添加：

```rust
prompts: TranslationPromptConfig {
    source_lang: "auto".to_string(),
    system_prompt: s.system_prompt.clone(),
    translation_prompt: s.translation_prompt.clone(),
    chain_of_thought: s.chain_of_thought.clone(),
},
```

并把函数签名增加 `source_lang: String` 参数，由调用处传入 `config.default_source_lang.clone()`。

- [ ] **步骤 5：Provider 使用请求提示词**

在 `OpenAiCompatibleProvider::request_body` 中替换固定文案：

```rust
messages: vec![
    ChatMessage {
        role: "system",
        content: request.system_prompt(),
    },
    ChatMessage {
        role: "user",
        content: request.user_prompt(),
    },
],
```

在 `ClaudeProvider::stream_translate` 中替换：

```rust
thinking: if request.thinking_enabled() {
    Some(ClaudeThinkingConfig {
        thinking_type: "adaptive".to_string(),
    })
} else {
    None
},
system: request.system_prompt(),
messages: vec![ClaudeMessage {
    role: "user".to_string(),
    content: request.user_prompt(),
}],
```

在 `provider_for_service` 中去掉 `enable_thinking: false` 的业务含义。保留字段也可以，但请求侧以 `request.thinking_enabled()` 为准。

- [ ] **步骤 6：运行翻译相关 Rust 测试**

运行：

```bash
cd src-tauri && cargo test translation:: batch:: llm::
```

预期：PASS。

- [ ] **步骤 7：Commit**

```bash
git add src-tauri/src/core/translation/types.rs src-tauri/src/core/translation/batch.rs src-tauri/src/core/llm/openai_compatible.rs src-tauri/src/core/llm/claude.rs src-tauri/src/core/llm/protocol.rs
git commit -m "feat(translation): 使用服务提示词配置"
```

---

### 任务 3：接通服务 API Key 校验和模型拉取

**文件：**
- 创建：`src-tauri/src/ui/service_probe.rs`
- 修改：`src-tauri/src/ui/mod.rs`
- 修改：`src-tauri/src/lib.rs`
- 修改：`frontend/src/lib/tauri.ts`
- 修改：`frontend/src/settings/panels/ServicesPanel.vue`

- [ ] **步骤 1：编写失败的后端探测测试**

创建 `src-tauri/src/ui/service_probe.rs` 并先写测试与类型骨架：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_endpoint_trims_trailing_slash() {
        assert_eq!(
            models_endpoint("https://api.example.com/v1/"),
            "https://api.example.com/v1/models"
        );
    }

    #[test]
    fn probe_request_rejects_missing_key_for_network_protocol() {
        let request = ServiceProbeRequest {
            protocol: "openai_chat".to_string(),
            endpoint: "https://api.example.com/v1".to_string(),
            api_key: None,
        };

        assert_eq!(request.validate().unwrap_err(), "请先填写 API Key");
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
cd src-tauri && cargo test service_probe
```

预期：FAIL，模块或函数未实现。

- [ ] **步骤 3：实现最小后端探测命令**

在 `service_probe.rs` 实现：

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceProbeRequest {
    pub protocol: String,
    pub endpoint: String,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelItem>,
}

#[derive(Debug, Clone, Deserialize)]
struct ModelItem {
    id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelsResult {
    pub models: Vec<String>,
}

fn models_endpoint(endpoint: &str) -> String {
    format!("{}/models", endpoint.trim_end_matches('/'))
}

impl ServiceProbeRequest {
    fn validate(&self) -> Result<(), String> {
        if !matches!(self.protocol.as_str(), "openai_chat" | "claude_messages") {
            return Err("当前协议不可用".to_string());
        }
        if self.api_key.as_deref().unwrap_or("").trim().is_empty() {
            return Err("请先填写 API Key".to_string());
        }
        let url = reqwest::Url::parse(self.endpoint.trim())
            .map_err(|_| "Endpoint 请输入有效的 http(s) 地址".to_string())?;
        if url.scheme() != "http" && url.scheme() != "https" {
            return Err("Endpoint 请输入有效的 http(s) 地址".to_string());
        }
        Ok(())
    }
}
```

新增 commands：

```rust
#[tauri::command]
pub async fn validate_service_credential(request: ServiceProbeRequest) -> Result<(), String> {
    let _ = list_service_models(request).await?;
    Ok(())
}

#[tauri::command]
pub async fn list_service_models(request: ServiceProbeRequest) -> Result<ModelsResult, String> {
    request.validate()?;
    let api_key = request.api_key.as_deref().unwrap_or("").trim();
    let client = reqwest::Client::new();
    let mut builder = client.get(models_endpoint(&request.endpoint));
    builder = match request.protocol.as_str() {
        "claude_messages" => builder
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01"),
        _ => builder.bearer_auth(api_key),
    };

    let response = builder.send().await.map_err(|e| format!("请求失败: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("服务返回 HTTP {}", response.status()));
    }
    let body = response.json::<ModelsResponse>().await.map_err(|e| format!("模型列表解析失败: {e}"))?;
    Ok(ModelsResult {
        models: body.data.into_iter().map(|m| m.id).filter(|id| !id.trim().is_empty()).collect(),
    })
}
```

- [ ] **步骤 4：注册命令**

在 `src-tauri/src/ui/mod.rs` 添加：

```rust
pub mod service_probe;
```

在 `src-tauri/src/lib.rs` import：

```rust
service_probe::{list_service_models, validate_service_credential},
```

在 `generate_handler!` 添加：

```rust
validate_service_credential,
list_service_models,
```

- [ ] **步骤 5：运行后端探测测试验证通过**

运行：

```bash
cd src-tauri && cargo test service_probe
```

预期：PASS。

- [ ] **步骤 6：前端封装 Tauri 命令**

在 `frontend/src/lib/tauri.ts` 添加：

```ts
export interface ServiceProbeRequest {
  protocol: string;
  endpoint: string;
  apiKey: string | null;
}

export async function invokeValidateServiceCredential(request: ServiceProbeRequest): Promise<void> {
  return requireInvoke()<void>('validate_service_credential', { request });
}

export async function invokeListServiceModels(request: ServiceProbeRequest): Promise<{ models: string[] }> {
  return requireInvoke<{ models: string[] }>('list_service_models', { request });
}
```

- [ ] **步骤 7：替换 ServicesPanel mock 逻辑**

在 `ServicesPanel.vue` 移除 `MOCK_PULLED_MODELS` import。

新增 helper：

```ts
const probeRequest = (inst: ServiceInstance) => ({
  protocol: inst.protocol,
  endpoint: inst.endpoint,
  apiKey: inst.apiKey.trim() || null,
})
```

把 `onKeyValidate` 改为：

```ts
const onKeyValidate = async (key: string): Promise<void> => {
  const inst = activeInstance.value
  if (!inst) return
  if (!key.trim()) {
    inst.keyStatus = 'invalid'
    toast.error('校验失败', '请先输入 API Key')
    return
  }
  if (inst.keyStatus === 'validating') return
  inst.keyStatus = 'validating'
  try {
    await invokeValidateServiceCredential(probeRequest(inst))
    inst.keyStatus = 'valid'
    toast.success('校验通过', `${inst.name} API Key 可用`)
  } catch (e) {
    inst.keyStatus = 'invalid'
    toast.error('校验失败', String(e))
  }
}
```

把 `onPullModels` 改为：

```ts
const onPullModels = async (instanceId: string): Promise<void> => {
  if (pulling.value[instanceId]) return
  const inst = props.state.services.find((s) => s.id === instanceId)
  if (!inst) return
  pulling.value[instanceId] = true
  try {
    const { models } = await invokeListServiceModels(probeRequest(inst))
    inst.pulledModels = Array.from(new Set([...inst.pulledModels, ...models]))
    if (models.length === 0) toast.info('未发现模型', '服务返回了空模型列表')
  } catch (e) {
    toast.error('拉取模型失败', String(e))
  } finally {
    pulling.value[instanceId] = false
  }
}
```

- [ ] **步骤 8：运行前端类型检查**

运行：

```bash
npm run typecheck
```

预期：PASS。

- [ ] **步骤 9：Commit**

```bash
git add src-tauri/src/ui/service_probe.rs src-tauri/src/ui/mod.rs src-tauri/src/lib.rs frontend/src/lib/tauri.ts frontend/src/settings/panels/ServicesPanel.vue
git commit -m "feat(services): 接通服务校验与模型拉取"
```

---

### 任务 4：接通自动复制、剪贴板恢复与 OCR 历史

**文件：**
- 修改：`src-tauri/src/core/selection/mod.rs`
- 修改：`src-tauri/src/app/shortcuts.rs`
- 修改：`frontend/public/translate.js`
- 修改：`frontend/src/settings/stores/settings.ts`
- 修改：`frontend/src/settings/stores/settings.test.ts`

- [ ] **步骤 1：编写失败的剪贴板恢复单测**

在 `src-tauri/src/core/selection/mod.rs` 中把快照恢复提取为纯函数：

```rust
fn should_restore_clipboard(restore_clipboard: bool, snapshot: &Option<String>) -> bool {
    restore_clipboard && snapshot.is_some()
}
```

先写测试：

```rust
#[test]
fn restore_clipboard_flag_controls_restore() {
    assert!(should_restore_clipboard(true, &Some("old".to_string())));
    assert!(!should_restore_clipboard(false, &Some("old".to_string())));
    assert!(!should_restore_clipboard(true, &None));
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
cd src-tauri && cargo test restore_clipboard_flag_controls_restore
```

预期：FAIL，函数不存在。

- [ ] **步骤 3：实现可配置剪贴板恢复**

把 `copy_selected_text` 签名改为：

```rust
pub fn copy_selected_text(restore_clipboard: bool) -> Result<String, SelectionError> {
```

把末尾恢复逻辑改为：

```rust
if should_restore_clipboard(restore_clipboard, &snapshot) {
    clipboard::restore_text_snapshot(snapshot);
}
```

在 `handle_selection_translate` 中先读取 config：

```rust
let restore_clipboard = app_handle
    .state::<AppState>()
    .config_store
    .get()
    .map(|config| config.restore_clipboard)
    .unwrap_or(true);
let selected_text = match copy_selected_text(restore_clipboard) {
```

- [ ] **步骤 4：运行剪贴板测试验证通过**

运行：

```bash
cd src-tauri && cargo test restore_clipboard_flag_controls_restore
```

预期：PASS。

- [ ] **步骤 5：修改 OCR 历史默认值测试**

在 `frontend/src/settings/stores/settings.test.ts` 增加：

```ts
it('首次启动 OCR 历史为空，不再展示样本数据', () => {
  window.localStorage.clear();
  const settings = useSettings();

  expect(settings.state.ocrHistory).toEqual([]);
});
```

- [ ] **步骤 6：运行测试验证失败**

运行：

```bash
npm run test -- frontend/src/settings/stores/settings.test.ts
```

预期：FAIL，当前默认会生成样本历史。

- [ ] **步骤 7：移除默认 OCR 样本**

在 `settings.ts` 删除 `seedOcrHistory` 样本生成逻辑，改为：

```ts
const seedOcrHistory = (): OcrHistoryEntry[] => []
```

- [ ] **步骤 8：弹窗读取配置并自动复制主结果**

在 `frontend/public/translate.js` 添加运行配置：

```js
let runtimeConfig = {
  autoCopy: false,
  defaultSourceLang: 'auto',
  targetLang: '中文',
};
let copiedBatchId = null;
let currentSourceType = null;
let currentSourceText = '';
```

在 `initCards` 读取配置后赋值：

```js
runtimeConfig.autoCopy = Boolean(config.autoCopy);
runtimeConfig.defaultSourceLang = config.defaultSourceLang || 'auto';
runtimeConfig.targetLang = config.targetLang || '中文';
```

在 `started` 事件中记录：

```js
currentSourceType = payload.sourceType;
currentSourceText = payload.sourceText ?? sourceText.value;
copiedBatchId = null;
```

在 `allFinished` 分支复制主结果：

```js
if (runtimeConfig.autoCopy && copiedBatchId !== currentBatchId) {
  const firstFinished = Array.from(resultCards.values()).find(c => c.status === 'finished' && c.text.textContent.trim());
  if (firstFinished) {
    copiedBatchId = currentBatchId;
    copyText(firstFinished.text.textContent, { classList: { add() {}, remove() {} } });
  }
}
```

为避免假按钮对象，实际实现时把 `copyText` 拆成：

```js
function writeClipboard(text) {
  return navigator.clipboard.writeText(text);
}
```

按钮复制继续调用 `writeClipboard`，自动复制直接调用 `writeClipboard`。

- [ ] **步骤 9：OCR 完成时写入真实历史**

在 `translate.js` 添加：

```js
const SETTINGS_KEY = 'app:settings:v1';

function addOcrHistory(payload, text) {
  if (currentSourceType !== 'ocrText') return;
  try {
    const raw = localStorage.getItem(SETTINGS_KEY);
    const state = raw ? JSON.parse(raw) : {};
    const history = Array.isArray(state.ocrHistory) ? state.ocrHistory : [];
    history.unshift({
      id: `hist-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`,
      timestamp: new Date().toISOString(),
      sourceLang: runtimeConfig.defaultSourceLang || 'auto',
      targetLang: runtimeConfig.targetLang || '中文',
      source: currentSourceText,
      translation: text,
      serviceInstanceId: payload.serviceInstanceId,
    });
    const limit = Math.max(1, Number(state.translation?.historyLimit || 500));
    state.ocrHistory = history.slice(0, limit);
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(state));
  } catch {
    showToast('OCR 历史写入失败');
  }
}
```

在 `finished` 事件中调用：

```js
addOcrHistory(payload, card.text.textContent);
```

- [ ] **步骤 10：运行前后端验证**

运行：

```bash
cd src-tauri && cargo test restore_clipboard_flag_controls_restore
npm run test -- frontend/src/settings/stores/settings.test.ts
npm run typecheck
```

预期：全部 PASS。

- [ ] **步骤 11：Commit**

```bash
git add src-tauri/src/core/selection/mod.rs src-tauri/src/app/shortcuts.rs frontend/public/translate.js frontend/src/settings/stores/settings.ts frontend/src/settings/stores/settings.test.ts
git commit -m "feat(translate): 接通复制行为与 OCR 历史"
```

---

### 任务 5：删除或置灰不该继续承诺的 UI 入口

**文件：**
- 修改：`frontend/public/translate.html`
- 修改：`frontend/public/translate.js`
- 修改：`frontend/public/translate.css`
- 修改：`frontend/src/settings/panels/TranslatePanel.vue`
- 修改：`frontend/src/settings/panels/ShortcutPanel.vue`
- 修改：`frontend/src/settings/panels/AdvancedPanel.vue`

- [ ] **步骤 1：先取得用户确认**

确认要执行的收口范围：

```text
推荐执行：
- 删除翻译弹窗收藏、书签按钮
- 语言栏保留但只读
- 隐藏自动粘贴、音标、备选翻译、取词延迟、取词快捷键、取词增强、语音输入
```

没有确认时，只能做计划一到任务 4，不能改这些 UI 入口。

- [ ] **步骤 2：删除翻译弹窗无后端能力按钮**

在 `translate.html` 删除：

```html
<button class="toolbar-btn" id="favBtn" title="收藏">...</button>
<button class="toolbar-btn" id="bookmarkBtn" title="书签">...</button>
```

在 `translate.js` 删除：

```js
const favBtn = document.getElementById('favBtn');
const bookmarkBtn = document.getElementById('bookmarkBtn');
function toggleFav() { ... }
favBtn.addEventListener('click', toggleFav);
bookmarkBtn.addEventListener('click', () => showToast('功能开发中'));
```

- [ ] **步骤 3：语言栏改只读**

在 `translate.html` 把三个 button 改成非交互元素：

```html
<div class="lang-side" id="langSource" aria-label="源语言">
  <span class="lang-label">自动检测</span>
</div>
<div class="lang-swap" id="langSwap" aria-hidden="true">...</div>
<div class="lang-side" id="langTarget" aria-label="目标语言">
  <span class="lang-label">简体中文</span>
</div>
```

在 `translate.js` 删除：

```js
langSource.addEventListener('click', () => showToast('功能开发中'));
langSwap.addEventListener('click', () => showToast('功能开发中'));
langTarget.addEventListener('click', () => showToast('功能开发中'));
```

并在读取 `runtimeConfig` 后更新标签：

```js
langSource.querySelector('.lang-label').textContent = runtimeConfig.defaultSourceLang === 'auto' ? '自动检测' : runtimeConfig.defaultSourceLang;
langTarget.querySelector('.lang-label').textContent = runtimeConfig.targetLang || '中文';
```

- [ ] **步骤 4：隐藏设置页未实现入口**

在 `TranslatePanel.vue` 隐藏这些 `SettingRow`：

```vue
<!-- 自动粘贴、显示音标、显示备选翻译、取词延迟先不渲染 -->
```

实现时直接删除对应 `SettingRow`，不要保留注释块。

在 `ShortcutPanel.vue` 过滤 `word-lookup`：

```vue
v-for="binding in state.shortcut.bindings.filter((b) => b.id !== 'word-lookup')"
```

在 `AdvancedPanel.vue` 删除取词增强和语音输入两行。

- [ ] **步骤 5：运行类型检查**

运行：

```bash
npm run typecheck
```

预期：PASS。

- [ ] **步骤 6：Commit**

```bash
git add frontend/public/translate.html frontend/public/translate.js frontend/public/translate.css frontend/src/settings/panels/TranslatePanel.vue frontend/src/settings/panels/ShortcutPanel.vue frontend/src/settings/panels/AdvancedPanel.vue
git commit -m "refactor(ui): 收口未接通功能入口"
```

---

### 任务 6：接通高级页配置导入导出

**文件：**
- 创建：`frontend/src/settings/config-io.ts`
- 创建：`frontend/src/settings/config-io.test.ts`
- 修改：`frontend/src/settings/panels/AdvancedPanel.vue`

- [ ] **步骤 1：编写失败的配置导入导出测试**

创建 `frontend/src/settings/config-io.test.ts`：

```ts
import { describe, expect, it } from 'vitest'
import { exportSettings, importSettings } from './config-io'
import type { AppSettings } from './types'

const state = {
  services: [
    { id: 'svc-1', apiKey: 'sk-live', name: 'DeepSeek' },
  ],
  translation: { defaultTargetLang: '中文' },
} as unknown as AppSettings

describe('config-io', () => {
  it('导出配置时剔除 API Key', () => {
    const exported = exportSettings(state)

    expect(exported.services[0].apiKey).toBe('')
  })

  it('导入配置时保留本地 API Key', () => {
    const incoming = exportSettings(state)
    incoming.services[0].name = '改名'

    const merged = importSettings(state, incoming)

    expect(merged.services[0].name).toBe('改名')
    expect(merged.services[0].apiKey).toBe('sk-live')
  })
})
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
npm run test -- frontend/src/settings/config-io.test.ts
```

预期：FAIL，模块不存在。

- [ ] **步骤 3：实现纯函数**

创建 `frontend/src/settings/config-io.ts`：

```ts
import type { AppSettings } from './types'

const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value)) as T

export function exportSettings(state: AppSettings): AppSettings {
  const exported = clone(state)
  exported.services = exported.services.map((service) => ({ ...service, apiKey: '' }))
  return exported
}

export function importSettings(current: AppSettings, incoming: AppSettings): AppSettings {
  const localKeys = new Map(current.services.map((service) => [service.id, service.apiKey]))
  const merged = clone(incoming)
  merged.services = merged.services.map((service) => ({
    ...service,
    apiKey: localKeys.get(service.id) ?? service.apiKey ?? '',
  }))
  return merged
}
```

- [ ] **步骤 4：接入 AdvancedPanel**

在 `AdvancedPanel.vue` 引入：

```ts
import { exportSettings, importSettings } from '../config-io'
```

把 `defineProps` 改成可使用 props：

```ts
const props = defineProps<{ state: AppSettings }>()
```

新增导出方法：

```ts
const exportConfig = (): void => {
  const blob = new Blob([JSON.stringify(exportSettings(props.state), null, 2)], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = 'shizi-settings.json'
  a.click()
  URL.revokeObjectURL(url)
  exportOpen.value = false
}
```

新增导入方法：

```ts
const importConfig = async (file: File): Promise<void> => {
  const text = await file.text()
  const incoming = JSON.parse(text) as AppSettings
  Object.assign(props.state, importSettings(props.state, incoming))
  importOpen.value = false
}
```

模板中增加：

```vue
<Button @click="exportConfig">导出 JSON</Button>
<input
  type="file"
  accept="application/json"
  @change="(e) => {
    const file = (e.target as HTMLInputElement).files?.[0]
    if (file) importConfig(file)
  }"
/>
```

- [ ] **步骤 5：运行前端测试与类型检查**

运行：

```bash
npm run test -- frontend/src/settings/config-io.test.ts
npm run typecheck
```

预期：PASS。

- [ ] **步骤 6：Commit**

```bash
git add frontend/src/settings/config-io.ts frontend/src/settings/config-io.test.ts frontend/src/settings/panels/AdvancedPanel.vue
git commit -m "feat(settings): 接通配置导入导出"
```

---

### 任务 7：整体验证与文档同步

**文件：**
- 修改：`docs/roadmap/progressive-development-plan.md`
- 修改：`docs/architecture/ui-decoupling-proposal.md`

- [ ] **步骤 1：运行完整前端验证**

运行：

```bash
npm run test
npm run typecheck
npm run build
```

预期：全部 exit 0。

- [ ] **步骤 2：运行完整后端验证**

运行：

```bash
cd src-tauri && cargo test
cd src-tauri && cargo build
```

预期：全部 exit 0。

- [ ] **步骤 3：同步文档**

在 `docs/roadmap/progressive-development-plan.md` 增加当前能力状态：

```markdown
- 设置页服务模块：API Key 校验和模型拉取已接通真实服务接口。
- 翻译行为：自动复制结果、划词剪贴板恢复、OCR 历史已接通。
- 未接通入口已从 UI 隐藏或降级为只读展示。
```

在 `docs/architecture/ui-decoupling-proposal.md` 增加：

```markdown
翻译弹窗继续保持静态页面，只通过 `get_app_config` 读取运行配置，并把 OCR 历史写回本地设置存储；复杂设置编辑仍由 Vue 设置页负责。
```

- [ ] **步骤 4：Commit**

```bash
git add docs/roadmap/progressive-development-plan.md docs/architecture/ui-decoupling-proposal.md
git commit -m "docs: 同步 UI 能力接通状态"
```

---

## 自检

- 规格覆盖度：已覆盖当前 UI 中明确展示且未接通的服务校验、模型拉取、提示词、思维链、自动复制、剪贴板恢复、OCR 历史、弹窗假按钮、取词/语音类入口、导入导出。
- 范围控制：不实现取词、语音输入、自动更新、开机启动、自定义 OCR 服务、收藏库、术语库、反思二次调用。
- 类型一致性：前后端新增字段统一使用 camelCase 序列化；Rust 内部字段使用 snake_case。
- 删除候选：任务 5 必须先取得用户确认再执行。

## 执行建议

推荐先执行任务 1 到任务 4，得到最小可用增量；任务 5 需用户确认删除/隐藏范围；任务 6 可以独立执行；任务 7 是收尾硬门禁。
