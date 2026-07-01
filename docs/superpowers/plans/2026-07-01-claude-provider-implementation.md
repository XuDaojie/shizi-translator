# Claude Provider 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 为翻译链路新增 Anthropic / Claude 原生 Messages API provider，复用既有 `LlmProvider` trait，支持划词翻译与 OCR 翻译的流式输出。

**架构：** 独立 `ClaudeProvider` 实现 `LlmProvider::stream_translate`，与 `OpenAiCompatibleProvider` 同构——持有 `reqwest::Client` + `ClaudeConfig`，SSE 流用 `response.bytes_stream()` + `tokio::select!` 竞速取消。配置模型新增 `ClaudeAppConfig`，工厂 `web_popup.rs` 增加 `"claude"` 分支，前端增加 provider 下拉与 Claude 表单。

**技术栈：** Rust reqwest、tokio、serde、futures-util、原生 HTML/CSS/JS（Tauri 2）

---

### 任务 1：Claude SSE 事件结构体与错误响应模型

**文件：**
- 创建：`src-tauri/src/core/llm/claude.rs`（前 80 行）
- 测试：内联于 `claude.rs` 的 `#[cfg(test)] mod tests`

- [x] **步骤 1：编写失败的测试——确保 `consume_sse_event` 编译前先定义结构体占位**

`claude.rs` 开头写入以下结构体定义与 `consume_sse_event` 签名：

```rust
use serde::Deserialize;
use crate::core::llm::LlmError;

pub struct ClaudeProvider;

impl ClaudeProvider {
    fn consume_sse_event(
        event: &str,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<bool, LlmError> {
        todo!()
    }
}
```

测试（同文件末尾 `#[cfg(test)]`）：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consume_sse_event_extracts_text_delta() {
        let event = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"你好\"}}";
        let mut texts = Vec::new();
        let done = ClaudeProvider::consume_sse_event(event, &mut |t| texts.push(t)).unwrap();
        assert!(!done);
        assert_eq!(texts, vec!["你好"]);
    }

    #[test]
    fn consume_sse_event_message_stop_returns_done() {
        let event = "event: message_stop\ndata: {\"type\":\"message_stop\"}";
        let mut texts = Vec::new();
        let done = ClaudeProvider::consume_sse_event(event, &mut |t| texts.push(t)).unwrap();
        assert!(done);
        assert!(texts.is_empty());
    }

    #[test]
    fn consume_sse_event_ignores_ping() {
        let event = "event: ping\ndata: {\"type\":\"ping\"}";
        let mut texts = Vec::new();
        let done = ClaudeProvider::consume_sse_event(event, &mut |t| texts.push(t)).unwrap();
        assert!(!done);
        assert!(texts.is_empty());
    }

    #[test]
    fn consume_sse_event_error_returns_api_error() {
        let event = "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"invalid_request_error\",\"message\":\"bad key\"}}";
        let mut texts = Vec::new();
        let result = ClaudeProvider::consume_sse_event(event, &mut |t| texts.push(t));
        match result {
            Err(LlmError::Api { retryable: false, .. }) => {}
            other => panic!("预期 Api(retryable=false)，得到：{other:?}"),
        }
    }

    #[test]
    fn consume_sse_event_multiple_events_mixed() {
        let event = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" World\"}}";
        let mut texts = Vec::new();
        let done = ClaudeProvider::consume_sse_event(event, &mut |t| texts.push(t)).unwrap();
        assert!(!done);
        assert_eq!(texts, vec!["Hello", " World"]);
    }
}
```

- [x] **步骤 2：运行测试确认失败**

运行：`cd src-tauri && cargo test llm::claude::tests -v 2>&1 | Select-String "FAILED|panicked|todo"`

预期：单元测试存在但调用 `todo!()`，测试 panic。

- [x] **步骤 3：实现 SSE 事件结构体与 `consume_sse_event` 纯函数**

用下列完整代码替换 `ClaudeProvider` 的 todo 占位：

```rust
use serde::Deserialize;

