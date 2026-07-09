# 微软翻译渠道与 Provider 抽象层重构 设计规格

- 日期：2026-07-09
- 状态：已确认，待实现
- 关联：
  - [service-protocol-batch-translation-design](./2026-07-03-service-protocol-batch-translation-design.md)（`services[]` 事实来源、`provider_for_service` 分发、批次翻译，本设计扩展其协议集与 provider 抽象）
  - [translation-popup-dropdown-and-lang-detection-design](./2026-07-08-translation-popup-dropdown-and-lang-detection-design.md)（`detectedSourceLang` 回传链路，本设计将其通用化为 provider 事件）
  - [claude-provider-design](./2026-07-01-claude-provider-design.md)（现有 `LlmProvider` trait 与流式 provider 实现，本设计将其重命名为通用 `TranslationProvider`）

## 1. 目的

为翻译链路新增「微软翻译」渠道（Edge 浏览器默认翻译引擎，免 Key 机器翻译），并借此机会在 provider 层引入 LLM / 机器翻译平级的通用抽象：把当前 LLM 专用的 `LlmProvider` 重构为通用的 `TranslationProvider`，让大模型翻译与机器翻译成为同一抽象下的平级实现，同时统一处理「流式」与「非流式」两种响应形态。

## 2. 范围

### 范围内

- provider 抽象层重构：`LlmProvider` -> `TranslationProvider`、`LlmStreamEvent` -> `TranslationStreamEvent`（新增 `DetectedSourceLang` 变体）、`LlmError` -> `TranslationError`
- 引入非流式抽象 `BatchTranslateProvider` + `StreamingAdapter`，让非流式 provider（机器翻译、未来非流式 LLM）实现极简，适配为高层流式接口
- auto 源语言检测的首行解析逻辑从 `TranslationService` 下沉到 LLM provider 内部（共享 `AutoLangHeaderParser`）
- `TranslationRequest` 通用化：`source_lang` 提升到 request 顶层
- 新增 `MicrosoftMtProvider`（`impl BatchTranslateProvider`），对接 Edge `translatetext` 接口
- WebView 初始化采集 `navigator.userAgent` / `navigator.languages`，经新 command 存 `AppState`，Rust 拼装请求时派生 `sec-mesh-client-*` / `sec-ch-ua` 等浏览器环境头
- 前端新增 `microsoft` 渠道与 `microsoft_edge` 协议；服务详情页对免 Key 机器翻译渠道精简为「标题 + 删除」
- 配置适配：`is_configured` / `normalized` 放行微软翻译
- 单元测试：解析器、语言映射、UA 解析、provider 分发、适配器事件顺序、配置模型

### 范围外（YAGNI）

- 其他机器翻译渠道（DeepL / Google / 百度 / 有道 / 腾讯 / 火山 / 讯飞）的真实对接--仅建立 `core/mt/` 抽象与微软翻译一个实现，其余渠道保持 `protocols: []`（开发中）
- 非流式 LLM provider 的真实实现--仅提供 `BatchTranslateProvider` 抽象与适配器供未来复用，不新增非流式 LLM 实现
- Edge 接口的 token / 用量统计（机器翻译无 token 概念，`TranslationResult.usage` 恒为 `None`）
- 批量翻译多行（接口入参虽为数组，但本版按用户决策「整段作为单元素」，不按行 / 按段拆分）
- 请求头环境信息持久化（进程级内存，每次启动重新采集）
- API Key 加密存储迁移（仍维持 MVP 明文策略）

## 3. 架构

### 3.1 分层总览

重构后 provider 层从「LLM 专用」变为「LLM / ML 平级」：

```
前后端交互层（通用，不变）
  TranslationEvent（Started/Delta/Finished/Failed/Cancelled） · commands · services[]
        │
        ▼
TranslationService（中立，只透传事件，不再做 LLM 首行解析）
        │  调用 TranslationProvider::translate(on_event)
        ▼
Provider 抽象层（通用，core/translation/provider.rs）
  trait TranslationProvider           ← service 唯一依赖
  trait BatchTranslateProvider        ← 非流式简化实现
  struct StreamingAdapter<T>          ← Batch -> Streaming 适配
        │
        ├── core/llm/（流式 LLM，impl TranslationProvider）
        │     OpenAiCompatibleProvider · ClaudeProvider · MockLlmProvider
        │     （内部用 AutoLangHeaderParser 解析 auto 首行）
        │
        └── core/mt/（非流式 ML，impl BatchTranslateProvider）
              MicrosoftMtProvider
              （translate_once 一次性请求，返回 TranslationResult）
```

