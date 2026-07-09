# 微软翻译渠道与 Provider 抽象层重构 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 新增「微软翻译」渠道（Edge 浏览器默认翻译引擎，免 Key 机器翻译），并把 LLM 专用的 `LlmProvider` 重构为 LLM/ML 平级的通用 `TranslationProvider`，统一处理流式与非流式响应。

**架构：** service 只依赖单一高层 `TranslationProvider` trait；非流式 provider 实现 `BatchTranslateProvider`，经 `StreamingAdapter` 适配为高层流式接口。auto 源语言检测的首行解析从 `TranslationService` 下沉到 LLM provider（共享 `AutoLangHeaderParser`），ML provider 从响应 `detectedLanguage` 填检测语言。`core/translation/`、`core/llm/`、`core/mt/` 三层平级目录。前端 main 窗口初始化采集 `navigator.userAgent`/`navigator.languages`，经 `save_edge_translate_env` command 存 `AppState` 进程级内存，后端从 UA 派生 `sec-mesh-client-*`/`sec-ch-ua` 等浏览器环境头。

**技术栈：** Rust（edition 2021，`async-trait`、`reqwest`、`serde`、`tokio`、`tokio-util` CancellationToken、`thiserror`）；Tauri 2 commands；前端 TypeScript + Vue 3 + vitest。

**关联 spec：** [docs/superpowers/specs/2026-07-09-microsoft-translate-and-provider-abstraction-design.md](../specs/2026-07-09-microsoft-translate-and-provider-abstraction-design.md)

---

## 文件结构

### 创建（后端）
- `src-tauri/src/core/translation/provider.rs` — 通用抽象层：`TranslationError`、`TranslationStreamEvent`、`TranslationProvider` trait、`TranslationResult`、`BatchTranslateProvider` trait、`StreamingAdapter<T>`（内容从 `core/llm/provider.rs` 移并重命名 + 新增非流式抽象与适配器）
- `src-tauri/src/core/translation/auto_lang.rs` — `AutoLangHeaderParser`（从 `service.rs` 的 `HeaderParseState`/`process_auto_delta`/`parse_detected_lang` 抽出，供 LLM provider 复用）
- `src-tauri/src/core/translation/protocol.rs` — `ProviderKind`/`protocol_to_kind`/`provider_for_service`（从 `core/llm/protocol.rs` 移入，跨 llm/mt 分发，新增 `Microsoft` 分支与 `env` 参数）
- `src-tauri/src/core/mt/mod.rs` — `EdgeTranslateEnv` 定义、默认 UA 常量、模块 re-export
- `src-tauri/src/core/mt/microsoft.rs` — `MicrosoftMtProvider`（`impl BatchTranslateProvider`）：语言映射、UA 解析、请求拼装、响应解析

### 创建（前端，无新文件，均为修改）

### 修改（后端）
- `src-tauri/src/core/mod.rs` — 加 `pub mod mt;`
- `src-tauri/src/core/translation/mod.rs` — 加 `pub mod provider; pub mod auto_lang; pub mod protocol;`，re-export 新类型
- `src-tauri/src/core/translation/types.rs` — `TranslationRequest` 加 `source_lang` 顶层字段；`TranslationPromptConfig` 去 `source_lang`；`user_prompt()` 等方法引用改为 `self.source_lang`
- `src-tauri/src/core/translation/batch.rs` — `build_batch_requests` 填 `request.source_lang` 顶层
- `src-tauri/src/core/translation/service.rs` — `TranslationService::new(provider: Arc<dyn TranslationProvider>)`；`translate_with` 中立化（移除 auto 解析，透传 `DetectedSourceLang` 事件）；移除 `TranslationError` 定义（移至 provider.rs）与 `HeaderParseState`/`process_auto_delta`/`parse_detected_lang`
- `src-tauri/src/core/llm/mod.rs` — 移除 `Llm*` 旧名 re-export，改为 re-export `TranslationProvider`/`TranslationStreamEvent`/`TranslationError`，`provider_for_service` 改从 `crate::core::translation::protocol` re-export
- `src-tauri/src/core/llm/openai_compatible.rs` — `impl TranslationProvider`，方法名 `stream_translate`→`translate`，内部用 `AutoLangHeaderParser` 处理 auto 首行
- `src-tauri/src/core/llm/claude.rs` — 同上
- `src-tauri/src/core/llm/mock.rs` — `impl TranslationProvider`，auto 时直接发 `DetectedSourceLang("英语")` + 纯译文 `Delta`（不再输出 `【源语言：】` 标记文本）
- `src-tauri/src/core/config/types.rs` — `ServiceInstanceConfig::normalized` 对 `microsoft_edge` 空 endpoint 填 Edge URL；`AppConfig::is_configured` 放行 `microsoft_edge`
- `src-tauri/src/app/state.rs` — `AppState` 加 `edge_translate_env` 字段 + `set_edge_translate_env`/`edge_translate_env` 方法
- `src-tauri/src/ui/web_popup.rs` — `start_translation_from_input` 调 `provider_for_service(&service_config, env)`；新增 `save_edge_translate_env` command
- `src-tauri/src/lib.rs` — `invoke_handler` 注册 `save_edge_translate_env`

### 修改（前端）
- `frontend/src/types/config.ts` — `ServiceProtocolId` 加 `'microsoft_edge'`
- `frontend/src/settings/types.ts` — `BuiltinServiceId` 加 `'microsoft'`
- `frontend/src/settings/tokens.ts` — 加 `MICROSOFT_EDGE` 协议常量、`BUILTIN_SERVICES` 加 microsoft 渠道、`MOCK_PULLED_MODELS` 加 `microsoft: []`
- `frontend/src/settings/service-validation.ts` — `keyRequired:false` 渠道放行 model 校验
- `frontend/src/lib/config.ts` — `AVAILABLE_PROTOCOLS` 加 `microsoft_edge`；`validateConfig` 对 `microsoft_edge` 放行 apiKey 与 model 校验
- `frontend/src/settings/panels/ServicesPanel.vue` — 详情区对 `activeService.id === 'microsoft'` 精简为「标题 + 删除」
- `frontend/public/translate.js` — 初始化采集 UA/Accept-Language，调 `save_edge_translate_env`

### 删除
- `src-tauri/src/core/llm/provider.rs`（内容移至 `core/translation/provider.rs`）
- `src-tauri/src/core/llm/protocol.rs`（内容移至 `core/translation/protocol.rs`）

### 文档同步（任务 7）
- `CLAUDE.md` / `AGENTS.md`（架构关键点、通信 command、核心层目录结构）
- `README`（当前能力补微软翻译）
- `roadmap`（完成状态）

---

## 任务 1：通用 provider 抽象层重构

本任务是核心重构：trait 重命名 + 事件增变体 + 非流式抽象 + auto 解析下沉 + 请求通用化 + service 中立化 + LLM 三 provider 迁移 + 目录移动。trait 重命名与 `source_lang` 迁移牵一发动全身，必须作为一个原子 commit 完成以保证可编译。TDD 体现为：先调整/新增测试（新签名），运行确认失败，再迁移实现让测试通过。

**文件：**
- 创建：`src-tauri/src/core/translation/provider.rs`、`src-tauri/src/core/translation/auto_lang.rs`
- 修改：`src-tauri/src/core/translation/types.rs`、`src-tauri/src/core/translation/batch.rs`、`src-tauri/src/core/translation/service.rs`、`src-tauri/src/core/translation/mod.rs`、`src-tauri/src/core/llm/mod.rs`、`src-tauri/src/core/llm/openai_compatible.rs`、`src-tauri/src/core/llm/claude.rs`、`src-tauri/src/core/llm/mock.rs`、`src-tauri/src/core/llm/protocol.rs`
- 删除：`src-tauri/src/core/llm/provider.rs`

- [ ] **步骤 1：创建 `core/translation/provider.rs`（通用 trait + 事件 + 错误 + 非流式抽象 + 适配器）**

```rust
use tokio_util::sync::CancellationToken;

use crate::core::translation::{TokenUsage, TranslationRequest};

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

/// 非流式 provider 的一次性翻译结果。
#[derive(Debug, Default, Clone)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::translation::{
        TranslationInput, TranslationPromptConfig, TranslationServiceMeta, TranslationSessionId,
    };

    fn request() -> TranslationRequest {
        TranslationRequest {
            session_id: TranslationSessionId("test".to_string()),
            input: TranslationInput::ManualText("hi".to_string()),
            source_lang: "auto".to_string(),
            target_lang: "中文".to_string(),
            service: TranslationServiceMeta::default(),
            prompts: TranslationPromptConfig::default(),
        }
    }

    struct BatchFake {
        text: String,
        usage: Option<TokenUsage>,
        detected: Option<String>,
    }

    #[async_trait::async_trait]
    impl BatchTranslateProvider for BatchFake {
        async fn translate_once(
            &self,
            _request: &TranslationRequest,
            _cancel: &CancellationToken,
        ) -> Result<TranslationResult, TranslationError> {
            Ok(TranslationResult {
                text: self.text.clone(),
                usage: self.usage,
                detected_source_lang: self.detected.clone(),
            })
        }
    }

    #[tokio::test]
    async fn streaming_adapter_emits_events_in_order() {
        let provider = StreamingAdapter(BatchFake {
            text: "译文".to_string(),
            usage: Some(TokenUsage { input_tokens: 1, output_tokens: 2 }),
            detected: Some("英语".to_string()),
        });
        let cancel = CancellationToken::new();
        let mut events = Vec::new();
        provider
            .translate(&request(), &mut |ev| events.push(ev), &cancel)
            .await
            .unwrap();
        let kinds: Vec<&str> = events
            .iter()
            .map(|e| match e {
                TranslationStreamEvent::DetectedSourceLang(_) => "detected",
                TranslationStreamEvent::Delta(_) => "delta",
                TranslationStreamEvent::Usage(_) => "usage",
            })
            .collect();
        assert_eq!(kinds, vec!["detected", "delta", "usage"]);
    }

    #[tokio::test]
    async fn streaming_adapter_skips_none_events() {
        let provider = StreamingAdapter(BatchFake {
            text: "译文".to_string(),
            usage: None,
            detected: None,
        });
        let cancel = CancellationToken::new();
        let mut events = Vec::new();
        provider
            .translate(&request(), &mut |ev| events.push(ev), &cancel)
            .await
            .unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], TranslationStreamEvent::Delta(_)));
    }

    #[tokio::test]
    async fn streaming_adapter_early_returns_when_cancelled() {
        let provider = StreamingAdapter(BatchFake {
            text: "译文".to_string(),
            usage: None,
            detected: None,
        });
        let cancel = CancellationToken::new();
        cancel.cancel();
        let mut events = Vec::new();
        provider
            .translate(&request(), &mut |ev| events.push(ev), &cancel)
            .await
            .unwrap();
        assert!(events.is_empty(), "取消时应早退不发任何事件");
    }
}
```