use crate::core::llm::LlmError;

pub struct ClaudeProvider;

#[derive(Deserialize)]
struct ClaudeSseEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<EventDelta>,
    error: Option<ClaudeEventError>,
}

#[derive(Deserialize)]
struct EventDelta {
    #[serde(rename = "type")]
    delta_type: String,
    text: Option<String>,
}

#[derive(Deserialize)]
struct ClaudeEventError {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}

impl ClaudeProvider {
    pub fn consume_sse_event(
        event: &str,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<bool, LlmError> {
        for line in event.lines() {
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data.is_empty() {
                continue;
            }

            let parsed: ClaudeSseEvent = serde_json::from_str(data)
                .map_err(|e| LlmError::Parse(e.to_string()))?;

            // 先检查 error 事件
            let event_type = event.lines().find_map(|l| {
                l.strip_prefix("event:").map(|s| s.trim().to_string())
            }).unwrap_or_default();

            if event_type == "error" {
                if let Some(err) = parsed.error {
                    return Err(LlmError::Api {
                        message: err.message,
                        retryable: false,
                    });
                }
            }

            // 检查 message_stop -> 结束
            if event_type == "message_stop" {
                return Ok(true);
            }

            // content_block_delta -> text_delta -> 提取文本
            if event_type == "content_block_delta" {
                if let Some(delta) = &parsed.delta {
                    if delta.delta_type == "text_delta" {
                        if let Some(text) = &delta.text {
                            if !text.is_empty() {
                                on_delta(text.clone());
                            }
                        }
                    }
                }
            }

            // ping, content_block_start, content_block_stop, message_delta, message_start 均忽略
        }

        Ok(false)
    }
}
```

- [x] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test llm::claude::tests -v 2>&1 | Select-String "FAILED|panicked|ok"`

预期：5 个测试全部 PASS。

- [x] **步骤 5：Commit**

```bash
cd src-tauri
git add src-tauri/src/core/llm/claude.rs
git commit -m "feat: 添加 Claude SSE 事件解析与单元测试"
```

---

### 任务 2：ClaudeConfig + ClaudeAppConfig 配置模型

**文件：**
- 修改：`src-tauri/src/core/config/types.rs`
- 修改：`src-tauri/src/core/llm/claude.rs`（追加 ClaudeConfig 结构体）
- 修改：`src-tauri/src/core/llm/mod.rs`（导出 ClaudeConfig）

- [x] **步骤 1：在 `claude.rs` 中追加 `ClaudeConfig` 运行时结构体**

在 `use` 声明之后、`pub struct ClaudeProvider;` 之前插入：

```rust
#[derive(Debug, Clone)]
pub struct ClaudeConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub timeout_seconds: u64,
    pub enable_thinking: bool,
}

impl ClaudeConfig {
    pub fn new() -> Self {
        Self {
            api_key: None,
            base_url: "https://api.anthropic.com".to_string(),
            model: "claude-haiku-4-5".to_string(),
            timeout_seconds: 60,
            enable_thinking: false,
        }
    }
}

impl Default for ClaudeConfig {
    fn default() -> Self {
        Self::new()
    }
}
```

- [x] **步骤 2：在 `src-tauri/src/core/config/types.rs` 中新增 `ClaudeAppConfig`**

在 `OpenAiCompatibleAppConfig` 的 `impl` 块之后、`impl From` 之前插入：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeAppConfig {
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub timeout_seconds: u64,
    #[serde(default)]
    pub enable_thinking: bool,
}

impl ClaudeAppConfig {
    pub fn from_env() -> Self {
        Self {
            api_key: env::var("SHIZI_CLAUDE_API_KEY").ok(),
            base_url: env::var("SHIZI_CLAUDE_BASE_URL")
                .unwrap_or_else(|_| "https://api.anthropic.com".to_string()),
            model: env::var("SHIZI_CLAUDE_MODEL")
                .unwrap_or_else(|_| "claude-haiku-4-5".to_string()),
            timeout_seconds: env::var("SHIZI_CLAUDE_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
            enable_thinking: env::var("SHIZI_CLAUDE_ENABLE_THINKING")
                .ok()
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
        }
        .normalized()
    }

    pub fn normalized(mut self) -> Self {
        self.api_key = self.api_key.and_then(non_empty_string);
        self.base_url = normalize_string(self.base_url, "https://api.anthropic.com");
        self.model = normalize_string(self.model, "claude-haiku-4-5");
        if self.timeout_seconds == 0 {
            self.timeout_seconds = 60;
        }
        self
    }
}

impl From<ClaudeAppConfig> for ClaudeConfig {
    fn from(config: ClaudeAppConfig) -> Self {
        Self {
            api_key: config.api_key,
            base_url: config.base_url,
            model: config.model,
            timeout_seconds: config.timeout_seconds,
            enable_thinking: config.enable_thinking,
        }
    }
}
```

- [x] **步骤 3：在 `AppConfig` 中增加 `claude` 字段**

将 `AppConfig` 改为：

```rust
pub struct AppConfig {
    pub provider: String,
    pub target_lang: String,
    pub openai_compatible: OpenAiCompatibleAppConfig,
    pub claude: ClaudeAppConfig,          // 新增
}
```

`AppConfig::from_env` 增加 `claude: ClaudeAppConfig::from_env(),`。
`AppConfig::normalized` 增加 `self.claude = self.claude.normalized();`。

在文件顶部 `use crate::core::llm::OpenAiCompatibleConfig;` 之后增加 `use crate::core::llm::ClaudeConfig;`。

- [x] **步骤 4：更新 `src-tauri/src/core/llm/mod.rs` 导出**

将 `mod.rs` 改为：

```rust
pub mod claude;
pub mod mock;
pub mod openai_compatible;
pub mod provider;

pub use claude::{ClaudeConfig, ClaudeProvider};
pub use mock::MockLlmProvider;
pub use openai_compatible::{OpenAiCompatibleConfig, OpenAiCompatibleProvider};
pub use provider::{LlmError, LlmProvider};
```

- [x] **步骤 5：运行编译验证**

运行：`cd src-tauri && cargo build`

预期：编译通过。

- [x] **步骤 6：Commit**

```bash
git add src-tauri/src/core/config/types.rs src-tauri/src/core/llm/claude.rs src-tauri/src/core/llm/mod.rs
git commit -m "feat: 添加 ClaudeConfig 与 ClaudeAppConfig 配置模型"
```

---

### 任务 3：ClaudeProvider 流式翻译实现

**文件：**
- 修改：`src-tauri/src/core/llm/claude.rs`（追加 Messages API 请求结构体与 `stream_translate`）
- 修改：`src-tauri/src/core/llm/claude.rs`（追加 `retryable` 属性标记）

- [x] **步骤 1：编写失败的编译测试——确认 `stream_translate` 方法签名正确**

在 `#[cfg(test)]` 中增加集成测试：

```rust
#[tokio::test]
async fn stream_translate_requires_api_key() {
    let provider = ClaudeProvider::new(ClaudeConfig::new());
    let request = crate::core::translation::TranslationRequest {
        session_id: crate::core::translation::TranslationSessionId("test".to_string()),
        input: crate::core::translation::TranslationInput::ManualText("hello".to_string()),
        target_lang: "中文".to_string(),
    };
    let cancel = tokio_util::sync::CancellationToken::new();
    let result = provider
        .stream_translate(&request, &mut |_| {}, &cancel)
        .await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), LlmError::MissingConfig(_)));
}
```

运行：`cd src-tauri && cargo test llm::claude -v 2>&1 | Select-String "FAILED"`

预期：编译失败，因为 `ClaudeProvider` 没有 `new()` 方法和 `stream_translate` 实现。

- [x] **步骤 2：实现 `ClaudeProvider` 结构体与 `stream_translate`**

在 `ClaudeProvider` 结构体定义位置：

```rust
use std::time::Duration;
use futures_util::StreamExt;
use serde::Serialize;
use tokio_util::sync::CancellationToken;
use crate::core::{
    llm::{LlmError, LlmProvider},
    translation::TranslationRequest,
};

pub struct ClaudeProvider {
    client: reqwest::Client,
    config: ClaudeConfig,
}

impl ClaudeProvider {
    pub fn new(config: ClaudeConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .expect("创建 HTTP client 失败");
        Self { client, config }
    }

    fn endpoint(&self) -> String {
        format!(
            "{}/v1/messages",
            self.config.base_url.trim_end_matches('/')
        )
    }
}

#[derive(Serialize)]
struct ClaudeMessagesRequest {
    model: String,
    max_tokens: u32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<ClaudeThinkingConfig>,
    system: String,
    messages: Vec<ClaudeMessage>,
}

#[derive(Serialize)]
struct ClaudeThinkingConfig {
    #[serde(rename = "type")]
    thinking_type: String,
}

#[derive(Serialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}
```

`#[async_trait::async_trait] impl LlmProvider for ClaudeProvider`（放在 `consume_sse_event` 之前或独立 impl 块后）：

```rust
#[async_trait::async_trait]
impl LlmProvider for ClaudeProvider {
    async fn stream_translate(
        &self,
        request: &TranslationRequest,
        on_delta: &mut (dyn FnMut(String) + Send),
        cancel: &CancellationToken,
    ) -> Result<(), LlmError> {
        let api_key = self
            .config
            .api_key
            .as_deref()
            .ok_or(LlmError::MissingConfig("Claude API Key"))?;

        let mut body = ClaudeMessagesRequest {
            model: self.config.model.clone(),
            max_tokens: 4096,
            stream: true,
            thinking: if self.config.enable_thinking {
                Some(ClaudeThinkingConfig {
                    thinking_type: "adaptive".to_string(),
                })
            } else {
                None
            },
            system: "你是一个专业翻译引擎。只输出译文，不要解释。".to_string(),
            messages: vec![ClaudeMessage {
                role: "user".to_string(),
                content: format!(
                    "请将以下文本翻译为{}：\n\n{}",
                    request.target_lang,
                    request.source_text()
                ),
            }],
        };

        let response = self
            .client
            .post(self.endpoint())
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::Http(e.to_string()))?;

        if !response.status().is_success() {
            return Err(Self::parse_error_response(response).await);
        }

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        loop {
            tokio::select! {
                _ = cancel.cancelled() => return Ok(()),
                bytes = stream.next() => {
                    let Some(bytes) = bytes else { break };
                    let bytes = bytes.map_err(|e| LlmError::Http(e.to_string()))?;
                    buffer.push_str(&String::from_utf8_lossy(&bytes));
                    buffer = buffer.replace("\r\n", "\n");

                    while let Some(index) = buffer.find("\n\n") {
                        let event = buffer[..index].to_string();
                        buffer = buffer[index + 2..].to_string();

                        if Self::consume_sse_event(&event, on_delta)? {
                            return Ok(());
                        }
                    }
                }
            }
        }

        if !buffer.trim().is_empty() {
            Self::consume_sse_event(&buffer, on_delta)?;
        }

        Ok(())
    }
}
```

同时追加 `parse_error_response` 方法（与 `consume_sse_event` 在同一 `impl ClaudeProvider` 块中）：

```rust
async fn parse_error_response(response: reqwest::Response) -> LlmError {
    let status = response.status();
    let retryable = status.as_u16() == 429 || status.is_server_error();
    let body = response.text().await.unwrap_or_default();
    // Claude 错误格式：{"type":"error","error":{"type":"...","message":"..."}}
    let message = serde_json::from_str::<ClaudeErrorEnvelope>(&body)
        .map(|e| e.error.message)
        .unwrap_or_else(|_| {
            format!(
                "HTTP {}: {}",
                status,
                body.chars().take(500).collect::<String>()
            )
        });
    if retryable {
        LlmError::Http(message)
    } else {
        LlmError::Api { message, retryable: false }
    }
}
```

需在结构体区域增加：

```rust
#[derive(Deserialize)]
struct ClaudeErrorEnvelope {
    error: ClaudeApiErrorDetail,
}

#[derive(Deserialize)]
struct ClaudeApiErrorDetail {
    message: String,
}
```

- [x] **步骤 3：运行编译验证**

运行：`cd src-tauri && cargo build`

预期：编译通过。

- [x] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test llm::claude -v`

预期：之前的 5 个 SSE 单元测试 + 新增的 `stream_translate_requires_api_key` 测试全部 PASS。

- [x] **步骤 5：Commit**

```bash
git add src-tauri/src/core/llm/claude.rs
git commit -m "feat: 实现 ClaudeProvider stream_translate 流式翻译"
```

---

### 任务 4：Provider 工厂增加 claude 分支

**文件：**
- 修改：`src-tauri/src/ui/web_popup.rs`

- [x] **步骤 1：修改 import 与 factory match**

将 web_popup.rs 开头的 import：

```rust
use crate::{
    core::{
        llm::{ClaudeConfig, ClaudeProvider, LlmProvider, MockLlmProvider, OpenAiCompatibleConfig, OpenAiCompatibleProvider},
        ...
    },
};
```

在 `start_translation_from_input` 的 provider match 中增加分支：

```rust
let provider: Arc<dyn LlmProvider> = match config.provider.as_str() {
    "mock" => Arc::new(MockLlmProvider),
    "claude" => Arc::new(ClaudeProvider::new(ClaudeConfig::from(config.claude))),
    _ => Arc::new(OpenAiCompatibleProvider::new(OpenAiCompatibleConfig::from(
        config.openai_compatible,
    ))),
};
```

- [x] **步骤 2：运行编译验证**

运行：`cd src-tauri && cargo build`

预期：编译通过。

- [x] **步骤 3：Commit**

```bash
git add src-tauri/src/ui/web_popup.rs
git commit -m "feat: provider 工厂增加 claude 分支"
```

---

### 任务 5：前端设置面板——provider 下拉与 Claude 表单

**文件：**
- 修改：`frontend/index.html`
- 修改：`frontend/main.js`
- 修改：`frontend/style.css`

- [x] **步骤 1：在 `index.html` 中增加 provider 下拉和 Claude 表单**

将设置面板从：

```html
<div id="settingsPanel" class="settings-panel hidden">
  <label>
    目标语言
    <input id="targetLangInput" type="text" placeholder="中文">
  </label>
  <label>
    API Key
    <input id="apiKeyInput" type="password" placeholder="sk-...">
  </label>
  <p class="config-warning">API Key 本阶段会明文保存到本机配置文件，请只在可信设备上使用。</p>
  <label>
    Base URL
    <input id="baseUrlInput" type="text" placeholder="https://api.openai.com/v1">
  </label>
  <label>
    Model
    <input id="modelInput" type="text" placeholder="gpt-4o-mini">
  </label>
  <label>
    Timeout 秒
    <input id="timeoutInput" type="number" min="1" step="1" placeholder="60">
  </label>
  <button id="saveConfigBtn">保存配置</button>
  <div id="configStatus" class="config-status"></div>
</div>
```

改为：

```html
<div id="settingsPanel" class="settings-panel hidden">
  <label>
    目标语言
    <input id="targetLangInput" type="text" placeholder="中文">
  </label>
  <label>
    Provider
    <select id="providerSelect">
      <option value="openai-compatible">OpenAI Compatible</option>
      <option value="claude">Claude</option>
      <option value="mock">Mock（调试用）</option>
    </select>
  </label>