`TranslationService` 完全中立：不区分 LLM / ML，不区分流式 / 非流式，只把 `TranslationStreamEvent` 透传成 `TranslationEvent`。LLM 流式体验不变（逐 chunk `Delta`）；ML 非流式经 `StreamingAdapter` 一次性发 `Delta`。

### 3.2 目录结构

```
src-tauri/src/core/translation/
  mod.rs
  provider.rs      新增：TranslationProvider trait + TranslationStreamEvent + TranslationError + StreamingAdapter + BatchTranslateProvider + TranslationResult（从 core/llm/provider.rs 移并重命名）
  auto_lang.rs     新增：AutoLangHeaderParser（从 service.rs 抽出的首行解析状态机）
  service.rs       简化：移除 HeaderParseState/process_auto_delta/extract_source_lang，只透传事件
  types.rs         TranslationRequest 增 source_lang 顶层字段；TranslationPromptConfig 去掉 source_lang
  batch.rs         build_batch_requests 改为填 request.source_lang
  protocol.rs      新增：provider_for_service（从 core/llm/protocol.rs 移入，跨 llm/mt 分发）
src-tauri/src/core/llm/
  mod.rs           导出调整：移除 Llm* 旧名，re-export TranslationProvider 等
  openai_compatible.rs  impl TranslationProvider，内部接 AutoLangHeaderParser
  claude.rs             impl TranslationProvider，内部接 AutoLangHeaderParser
  mock.rs               impl TranslationProvider，内部接 AutoLangHeaderParser（或直接发 DetectedSourceLang）
src-tauri/src/core/mt/
  mod.rs           新增：EdgeTranslateEnv 定义与 re-export
  microsoft.rs     新增：MicrosoftMtProvider（impl BatchTranslateProvider）
```

`core/llm/provider.rs` 与 `core/llm/protocol.rs` 删除（内容分别移至 `core/translation/provider.rs` 与 `core/translation/protocol.rs`）。

### 3.3 设计决策

| 决策 | 选择 | 理由 |
|---|---|---|
| 抽象形态 | 单一高层 `TranslationProvider` trait + 非流式 `BatchTranslateProvider` + `StreamingAdapter` 适配 | service 只依赖一个高层接口，保持中立；非流式 provider 实现极简且事件顺序统一；未来非流式 LLM 可复用 `BatchTranslateProvider` |
| auto 检测回传 | `TranslationStreamEvent::DetectedSourceLang(String)` 事件 | 替代原 service 层 `【源语言：xxx】` 首行解析；LLM 在 provider 内部解析后发事件，ML 从响应 `detectedLanguage` 填 `TranslationResult`；service 统一处理 |
| auto 解析位置 | 下沉到 LLM provider（共享 `AutoLangHeaderParser`） | service 中立，不再 LLM-centric；LLM provider 仍按 `request.user_prompt()` 输出首行标记（prompt 逻辑不变），仅解析点移动 |
| `source_lang` 位置 | 从 `TranslationPromptConfig` 提升到 `TranslationRequest` 顶层 | source_lang 是通用字段（ML 也要 `from`）；prompts 只留 LLM 专用字段，ML provider 忽略 prompts |
| 流式 / 非流式 | 高层统一流式接口；非流式走 `BatchTranslateProvider` + `StreamingAdapter` | 前端流式渲染体验不变；ML 不必模拟流式 on_event 调用；事件顺序由适配器保证 |
| 微软翻译接口入参 | 整段文本作为数组单元素 `["全文"]` | 用户决策：保留跨行上下文，翻译质量优先；接口的批量能力暂不利用 |
| 浏览器环境头采集 | 前端采 `navigator.userAgent` + `navigator.languages`，后端从 UA 派生其余 | `sec-mesh-client-*` 是浏览器内部头，JS 读不到；UA 已含 Edg 版本 / Windows NT 版本 / 架构，后端解析派生最简可靠 |
| 环境头保存位置 | `AppState` 进程级内存，不持久化 | 运行时环境信息非用户配置；UA 随 Edge 更新变化，持久化易过时；每次启动重新采集 |
| 协议 id | `microsoft_edge` | 与 `openai_chat` / `claude_messages` / `mock` 同级；前后端统一 |
| 详情页精简判断 | `activeService.id === 'microsoft'` | YAGNI，不新增 `ServiceMeta` 字段；后续 OpenDesign 重做详情页时再通用化 |

