# Web / Slint 可替换 UI 架构方案

## 目标

Shizi 的 UI 长期会分为两个模块：

1. **设置页**：偏表单和配置管理，适合长期使用 WebView。
2. **翻译弹窗**：性能敏感，MVP 用 WebView 快速实现，后续替换为 Slint。

本方案的目标是让翻译弹窗从 WebView 替换为 Slint 时，只替换 UI adapter，不重写 Rust 核心业务、LLM provider、配置系统和 OCR / 剪贴板能力。

## 设计原则

1. **Rust 核心层是主系统**：翻译、配置、LLM 调度、剪贴板、OCR、截图、窗口编排都在 Rust。
2. **UI 只展示状态和提交用户动作**：Web / Slint 都不承载翻译业务逻辑。
3. **事件统一**：Web 阶段用 Tauri Event，Slint 阶段用 property / callback，但两者都消费同一份 `TranslationEvent`。
4. **Provider 不接触 UI**：LLM adapter 只向 `TranslationEventSink` 输出事件，不知道 Web、Slint 或 Tauri。
5. **平台能力通过 trait 注入**：剪贴板、截图、OCR、SecretStore 都通过接口隔离。

## 分层结构

推荐分层：

```text
app 层
  启动编排、托盘、快捷键、窗口生命周期

core 层
  translation、llm、config、clipboard、capture、ocr

ui port 层
  TranslationPopupPort、TranslationEventSink、Settings command

adapter 层
  WebPopup、SlintPopup、TauriEventSink、WindowsClipboard、WindowsOcr
```

依赖方向：

```text
adapter -> ui port -> core
app -> core + ui port
core 不依赖 Web / Slint / Tauri window 细节
```

禁止方向：

```text
TranslationService 直接 app.emit(...)
LlmProvider 直接调用 frontend event
OCR 识别后直接操作窗口
Web 前端直接拼 provider 请求
```