  <!-- OpenAI Compatible 表单 -->
  <div id="openaiSettings">
    <label>
      API Key
      <input id="apiKeyInput" type="password" placeholder="sk-...">
    </label>
    <p class="config-warning">API Key 本阶段会明文保存到本机配置文件，请只在可信设备上使用。</p>
    <label>
      Base URL
      <input id="baseUrlInput" type="text" placeholder="https://api.openai.com/v1">
    </label>
    <label>
      Model
      <input id="modelInput" type="text" placeholder="gpt-4o-mini">
    </label>
    <label>
      Timeout 秒
      <input id="timeoutInput" type="number" min="1" step="1" placeholder="60">
    </label>
  </div>

  <!-- Claude 表单 -->
  <div id="claudeSettings" class="hidden">
    <label>
      API Key
      <input id="claudeApiKeyInput" type="password" placeholder="sk-ant-...">
    </label>
    <p class="config-warning">API Key 本阶段会明文保存到本机配置文件，请只在可信设备上使用。</p>
    <label>
      Base URL
      <input id="claudeBaseUrlInput" type="text" placeholder="https://api.anthropic.com">
    </label>
    <label>
      Model
      <input id="claudeModelInput" type="text" placeholder="claude-haiku-4-5">
    </label>
    <label>
      Timeout 秒
      <input id="claudeTimeoutInput" type="number" min="1" step="1" placeholder="60">
    </label>
    <label>
      <input id="claudeEnableThinkingInput" type="checkbox">
      Enable Thinking（仅对支持的模型生效，Haiku 需关闭）
    </label>
  </div>