## 4. Provider 抽象层

### 4.1 通用 trait 与事件（`core/translation/provider.rs`）

```rust
#[derive(Debug, thiserror::Error)]
pub enum TranslationError {
    #[error("缺少配置 {0}")]
    MissingConfig(&'static str),
    #[error("HTTP 请求失败：{0}")]
    Http(String),
    #[error("服务返回错误：{message}")]
    Api { message: String, retryable: bool },
    #[error("响应解析失败：{0}")]
    Parse(String),
}

impl TranslationError {
    pub fn retryable(&self) -> bool {
        match self {
            Self::MissingConfig(_) | Self::Parse(_) => false,
            Self::Http(_) => true,
            Self::Api { retryable, .. } => *retryable,
        }
    }
}

/// provider 向 service 输出的流事件。
/// - Delta：译文增量（流式逐 chunk，或非流式一次性）
/// - Usage：token 用量（仅 LLM 发，ML 恒不发）
/// - DetectedSourceLang：auto 检测到的源语言（LLM 首行解析后发，ML 从响应填）
#[derive(Debug)]
pub enum TranslationStreamEvent {
    Delta(String),
    Usage(TokenUsage),
    DetectedSourceLang(String),
}

#[async_trait::async_trait]
pub trait TranslationProvider: Send + Sync {
    async fn translate(
        &self,
        request: &TranslationRequest,
        on_event: &mut (dyn FnMut(TranslationStreamEvent) + Send),
        cancel: &CancellationToken,
    ) -> Result<(), TranslationError>;
}
```

### 4.2 非流式 trait 与适配器

```rust
/// 非流式 provider 的一次性翻译结果。
pub struct TranslationResult {
    pub text: String,
    pub usage: Option<TokenUsage>,
    pub detected_source_lang: Option<String>,
}

/// 非流式翻译 provider（机器翻译、未来非流式 LLM）。只返回完整结果，不接触 on_event。
#[async_trait::async_trait]
pub trait BatchTranslateProvider: Send + Sync {
    async fn translate_once(
        &self,
        request: &TranslationRequest,
        cancel: &CancellationToken,
    ) -> Result<TranslationResult, TranslationError>;
}

/// 把 BatchTranslateProvider 适配为 TranslationProvider。
/// 事件顺序：取消检查 -> DetectedSourceLang -> Delta -> Usage。
pub struct StreamingAdapter<T>(pub T);

#[async_trait::async_trait]
impl<T: BatchTranslateProvider> TranslationProvider for StreamingAdapter<T> {
    async fn translate(
        &self,
        request: &TranslationRequest,
        on_event: &mut (dyn FnMut(TranslationStreamEvent) + Send),
        cancel: &CancellationToken,
    ) -> Result<(), TranslationError> {
        if cancel.is_cancelled() {
            return Ok(());
        }
        let result = self.0.translate_once(request, cancel).await?;
        if let Some(lang) = result.detected_source_lang {
            on_event(TranslationStreamEvent::DetectedSourceLang(lang));
        }
        on_event(TranslationStreamEvent::Delta(result.text));
        if let Some(usage) = result.usage {
            on_event(TranslationStreamEvent::Usage(usage));
        }
        Ok(())
    }
}
```

### 4.3 auto 首行解析下沉（`core/translation/auto_lang.rs`）

把 `service.rs` 现有的 `HeaderParseState` / `process_auto_delta` / `extract_source_lang` 抽为独立状态机：

```rust
/// source=auto 时的首行 `【源语言：xxx】` 解析状态机，供 LLM provider 复用。
pub struct AutoLangHeaderParser {
    pending: String,
    parsed: bool,
    detected: Option<String>,
}

impl AutoLangHeaderParser {
    pub fn new() -> Self { ... }
    /// 喂入一段 delta，返回 (本次可输出的纯译文片段, 是否已解析到语言)。
    /// 非 auto 场景 provider 不应使用此 parser（直通 Delta）。
    pub fn feed(&mut self, delta: &str) -> Vec<String> { ... }
    /// 流结束后：若首行未解析且 pending 非空，作为译文补出；返回检测到的语言。
    pub fn finish(&mut self) -> (Vec<String>, Option<String>) { ... }
}
```