- [ ] **步骤 2：创建 `core/translation/auto_lang.rs`（AutoLangHeaderParser，迁移自 service.rs）**

```rust
/// source=auto 时的首行 `【源语言：xxx】` 解析状态机，供 LLM provider 复用。
/// 非 auto 场景 provider 不应使用此 parser（直通 Delta）。
pub struct AutoLangHeaderParser {
    pending: String,
    parsed: bool,
    detected: Option<String>,
}

impl AutoLangHeaderParser {
    pub fn new() -> Self {
        Self {
            pending: String::new(),
            parsed: false,
            detected: None,
        }
    }

    /// 喂入一段 delta，返回本次可输出的纯译文片段（标记行被吞掉；标记不匹配则首行作 Delta 补发）。
    pub fn feed(&mut self, delta: &str) -> Vec<String> {
        if self.parsed {
            return vec![delta.to_string()];
        }
        self.pending.push_str(delta);
        let Some(pos) = self.pending.find('\n') else {
            return Vec::new();
        };
        let first_line = self.pending[..pos].to_string();
        let rest = self.pending[pos + 1..].to_string();
        self.parsed = true;
        self.detected = parse_detected_lang(&first_line);
        self.pending.clear();
        let mut out = Vec::new();
        if self.detected.is_none() {
            out.push(first_line);
        }
        if !rest.is_empty() {
            out.push(rest);
        }
        out
    }

    /// 流结束后：若首行未解析且 pending 非空，作为译文补出；返回检测到的语言。
    pub fn finish(&mut self) -> (Vec<String>, Option<String>) {
        let mut out = Vec::new();
        if !self.parsed && !self.pending.is_empty() {
            out.push(std::mem::take(&mut self.pending));
        }
        (out, self.detected.clone())
    }
}

impl Default for AutoLangHeaderParser {
    fn default() -> Self {
        Self::new()
    }
}

/// 从首行 `【源语言：xxx】` 提取语言名；不匹配返回 None。
fn parse_detected_lang(first_line: &str) -> Option<String> {
    const PREFIX: &str = "【源语言：";
    let start = first_line.find(PREFIX)?;
    let after = &first_line[start + PREFIX.len()..];
    let end = after.find('】')?;
    let name = after[..end].trim();
    if name.is_empty() {
        return None;
    }
    Some(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feed_parses_marker_and_returns_translation_after_newline() {
        let mut p = AutoLangHeaderParser::new();
        let pieces = p.feed("【源语言：英语】\n译文内容");
        assert_eq!(pieces, vec!["译文内容".to_string()]);
        let (_, detected) = p.finish();
        assert_eq!(detected, Some("英语".to_string()));
    }

    #[test]
    fn feed_passes_through_when_no_marker() {
        let mut p = AutoLangHeaderParser::new();
        let pieces = p.feed("译文无标记");
        assert!(pieces.is_empty(), "无 \\n 时 feed 不输出");
        let (pieces, detected) = p.finish();
        assert_eq!(pieces, vec!["译文无标记".to_string()]);
        assert_eq!(detected, None);
    }

    #[test]
    fn feed_handles_marker_split_across_chunks() {
        let mut p = AutoLangHeaderParser::new();
        assert!(p.feed("【源语言：英").is_empty());
        let pieces = p.feed("语】\n译文");
        assert_eq!(pieces, vec!["译文".to_string()]);
        let (_, detected) = p.finish();
        assert_eq!(detected, Some("英语".to_string()));
    }

    #[test]
    fn feed_passes_through_first_line_when_newline_but_no_marker() {
        let mut p = AutoLangHeaderParser::new();
        let pieces = p.feed("译文第一行\n译文第二行");
        // 无标记但含 \n：首行补发 + 后续行
        assert_eq!(pieces, vec!["译文第一行".to_string(), "译文第二行".to_string()]);
        let (_, detected) = p.finish();
        assert_eq!(detected, None);
    }

    #[test]
    fn feed_passes_through_after_parsed() {
        let mut p = AutoLangHeaderParser::new();
        p.feed("【源语言：英语】\n");
        let pieces = p.feed("后续译文");
        assert_eq!(pieces, vec!["后续译文".to_string()]);
    }
}
```

- [ ] **步骤 3：修改 `core/translation/types.rs`（source_lang 提升到 request 顶层）**

将 `TranslationPromptConfig` 去掉 `source_lang`，`TranslationRequest` 加 `source_lang: String`，方法引用改为 `self.source_lang`。

`TranslationPromptConfig` 改为：
```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationPromptConfig {
    pub system_prompt: String,
    pub translation_prompt: String,
    pub chain_of_thought: String,
}

impl Default for TranslationPromptConfig {
    fn default() -> Self {
        Self {
            system_prompt: String::new(),
            translation_prompt: String::new(),
            chain_of_thought: "off".to_string(),
        }
    }
}
```

`TranslationRequest` 改为（注意 `source_lang` 放在 `input` 之后、`target_lang` 之前）：
```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationRequest {
    pub session_id: TranslationSessionId,
    pub input: TranslationInput,
    pub source_lang: String,
    pub target_lang: String,
    pub service: TranslationServiceMeta,
    pub prompts: TranslationPromptConfig,
}
```

`user_prompt()` 中 `self.prompts.source_lang` 的两处引用（`replace("{source_lang}", ...)` 与 `if self.prompts.source_lang == "auto"`）改为 `self.source_lang`。其余方法（`source_text`/`system_prompt`/`thinking_enabled`）不变。

`types.rs` 测试中所有构造 `TranslationRequest` 的地方补 `source_lang` 字段、`TranslationPromptConfig` 字面量去掉 `source_lang`：
- `request_with_source_lang(source_lang)` 改为把 `source_lang` 填到 request 顶层：`source_lang: source_lang.to_string()`，`prompts: TranslationPromptConfig::default()`。
- 其余 `TranslationRequest { ... }` 字面量（`translation_request_source_text_reads_input_text` 等）补 `source_lang: String::new()` 或 `source_lang: "auto".to_string()`，`TranslationPromptConfig { source_lang: ..., ... }` 去掉 `source_lang` 字段。

- [ ] **步骤 4：修改 `core/translation/batch.rs`（build_batch_requests 填 request.source_lang）**

`build_batch_requests` 中 `TranslationRequest { ... }` 字面量：把 `source_lang: source_lang.clone()` 从 `prompts: TranslationPromptConfig { source_lang: ..., ... }` 移到 request 顶层，`TranslationPromptConfig` 只留 `system_prompt`/`translation_prompt`/`chain_of_thought`：
```rust
.map(|s| TranslationRequest {
    session_id: TranslationSessionId(format!("{}:{}", batch_id, s.id)),
    input: input.clone(),
    source_lang: source_lang.clone(),
    target_lang: target_lang.clone(),
    service: TranslationServiceMeta {
        service_instance_id: s.id.clone(),
        service_name: s.name.clone(),
        service_type: s.service_type.clone(),
        protocol: s.protocol.clone(),
    },
    prompts: TranslationPromptConfig {
        system_prompt: s.system_prompt.clone(),
        translation_prompt: s.translation_prompt.clone(),
        chain_of_thought: s.chain_of_thought.clone(),
    },
})
```

`batch.rs` 测试 `build_batch_copies_prompt_config` 断言 `requests[0].prompts.source_lang` 改为 `requests[0].source_lang`。

- [ ] **步骤 5：修改 `core/translation/service.rs`（中立化）**

整文件改写要点：
1. 删除 `TranslationError` 定义（已移至 provider.rs）。
2. `use` 改为 `use crate::core::translation::provider::{TranslationError, TranslationProvider, TranslationStreamEvent};`（`TranslationError` 现来自 provider 模块）。
3. 删除 `HeaderParseState`、`process_auto_delta`、`parse_detected_lang`（已迁至 auto_lang.rs）。
4. `TranslationService::new(provider: Arc<dyn TranslationProvider>)`；`provider` 字段类型 `Arc<dyn TranslationProvider>`。
5. `translate_with` 中立化：移除 `is_auto`/`header_state`/`process_auto_delta` 调用与流末 pending 补发逻辑；`on_event` 闭包新增 `TranslationStreamEvent::DetectedSourceLang(lang)` 分支写入 `detected` slot。

新的 `translate_with` 主体：
```rust
pub async fn translate_with<F>(
    &self,
    request: TranslationRequest,
    collect_usage: bool,
    cancel: CancellationToken,
    mut emit: F,
) -> Result<(), TranslationError>
where
    F: FnMut(TranslationEvent) + Send,
{
    log::info!(
        "翻译开始: service={} protocol={} session={}",
        request.service.service_name,
        request.service.protocol,
        request.session_id.0
    );

    let full_text = Arc::new(Mutex::new(String::new()));
    let usage: Arc<Mutex<Option<TokenUsage>>> = Arc::new(Mutex::new(None));
    let detected: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let delta_text = full_text.clone();
    let usage_slot = usage.clone();
    let detected_slot = detected.clone();
    let delta_session_id = request.session_id.clone();
    let delta_service = request.service.clone();

    self.provider
        .translate(&request, &mut |ev| match ev {
            TranslationStreamEvent::Delta(text) => {
                if let Ok(mut t) = delta_text.lock() {
                    t.push_str(&text);
                }
                emit(TranslationEvent::Delta {
                    session_id: delta_session_id.clone(),
                    service: delta_service.clone(),
                    text,
                });
            }
            TranslationStreamEvent::Usage(u) => {
                if collect_usage {
                    if let Ok(mut slot) = usage_slot.lock() {
                        *slot = Some(u);
                    }
                }
            }
            TranslationStreamEvent::DetectedSourceLang(lang) => {
                if let Ok(mut slot) = detected_slot.lock() {
                    *slot = Some(lang);
                }
            }
        }, &cancel)
        .await?;

    let full_text = full_text.lock().map(|t| t.clone()).unwrap_or_default();
    let detected = detected.lock().map(|d| d.clone()).unwrap_or(None);

    if cancel.is_cancelled() {
        log::warn!(
            "翻译取消: service={} session={}",
            request.service.service_name,
            request.session_id.0
        );
        emit(TranslationEvent::Cancelled {
            session_id: request.session_id,
            service: request.service,
        });
    } else {
        let usage = usage.lock().map(|slot| slot.clone()).unwrap_or(None);
        log::info!(
            "翻译完成: service={} session={} len={}",
            request.service.service_name,
            request.session_id.0,
            full_text.chars().count()
        );
        emit(TranslationEvent::Finished {
            session_id: request.session_id,
            service: request.service,
            full_text,
            usage,
            detected_source_lang: detected,
        });
    }

    Ok(())
}
```