## 核心类型

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationSessionId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TranslationMode {
    Translate,
    Explain,
    Polish,
    Summarize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationRequest {
    pub session_id: TranslationSessionId,
    pub input: TranslationInput,
    pub source_lang: Option<String>,
    pub target_lang: String,
    pub mode: TranslationMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TranslationInput {
    SelectedText(String),
    OcrText {
        text: String,
        image_id: Option<String>,
    },
    ManualText(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TranslationEvent {
    Started {
        session_id: TranslationSessionId,
        source_text: String,
    },
    Delta {
        session_id: TranslationSessionId,
        text: String,
    },
    Finished {
        session_id: TranslationSessionId,
        full_text: String,
        usage: Option<TokenUsage>,
    },
    Failed {
        session_id: TranslationSessionId,
        message: String,
        retryable: bool,
    },
    Cancelled {
        session_id: TranslationSessionId,
    },
}
```

这些类型同时服务于：

- Tauri Event payload。
- Slint property 更新。
- 测试 fixture。
- 日志与性能追踪。
- 后续插件系统事件。

## TranslationService

`TranslationService` 是翻译用例入口，负责：

- 读取当前配置。
- 选择 active provider。
- 发出 `Started` / `Failed` 等用例级事件。
- 调用 provider 流式翻译。
- 将 provider delta 转交给 sink。

示例：

```rust
pub struct TranslationService {
    provider_registry: Arc<ProviderRegistry>,
    config_store: Arc<dyn ConfigStore>,
}

impl TranslationService {
    pub async fn translate(
        &self,
        request: TranslationRequest,
        sink: Arc<dyn TranslationEventSink>,
    ) -> Result<(), TranslationError> {
        sink.on_event(TranslationEvent::Started {
            session_id: request.session_id.clone(),
            source_text: request.input.text().to_owned(),
        }).await?;

        let config = self.config_store.load().await?;
        let provider = self.provider_registry.active_provider(&config)?;

        provider.stream_translate(request, sink.clone()).await?;

        Ok(())
    }
}
```

## TranslationEventSink

`TranslationEventSink` 是核心层向外输出事件的唯一接口。

```rust
#[async_trait::async_trait]
pub trait TranslationEventSink: Send + Sync {
    async fn on_event(&self, event: TranslationEvent) -> Result<(), TranslationError>;
}
```

### Web sink

Web 阶段将事件映射为 Tauri Event：

```rust
pub struct TauriEventSink {
    app: tauri::AppHandle,
}

#[async_trait::async_trait]
impl TranslationEventSink for TauriEventSink {
    async fn on_event(&self, event: TranslationEvent) -> Result<(), TranslationError> {
        self.app.emit("translation:event", event)?;
        Ok(())
    }
}
```

### Slint sink

Slint 阶段将事件映射为 property 更新：

```rust
pub struct SlintEventSink {
    popup: slint::Weak<TranslationPopup>,
}

#[async_trait::async_trait]
impl TranslationEventSink for SlintEventSink {
    async fn on_event(&self, event: TranslationEvent) -> Result<(), TranslationError> {
        let popup = self.popup.clone();

        slint::invoke_from_event_loop(move || {
            if let Some(popup) = popup.upgrade() {
                apply_translation_event(&popup, event);
            }
        })?;

        Ok(())
    }
}
```

## TranslationPopupPort

`TranslationPopupPort` 表示“翻译弹窗能力”，而不是某种具体 UI 技术。

```rust
#[async_trait::async_trait]
pub trait TranslationPopupPort: Send + Sync {
    async fn show(&self, anchor: PopupAnchor) -> anyhow::Result<()>;
    async fn push_event(&self, event: TranslationEvent) -> anyhow::Result<()>;
    async fn hide(&self) -> anyhow::Result<()>;
}
```

### WebPopup

MVP 阶段实现：

```rust
pub struct WebPopup {
    app: tauri::AppHandle,
}

#[async_trait::async_trait]
impl TranslationPopupPort for WebPopup {
    async fn show(&self, anchor: PopupAnchor) -> anyhow::Result<()> {
        // show Tauri webview window and set position
        Ok(())
    }

    async fn push_event(&self, event: TranslationEvent) -> anyhow::Result<()> {
        self.app.emit("translation:event", event)?;
        Ok(())
    }

    async fn hide(&self) -> anyhow::Result<()> {
        // hide Tauri webview window
        Ok(())
    }
}
```

### SlintPopup

原生优化阶段实现：

```rust
pub struct SlintPopup {
    window: slint::Weak<TranslationPopup>,
}

#[async_trait::async_trait]
impl TranslationPopupPort for SlintPopup {
    async fn show(&self, anchor: PopupAnchor) -> anyhow::Result<()> {
        // set position, show without activation
        Ok(())
    }

    async fn push_event(&self, event: TranslationEvent) -> anyhow::Result<()> {
        // update Slint properties through event loop
        Ok(())
    }

    async fn hide(&self) -> anyhow::Result<()> {
        // hide Slint window
        Ok(())
    }
}
```

## PopupBackend

通过配置选择弹窗实现：

```rust
pub enum PopupBackend {
    Web,
    Slint,
}

pub fn create_popup_backend(
    backend: PopupBackend,
    deps: PopupDeps,
) -> Arc<dyn TranslationPopupPort> {
    match backend {
        PopupBackend::Web => Arc::new(WebPopup::new(deps.app_handle)),
        PopupBackend::Slint => Arc::new(SlintPopup::new(deps.slint_handle)),
    }
}
```

这样里程碑 3 可以先并行保留 Web 和 Slint：

```toml
popup_backend = "web"
# popup_backend = "slint"
```

## LLM Provider 抽象

Provider 只负责将平台专属流式响应转换成统一事件。

```rust
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    fn id(&self) -> ProviderId;

    async fn stream_translate(
        &self,
        request: TranslationRequest,
        sink: Arc<dyn TranslationEventSink>,
    ) -> Result<(), LlmError>;

    async fn test_connection(&self) -> Result<(), LlmError>;
}
```

Provider 内部处理：

- Anthropic `content_block_delta`。
- OpenAI-compatible `choices[0].delta.content`。
- SSE `[DONE]`。
- 网络错误。
- 认证错误。
- 可重试错误。
- cancellation。

Provider 外部只看到：

```rust
TranslationEvent::Delta { text, .. }
TranslationEvent::Finished { full_text, usage, .. }
TranslationEvent::Failed { message, retryable, .. }
```

## 输入来源统一

划词、OCR、手动输入都必须进入同一个翻译入口。

```rust
pub enum TranslationInput {
    SelectedText(String),
    OcrText {
        text: String,
        image_id: Option<String>,
    },
    ManualText(String),
}
```

禁止 OCR 走独立 UI 流程：

```text
OCR -> frontend -> frontend calls translate
```

推荐：

```text
OCR -> TranslationInput::OcrText -> TranslationService
```

## 设置页与翻译弹窗的关系

设置页长期可以保留 WebView，因为它不是性能敏感路径。

```text
Web 设置页
  -> Tauri command
  -> ConfigStore / SecretStore

翻译弹窗
  -> TranslationPopupPort
  -> WebPopup 或 SlintPopup
```

设置页不应直接影响翻译弹窗内部状态；它只修改配置，翻译流程在下一次请求时读取配置。

## Slint 弹窗状态映射

Slint UI 建议暴露：

```slint
export component TranslationPopup inherits Window {
    in-out property <string> source_text;
    in-out property <string> translated_text;
    in-out property <string> status;
    in-out property <bool> is_streaming;
    in-out property <bool> has_error;
    in-out property <string> error_message;

    callback retry();
    callback copy_result();
    callback close_popup();
}
```

事件映射：

```rust
match event {
    TranslationEvent::Started { source_text, .. } => {
        popup.set_source_text(source_text.into());
        popup.set_translated_text("".into());
        popup.set_status("translating".into());
        popup.set_is_streaming(true);
        popup.set_has_error(false);
    }
    TranslationEvent::Delta { text, .. } => {
        let current = popup.get_translated_text();
        popup.set_translated_text(format!("{current}{text}").into());
    }
    TranslationEvent::Finished { full_text, .. } => {
        popup.set_translated_text(full_text.into());
        popup.set_status("finished".into());
        popup.set_is_streaming(false);
    }
    TranslationEvent::Failed { message, .. } => {
        popup.set_error_message(message.into());
        popup.set_has_error(true);
        popup.set_status("failed".into());
        popup.set_is_streaming(false);
    }
    TranslationEvent::Cancelled { .. } => {
        popup.set_status("cancelled".into());
        popup.set_is_streaming(false);
    }
}
```

## 性能热路径

里程碑 3 的热路径必须避免创建重资源。

应用启动时：

```text
创建 Slint popup
初始化字体 / 样式
隐藏窗口
初始化 provider registry
加载配置
注册快捷键
```

快捷键触发时：

```text
读取选区
创建 TranslationSessionId
清空弹窗状态
设置弹窗位置
show
启动翻译任务
持续 push TranslationEvent
```

不要在快捷键触发时：

- 创建窗口。
- 初始化 WebView。
- 初始化 Slint runtime。
- 重新创建 HTTP client。
- 阻塞等待完整翻译结果。

## 近期落地顺序

建议按以下顺序实现，保证后续 Slint 替换成本最低：

```text
1. 定义 TranslationRequest / TranslationInput / TranslationEvent。
2. 定义 TranslationEventSink。
3. 定义 LlmProvider。
4. 实现 mock provider。
5. 实现 TauriEventSink 或 WebPopup。
6. 让 Web 弹窗消费统一 translation:event。
7. 接真实 provider。
8. 接划词复制。
9. 接 OCR。
10. 新增 SlintPopup，替换 WebPopup。
```

## 架构验收清单

实现过程中持续检查：

- `core` 中没有直接引用 Web 前端文件。
- `core` 中没有直接操作 Slint window。
- `LlmProvider` 不调用 `app.emit`。
- `TranslationService` 不知道 provider 的 SSE 细节。
- OCR 和划词都通过 `TranslationInput` 进入翻译流程。
- Web 和 Slint 都消费同一份 `TranslationEvent`。
- 设置页只改配置，不承载翻译业务逻辑。
- 可以用 mock provider 不访问网络地验证完整 UI 流式展示。