OpenAI / Claude provider 在 `translate()` 内部：`is_auto = request.source_lang == "auto"`；auto 时每个 SSE delta 经 `parser.feed()`，解析出语言后发一次 `DetectedSourceLang`，纯译文片段发 `Delta`；流末 `parser.finish()` 补出残留译文。这两个 provider 的 prompt 逻辑（`request.user_prompt()` 在 auto 时追加检测指令，LLM 仍输出 `【源语言：xxx】\n译文`）不变，仅解析点从 service 移到 provider。

Mock provider 不调真实 LLM、不走 SSE 与 parser：auto 时直接发 `DetectedSourceLang("英语")` + `Delta(纯译文)`，不输出标记文本；非 auto 直接发 `Delta`。其单测相应调整（不再断言输出 `【源语言：】` 首行，改为断言发 `DetectedSourceLang` 事件）。

### 4.4 `TranslationRequest` 通用化（`core/translation/types.rs`）

```rust
pub struct TranslationRequest {
    pub session_id: TranslationSessionId,
    pub input: TranslationInput,
    pub source_lang: String,          // 新增：从 prompts 提升，通用
    pub target_lang: String,
    pub service: TranslationServiceMeta,
    pub prompts: TranslationPromptConfig,
}

pub struct TranslationPromptConfig {
    // 移除 source_lang（已提升到 request 顶层）
    pub system_prompt: String,
    pub translation_prompt: String,
    pub chain_of_thought: String,
}
```

`TranslationRequest::user_prompt()` 等方法内部把 `self.prompts.source_lang` 的引用改为 `self.source_lang`。`build_batch_requests` 改为 `source_lang: source_lang.clone()` 填 request 顶层，`TranslationPromptConfig` 不再带 source_lang。

ML provider 只用 `request.source_lang` / `request.target_lang` / `request.source_text()`，忽略 `request.prompts`。

### 4.5 `TranslationService` 中立化（`core/translation/service.rs`）

```rust
pub async fn translate_with<F>(&self, request, collect_usage, cancel, mut emit: F) -> Result<(), TranslationError>
where F: FnMut(TranslationEvent) + Send,
{
    let full_text = Arc::new(Mutex::new(String::new()));
    let usage = Arc::new(Mutex::new(None));
    let detected = Arc::new(Mutex::new(None));

    self.provider.translate(&request, &mut |ev| match ev {
        TranslationStreamEvent::Delta(text) => {
            // 累积 full_text + emit TranslationEvent::Delta
        }
        TranslationStreamEvent::Usage(u) => {
            if collect_usage { *usage.lock() = Some(u); }
        }
        TranslationStreamEvent::DetectedSourceLang(lang) => {
            *detected.lock() = Some(lang);
        }
    }, &cancel).await?;

    // 取消 -> emit Cancelled；否则 emit Finished { full_text, usage, detected_source_lang: detected }
}
```

移除 `HeaderParseState` / `process_auto_delta` / `extract_source_lang` 及 `is_auto` 分支。`TranslationService::new(provider: Arc<dyn TranslationProvider>)` 签名不变（trait 改名）。

### 4.6 provider 分发（`core/translation/protocol.rs`）

```rust
#[derive(Debug)]
pub enum ProviderKind { OpenAiCompatible, Claude, Mock, Microsoft }  // 新增 Microsoft

pub fn protocol_to_kind(protocol: &str) -> Result<ProviderKind, String> {
    match protocol {
        "openai_chat" => Ok(ProviderKind::OpenAiCompatible),
        "claude_messages" => Ok(ProviderKind::Claude),
        "mock" => Ok(ProviderKind::Mock),
        "microsoft_edge" => Ok(ProviderKind::Microsoft),           // 新增
        other => Err(format!("未支持的协议：{other}")),
    }
}

pub fn provider_for_service(
    config: &ServiceInstanceConfig,
    env: Option<&EdgeTranslateEnv>,                                 // 新增参数
) -> Result<Arc<dyn TranslationProvider>, String> {
    match protocol_to_kind(&config.protocol)? {
        ProviderKind::Mock => Ok(Arc::new(MockLlmProvider)),
        ProviderKind::Claude => Ok(Arc::new(ClaudeProvider::new(ClaudeConfig { ... }))),
        ProviderKind::OpenAiCompatible => Ok(Arc::new(OpenAiCompatibleProvider::new(...))),
        ProviderKind::Microsoft => Ok(Arc::new(StreamingAdapter(
            MicrosoftMtProvider::new(env.cloned().unwrap_or_default()),
        ))),
    }
}
```