`service.rs` 测试改写要点（auto 解析测试迁移到 auto_lang.rs，service 只测透传）：
- `use crate::core::llm::{LlmProvider, LlmStreamEvent};` 改为 `use crate::core::translation::provider::{TranslationProvider, TranslationStreamEvent};`
- `CancelAwareFakeProvider`/`UsageFakeProvider`/`DetectFakeProvider` 改 `impl TranslationProvider`，方法 `async fn translate(...)`，事件 `TranslationStreamEvent`，错误 `TranslationError`。
- `request()` 补 `source_lang: String::new()`；`request_with_source(source_lang)` 改为 `source_lang: source_lang.to_string()` 顶层 + `prompts: TranslationPromptConfig::default()`。
- 删除 `translate_detects_source_lang_from_header`/`translate_fallbacks_when_no_header_marker`/`translate_passes_through_when_no_marker_but_has_newline`/`translate_handles_marker_across_chunks`/`translate_does_not_parse_when_source_specific` 五个测试（解析逻辑已迁至 auto_lang.rs，其测试覆盖）。
- 新增 service 透传测试：`DetectFakeProvider` 改为按参数决定是否发 `DetectedSourceLang`：
```rust
struct DetectFakeProvider {
    chunks: Vec<String>,
    detected: Option<String>,
}

#[async_trait::async_trait]
impl TranslationProvider for DetectFakeProvider {
    async fn translate(
        &self,
        _request: &TranslationRequest,
        on_event: &mut (dyn FnMut(TranslationStreamEvent) + Send),
        _cancel: &CancellationToken,
    ) -> Result<(), TranslationError> {
        if let Some(lang) = &self.detected {
            on_event(TranslationStreamEvent::DetectedSourceLang(lang.clone()));
        }
        for chunk in &self.chunks {
            on_event(TranslationStreamEvent::Delta(chunk.clone()));
        }
        Ok(())
    }
}
```
新增测试：
```rust
#[tokio::test]
async fn finished_carries_detected_source_lang_from_event() {
    let events = run_translate(
        DetectFakeProvider {
            chunks: vec!["译文内容".to_string()],
            detected: Some("英语".to_string()),
        },
        "auto",
    )
    .await;
    assert_eq!(collect_deltas(&events), "译文内容");
    assert_eq!(collect_detected(&events), Some("英语".to_string()));
}

#[tokio::test]
async fn finished_detected_none_when_provider_does_not_emit() {
    let events = run_translate(
        DetectFakeProvider {
            chunks: vec!["译文".to_string()],
            detected: None,
        },
        "auto",
    )
    .await;
    assert_eq!(collect_deltas(&events), "译文");
    assert_eq!(collect_detected(&events), None);
}
```
`run_translate`/`collect_deltas`/`collect_detected` 辅助函数保留（`request_with_source` 已调整）。

- [ ] **步骤 6：修改 `core/translation/mod.rs`（导出新模块）**

```rust
pub mod auto_lang;
pub mod batch;
pub mod provider;
pub mod service;
pub mod types;

pub use provider::{
    BatchTranslateProvider, StreamingAdapter, TranslationError, TranslationProvider,
    TranslationResult, TranslationStreamEvent,
};
pub use service::TranslationService;
pub use types::{
    TokenUsage, TranslationEvent, TranslationInput, TranslationPromptConfig, TranslationRequest,
    TranslationServiceMeta, TranslationSessionId,
};
```
（`protocol` 模块在任务 3 加入；本任务 provider_for_service 仍在 `core::llm::protocol`。）

- [ ] **步骤 7：修改三个 LLM provider 为 `impl TranslationProvider`**

通用改动（openai_compatible.rs / claude.rs / mock.rs）：
- `use crate::core::llm::{LlmError, LlmProvider, LlmStreamEvent};` 改为 `use crate::core::translation::provider::{TranslationError, TranslationProvider, TranslationStreamEvent};`
- `LlmError` → `TranslationError`、`LlmStreamEvent` → `TranslationStreamEvent`、`impl LlmProvider` → `impl TranslationProvider`、方法 `async fn stream_translate` → `async fn translate`。
- `consume_sse_event` 签名里的 `LlmStreamEvent`/`LlmError` 同步改名（openai/claude）。
- provider 内部 `request.prompts.source_lang` 引用改为 `request.source_lang`。

**openai_compatible.rs / claude.rs 的 auto 解析下沉**：在 `translate` 内用闭包包装 `on_event`，注入 `AutoLangHeaderParser`。以 openai 为例（claude 同理，把 `Self::consume_sse_event(&event, &mut forward)` 替换对应调用）：

```rust
async fn translate(
    &self,
    request: &TranslationRequest,
    on_event: &mut (dyn FnMut(TranslationStreamEvent) + Send),
    cancel: &CancellationToken,
) -> Result<(), TranslationError> {
    let api_key = self
        .config
        .api_key
        .as_deref()
        .ok_or(TranslationError::MissingConfig("OpenAI API Key"))?;
    // ... log、构造请求、send、状态码校验（错误类型改 TranslationError）...

    let is_auto = request.source_lang == "auto";
    let mut parser = AutoLangHeaderParser::new();

    let mut forward = |ev: TranslationStreamEvent| {
        if let TranslationStreamEvent::Delta(text) = ev {
            if is_auto {
                for piece in parser.feed(&text) {
                    on_event(TranslationStreamEvent::Delta(piece));
                }
            } else {
                on_event(TranslationStreamEvent::Delta(text));
            }
        } else {
            on_event(ev);
        }
    };

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            bytes = stream.next() => {
                let Some(bytes) = bytes else { break };
                let bytes = bytes.map_err(|e| TranslationError::Http(e.to_string()))?;
                buffer.push_str(&String::from_utf8_lossy(&bytes));
                buffer = buffer.replace("\r\n", "\n");
                while let Some(index) = buffer.find("\n\n") {
                    let event = buffer[..index].to_string();
                    buffer = buffer[index + 2..].to_string();
                    if Self::consume_sse_event(&event, &mut forward)? {
                        return Ok(());
                    }
                }
            }
        }
    }
    if !buffer.trim().is_empty() {
        Self::consume_sse_event(&buffer, &mut forward)?;
    }

    if is_auto {
        let (pieces, lang) = parser.finish();
        for piece in pieces {
            on_event(TranslationStreamEvent::Delta(piece));
        }
        if let Some(lang) = lang {
            on_event(TranslationStreamEvent::DetectedSourceLang(lang));
        }
    }

    Ok(())
}
```

`use` 顶部加 `use crate::core::translation::auto_lang::AutoLangHeaderParser;`。`parse_error_response` 返回类型与内部 `LlmError` 改 `TranslationError`。`consume_sse_event` 内 `LlmError::Parse/Api` 改 `TranslationError::Parse/Api`。

openai_compatible.rs 测试：`request()` 补 `source_lang: String::new()`；`request_body_uses_request_prompts` 中 `TranslationPromptConfig { source_lang: ..., ... }` 去掉 `source_lang`（该字段已在 request 顶层，但此测试构造的 request 需补 `source_lang: "English".to_string()` 顶层以保持 `{source_lang}` 占位符渲染）。

claude.rs 测试：所有 `TranslationRequest { ... }` 字面量补 `source_lang` 顶层，`TranslationPromptConfig { source_lang: ..., ... }` 去掉 `source_lang`。`stream_translate` 调用改为 `translate`。`LlmError::MissingConfig` 改 `TranslationError::MissingConfig`。

**mock.rs 改写**（auto 时直接发 `DetectedSourceLang`，不再输出标记文本）：
```rust
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use crate::core::translation::provider::{TranslationError, TranslationProvider, TranslationStreamEvent};
use crate::core::translation::{TokenUsage, TranslationRequest};

pub struct MockLlmProvider;

#[async_trait::async_trait]
impl TranslationProvider for MockLlmProvider {
    async fn translate(
        &self,
        request: &TranslationRequest,
        on_event: &mut (dyn FnMut(TranslationStreamEvent) + Send),
        cancel: &CancellationToken,
    ) -> Result<(), TranslationError> {
        let is_auto = request.source_lang == "auto";
        if is_auto {
            on_event(TranslationStreamEvent::DetectedSourceLang("英语".to_string()));
        }
        let chunks: Vec<String> = vec![
            "[Mock 翻译] ".to_string(),
            request.source_text().to_string(),
            " -> ".to_string(),
            request.target_lang.clone(),
        ];
        for chunk in chunks {
            on_event(TranslationStreamEvent::Delta(chunk));
            tokio::select! {
                _ = cancel.cancelled() => return Ok(()),
                _ = tokio::time::sleep(Duration::from_millis(180)) => {}
            }
        }
        on_event(TranslationStreamEvent::Usage(TokenUsage { input_tokens: 2, output_tokens: 2 }));
        Ok(())
    }
}
```
mock.rs 测试：`request()` 补 `source_lang: String::new()`；`stream_translate` 调用改 `translate`；`LlmStreamEvent` 改 `TranslationStreamEvent`。`mock_emits_detection_header_when_auto` 改为断言发 `DetectedSourceLang` 事件且 Delta 不含标记：
```rust
#[tokio::test]
async fn mock_emits_detected_source_lang_when_auto() {
    let provider = MockLlmProvider;
    let cancel = CancellationToken::new();
    let mut events = Vec::new();
    let mut req = request();
    req.source_lang = "auto".to_string();
    provider.translate(&req, &mut |ev: TranslationStreamEvent| events.push(ev), &cancel)
        .await
        .expect("mock 应成功");
    let detected = events.iter().find_map(|ev| match ev {
        TranslationStreamEvent::DetectedSourceLang(l) => Some(l.clone()),
        _ => None,
    });
    assert_eq!(detected, Some("英语".to_string()));
    let text: String = events.iter().filter_map(|ev| match ev {
        TranslationStreamEvent::Delta(t) => Some(t.clone()),
        _ => None,
    }).collect();
    assert!(!text.contains("【源语言："), "auto 时不应输出标记文本: {}", text);
}
```

- [ ] **步骤 8：修改 `core/llm/protocol.rs`（类型引用改名，暂不移动）**