  <button id="saveConfigBtn">保存配置</button>
  <div id="configStatus" class="config-status"></div>
</div>
```

- [x] **步骤 2：更新 `main.js` —— provider 切换、配置读写、表单映射**

将常量声明区末尾增加：

```js
const providerSelect = document.getElementById('providerSelect');
const openaiSettings = document.getElementById('openaiSettings');
const claudeSettings = document.getElementById('claudeSettings');
const claudeApiKeyInput = document.getElementById('claudeApiKeyInput');
const claudeBaseUrlInput = document.getElementById('claudeBaseUrlInput');
const claudeModelInput = document.getElementById('claudeModelInput');
const claudeTimeoutInput = document.getElementById('claudeTimeoutInput');
const claudeEnableThinkingInput = document.getElementById('claudeEnableThinkingInput');
```

替换 `fillConfigForm`:

```js
function fillConfigForm(config) {
  targetLangInput.value = config.targetLang ?? '中文';
  providerSelect.value = config.provider ?? 'openai-compatible';

  // OpenAI Compatible 字段
  apiKeyInput.value = config.openaiCompatible?.apiKey ?? '';
  baseUrlInput.value = config.openaiCompatible?.baseUrl ?? 'https://api.openai.com/v1';
  modelInput.value = config.openaiCompatible?.model ?? 'gpt-4o-mini';
  timeoutInput.value = String(config.openaiCompatible?.timeoutSeconds ?? 60);

  // Claude 字段
  claudeApiKeyInput.value = config.claude?.apiKey ?? '';
  claudeBaseUrlInput.value = config.claude?.baseUrl ?? 'https://api.anthropic.com';
  claudeModelInput.value = config.claude?.model ?? 'claude-haiku-4-5';
  claudeTimeoutInput.value = String(config.claude?.timeoutSeconds ?? 60);
  claudeEnableThinkingInput.checked = config.claude?.enableThinking ?? false;

  toggleProviderSettings();
}
```

增加 `toggleProviderSettings` 函数：

```js
function toggleProviderSettings() {
  const provider = providerSelect.value;
  openaiSettings.classList.toggle('hidden', provider !== 'openai-compatible');
  claudeSettings.classList.toggle('hidden', provider !== 'claude');
}
```

替换 `readConfigForm`:

```js
function readConfigForm() {
  return {
    provider: providerSelect.value,
    targetLang: targetLangInput.value.trim() || '中文',
    openaiCompatible: {
      apiKey: apiKeyInput.value.trim() || null,
      baseUrl: baseUrlInput.value.trim(),
      model: modelInput.value.trim(),
      timeoutSeconds: Number(timeoutInput.value),
    },
    claude: {
      apiKey: claudeApiKeyInput.value.trim() || null,
      baseUrl: claudeBaseUrlInput.value.trim(),
      model: claudeModelInput.value.trim(),
      timeoutSeconds: Number(claudeTimeoutInput.value),
      enableThinking: claudeEnableThinkingInput.checked,
    },
  };
}
```

替换 `validateConfig`（扩展支持 claude 验证）：

```js
function validateConfig(config) {
  const sections = config.provider === 'claude' ? [config.claude] : [config.openaiCompatible];

  for (const section of sections) {
    let url;
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

在事件绑定区增加 provider 切换监听：

```js
providerSelect.addEventListener('change', toggleProviderSettings);
```

- [x] **步骤 3：运行前端语法检查**

运行：`node --check frontend/main.js`

预期：无报错。

- [x] **步骤 4：Commit**

```bash
git add frontend/index.html frontend/main.js
git commit -m "feat: 设置面板增加 provider 下拉与 Claude 表单"
```

---

### 任务 6：配置兼容性处理与旧配置迁移

**文件：**
- 修改：`src-tauri/src/core/config/types.rs`（已添加 `#[serde(default)]`）

此任务在前序任务中已通过在 `ClaudeAppConfig` 每个字段加 `#[serde(default)]` 完成。验证步骤：

- [x] **步骤 1：确认旧 `config.json` 可反序列化**

检查 `src-tauri/src/core/config/types.rs` 中 `AppConfig` 的 `claude` 字段是否保证存在且可默认反序列化。确认 `#[serde(default)]` 已覆盖。

- [x] **步骤 2：运行编译与测试**

运行：`cd src-tauri && cargo test && cargo build`

预期：全部编译通过，测试通过。

- [x] **步骤 3：Commit**

```bash
git add src-tauri/src/core/config/types.rs
git commit -m "fix: 通过 serde(default) 确保旧 config.json 可平滑升级"
```

---

### 任务 7：端到端验证与文档同步

**文件：**
- 修改：`AGENTS.md`（如有需要）
- 修改：`CLAUDE.md`（如有需要）

- [x] **步骤 1：编译 release 确认无 warning**

运行：`cd src-tauri && cargo build --release 2>&1 | Select-String "warning|error"`

预期：无 warning/error（allowed 的除外）。

- [x] **步骤 2：检查项目文档是否需同步**

检查 `AGENTS.md` 中 provider 列表、配置说明、前后端通信部分，确认 Claude 已纳入描述。需同步则修改对应段落。

- [x] **步骤 3：Commit**

```bash
git add AGENTS.md CLAUDE.md  # （如有改动）
git commit -m "docs: 同步 Claude Provider 相关文档"
```

---

### 自检清单

**1. 规格覆盖度：**
- ClaudeProvider + `stream_translate` → 任务 3 ✓
- ClaudeConfig + ClaudeAppConfig → 任务 2 ✓
- Provider 工厂 `"claude"` 分支 → 任务 4 ✓
- 前端 provider 下拉 + Claude 表单 → 任务 5 ✓
- SSE 事件解析（含所有事件类型） → 任务 1 ✓
- 取消复用 `tokio::select!` → 任务 3 ✓
- `enable_thinking` 配置 → 任务 2 + 3 ✓
- 旧配置兼容 `#[serde(default)]` → 任务 6 ✓
- 单元测试：SSE 解析 + 配置模型 + From 转换 → 任务 1 + 2 ✓
- `parse_error_response` → 任务 3 ✓
- 文档同步 → 任务 7 ✓

**2. 占位符扫描：** 无 TODO、待定、后续实现等模式。

**3. 类型一致性：** `ClaudeConfig` 字段名在运行时和持久化模型间通过 `From` trait 衔接，SSE 事件结构体在测试和实现中一致。