## 5. MicrosoftMtProvider（`core/mt/microsoft.rs`）

### 5.1 EdgeTranslateEnv

```rust
// core/mt/mod.rs
/// WebView 初始化时采集的浏览器环境信息，供 MicrosoftMtProvider 拼装请求头。
#[derive(Debug, Clone, Default)]
pub struct EdgeTranslateEnv {
    pub user_agent: String,
    pub accept_language: String,   // 如 "zh-CN,zh;q=0.9,en;q=0.8"
}
```

编译期默认 UA 兜底常量（当前 Edge 稳定版 UA 字符串），`unwrap_or_default()` 在 env 未采集到时使用。

```rust
// core/mt/microsoft.rs
pub struct MicrosoftMtProvider {
    client: reqwest::Client,
    env: EdgeTranslateEnv,
}

impl MicrosoftMtProvider {
    pub fn new(env: EdgeTranslateEnv) -> Self {
        Self { client: reqwest::Client::new(), env }
    }
}
```

### 5.2 语言映射

内部 code（`auto`/`zh-CN`/`zh-TW`/`en-US`/`ja-JP`/`ko-KR`/`fr-FR`/`de-DE`/`es-ES`/`ru-RU`） ↔ Edge code：

| 内部 | Edge from | Edge to |
|---|---|---|
| auto | 省略 `from`（自动检测） | - |
| zh-CN | zh-Hans | zh-Hans |
| zh-TW | zh-Hant | zh-Hant |
| en-US | en | en |
| ja-JP | ja | ja |
| ko-KR | ko | ko |
| fr-FR | fr | fr |
| de-DE | de | de |
| es-ES | es | es |
| ru-RU | ru | ru |

`map_source_lang`：auto -> `None`（省略 from）；其余 -> `Some(edge_code)`。`map_target_lang` -> `edge_code`。`detected_to_internal`：Edge `detectedLanguage.language`（如 `en`）反向映射回内部 code（如 `en-US`），供 `DetectedSourceLang`。

### 5.3 请求拼装

`POST https://edge.microsoft.com/translate/translatetext?to={to}&isEnterpriseClient=false`（非 auto 追加 `&from={from}`）

Body（整段单元素）：
```json
["整段文本"]
```

请求头三类：

| 类别 | 头 |
|---|---|
| 常量 | `accept: */*`、`content-type: application/json`、`origin: https://github.com`、`referer: https://github.com/`、`priority: u=1, i`、`sec-fetch-dest: empty`、`sec-fetch-mode: cors`、`sec-fetch-site: cross-site`、`sec-ch-ua-platform: "Windows"`、`sec-ch-ua-mobile: ?0`、`sec-mesh-client-os: Windows`、`sec-mesh-client-edge-channel: stable`、`sec-mesh-client-webview: 0`、`x-edge-shopping-flag: 0` |
| 来自 env | `user-agent`、`accept-language` |
| 从 UA 解析派生 | `sec-mesh-client-edge-version`（`Edg/x.x.x.x`）、`sec-mesh-client-os-version`（`Windows NT x.x.x`）、`sec-mesh-client-arch`（`Win64; x64` -> `x86_64`）、`sec-ch-ua`（由 `Chrome/` 与 `Edg/` 版本构造 `"Not;A=Brand";v="8", "Chromium";v="150", "Microsoft Edge";v="150"`） |

UA 解析为纯函数 `parse_edge_headers(ua: &str) -> EdgeHeaders`，单测覆盖。

### 5.4 translate_once 实现