仅改名，文件仍留在 `core/llm/protocol.rs`（任务 3 再移动）：
- `use crate::core::llm::{ClaudeConfig, ClaudeProvider, LlmProvider, MockLlmProvider, OpenAiCompatibleConfig, OpenAiCompatibleProvider};` 改为 `use crate::core::llm::{ClaudeConfig, ClaudeProvider, MockLlmProvider, OpenAiCompatibleConfig, OpenAiCompatibleProvider}; use crate::core::translation::provider::TranslationProvider;`
- `provider_for_service` 返回类型 `Arc<dyn LlmProvider>` → `Arc<dyn TranslationProvider>`。
- 其余（`ProviderKind`/`protocol_to_kind`）不变。
- 测试 `provider_for_service_claude_messages_ok`/`provider_for_service_unknown_returns_err` 不变（仍调 `provider_for_service(&config)`）。

- [ ] **步骤 9：修改 `core/llm/mod.rs`（导出调整）**

```rust
pub mod mock;
pub mod openai_compatible;
pub mod claude;
pub mod protocol;

pub use claude::{ClaudeConfig, ClaudeProvider};
pub use mock::MockLlmProvider;
pub use openai_compatible::{OpenAiCompatibleConfig, OpenAiCompatibleProvider};
// 通用 provider 抽象已迁至 core::translation::provider，旧 Llm* 名移除。
// provider_for_service 已迁至 core::translation::protocol（任务 3 完成后），
// 当前仍由本模块 protocol 提供：
pub use protocol::provider_for_service;
```
（删除 `pub mod provider;` 与 `pub use provider::{LlmError, LlmProvider, LlmStreamEvent};`。）

- [ ] **步骤 10：删除 `core/llm/provider.rs`**

`git rm src-tauri/src/core/llm/provider.rs`（内容已移至 `core/translation/provider.rs`）。

- [ ] **步骤 11：运行测试验证通过**

运行：`cd src-tauri && cargo test`
预期：全部通过（含 provider.rs 的 StreamingAdapter 测试、auto_lang.rs 测试、service.rs 透传测试、三 LLM provider 改名后测试、protocol.rs 测试）。

运行：`cd src-tauri && cargo build`
预期：编译通过，无 `Llm*` 残留引用。

- [ ] **步骤 12：Commit**

```bash
git add src-tauri/src/core/translation/provider.rs src-tauri/src/core/translation/auto_lang.rs src-tauri/src/core/translation/types.rs src-tauri/src/core/translation/batch.rs src-tauri/src/core/translation/service.rs src-tauri/src/core/translation/mod.rs src-tauri/src/core/llm/mod.rs src-tauri/src/core/llm/openai_compatible.rs src-tauri/src/core/llm/claude.rs src-tauri/src/core/llm/mock.rs src-tauri/src/core/llm/protocol.rs
git rm src-tauri/src/core/llm/provider.rs
git commit -m "refactor(translation): provider 抽象层通用化（LlmProvider->TranslationProvider，auto 解析下沉，非流式 BatchTranslateProvider+StreamingAdapter）"
```

---

## 任务 2：MicrosoftMtProvider + EdgeTranslateEnv（core/mt/）

实现微软翻译 provider 与浏览器环境抽象，含语言映射、UA 解析、请求拼装、响应解析。此任务 provider 已实现并单测，但尚未接入 `provider_for_service`（任务 3 接入）。用离线 fixture 测试，不真实联网。

**文件：**
- 创建：`src-tauri/src/core/mt/mod.rs`、`src-tauri/src/core/mt/microsoft.rs`
- 修改：`src-tauri/src/core/mod.rs`（加 `pub mod mt;`）

- [ ] **步骤 1：修改 `core/mod.rs` 加 `pub mod mt;`**

在文件末尾加一行 `pub mod mt;`（保持字母序可读，置于 `pub mod llm;` 之后）。

- [ ] **步骤 2：创建 `core/mt/mod.rs`（EdgeTranslateEnv + 默认 UA + re-export）**

```rust
pub mod microsoft;

/// WebView 初始化时采集的浏览器环境信息，供 MicrosoftMtProvider 拼装请求头。
#[derive(Debug, Clone, Default)]
pub struct EdgeTranslateEnv {
    pub user_agent: String,
    pub accept_language: String, // 如 "zh-CN,zh;q=0.9,en;q=0.8"
}

/// 编译期默认 UA 兜底（当前 Edge 稳定版 UA 字符串），env 未采集到时使用。
/// Edge 更新后此值可能过时，但接口对 UA 版本不敏感（仅用于派生 sec-mesh-client-* 头）。
pub const DEFAULT_EDGE_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36 Edg/150.0.0.0";

pub use microsoft::MicrosoftMtProvider;
```

- [ ] **步骤 3：写 `parse_edge_headers` 测试（先写测试）**

在 `core/mt/microsoft.rs` 的 `#[cfg(test)] mod tests` 内写：
```rust
#[test]
fn parse_edge_headers_extracts_versions_from_real_ua() {
    let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36 Edg/150.0.0.0";
    let h = parse_edge_headers(ua);
    assert_eq!(h.edge_version, "150.0.0.0");
    assert_eq!(h.os_version, "10.0.0");
    assert_eq!(h.arch, "x86_64");
    assert!(h.sec_ch_ua.contains("\"Chromium\";v=\"150\""), "sec-ch-ua 应含 Chromium 150: {}", h.sec_ch_ua);
    assert!(h.sec_ch_ua.contains("\"Microsoft Edge\";v=\"150\""), "sec-ch-ua 应含 Edge 150: {}", h.sec_ch_ua);
}

#[test]
fn parse_edge_headers_fallbacks_when_fields_missing() {
    let h = parse_edge_headers("some unknown ua string");
    assert!(h.edge_version.is_empty());
    assert!(h.os_version.is_empty());
    assert!(h.arch.is_empty());
    assert!(h.sec_ch_ua.is_empty());
}
```

- [ ] **步骤 4：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib core::mt`
预期：FAIL，`parse_edge_headers` 未定义。

- [ ] **步骤 5：实现 `core/mt/microsoft.rs`（MicrosoftMtProvider 全量）**

```rust
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use crate::core::mt::{EdgeTranslateEnv, DEFAULT_EDGE_USER_AGENT};
use crate::core::translation::provider::{BatchTranslateProvider, TranslationError, TranslationResult};
use crate::core::translation::TranslationRequest;

const EDGE_TRANSLATE_URL: &str = "https://edge.microsoft.com/translate/translatetext";

pub struct MicrosoftMtProvider {
    client: reqwest::Client,
    env: EdgeTranslateEnv,
}

impl MicrosoftMtProvider {
    pub fn new(env: EdgeTranslateEnv) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("创建 HTTP client 失败");
        Self { client, env }
    }

    fn effective_ua(&self) -> &str {
        if self.env.user_agent.trim().is_empty() {
            DEFAULT_EDGE_USER_AGENT
        } else {
            &self.env.user_agent
        }
    }
}

// ── 语言映射 ──────────────────────────────────────────────
// 内部 code（与前端 LANGUAGES 同源）↔ Edge code。
fn map_source_lang(internal: &str) -> Option<&'static str> {
    match internal {
        "auto" => None, // 省略 from，自动检测
        "zh-CN" => Some("zh-Hans"),
        "zh-TW" => Some("zh-Hant"),
        "en-US" => Some("en"),
        "ja-JP" => Some("ja"),
        "ko-KR" => Some("ko"),
        "fr-FR" => Some("fr"),
        "de-DE" => Some("de"),
        "es-ES" => Some("es"),
        "ru-RU" => Some("ru"),
        _ => None, // 未知语言省略 from（交由 Edge 自动检测），不阻断翻译
    }
}

fn map_target_lang(internal: &str) -> &str {
    match internal {
        "zh-CN" => "zh-Hans",
        "zh-TW" => "zh-Hant",
        "en-US" => "en",
        "ja-JP" => "ja",
        "ko-KR" => "ko",
        "fr-FR" => "fr",
        "de-DE" => "de",
        "es-ES" => "es",
        "ru-RU" => "ru",
        _ => "en", // 未知目标语言兜底英语
    }
}

/// Edge detectedLanguage.language（如 "en"）反向映射回内部 code（如 "en-US"）。
fn detected_to_internal(edge: &str) -> String {
    match edge {
        "zh-Hans" => "zh-CN",
        "zh-Hant" => "zh-TW",
        "en" => "en-US",
        "ja" => "ja-JP",
        "ko" => "ko-KR",
        "fr" => "fr-FR",
        "de" => "de-DE",
        "es" => "es-ES",
        "ru" => "ru-RU",
        other => other.to_string(),
    }
}

// ── UA 解析 ───────────────────────────────────────────────
struct EdgeHeaders {
    edge_version: String,
    os_version: String,
    arch: String,
    sec_ch_ua: String,
}

/// 从 UA 解析派生 sec-mesh-client-* / sec-ch-ua 头所需字段。纯函数，单测覆盖。
fn parse_edge_headers(ua: &str) -> EdgeHeaders {
    let edge_version = extract_token(ua, "Edg/").unwrap_or_default();
    let chrome_version = extract_token(ua, "Chrome/").unwrap_or_default();
    let os_version = extract_paren_token(ua, "Windows NT ").unwrap_or_default();
    let arch = if ua.contains("Win64; x64") {
        "x86_64".to_string()
    } else if ua.contains("WoW64") {
        "x86_64".to_string()
    } else {
        String::new()
    };
    let sec_ch_ua = if !edge_version.is_empty() {
        let v = major(&chrome_version);
        let ve = major(&edge_version);
        format!(
            "\"Not;A=Brand\";v=\"8\", \"Chromium\";v=\"{}\", \"Microsoft Edge\";v=\"{}\"",
            v, ve
        )
    } else {
        String::new()
    };
    EdgeHeaders {
        edge_version,
        os_version,
        arch,
        sec_ch_ua,
    }
}