```rust
#[async_trait]
impl BatchTranslateProvider for MicrosoftMtProvider {
    async fn translate_once(&self, request, cancel) -> Result<TranslationResult, TranslationError> {
        let text = request.source_text();
        let from = map_source_lang(&request.source_lang);
        let to = map_target_lang(&request.target_lang);
        let url = build_url(from.as_deref(), &to);
        let headers = build_headers(&self.env);   // 常量 + env + parse_edge_headers(ua)

        let req = self.client.post(url).body(serde_json::to_string(&[text]).unwrap()).headers(headers);
        let resp = tokio::select! {
            _ = cancel.cancelled() => return Ok(TranslationResult::default()),  // 取消返回空，service 发 Cancelled
            r = req.send() => r.map_err(|e| TranslationError::Http(e.to_string()))?,
        };

        // 状态码映射 -> TranslationError
        // 解析响应 [{ "translations":[{"text":"译文","to":"zh-Hans"}], "detectedLanguage":{"language":"en","score":1.0} }]
        let body: Vec<EdgeTranslation> = serde_json::from_slice(&resp.bytes().await...)
            .map_err(|e| TranslationError::Parse(e.to_string()))?;
        let first = body.into_iter().next().ok_or(TranslationError::Parse("响应数组为空".into()))?;

        let detected = if request.source_lang == "auto" {
            first.detected_language.map(|d| detected_to_internal(&d.language))
        } else { None };

        Ok(TranslationResult {
            text: first.translations.into_iter().next().map(|t| t.text).unwrap_or_default(),
            usage: None,                // 机器翻译无用量
            detected_source_lang: detected,
        })
    }
}
```

> 实现时用真实响应校验字段名（`translations` / `detectedLanguage`）。若 Edge 端点响应结构与 Azure Translator 不同，按实际调整反序列化结构体，spec 不锁定字段名。

## 6. UA / env 采集与注入

### 6.1 前端采集

`translate.html`（main 窗口）初始化时：

```js
const userAgent = navigator.userAgent;
const acceptLanguage = (navigator.languages ?? [navigator.language])
  .map((l, i) => i === 0 ? l : `${l};q=${(1 - i * 0.1).toFixed(1)}`)
  .join(',');
await window.__TAURI__.core.invoke('save_edge_translate_env', { userAgent, acceptLanguage });
```