/// 取 `Edg/` / `Chrome/` 后到首个非版本字符（空格/`)`）的子串。
fn extract_token(ua: &str, prefix: &str) -> Option<String> {
    let start = ua.find(prefix)? + prefix.len();
    let rest = &ua[start..];
    let end = rest.find(|c: char| c == ' ' || c == ')').unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

/// 取 `Windows NT ` 后到 `)` 或 `;` 的子串。
fn extract_paren_token(ua: &str, prefix: &str) -> Option<String> {
    let start = ua.find(prefix)? + prefix.len();
    let rest = &ua[start..];
    let end = rest.find(|c: char| c == ';' || c == ')').unwrap_or(rest.len());
    Some(rest[..end].trim().to_string())
}

fn major(version: &str) -> &str {
    version.split('.next().unwrap_or(version)
}

// ── 请求拼装 ──────────────────────────────────────────────
fn build_url(from: Option<&str>, to: &str) -> String {
    let mut url = format!("{}?to={}&isEnterpriseClient=false", EDGE_TRANSLATE_URL, to);
    if let Some(from) = from {
        url.push_str(&format!("&from={}", from));
    }
    url
}

fn build_headers(env: &EdgeTranslateEnv, ua: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let put = |headers: &mut HeaderMap, name: &str, value: &str| {
        if let (Ok(n), Ok(v)) = (
            HeaderName::try_from(name),
            HeaderValue::from_str(value),
        ) {
            headers.insert(n, v);
        }
    };
    // 常量头
    put(&mut headers, "accept", "*/*");
    put(&mut headers, "content-type", "application/json");
    put(&mut headers, "origin", "https://github.com");
    put(&mut headers, "referer", "https://github.com/");
    put(&mut headers, "priority", "u=1, i");
    put(&mut headers, "sec-fetch-dest", "empty");
    put(&mut headers, "sec-fetch-mode", "cors");
    put(&mut headers, "sec-fetch-site", "cross-site");
    put(&mut headers, "sec-ch-ua-platform", "\"Windows\"");
    put(&mut headers, "sec-ch-ua-mobile", "?0");
    put(&mut headers, "sec-mesh-client-os", "Windows");
    put(&mut headers, "sec-mesh-client-edge-channel", "stable");
    put(&mut headers, "sec-mesh-client-webview", "0");
    put(&mut headers, "x-edge-shopping-flag", "0");
    // 来自 env
    put(&mut headers, "user-agent", ua);
    let accept_language = if env.accept_language.trim().is_empty() {
        "zh-CN,zh;q=0.9,en;q=0.8"
    } else {
        env.accept_language.as_str()
    };
    put(&mut headers, "accept-language", accept_language);
    // 从 UA 派生
    let derived = parse_edge_headers(ua);
    if !derived.edge_version.is_empty() {
        put(&mut headers, "sec-mesh-client-edge-version", &derived.edge_version);
    }
    if !derived.os_version.is_empty() {
        put(&mut headers, "sec-mesh-client-os-version", &derived.os_version);
    }
    if !derived.arch.is_empty() {
        put(&mut headers, "sec-mesh-client-arch", &derived.arch);
    }
    if !derived.sec_ch_ua.is_empty() {
        put(&mut headers, "sec-ch-ua", &derived.sec_ch_ua);
    }
    headers
}

// ── 响应解析 ──────────────────────────────────────────────
// 基于通用结构（Azure Translator 同族字段名 translations/detectedLanguage）。
// 若 Edge 端点实测响应结构不同，按真实响应调整反序列化结构体（spec 5.4 不锁定字段名）。
#[derive(Deserialize)]
struct EdgeTranslation {
    translations: Vec<EdgeTranslationText>,
    #[serde(rename = "detectedLanguage")]
    detected_language: Option<EdgeDetectedLanguage>,
}

#[derive(Deserialize)]
struct EdgeTranslationText {
    text: String,
}

#[derive(Deserialize)]
struct EdgeDetectedLanguage {
    language: String,
}

#[async_trait::async_trait]
impl BatchTranslateProvider for MicrosoftMtProvider {
    async fn translate_once(
        &self,
        request: &TranslationRequest,
        cancel: &CancellationToken,
    ) -> Result<TranslationResult, TranslationError> {
        let text = request.source_text();
        let from = map_source_lang(&request.source_lang);
        let to = map_target_lang(&request.target_lang);
        let url = build_url(from, to);
        let ua = self.effective_ua().to_string();
        let headers = build_headers(&self.env, &ua);
        let body = serde_json::to_string(&[text])
            .map_err(|e| TranslationError::Parse(e.to_string()))?;

        let req = self.client.post(&url).body(body).headers(headers);

        let resp = tokio::select! {
            _ = cancel.cancelled() => return Ok(TranslationResult::default()),
            r = req.send() => r.map_err(|e| TranslationError::Http(e.to_string()))?,
        };

        let status = resp.status();
        if !status.is_success() {
            let retryable = status.as_u16() == 429 || status.is_server_error();
            let body = resp.text().await.unwrap_or_default();
            let message = format!(
                "HTTP {}: {}",
                status,
                body.chars().take(500).collect::<String>()
            );
            log::warn!("Edge 翻译响应非 2xx: status={} retryable={}", status, retryable);
            return if retryable {
                Err(TranslationError::Http(message))
            } else {
                Err(TranslationError::Api { message, retryable: false })
            };
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| TranslationError::Http(e.to_string()))?;
        let parsed: Vec<EdgeTranslation> = serde_json::from_slice(&bytes)
            .map_err(|e| TranslationError::Parse(e.to_string()))?;
        let first = parsed
            .into_iter()
            .next()
            .ok_or_else(|| TranslationError::Parse("响应数组为空".to_string()))?;

        let detected = if request.source_lang == "auto" {
            first
                .detected_language
                .map(|d| detected_to_internal(&d.language))
        } else {
            None
        };
        let translated = first
            .translations
            .into_iter()
            .next()
            .map(|t| t.text)
            .unwrap_or_default();

        Ok(TranslationResult {
            text: translated,
            usage: None,
            detected_source_lang: detected,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::translation::{
        TranslationInput, TranslationPromptConfig, TranslationServiceMeta, TranslationSessionId,
    };

    fn request(source: &str, target: &str) -> TranslationRequest {
        TranslationRequest {
            session_id: TranslationSessionId("test".to_string()),
            input: TranslationInput::ManualText("hello".to_string()),
            source_lang: source.to_string(),
            target_lang: target.to_string(),
            service: TranslationServiceMeta::default(),
            prompts: TranslationPromptConfig::default(),
        }
    }

    // parse_edge_headers 测试见步骤 3（此处含上述两个 #[test]）

    #[test]
    fn map_source_lang_auto_is_none() {
        assert_eq!(map_source_lang("auto"), None);
    }
    #[test]
    fn map_source_lang_known() {
        assert_eq!(map_source_lang("zh-CN"), Some("zh-Hans"));
        assert_eq!(map_source_lang("en-US"), Some("en"));
    }
    #[test]
    fn map_target_lang_known_and_fallback() {
        assert_eq!(map_target_lang("ja-JP"), "ja");
        assert_eq!(map_target_lang("unknown"), "en");
    }
    #[test]
    fn detected_to_internal_roundtrip() {
        assert_eq!(detected_to_internal("en"), "en-US");
        assert_eq!(detected_to_internal("zh-Hans"), "zh-CN");
        assert_eq!(detected_to_internal("fr"), "fr-FR");
    }
    #[test]
    fn build_url_omits_from_for_auto() {
        assert_eq!(
            build_url(None, "zh-Hans"),
            "https://edge.microsoft.com/translate/translatetext?to=zh-Hans&isEnterpriseClient=false"
        );
        assert_eq!(
            build_url(Some("en"), "zh-Hans"),
            "https://edge.microsoft.com/translate/translatetext?to=zh-Hans&isEnterpriseClient=false&from=en"
        );
    }
    #[test]
    fn build_headers_includes_env_and_derived() {
        let env = EdgeTranslateEnv {
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/150.0.0.0 Edg/150.0.0.0".to_string(),
            accept_language: "zh-CN,zh;q=0.9".to_string(),
        };
        let h = build_headers(&env, &env.user_agent);
        assert_eq!(h.get("user-agent").unwrap(), &env.user_agent);
        assert_eq!(h.get("accept-language").unwrap(), "zh-CN,zh;q=0.9");
        assert_eq!(h.get("sec-mesh-client-edge-version").unwrap(), "150.0.0.0");
        assert_eq!(h.get("sec-mesh-client-arch").unwrap(), "x86_64");
        assert!(h.get("sec-ch-ua").unwrap().to_str().unwrap().contains("Microsoft Edge"));
    }
    #[test]
    fn build_headers_uses_default_ua_when_env_empty() {
        let env = EdgeTranslateEnv::default();
        let h = build_headers(&env, DEFAULT_EDGE_USER_AGENT);
        assert_eq!(h.get("user-agent").unwrap(), DEFAULT_EDGE_USER_AGENT);
    }

    // 响应解析：构造离线 fixture，不联网
    #[test]
    fn parse_response_extracts_text_and_detected() {
        let json = r#"[{"translations":[{"text":"你好","to":"zh-Hans"}],"detectedLanguage":{"language":"en","score":1.0}}]"#;
        let parsed: Vec<EdgeTranslation> = serde_json::from_str(json).unwrap();
        let first = parsed.into_iter().next().unwrap();
        assert_eq!(first.translations[0].text, "你好");
        assert_eq!(first.detected_language.unwrap().language, "en");
    }

    #[test]
    fn effective_ua_falls_back_to_default_when_empty() {
        let provider = MicrosoftMtProvider::new(EdgeTranslateEnv::default());
        assert_eq!(provider.effective_ua(), DEFAULT_EDGE_USER_AGENT);
    }

    #[test]
    fn detected_to_internal_applied_for_auto_request() {
        // 验证 auto 时 detected_source_lang 经 detected_to_internal 映射
        let lang = detected_to_internal("en");
        assert_eq!(lang, "en-US");
    }
}
```

> 注：步骤 3 的两个 `parse_edge_headers` 测试与本步骤测试同属 `mod tests`，合并写入。`major` 函数用 `version.split('.').next()`（计划中 `split('.next()` 是笔误，实现时为 `split('.').next()`）。

- [ ] **步骤 6：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib core::mt`
预期：全部通过（parse_edge_headers、语言映射、build_url、build_headers、响应解析 fixture）。

运行：`cd src-tauri && cargo build`
预期：编译通过。

- [ ] **步骤 7：Commit**

```bash
git add src-tauri/src/core/mod.rs src-tauri/src/core/mt/mod.rs src-tauri/src/core/mt/microsoft.rs
git commit -m "feat(mt): 新增 MicrosoftMtProvider 与 EdgeTranslateEnv（Edge 翻译免 Key，BatchTranslateProvider 实现）"
```

---

## 任务 3：protocol.rs 迁移 + Microsoft 分支接入

把 `provider_for_service` 从 `core/llm/protocol.rs` 移至 `core/translation/protocol.rs`，新增 `Microsoft` 分支（返回 `StreamingAdapter(MicrosoftMtProvider)`）与 `env` 参数。调用点 `web_popup.rs` 此任务暂传 `None`（用默认 UA 兜底），任务 4 接入真实 env。

**文件：**
- 创建：`src-tauri/src/core/translation/protocol.rs`
- 修改：`src-tauri/src/core/translation/mod.rs`、`src-tauri/src/core/llm/mod.rs`、`src-tauri/src/ui/web_popup.rs`
- 删除：`src-tauri/src/core/llm/protocol.rs`

- [ ] **步骤 1：写 `provider_for_service` Microsoft 分支测试（先写测试）**

在新建的 `core/translation/protocol.rs` 的 `#[cfg(test)] mod tests` 内：
```rust
#[test]
fn protocol_to_kind_microsoft_edge() {
    assert!(matches!(
        protocol_to_kind("microsoft_edge"),
        Ok(ProviderKind::Microsoft)
    ));
}

#[test]
fn provider_for_service_microsoft_returns_streaming_adapter() {
    let config = svc("microsoft_edge");
    assert!(provider_for_service(&config, None).is_ok());
}
```
（`svc` helper 复用自现有 protocol.rs 测试，补 `microsoft_edge` 用例。）

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib core::translation::protocol`
预期：FAIL，模块未创建。

- [ ] **步骤 3：创建 `core/translation/protocol.rs`（迁移 + Microsoft 分支）**

```rust
use std::sync::Arc;

use crate::core::config::ServiceInstanceConfig;
use crate::core::llm::{ClaudeConfig, ClaudeProvider, MockLlmProvider, OpenAiCompatibleConfig, OpenAiCompatibleProvider};
use crate::core::mt::{EdgeTranslateEnv, MicrosoftMtProvider};
use crate::core::translation::provider::{StreamingAdapter, TranslationProvider};

/// 协议 id 映射到的 provider 类型，供 `provider_for_service` 分发与单测断言。
#[derive(Debug)]
pub enum ProviderKind {
    OpenAiCompatible,
    Claude,
    Mock,
    Microsoft,
}

/// 把协议 id 字符串映射到 `ProviderKind`。
/// 与前端 `frontend/src/types/config.ts` 的 `ServiceProtocolId` 保持一致：
/// - `"openai_chat"` -> `OpenAiCompatible`
/// - `"claude_messages"` -> `Claude`
/// - `"mock"` -> `Mock`
/// - `"microsoft_edge"` -> `Microsoft`
/// - 其他 -> 返回错误，不静默走 OpenAI 兼容。
pub fn protocol_to_kind(protocol: &str) -> Result<ProviderKind, String> {
    match protocol {
        "openai_chat" => Ok(ProviderKind::OpenAiCompatible),
        "claude_messages" => Ok(ProviderKind::Claude),
        "mock" => Ok(ProviderKind::Mock),
        "microsoft_edge" => Ok(ProviderKind::Microsoft),
        other => Err(format!("未支持的协议：{other}")),
    }
}

/// 根据 `ServiceInstanceConfig` 的 `protocol` 字段创建对应的 provider。
/// `env` 为微软翻译所需的浏览器环境信息（UA/Accept-Language），仅 `microsoft_edge` 使用，
/// 传 `None` 时用编译期默认 UA 兜底。
pub fn provider_for_service(
    config: &ServiceInstanceConfig,
    env: Option<&EdgeTranslateEnv>,
) -> Result<Arc<dyn TranslationProvider>, String> {
    match protocol_to_kind(&config.protocol)? {
        ProviderKind::Mock => Ok(Arc::new(MockLlmProvider)),
        ProviderKind::Claude => Ok(Arc::new(ClaudeProvider::new(ClaudeConfig {
            api_key: config.api_key.clone(),
            base_url: config.endpoint.clone(),
            model: config.model.clone(),
            timeout_seconds: config.timeout_seconds as u64,
        }))),
        ProviderKind::OpenAiCompatible => Ok(Arc::new(OpenAiCompatibleProvider::new(
            OpenAiCompatibleConfig {
                api_key: config.api_key.clone(),
                base_url: config.endpoint.clone(),
                model: config.model.clone(),
                timeout_seconds: config.timeout_seconds as u64,
            },
        ))),
        ProviderKind::Microsoft => Ok(Arc::new(StreamingAdapter(MicrosoftMtProvider::new(
            env.cloned().unwrap_or_default(),
        )))),
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
            system_prompt: String::new(),
            translation_prompt: String::new(),
            reflection_prompt: String::new(),
            reflection_enabled: false,
            chain_of_thought: "off".to_string(),
        }
    }

    #[test]
    fn protocol_to_kind_openai_chat() {
        assert!(matches!(protocol_to_kind("openai_chat"), Ok(ProviderKind::OpenAiCompatible)));
    }
    #[test]
    fn protocol_to_kind_claude_messages() {
        assert!(matches!(protocol_to_kind("claude_messages"), Ok(ProviderKind::Claude)));
    }
    #[test]
    fn protocol_to_kind_mock() {
        assert!(matches!(protocol_to_kind("mock"), Ok(ProviderKind::Mock)));
    }
    #[test]
    fn protocol_to_kind_microsoft_edge() {
        assert!(matches!(protocol_to_kind("microsoft_edge"), Ok(ProviderKind::Microsoft)));
    }
    #[test]
    fn protocol_to_kind_unknown_returns_err() {
        let err = protocol_to_kind("openai-compatible").unwrap_err();
        assert!(err.contains("openai-compatible"), "错误信息应包含协议名: {err}");
    }
    #[test]
    fn provider_for_service_claude_messages_ok() {
        let config = svc("claude_messages");
        assert!(provider_for_service(&config, None).is_ok());
    }
    #[test]
    fn provider_for_service_microsoft_returns_streaming_adapter() {
        let config = svc("microsoft_edge");
        assert!(provider_for_service(&config, None).is_ok());
    }
    #[test]
    fn provider_for_service_unknown_returns_err() {
        let config = svc("openai-compatible");
        assert!(provider_for_service(&config, None).is_err());
    }
}
```

- [ ] **步骤 4：修改 `core/translation/mod.rs` 加 protocol 模块与 re-export**

在 `pub mod provider;` 后加 `pub mod protocol;`，并在 `pub use` 块加 `pub use protocol::{provider_for_service, ProviderKind};`。

- [ ] **步骤 5：修改 `core/llm/mod.rs` 移除 protocol 模块**

删除 `pub mod protocol;` 与 `pub use protocol::provider_for_service;`（provider_for_service 已由 `core::translation` 提供）。

- [ ] **步骤 6：删除 `core/llm/protocol.rs`**

`git rm src-tauri/src/core/llm/protocol.rs`

- [ ] **步骤 7：修改 `ui/web_popup.rs`（use 路径 + 调用点暂传 None）**

- 顶部 `use crate::core::llm::provider_for_service;` 改为 `use crate::core::translation::provider_for_service;`。
- `start_translation_from_input` 中 `provider_for_service(&service_config)` 改为 `provider_for_service(&service_config, None)`（任务 4 接入真实 env）。

- [ ] **步骤 8：运行测试与构建验证**

运行：`cd src-tauri && cargo test`
预期：全部通过，含 `core::translation::protocol` 的 Microsoft 分支测试。

运行：`cd src-tauri && cargo build`
预期：编译通过，无 `core::llm::protocol` 残留引用。

- [ ] **步骤 9：Commit**

```bash
git add src-tauri/src/core/translation/protocol.rs src-tauri/src/core/translation/mod.rs src-tauri/src/core/llm/mod.rs src-tauri/src/ui/web_popup.rs
git rm src-tauri/src/core/llm/protocol.rs
git commit -m "refactor(translation): provider_for_service 迁移至 translation/protocol 并接入 Microsoft 分支"
```

---

## 任务 4：AppState env + save_edge_translate_env command + web_popup 接入真实 env

`AppState` 新增 `edge_translate_env` 进程级内存字段；新增 `save_edge_translate_env` command 并注册；`web_popup.rs` 把真实 env 传入 `provider_for_service`；前端 `translate.js` 初始化采集 UA/Accept-Language。

**文件：**
- 修改：`src-tauri/src/app/state.rs`、`src-tauri/src/ui/web_popup.rs`、`src-tauri/src/lib.rs`、`frontend/public/translate.js`

- [ ] **步骤 1：写 `AppState` env 测试（先写测试）**

在 `state.rs` 的 `#[cfg(test)] mod tests` 内：
```rust
#[test]
fn edge_translate_env_round_trips() {
    let state = app_state();
    assert!(state.edge_translate_env().is_none(), "初始应为 None");
    state.set_edge_translate_env(crate::core::mt::EdgeTranslateEnv {
        user_agent: "UA".to_string(),
        accept_language: "zh-CN".to_string(),
    }).expect("写入 env");
    let env = state.edge_translate_env().expect("读取 env");
    assert_eq!(env.user_agent, "UA");
    assert_eq!(env.accept_language, "zh-CN");
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib app::state`
预期：FAIL，`edge_translate_env` 方法未定义。

- [ ] **步骤 3：修改 `app/state.rs`（加 env 字段 + 方法）**

- `use crate::core::translation::TranslationInput;` 旁加 `use crate::core::mt::EdgeTranslateEnv;`。
- `AppState` 结构体加字段（置于 `session_target_lang` 之后）：
```rust
// WebView 初始化采集的浏览器环境信息（UA/Accept-Language），供微软翻译拼装请求头。
// 进程级内存，不持久化；每次启动由前端 main 窗口重新采集写入。
edge_translate_env: Arc<Mutex<Option<EdgeTranslateEnv>>>,
```
- `AppState::new` 的 `Self { ... }` 补 `edge_translate_env: Arc::new(Mutex::new(None)),`。
- 在 `set_session_languages` 方法之后加：
```rust
/// 写入前端采集的浏览器环境信息。锁毒化返回 Err。不持久化。
pub fn set_edge_translate_env(&self, env: EdgeTranslateEnv) -> Result<(), String> {
    let mut slot = self.edge_translate_env.lock().map_err(|_| "Edge 翻译环境锁已损坏".to_string())?;
    *slot = Some(env);
    Ok(())
}

/// 读浏览器环境信息（clone）。锁毒化返回 None，不返回 Err。
pub fn edge_translate_env(&self) -> Option<EdgeTranslateEnv> {
    self.edge_translate_env.lock().map(|slot| slot.clone()).unwrap_or(None)
}
```

- [ ] **步骤 4：修改 `ui/web_popup.rs`（接入真实 env + 新 command）**

- `start_translation_from_input` 在 `let state_for_task = state.clone();` 旁（spawn 前）取 env：
```rust
let edge_env = state.edge_translate_env();
```
- spawn 闭包内 `provider_for_service(&service_config, None)` 改为 `provider_for_service(&service_config, edge_env.as_ref())`（`edge_env` 为 `Option<EdgeTranslateEnv>`，`as_ref()` 得 `Option<&EdgeTranslateEnv>`，需 move 进闭包；在 `let app_handle = app.clone(); let state_for_task = state.clone();` 旁加 `let edge_env = state.edge_translate_env();`，闭包捕获 `edge_env`）。
- 文件末尾加 command：
```rust
#[tauri::command]
pub async fn save_edge_translate_env(
    user_agent: String,
    accept_language: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state.set_edge_translate_env(crate::core::mt::EdgeTranslateEnv {
        user_agent,
        accept_language,
    })
}
```

- [ ] **步骤 5：修改 `lib.rs`（注册 command）**

- `use ui::web_popup::{...}` 列表加 `save_edge_translate_env`。
- `invoke_handler` 的 `generate_handler!` 列表加 `save_edge_translate_env`（置于 `start_translation` 附近）。

- [ ] **步骤 6：修改 `frontend/public/translate.js`（初始化采集 UA）**

在 `initCards()` 定义之前（约 `applyPendingSourceText();` 之前）加采集函数并在初始化区调用：
```js
/* === 采集浏览器环境信息供微软翻译拼装请求头 === */
async function collectEdgeTranslateEnv() {
  if (!invoke) return;
  try {
    const userAgent = navigator.userAgent;
    const langs = navigator.languages ?? [navigator.language];
    const acceptLanguage = langs
      .map((l, i) => (i === 0 ? l : `${l};q=${(1 - i * 0.1).toFixed(1)}`))
      .join(',');
    await invoke('save_edge_translate_env', { userAgent, acceptLanguage });
  } catch (e) {
    // 采集失败不阻塞翻译，后端用默认 UA 兜底
    logger.warn('采集 Edge 翻译环境失败', String(e));
  }
}
```
在初始化区（`initCards();` 附近）加 `collectEdgeTranslateEnv();`（不 await，与 initCards 并行，失败不影响主流程）。

- [ ] **步骤 7：运行测试与构建验证**

运行：`cd src-tauri && cargo test`
预期：全部通过，含 `edge_translate_env_round_trips`。

运行：`cd src-tauri && cargo build`
预期：编译通过。

- [ ] **步骤 8：Commit**

```bash
git add src-tauri/src/app/state.rs src-tauri/src/ui/web_popup.rs src-tauri/src/lib.rs frontend/public/translate.js
git commit -m "feat(mt): AppState 保存 EdgeTranslateEnv + save_edge_translate_env command + 前端 UA 采集"
```

---

## 任务 5：前端协议/渠道注册 + 详情页精简 + 校验放行

前端新增 `microsoft_edge` 协议与 `microsoft` 渠道，详情页对 microsoft 精简为「标题 + 删除」，校验对免 Key 渠道放行 model/apiKey。

**文件：**
- 修改：`frontend/src/types/config.ts`、`frontend/src/settings/types.ts`、`frontend/src/settings/tokens.ts`、`frontend/src/settings/service-validation.ts`、`frontend/src/lib/config.ts`、`frontend/src/settings/panels/ServicesPanel.vue`

- [ ] **步骤 1：写前端校验放行测试（先写测试）**

在 `frontend/src/settings/service-validation.test.ts` 的 `describe` 内加：
```ts
it('免 Key 渠道 model 为空时允许开启', () => {
  const noKeyMeta = { ...meta, id: 'microsoft' as any, keyRequired: false, category: 'ml' as const }
  expect(validateServiceForEnable(inst({ apiKey: '', model: '' }), noKeyMeta)).toBeNull()
})
```

在 `frontend/src/lib/config.test.ts` 的 `describe('validateConfig')` 内加：
```ts
it('microsoft_edge 免 Key 渠道允许保存', () => {
  expect(validateConfig({
    ...base,
    services: [{
      id: 'ms-1', serviceType: 'microsoft', name: '微软翻译', enabled: true,
      protocol: 'microsoft_edge', apiKey: null, endpoint: 'https://edge.microsoft.com/translate/translatetext',
      model: '', timeoutSeconds: 60, systemPrompt: '', translationPrompt: '',
      reflectionPrompt: '', reflectionEnabled: false, chainOfThought: 'off',
    }],
  })).toBeNull()
})
```

- [ ] **步骤 2：运行测试验证失败**

运行：`npm run test`
预期：FAIL，microsoft 渠道仍被 model/apiKey 校验拦截。

- [ ] **步骤 3：修改 `frontend/src/types/config.ts`（ServiceProtocolId 加 microsoft_edge）**

```ts
export type ServiceProtocolId = 'openai_chat' | 'claude_messages' | 'microsoft_edge';
```

- [ ] **步骤 4：修改 `frontend/src/settings/types.ts`（BuiltinServiceId 加 microsoft）**

在 `BuiltinServiceId` 联合类型中 `'claude'` 之后加 `| 'microsoft'`。

- [ ] **步骤 5：修改 `frontend/src/settings/tokens.ts`（协议常量 + 渠道 + MOCK_PULLED_MODELS）**

在 `CLAUDE_MESSAGES` 常量之后加：
```ts
const MICROSOFT_EDGE = {
  id: 'microsoft_edge' as const,
  label: 'Edge 翻译',
  defaultEndpoint: 'https://edge.microsoft.com/translate/translatetext',
  defaultModel: '',
  editableEndpoint: false,
  status: 'available' as const,
}
```
在 `BUILTIN_SERVICES` 数组中 `claude` 之后加：
```ts
{
  id: 'microsoft',
  name: '微软翻译',
  description: 'Edge 浏览器默认翻译引擎，免 Key，复用浏览器环境信息调用。',
  builtin: true,
  defaultModel: '',
  iconifyId: 'simple-icons:microsoftedge',
  category: 'ml',
  keyRequired: false,
  protocols: [MICROSOFT_EDGE],
},
```
在 `MOCK_PULLED_MODELS` 的 `claude: [...]` 之后加 `microsoft: [],`。

- [ ] **步骤 6：修改 `frontend/src/settings/service-validation.ts`（免 Key 放行 model）**

将 `if (!instance.model.trim())` 改为：
```ts
if (meta?.keyRequired !== false && !instance.model.trim()) {
  return 'Model 不能为空'
}
```

- [ ] **步骤 7：修改 `frontend/src/lib/config.ts`（AVAILABLE_PROTOCOLS + validateConfig 放行）**

```ts
const AVAILABLE_PROTOCOLS: readonly ServiceProtocolId[] = ['openai_chat', 'claude_messages', 'microsoft_edge'];
```
`validateConfig` 中 apiKey 与 model 校验对 `microsoft_edge` 放行：
```ts
const isKeyless = service.protocol === 'microsoft_edge';
if (!isKeyless && !service.apiKey?.trim()) {
  return `${service.name} 请先填写 API Key`;
}
// ... endpoint 校验不变 ...
if (!isKeyless && !service.model.trim()) {
  return `${service.name} Model 不能为空`;
}
```

- [ ] **步骤 8：修改 `frontend/src/settings/panels/ServicesPanel.vue`（microsoft 详情页精简）**

详情区 `v-else-if="activeInstance && activeService"` 分支内，用 `v-if`/`v-else` 区分 microsoft 与其他渠道。在 `<header>...</header>` 之后：
- 加 `v-if="activeService.id !== 'microsoft'"` 包裹「该渠道尚未对接」警告 + 接入点 + 凭据 + 模型 + 思维链 + 提示词 + 备注 五个 `SettingGroup`（即现有 669-833 行的所有 SettingGroup）。
- 「危险操作」`SettingGroup`（835-845 行）保持始终渲染（microsoft 也需删除）。
- 底部「未配置 API Key」警告（847-853 行）加 `v-if="activeService.id !== 'microsoft' && !activeInstance.apiKey"`。

即：把「尚未对接警告 + 接入点 + 凭据 + 模型 + 思维链 + 提示词 + 备注」整体包进 `<template v-if="activeService.id !== 'microsoft'">...</template>`，使 microsoft 渠道详情区只显示 `<header>`（标题+描述）+「危险操作 / 删除实例」。

- [ ] **步骤 9：运行前端测试与类型检查**

运行：`npm run test`
预期：全部通过，含 microsoft 放行测试。

运行：`npm run typecheck`
预期：通过（`ServiceProtocolId` 含 `microsoft_edge`，`BuiltinServiceId` 含 `microsoft`，`MOCK_PULLED_MODELS` 含 `microsoft` key，ServicesPanel 精简分支类型正确）。

- [ ] **步骤 10：Commit**

```bash
git add frontend/src/types/config.ts frontend/src/settings/types.ts frontend/src/settings/tokens.ts frontend/src/settings/service-validation.ts frontend/src/lib/config.ts frontend/src/settings/panels/ServicesPanel.vue frontend/src/settings/service-validation.test.ts frontend/src/lib/config.test.ts
git commit -m "feat(settings): 新增微软翻译渠道与 microsoft_edge 协议，详情页精简，免 Key 校验放行"
```

---

## 任务 6：配置适配（is_configured / normalized）

后端配置模型对 `microsoft_edge` 放行：`is_configured` 免 Key 视为已配置，`normalized` 空 endpoint 填 Edge URL。

**文件：**
- 修改：`src-tauri/src/core/config/types.rs`

- [ ] **步骤 1：写配置适配测试（先写测试）**

在 `core/config/types.rs` 的 `#[cfg(test)] mod tests` 内加：
```rust
#[test]
fn is_configured_true_with_microsoft_edge_no_key() {
    let mut config = AppConfig::from_env();
    config.services[0].protocol = "microsoft_edge".to_string();
    config.services[0].api_key = None;
    config.services[0].model = String::new();
    assert!(config.is_configured());
}

#[test]
fn normalized_fills_edge_url_for_microsoft_edge() {
    let svc = ServiceInstanceConfig {
        id: "ms".to_string(),
        service_type: "microsoft".to_string(),
        name: "微软翻译".to_string(),
        enabled: true,
        protocol: "microsoft_edge".to_string(),
        api_key: None,
        endpoint: "".to_string(),
        model: String::new(),
        timeout_seconds: 0,
        system_prompt: String::new(),
        translation_prompt: String::new(),
        reflection_prompt: String::new(),
        reflection_enabled: false,
        chain_of_thought: default_chain_of_thought(),
    }.normalized();
    assert_eq!(svc.endpoint, "https://edge.microsoft.com/translate/translatetext");
    assert!(svc.api_key.is_none());
    assert_eq!(svc.model, DEFAULT_MODEL); // model 空走默认（不影响 ML，运行时忽略）
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib core::config::types`
预期：FAIL，microsoft_edge 未放行。

- [ ] **步骤 3：修改 `core/config/types.rs`（is_configured + normalized）**

顶部常量区加：
```rust
const DEFAULT_EDGE_TRANSLATE_URL: &str = "https://edge.microsoft.com/translate/translatetext";
```
`ServiceInstanceConfig::normalized` 中 endpoint 空分支补 `microsoft_edge`：
```rust
if self.endpoint.trim().is_empty() {
    self.endpoint = match self.protocol.as_str() {
        "claude_messages" => DEFAULT_CLAUDE_BASE_URL.to_string(),
        "microsoft_edge" => DEFAULT_EDGE_TRANSLATE_URL.to_string(),
        _ => DEFAULT_BASE_URL.to_string(),
    };
}
```
`AppConfig::is_configured` 的 `match s.protocol.as_str()` 加 `microsoft_edge` 分支：
```rust
match s.protocol.as_str() {
    "mock" | "microsoft_edge" => true,
    _ => s.api_key.is_some(),
}
```

- [ ] **步骤 4：运行测试与构建验证**

运行：`cd src-tauri && cargo test`
预期：全部通过，含 microsoft_edge 配置适配测试。

运行：`cd src-tauri && cargo build`
预期：编译通过。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/core/config/types.rs
git commit -m "feat(config): is_configured 与 normalized 放行 microsoft_edge 协议"
```

---

## 任务 7：文档同步

按 spec 第 13 节同步 `CLAUDE.md`/`AGENTS.md`、README、roadmap。`plugins.md` 本特性未新增插件/技能，无需同步。

**文件：**
- 修改：`CLAUDE.md`、`AGENTS.md`、`README`（或等价 README 文件）、`roadmap`（或等价文件）

- [ ] **步骤 1：定位文档文件**

运行：`ls CLAUDE.md AGENTS.md` 与查找 README/roadmap（`Glob` `**/README*`、`**/roadmap*` 或 `docs/**` 下文件），确认实际文件名与路径。

- [ ] **步骤 2：修改 `CLAUDE.md` 与 `AGENTS.md`（同步内容）**

- 「架构关键点 / 服务协议配置」：协议列表补 `microsoft_edge`；provider 抽象层说明改为 `TranslationProvider`（LLM/ML 平级）+ `BatchTranslateProvider` 非流式适配 + `StreamingAdapter`。
- 「架构关键点 / 前后端通信」：command 列表补 `save_edge_translate_env`；`translation:event` 的 `Finished.detectedSourceLang` 回传机制说明改为 provider 事件（`TranslationStreamEvent::DetectedSourceLang`）。
- 「核心层」目录结构补 `core/translation/provider.rs` / `auto_lang.rs` / `protocol.rs` 与 `core/mt/`（`mod.rs` + `microsoft.rs`）。
- 「项目结构」注释中 `src/core/llm/` 描述补「通用 provider 抽象已迁至 `core/translation/provider.rs`」，`src/core/` 补 `mt/ 微软翻译机器翻译 provider`。
- 两文件内容保持一致（CLAUDE.md 开发说明第 1 条要求同步）。

- [ ] **步骤 3：修改 README（当前能力）**

「当前能力」补「微软翻译（Edge 引擎，免 Key 机器翻译）」；机器翻译渠道说明（当前仅微软翻译已对接，其余 DeepL/Google/百度等保持开发中）。

- [ ] **步骤 4：修改 roadmap（完成状态）**

微软翻译渠道与 provider 抽象层重构标记为已完成。

- [ ] **步骤 5：Commit**

```bash
git add CLAUDE.md AGENTS.md README.md docs/roadmap.md  # 按步骤 1 实际路径调整
git commit -m "docs: 同步微软翻译渠道与 provider 抽象层重构文档"
```

- [ ] **步骤 6：全量验证（最终）**

```bash
cd src-tauri && cargo test
cd src-tauri && cargo build
npm run typecheck
npm run test
```
预期：全部通过。手动验证（Tauri dev，spec 11.4）：设置页添加微软翻译→详情页只显示标题+删除→启用→翻译弹窗输入英文→微软翻译卡片渲染译文→auto 源语言 badge 显示→切换语言/取消/断网失败/LLM 渠道不回归。

---

## 自检

**1. 规格覆盖度：** 逐章对照 spec：
- §1-2 目的/范围：任务 1-6 覆盖抽象层重构、微软翻译、UA 采集、前端渠道、配置适配；YAGNI 项（其他 ML 渠道、非流式 LLM、token 统计、批量多行、env 持久化、Key 加密）均不在计划内，符合范围外。✓
- §3 架构（分层/目录/决策）：任务 1 建 provider.rs/auto_lang.rs、任务 2 建 core/mt/、任务 3 迁 protocol.rs，目录结构与 spec 3.2 一致；决策表各项（抽象形态/DetectedSourceLang 事件/解析下沉/source_lang 顶层/流式适配/整段单元素/UA 派生/env 内存/microsoft_edge 协议/id 判断）均有对应任务。✓
- §4 Provider 抽象层：4.1 trait/事件（任务 1 步骤 1）、4.2 非流式+适配器（任务 1 步骤 1 + 测试）、4.3 auto 解析下沉（任务 1 步骤 2 + 步骤 7）、4.4 TranslationRequest 通用化（任务 1 步骤 3-4）、4.5 service 中立化（任务 1 步骤 5）、4.6 provider 分发（任务 3）。✓
- §5 MicrosoftMtProvider：5.1 EdgeTranslateEnv（任务 2 步骤 2）、5.2 语言映射（任务 2 步骤 5）、5.3 请求拼装（任务 2 步骤 5 build_url/build_headers）、5.4 translate_once（任务 2 步骤 5，含真实响应校验注记）。✓
- §6 UA/env 采集注入：6.1 前端采集（任务 4 步骤 6）、6.2 后端存储（任务 4 步骤 3-5）、6.3 注入调用点（任务 4 步骤 4）。✓
- §7 协议与渠道注册：7.1 后端（任务 3）、7.2 前端类型（任务 5 步骤 3-4）、7.3 渠道元数据（任务 5 步骤 5）。✓
- §8 配置适配：is_configured/normalized（任务 6）、addService 默认值（stores/settings.ts 现有 defaultInstanceFor 已通用，无需改）、validateServiceForEnable 放行（任务 5 步骤 6）。✓
- §9 前端详情页简化（任务 5 步骤 8）。✓
- §10 错误处理：状态码映射（任务 2 步骤 5）、取消（任务 2 步骤 5 tokio::select + TranslationResult::default）。✓
- §11 测试策略：Rust 单测（任务 1-6 各步骤）、前端测试（任务 5）、验证命令（任务 7 步骤 6）、人工验证（任务 7 步骤 6）。✓
- §12 不向后兼容：trait 重命名（任务 1）、source_lang 迁移（任务 1）、config.json 兼容（任务 6 不增字段）。✓
- §13 文档同步（任务 7）。✓
- §14 实现顺序：计划任务 1-7 与 spec 7 步建议对应；调整点--MicrosoftMtProvider（任务 2）先于 protocol 迁移（任务 3），避免 spec 第 14 节第 2 步的「占位 provider」，符合「无占位符」原则。✓

**遗漏补正（spec 隐含但未显式列出，计划已覆盖）：**
- spec §8 只提 `validateServiceForEnable` 放行 apiKey，未提 model 放行与 `validateConfig` 放行。计划任务 5 步骤 6-7 补齐 model 放行（`keyRequired:false`）与 `validateConfig` 对 `microsoft_edge` 放行 apiKey/model（保存路径会走 validateConfig，否则 microsoft 实例保存报错）。✓
- `stores/settings.ts` 的 `defaultInstanceFor` 现有逻辑已按 `protocol.defaultEndpoint/defaultModel` 填充，microsoft 无需特判，计划未列改动（已确认）。✓

**2. 占位符扫描：** 全计划无「TODO/待定/后续实现/类似任务 N/添加适当错误处理」等模式。每个代码步骤含完整代码或精确 diff 要点；迁移改名的步骤给出新签名与字段差异，不依赖工程师推断。MicrosoftMtProvider 响应字段名标注「实测校验」是 spec 5.4 明确允许的，非占位。✓

**3. 类型一致性：**
- `TranslationProvider::translate(&self, request, on_event, cancel) -> Result<(), TranslationError>` -- 任务 1 定义，任务 2/3/4 使用一致。
- `BatchTranslateProvider::translate_once(&self, request, cancel) -> Result<TranslationResult, TranslationError>` -- 任务 1 定义，任务 2 实现。
- `TranslationStreamEvent::{Delta, Usage, DetectedSourceLang}` -- 任务 1 定义，任务 1（provider/LLM）、任务 2（StreamingAdapter 不直接用但 BatchTranslateProvider 返回 TranslationResult 含 detected_source_lang）一致。
- `TranslationResult { text, usage, detected_source_lang }` -- 任务 1 定义，任务 2 translate_once 返回一致。
- `provider_for_service(config, env: Option<&EdgeTranslateEnv>) -> Result<Arc<dyn TranslationProvider>, String>` -- 任务 3 定义，任务 4 调用 `provider_for_service(&service_config, edge_env.as_ref())` 一致。
- `EdgeTranslateEnv { user_agent, accept_language }` -- 任务 2 定义，任务 4 state/command/前端一致。
- `AutoLangHeaderParser::feed/finish` 签名 -- 任务 1 步骤 2 定义，步骤 7 LLM provider 调用 `parser.feed(&text)` / `parser.finish()` 一致。
- `TranslationRequest.source_lang` 顶层字段 -- 任务 1 步骤 3 定义，步骤 4（batch）、步骤 5（service 不直接读但 provider 读）、步骤 7（provider `request.source_lang`）、任务 2（`request.source_lang`）一致。
- 前端 `ServiceProtocolId` 含 `microsoft_edge` -- 任务 5 步骤 3 定义，步骤 5/7 使用一致；`BuiltinServiceId` 含 `microsoft` -- 步骤 4 定义，步骤 5 `MOCK_PULLED_MODELS` key 与 `BUILTIN_SERVICES` id 一致。✓

**4. 任务依赖与顺序：** 任务 1（抽象层）→ 任务 2（MicrosoftMtProvider，依赖 BatchTranslateProvider）→ 任务 3（protocol 迁移接入，依赖 MicrosoftMtProvider + StreamingAdapter）→ 任务 4（env 注入，依赖 provider_for_service env 参数）→ 任务 5（前端，独立）→ 任务 6（配置，独立）→ 任务 7（文档）。任务 5/6 可与 3/4 并行但计划按序执行以简化。每个任务结束可编译可测试。✓