复用 `translate.js` 现有 invoke 模式（`const invoke = window.__TAURI__?.core?.invoke;`，见 [translate.js:21](../../../frontend/public/translate.js#L21)），不直接内联 `window.__TAURI__`。采集失败（command 抛错）不阻塞，后端用默认 UA 兜底。`settings.html` 不采集（env 只用于翻译调用，main 窗口启动即采集）。

### 6.2 后端存储

`AppState` 新增：

```rust
edge_translate_env: Arc<Mutex<Option<EdgeTranslateEnv>>>,

pub fn set_edge_translate_env(&self, env: EdgeTranslateEnv) -> Result<(), String> { ... }
pub fn edge_translate_env(&self) -> Option<EdgeTranslateEnv> { ... }   // clone，锁毒化返回 None
```

新 command（注册到 `lib.rs` `invoke_handler`）：

```rust
#[tauri::command]
pub async fn save_edge_translate_env(
    user_agent: String,
    accept_language: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state.set_edge_translate_env(EdgeTranslateEnv { user_agent, accept_language })
}
```

### 6.3 注入调用点

[web_popup.rs](../../../src-tauri/src/ui/web_popup.rs) `start_translation_from_input` 内 `provider_for_service(&service_config)` 改为 `provider_for_service(&service_config, state.edge_translate_env().as_ref())`（`state` 在闭包外可取 clone）。

## 7. 协议与渠道注册

### 7.1 后端

`protocol_to_kind` / `ProviderKind` 新增 `microsoft_edge` / `Microsoft`（见 4.6）。

### 7.2 前端类型

`frontend/src/types/config.ts`：
```ts
export type ServiceProtocolId = 'openai_chat' | 'claude_messages' | 'microsoft_edge';
```

`frontend/src/settings/types.ts` `BuiltinServiceId` 新增 `'microsoft'`。

### 7.3 渠道元数据（`frontend/src/settings/tokens.ts`）

```ts
const MICROSOFT_EDGE = {
  id: 'microsoft_edge' as const,
  label: 'Edge 翻译',
  defaultEndpoint: 'https://edge.microsoft.com/translate/translatetext',
  defaultModel: '',
  editableEndpoint: false,
  status: 'available' as const,
};

// BUILTIN_SERVICES 新增
{
  id: 'microsoft',
  name: '微软翻译',
  description: 'Edge 浏览器默认翻译引擎，免 Key，复用浏览器环境信息调用。',
  builtin: true,
  defaultModel: '',
  iconifyId: 'simple-icons:microsoftedge',   // 候选，无则 Lucide 兜底
  category: 'ml',
  keyRequired: false,
  protocols: [MICROSOFT_EDGE],
},
```

`MOCK_PULLED_MODELS` 增 `microsoft: []`。

## 8. 配置适配

- `AppConfig::is_configured`（`#[cfg(test)]`）：`"mock" | "microsoft_edge" => true`，其余仍要求 `api_key.is_some()`。
- `ServiceInstanceConfig::normalized`：`microsoft_edge` 空 endpoint 填 `https://edge.microsoft.com/translate/translatetext`；`api_key` / `model` 留空忽略。
- 前端 `addService('microsoft')`：实例 `protocol='microsoft_edge'`、`endpoint=Edge URL`、`apiKey=''`、`model=''`（按 `ServiceProtocolMeta.defaultEndpoint` 填充，`editableEndpoint:false` 禁止编辑）。
- `frontend/src/settings/service-validation.ts` `validateServiceForEnable`：`keyRequired:false` 的渠道免 Key 校验放行（确认现有逻辑不拦 `apiKey` 为空）。

## 9. 前端详情页简化

[ServicesPanel.vue](../../../frontend/src/settings/panels/ServicesPanel.vue) 详情区（`v-else-if="activeInstance && activeService"` 分支）新增精简条件：

- `activeService.id === 'microsoft'` 时，只渲染 `<header>`（标题 + 描述）+ 底部「危险操作 / 删除实例」按钮。
- 隐藏：接入点（协议 + endpoint）、凭据（API Key）、模型、思维链、提示词、备注。
- 保留删除以维持可用性（否则添加后无法移除）。
- 判断先用 `id === 'microsoft'`（YAGNI），后续 OpenDesign 重做详情页时再通用化为 `ServiceMeta` 字段。

## 10. 错误处理

Edge 接口 HTTP 状态码 -> `TranslationError` 映射（与 LLM provider 一致）：

| 状态 | 映射 | retryable |
|---|---|---|
| 429 / 5xx | `TranslationError::Http(message)` | true |
| 401 / 403 / 400 | `TranslationError::Api { message, retryable: false }` | false |
| 响应 JSON 解析失败 | `TranslationError::Parse(error)` | false |
| 响应数组为空 | `TranslationError::Parse("响应数组为空")` | false |

取消：`tokio::select!` 包裹 reqwest future 与 `cancel.cancelled()`；取消时 `translate_once` 返回 `TranslationResult::default()`（空 text），`StreamingAdapter` 正常返回 `Ok(())`，`TranslationService` 检测 `cancel.is_cancelled()` 发 `Cancelled` 事件（与现有取消链路一致）。

## 11. 测试策略

### 11.1 单元测试（Rust）

- `AutoLangHeaderParser`：首行 `【源语言：xxx】` 解析、无标记直通、流末残留补出、多 chunk 拼接（从 service.rs 现有测试迁移并补充）。
- `parse_edge_headers`：从真实 UA 解析 edge-version / os-version / arch / sec-ch-ua；缺失字段兜底。
- 语言映射：`map_source_lang`（auto 省略）、`map_target_lang`、`detected_to_internal` 双向。
- `protocol_to_kind` / `provider_for_service`：`microsoft_edge` -> `Microsoft` 分支返回 `StreamingAdapter<MicrosoftMtProvider>`；未知协议仍报错。
- `StreamingAdapter`：事件顺序 `DetectedSourceLang -> Delta -> Usage`；`usage`/`detected` 为 `None` 时不发对应事件；取消时早退。
- `TranslationService` 透传：`DetectedSourceLang` 事件填入 `Finished.detected_source_lang`；LLM provider 不发该事件时为 `None`（不回归）。
- `MicrosoftMtProvider::translate_once`：用离线 fixture（构造 `EdgeTranslation` JSON 字符串，不真实联网）验证响应解析、auto 检测回填、错误状态码映射。
- `AppConfig::is_configured`：`microsoft_edge` + 无 key 放行；`normalized`：空 endpoint 填 Edge URL。
- 现有 LLM provider 测试：`MockLlmProvider` auto 行为调整后（解析下沉）仍输出 `DetectedSourceLang` + 译文；OpenAI/Claude 测试 trait 改名后通过。

### 11.2 前端测试

- `frontend/src/lib/config.test.ts`：`ServiceProtocolId` 含 `microsoft_edge`；序列化/反序列化。
- `frontend/src/settings/service-validation.test.ts`：`microsoft` 渠道（`keyRequired:false`）免 Key 放行。
- `npm run typecheck`（vue-tsc）：新类型与 `ServicesPanel.vue` 精简分支通过。
- `npm run test`（vitest）。

### 11.3 验证命令

```bash
cd src-tauri && cargo test
cd src-tauri && cargo build
npm run typecheck
npm run test
```

### 11.4 人工验证（Tauri dev）

- 设置页添加「微软翻译」，详情页只显示标题 + 删除按钮，启用开关可用。
- 翻译弹窗输入英文，微软翻译卡片流式（一次性）渲染译文；auto 源语言时 `.lang-badge` 显示检测到的语言。
- 切换源 / 目标语言（含 zh-CN / en-US / ja-JP）译文正确。
- 取消按钮中断微软翻译（复用取消链路）。
- Edge 接口异常（断网）-> `Failed` 事件正确显示，retryable=true。
- LLM 渠道（OpenAI / Claude / Mock）不回归：流式渲染、auto 检测 badge 仍正常。

## 12. 不向后兼容性

- `LlmProvider` / `LlmStreamEvent` / `LlmError` 重命名为 `TranslationProvider` / `TranslationStreamEvent` / `TranslationError`，是后端内部 trait 重命名，不影响 `config.json` 与前端 IPC（`TranslationEvent` 序列化不变）。
- `TranslationRequest` 增 `source_lang` 顶层字段、`TranslationPromptConfig` 去 `source_lang`：`TranslationRequest` 不参与序列化（仅后端内存传递），无持久化兼容问题。
- `ServiceInstanceConfig` 未增字段（`protocol` 已是字符串，`microsoft_edge` 自然支持），旧 `config.json` 平滑兼容。
- `protocol_to_kind` 未知协议仍报错（不静默走 OpenAI），存量 `openai_chat` / `claude_messages` / `mock` 协议不受影响。

## 13. 文档同步

- `CLAUDE.md` 与 `AGENTS.md`：
  - 「架构关键点 / 服务协议配置」协议列表补 `microsoft_edge`；provider 抽象层说明改为 `TranslationProvider`（LLM/ML 平级）+ `BatchTranslateProvider` 非流式适配。
  - 「前后端通信」command 列表补 `save_edge_translate_env`；`translation:event` 的 `Finished.detectedSourceLang` 回传机制说明改为 provider 事件。
  - 「核心层」目录结构补 `core/translation/provider.rs` / `auto_lang.rs` / `protocol.rs` 与 `core/mt/`。
- `README`：当前能力补「微软翻译（免 Key）」；机器翻译渠道说明。
- `roadmap`：微软翻译渠道与 provider 抽象层重构完成状态。
- `plugins.md`：本特性未新增插件/技能，无需同步。

## 14. 实现顺序建议（供 writing-plans 参考）

1. provider 抽象层重构（trait 重命名 + 事件增变体 + `BatchTranslateProvider` + `StreamingAdapter` + `AutoLangHeaderParser` 下沉 + `TranslationRequest` 通用化 + `service.rs` 中立化 + 目录移动），LLM 三 provider 改 impl 并通过现有测试。
2. `provider_for_service` 移至 `core/translation/protocol.rs`，加 `env` 参数与 `Microsoft` 分支（暂返回占位 provider）。
3. `MicrosoftMtProvider` + `EdgeTranslateEnv` + 语言映射 + UA 解析 + 请求拼装 + 响应解析。
4. `AppState.edge_translate_env` + `save_edge_translate_env` command + `lib.rs` 注册 + `web_popup.rs` 注入。
5. 前端协议/渠道注册 + 详情页精简 + `addService` 默认值 + `service-validation` 放行。
6. 配置适配（`is_configured` / `normalized`）。
7. 测试补全 + 文档同步。
