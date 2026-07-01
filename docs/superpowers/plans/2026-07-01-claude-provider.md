# Claude 专用 Provider 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 为翻译链路新增 Anthropic Claude 原生 Messages API provider，用户可在设置面板选择 Claude 作为翻译后端，复用既有 `LlmProvider` trait、流式 SSE 与取消链路。

**架构：** 新增 `ClaudeProvider`（`src-tauri/src/core/llm/claude.rs`），与 [openai_compatible.rs](../../../src-tauri/src/core/llm/openai_compatible.rs) 同构：持有 `reqwest::Client` + `ClaudeConfig`，`stream_translate` 走 `POST /v1/messages` 的 SSE 流式响应，用 `response.bytes_stream()` + `tokio::select!`（`cancel.cancelled()` 与 `stream.next()` 竞速）。新增 `ClaudeAppConfig` 持久化模型挂到 `AppConfig.claude`（`#[serde(default)]` 兼容旧配置）。provider 工厂（`web_popup.rs`）增加 `"claude"` 分支。前端设置面板重构为 provider 下拉 + 按 provider 切换的表单。

**技术栈：** Rust（edition 2021）、Tauri 2、reqwest、tokio（`tokio::select!`、`tokio_util::sync::CancellationToken`）、futures_util（`StreamExt`）、serde、async_trait、thiserror；原生 HTML/JS/CSS 前端（无构建步骤）。

**规格来源：** [docs/superpowers/specs/2026-07-01-claude-provider-design.md](../specs/2026-07-01-claude-provider-design.md)

---

## 文件结构

| 文件 | 职责 | 动作 |
|---|---|---|
| `src-tauri/src/core/llm/claude.rs` | `ClaudeConfig` 运行时配置 + `ClaudeProvider`（new/endpoint/request_body/parse_error_response/consume_sse_event/stream_translate）+ SSE 反序列化结构 + 单元测试 | 创建 |
| `src-tauri/src/core/llm/mod.rs` | 注册 `claude` 模块并 re-export `ClaudeConfig`/`ClaudeProvider` | 修改 |
| `src-tauri/src/core/config/types.rs` | `ClaudeAppConfig` 持久化模型 + 默认常量 + `AppConfig.claude` 字段 + `from_env`/`normalized` 扩展 + `From<ClaudeAppConfig> for ClaudeConfig` + 单元测试 | 修改 |
| `src-tauri/src/ui/web_popup.rs` | provider 工厂 `match` 增加 `"claude"` 分支，import 增加 `ClaudeConfig`/`ClaudeProvider` | 修改 |
| `frontend/index.html` | 设置面板：provider 下拉 + 按 provider 分组的表单（OpenAI 组 / Claude 组） | 修改 |
| `frontend/style.css` | `select` 样式 + `.checkbox-row` 横向布局 | 修改 |
| `frontend/main.js` | `fillConfigForm`/`readConfigForm`/`validateConfig` 扩展为多 provider + provider 下拉切换可见性 | 修改 |
| `CLAUDE.md` / `AGENTS.md` | 同步项目结构 llm 行、配置存储段 | 修改 |

依赖顺序（被依赖者先做）：任务 1（ClaudeConfig 骨架）→ 任务 2（ClaudeAppConfig + From，依赖 ClaudeConfig）→ 任务 3（consume_sse_event 纯函数）→ 任务 4（stream_translate，依赖任务 3 的结构）→ 任务 5（工厂，依赖 trait 实现）→ 任务 6（前端）→ 任务 7（文档）。

---

## 任务 1：创建 claude.rs 模块骨架并注册

**文件：**
- 创建：`src-tauri/src/core/llm/claude.rs`
- 修改：`src-tauri/src/core/llm/mod.rs`

- [ ] **步骤 1：创建 claude.rs 骨架**

创建 `src-tauri/src/core/llm/claude.rs`，写入以下内容。此阶段只定义 `ClaudeConfig` 运行时配置与 `ClaudeProvider` struct + `new()`；SSE 解析、请求构造、trait 实现分别在任务 3、任务 4 追加。

```rust
use std::time::Duration;

pub struct ClaudeProvider {
    client: reqwest::Client,
    config: ClaudeConfig,
}

#[derive(Debug, Clone)]
pub struct ClaudeConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub timeout_seconds: u64,
    pub enable_thinking: bool,
}

impl ClaudeProvider {
    pub fn new(config: ClaudeConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .expect("创建 HTTP client 失败");

        Self { client, config }
    }
}
```

- [ ] **步骤 2：注册 claude 模块并 re-export**

将 `src-tauri/src/core/llm/mod.rs` 全文替换为：

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

- [ ] **步骤 3：编译验证**

运行：`cd src-tauri && cargo build`
预期：编译成功（可能有 unused 警告，本阶段 `ClaudeProvider` 尚未被使用，属正常）。

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/core/llm/claude.rs src-tauri/src/core/llm/mod.rs
git commit -m "feat(llm): 新增 ClaudeProvider 模块骨架与 ClaudeConfig"
```

---

## 任务 2：ClaudeAppConfig 持久化配置 + AppConfig 扩展（TDD）

**文件：**
- 修改：`src-tauri/src/core/config/types.rs`

本任务先写失败测试，再实现 `ClaudeAppConfig`、默认常量、`AppConfig.claude` 字段与 `From` 转换，并验证旧 `config.json`（缺 `claude` 字段）可平滑反序列化。

- [ ] **步骤 1：编写失败的测试**

在 `src-tauri/src/core/config/types.rs` 文件末尾追加测试模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_app_config_default_then_normalized_uses_defaults() {
        let config = ClaudeAppConfig::default().normalized();
        assert_eq!(config.base_url, DEFAULT_CLAUDE_BASE_URL);
        assert_eq!(config.model, DEFAULT_CLAUDE_MODEL);
        assert_eq!(config.timeout_seconds, DEFAULT_TIMEOUT_SECONDS);
        assert!(!config.enable_thinking);
        assert!(config.api_key.is_none());
    }

    #[test]
    fn claude_app_config_normalized_fills_empty_strings() {
        let config = ClaudeAppConfig {
            api_key: Some("   ".to_string()),
            base_url: "".to_string(),
            model: "".to_string(),
            timeout_seconds: 0,
            enable_thinking: false,
        }
        .normalized();
        assert!(config.api_key.is_none());
        assert_eq!(config.base_url, DEFAULT_CLAUDE_BASE_URL);
        assert_eq!(config.model, DEFAULT_CLAUDE_MODEL);
        assert_eq!(config.timeout_seconds, DEFAULT_TIMEOUT_SECONDS);
    }

    #[test]
    fn claude_app_config_from_env_reads_overrides() {
        std::env::set_var("SHIZI_CLAUDE_API_KEY", "sk-claude-test");
        std::env::set_var("SHIZI_CLAUDE_BASE_URL", "https://gateway.example.com");
        std::env::set_var("SHIZI_CLAUDE_MODEL", "claude-haiku-4-5");
        std::env::set_var("SHIZI_CLAUDE_TIMEOUT_SECS", "120");
        std::env::set_var("SHIZI_CLAUDE_ENABLE_THINKING", "true");

        let config = ClaudeAppConfig::from_env();

        std::env::remove_var("SHIZI_CLAUDE_API_KEY");
        std::env::remove_var("SHIZI_CLAUDE_BASE_URL");
        std::env::remove_var("SHIZI_CLAUDE_MODEL");
        std::env::remove_var("SHIZI_CLAUDE_TIMEOUT_SECS");
        std::env::remove_var("SHIZI_CLAUDE_ENABLE_THINKING");

        assert_eq!(config.api_key.as_deref(), Some("sk-claude-test"));
        assert_eq!(config.base_url, "https://gateway.example.com");
        assert_eq!(config.model, "claude-haiku-4-5");
        assert_eq!(config.timeout_seconds, 120);
        assert!(config.enable_thinking);
    }

    #[test]
    fn from_claude_app_config_maps_all_fields() {
        let app_config = ClaudeAppConfig {
            api_key: Some("sk-test".to_string()),
            base_url: "https://api.anthropic.com".to_string(),
            model: "claude-haiku-4-5".to_string(),
            timeout_seconds: 90,
            enable_thinking: true,
        };
        let runtime: ClaudeConfig = app_config.into();
        assert_eq!(runtime.api_key.as_deref(), Some("sk-test"));
        assert_eq!(runtime.base_url, "https://api.anthropic.com");
        assert_eq!(runtime.model, "claude-haiku-4-5");
        assert_eq!(runtime.timeout_seconds, 90);
        assert!(runtime.enable_thinking);
    }

    #[test]
    fn app_config_deserializes_without_claude_field() {
        let json = r#"{
            "provider": "openai-compatible",
            "targetLang": "中文",
            "openaiCompatible": {
                "apiKey": "sk-x",
                "baseUrl": "https://api.openai.com/v1",
                "model": "gpt-4o-mini",
                "timeoutSeconds": 60
            }
        }"#;
        let config: AppConfig = serde_json::from_str(json)
            .expect("旧配置缺少 claude 字段应可反序列化")
            .normalized();
        assert_eq!(config.provider, "openai-compatible");
        assert_eq!(config.claude.base_url, DEFAULT_CLAUDE_BASE_URL);
        assert_eq!(config.claude.model, DEFAULT_CLAUDE_MODEL);
        assert_eq!(config.claude.timeout_seconds, DEFAULT_TIMEOUT_SECONDS);
        assert!(!config.claude.enable_thinking);
        assert!(config.claude.api_key.is_none());
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib config::types`
预期：编译失败，报错 `cannot find type 'ClaudeAppConfig'` / `cannot find value 'DEFAULT_CLAUDE_BASE_URL'` 等（类型与常量尚未定义）。

- [ ] **步骤 3：新增 Claude 默认常量**

用 Edit 将：

```rust
const DEFAULT_TIMEOUT_SECONDS: u64 = 60;
```

替换为：

```rust
const DEFAULT_TIMEOUT_SECONDS: u64 = 60;
const DEFAULT_CLAUDE_BASE_URL: &str = "https://api.anthropic.com";
const DEFAULT_CLAUDE_MODEL: &str = "claude-haiku-4-5";
```

- [ ] **步骤 4：import 增加 ClaudeConfig**

用 Edit 将：

```rust
use crate::core::llm::OpenAiCompatibleConfig;
```

替换为：

```rust
use crate::core::llm::{ClaudeConfig, OpenAiCompatibleConfig};
```

- [ ] **步骤 5：AppConfig 增加 claude 字段**

用 Edit 将：

```rust
pub struct AppConfig {
    pub provider: String,
    pub target_lang: String,
    pub openai_compatible: OpenAiCompatibleAppConfig,
}
```

替换为：

```rust
pub struct AppConfig {
    pub provider: String,
    pub target_lang: String,
    pub openai_compatible: OpenAiCompatibleAppConfig,
    #[serde(default)]
    pub claude: ClaudeAppConfig,
}
```

- [ ] **步骤 6：AppConfig::from_env 与 normalized 接入 claude**

用 Edit 将：

```rust
    pub fn from_env() -> Self {
        Self {
            provider: env::var("SHIZI_LLM_PROVIDER")
                .unwrap_or_else(|_| DEFAULT_PROVIDER.to_string()),
            target_lang: env::var("SHIZI_TARGET_LANG")
                .unwrap_or_else(|_| DEFAULT_TARGET_LANG.to_string()),
            openai_compatible: OpenAiCompatibleAppConfig::from_env(),
        }
        .normalized()
    }

    pub fn normalized(mut self) -> Self {
        self.provider = normalize_string(self.provider, DEFAULT_PROVIDER);
        self.target_lang = normalize_string(self.target_lang, DEFAULT_TARGET_LANG);
        self.openai_compatible = self.openai_compatible.normalized();
        self
    }
```

替换为：

```rust
    pub fn from_env() -> Self {
        Self {
            provider: env::var("SHIZI_LLM_PROVIDER")
                .unwrap_or_else(|_| DEFAULT_PROVIDER.to_string()),
            target_lang: env::var("SHIZI_TARGET_LANG")
                .unwrap_or_else(|_| DEFAULT_TARGET_LANG.to_string()),
            openai_compatible: OpenAiCompatibleAppConfig::from_env(),
            claude: ClaudeAppConfig::from_env(),
        }
        .normalized()
    }

    pub fn normalized(mut self) -> Self {
        self.provider = normalize_string(self.provider, DEFAULT_PROVIDER);
        self.target_lang = normalize_string(self.target_lang, DEFAULT_TARGET_LANG);
        self.openai_compatible = self.openai_compatible.normalized();
        self.claude = self.claude.normalized();
        self
    }
```

- [ ] **步骤 7：新增 ClaudeAppConfig 结构、from_env/normalized、From 转换**

用 Edit 将：

```rust
impl From<OpenAiCompatibleAppConfig> for OpenAiCompatibleConfig {
    fn from(config: OpenAiCompatibleAppConfig) -> Self {
        Self {
            api_key: config.api_key,
            base_url: config.base_url,
            model: config.model,
            timeout_seconds: config.timeout_seconds,
        }
    }
}
```

替换为：

```rust
impl From<OpenAiCompatibleAppConfig> for OpenAiCompatibleConfig {
    fn from(config: OpenAiCompatibleAppConfig) -> Self {
        Self {
            api_key: config.api_key,
            base_url: config.base_url,
            model: config.model,
            timeout_seconds: config.timeout_seconds,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeAppConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub timeout_seconds: u64,
    pub enable_thinking: bool,
}

impl ClaudeAppConfig {
    pub fn from_env() -> Self {
        Self {
            api_key: env::var("SHIZI_CLAUDE_API_KEY").ok(),
            base_url: env::var("SHIZI_CLAUDE_BASE_URL")
                .unwrap_or_else(|_| DEFAULT_CLAUDE_BASE_URL.to_string()),
            model: env::var("SHIZI_CLAUDE_MODEL")
                .unwrap_or_else(|_| DEFAULT_CLAUDE_MODEL.to_string()),
            timeout_seconds: env::var("SHIZI_CLAUDE_TIMEOUT_SECS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(DEFAULT_TIMEOUT_SECONDS),
            enable_thinking: env::var("SHIZI_CLAUDE_ENABLE_THINKING")
                .map(|value| value.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
        }
        .normalized()
    }

    pub fn normalized(mut self) -> Self {
        self.api_key = self.api_key.and_then(non_empty_string);
        self.base_url = normalize_string(self.base_url, DEFAULT_CLAUDE_BASE_URL);
        self.model = normalize_string(self.model, DEFAULT_CLAUDE_MODEL);
        if self.timeout_seconds == 0 {
            self.timeout_seconds = DEFAULT_TIMEOUT_SECONDS;
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

- [ ] **步骤 8：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib config::types`
预期：5 个测试全部 PASS（`claude_app_config_default_then_normalized_uses_defaults`、`claude_app_config_normalized_fills_empty_strings`、`claude_app_config_from_env_reads_overrides`、`from_claude_app_config_maps_all_fields`、`app_config_deserializes_without_claude_field`）。

- [ ] **步骤 9：全量构建与测试确认无回归**

运行：`cd src-tauri && cargo build && cargo test`
预期：编译成功；全部测试 PASS（含既有 web_popup 的 `app_state()` 测试，它调用 `AppConfig::from_env()` 现会读取 `SHIZI_CLAUDE_*`，但断言不涉及 claude 字段，不受影响）。

- [ ] **步骤 10：Commit**

```bash
git add src-tauri/src/core/config/types.rs
git commit -m "feat(config): ClaudeAppConfig 持久化配置与 AppConfig 扩展"
```

---

## 任务 3：Claude SSE 事件解析纯函数（TDD）

**文件：**
- 修改：`src-tauri/src/core/llm/claude.rs`

`consume_sse_event` 是无 IO 的纯函数，先 TDD 覆盖各事件类型，再实现。解析规则见规格 4.2 节。

- [ ] **步骤 1：替换 import 块以引入 serde 与 LlmError**

用 Edit 将 `src-tauri/src/core/llm/claude.rs` 顶部的：

```rust
use std::time::Duration;

pub struct ClaudeProvider {
```

替换为：

```rust
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::core::llm::LlmError;

pub struct ClaudeProvider {
```

说明：`futures_util::StreamExt`、`tokio_util::sync::CancellationToken`、`LlmProvider`、`TranslationRequest` 将在任务 4 引入；本任务后这些 import 暂未使用，`cargo build` 会有 unused 警告，任务 4 完成后消除。

- [ ] **步骤 2：编写失败的 SSE 解析测试**

在 `src-tauri/src/core/llm/claude.rs` 文件末尾追加测试模块：

```rust
#[cfg(test)]
mod consume_sse_tests {
    use super::*;

    fn collect(event: &str) -> (Vec<String>, Result<bool, LlmError>) {
        let mut deltas = Vec::new();
        let result = ClaudeProvider::consume_sse_event(event, &mut |text| {
            deltas.push(text);
        });
        (deltas, result)
    }

    #[test]
    fn extracts_text_delta() {
        let event = concat!(
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"你好\"}}"
        );
        let (deltas, result) = collect(event);
        assert_eq!(result.unwrap(), false);
        assert_eq!(deltas, vec!["你好".to_string()]);
    }

    #[test]
    fn stops_on_message_stop() {
        let event = "event: message_stop\ndata: {\"type\":\"message_stop\"}";
        let (_, result) = collect(event);
        assert_eq!(result.unwrap(), true);
    }

    #[test]
    fn ignores_ping_and_message_start() {
        let event = concat!(
            "event: ping\n",
            "data: {\"type\":\"ping\"}\n",
            "\n",
            "event: message_start\n",
            "data: {\"type\":\"message_start\",\"message\":{}}"
        );
        let (deltas, result) = collect(event);
        assert_eq!(result.unwrap(), false);
        assert!(deltas.is_empty());
    }

    #[test]
    fn returns_api_error_on_error_event() {
        let event = concat!(
            "event: error\n",
            "data: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"服务过载\"}}"
        );
        let (deltas, result) = collect(event);
        assert!(deltas.is_empty());
        match result {
            Err(LlmError::Api { message, retryable }) => {
                assert_eq!(message, "服务过载");
                assert!(!retryable);
            }
            other => panic!("期望 Api 错误，得到 {:?}", other),
        }
    }

    #[test]
    fn maps_refusal_stop_reason() {
        let event = concat!(
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"refusal\",\"stop_sequence\":null}}"
        );
        let (_, result) = collect(event);
        match result {
            Err(LlmError::Api { message, retryable }) => {
                assert_eq!(message, "翻译被拒绝");
                assert!(!retryable);
            }
            other => panic!("期望 refusal 错误，得到 {:?}", other),
        }
    }

    #[test]
    fn handles_mixed_event_sequence() {
        let events = [
            concat!(
                "event: message_start\n",
                "data: {\"type\":\"message_start\",\"message\":{}}"
            ),
            concat!(
                "event: content_block_delta\n",
                "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"你好\"}}"
            ),
            concat!(
                "event: content_block_delta\n",
                "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"世界\"}}"
            ),
            "event: message_stop\ndata: {\"type\":\"message_stop\"}",
        ];
        let mut deltas = Vec::new();
        let mut stopped = false;
        for event in events {
            if ClaudeProvider::consume_sse_event(event, &mut |t| deltas.push(t)).unwrap() {
                stopped = true;
                break;
            }
        }
        assert_eq!(deltas, vec!["你好".to_string(), "世界".to_string()]);
        assert!(stopped);
    }
}
```

- [ ] **步骤 3：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib llm::claude::consume_sse_tests`
预期：编译失败，报错 `no function named 'consume_sse_event'` / `cannot find type 'ClaudeStreamEvent'` 等。

- [ ] **步骤 4：实现 SSE 反序列化结构与 consume_sse_event**

在 `src-tauri/src/core/llm/claude.rs` 中，用 Edit 将现有的：

```rust
impl ClaudeProvider {
    pub fn new(config: ClaudeConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .expect("创建 HTTP client 失败");

        Self { client, config }
    }
}
```

替换为：

```rust
#[derive(Deserialize)]
struct ClaudeStreamEvent {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    delta: Option<ClaudeDelta>,
    #[serde(default)]
    error: Option<ClaudeErrorBody>,
}

#[derive(Deserialize)]
struct ClaudeDelta {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
struct ClaudeErrorBody {
    #[allow(dead_code)]
    #[serde(rename = "type", default)]
    kind: String,
    message: String,
}

impl ClaudeProvider {
    pub fn new(config: ClaudeConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .expect("创建 HTTP client 失败");

        Self { client, config }
    }

    fn consume_sse_event(
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

            let parsed = serde_json::from_str::<ClaudeStreamEvent>(data)
                .map_err(|error| LlmError::Parse(error.to_string()))?;

            if let Some(error) = parsed.error {
                return Err(LlmError::Api {
                    message: error.message,
                    retryable: false,
                });
            }

            match parsed.kind.as_str() {
                "content_block_delta" => {
                    if let Some(delta) = parsed.delta {
                        if delta.kind == "text_delta" {
                            if let Some(text) = delta.text {
                                if !text.is_empty() {
                                    on_delta(text);
                                }
                            }
                        }
                    }
                }
                "message_delta" => {
                    if let Some(delta) = parsed.delta {
                        if delta.stop_reason.as_deref() == Some("refusal") {
                            return Err(LlmError::Api {
                                message: "翻译被拒绝".to_string(),
                                retryable: false,
                            });
                        }
                    }
                }
                "message_stop" => return Ok(true),
                _ => {}
            }
        }

        Ok(false)
    }
}
```

- [ ] **步骤 5：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib llm::claude::consume_sse_tests`
预期：6 个测试全部 PASS。

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/core/llm/claude.rs
git commit -m "feat(llm): Claude SSE 事件解析纯函数"
```

---

## 任务 4：ClaudeProvider 请求构造与流式翻译实现

**文件：**
- 修改：`src-tauri/src/core/llm/claude.rs`

补齐 `endpoint`/`request_body`/`parse_error_response` 与 `LlmProvider` trait 实现，并补 `request_body` 序列化测试。

- [ ] **步骤 1：补齐 stream_translate 所需 import**

用 Edit 将 `src-tauri/src/core/llm/claude.rs` 顶部的：

```rust
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::core::llm::LlmError;

pub struct ClaudeProvider {
```

替换为：

```rust
use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

use tokio_util::sync::CancellationToken;

use crate::core::{
    llm::{LlmError, LlmProvider},
    translation::TranslationRequest,
};

pub struct ClaudeProvider {
```

- [ ] **步骤 2：编写失败的 request_body 序列化测试**

在 `src-tauri/src/core/llm/claude.rs` 文件末尾追加测试模块：

```rust
#[cfg(test)]
mod request_body_tests {
    use super::*;
    use crate::core::translation::{TranslationInput, TranslationRequest, TranslationSessionId};

    fn request() -> TranslationRequest {
        TranslationRequest {
            session_id: TranslationSessionId("test".to_string()),
            input: TranslationInput::ManualText("hello".to_string()),
            target_lang: "中文".to_string(),
        }
    }

    fn provider(enable_thinking: bool) -> ClaudeProvider {
        ClaudeProvider::new(ClaudeConfig {
            api_key: Some("sk-test".to_string()),
            base_url: "https://api.anthropic.com".to_string(),
            model: "claude-haiku-4-5".to_string(),
            timeout_seconds: 60,
            enable_thinking,
        })
    }

    #[test]
    fn includes_system_and_user_message_without_thinking() {
        let value = serde_json::to_value(&provider(false).request_body(&request())).unwrap();
        assert_eq!(value["model"], "claude-haiku-4-5");
        assert_eq!(value["max_tokens"], 4096);
        assert_eq!(value["stream"], true);
        assert_eq!(value["system"], "你是一个专业翻译引擎。只输出译文，不要解释。");
        assert_eq!(value["messages"][0]["role"], "user");
        assert!(value["messages"][0]["content"].as_str().unwrap().contains("hello"));
        assert!(value.get("thinking").is_none());
    }

    #[test]
    fn includes_adaptive_thinking_when_enabled() {
        let value = serde_json::to_value(&provider(true).request_body(&request())).unwrap();
        assert_eq!(value["thinking"]["type"], "adaptive");
    }
}
```

- [ ] **步骤 3：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib llm::claude::request_body_tests`
预期：编译失败，报错 `no method named 'request_body'`。

- [ ] **步骤 4：实现请求结构、endpoint/request_body/parse_error_response 与 trait**

在 `src-tauri/src/core/llm/claude.rs` 中，紧接 `ClaudeErrorBody` 结构定义之后、`impl ClaudeProvider` 之前插入请求/错误结构，并在 `consume_sse_event` 之后追加方法与 trait 实现。具体：用 Edit 将：

```rust
struct ClaudeErrorBody {
    #[allow(dead_code)]
    #[serde(rename = "type", default)]
    kind: String,
    message: String,
}

impl ClaudeProvider {
```

替换为：

```rust
struct ClaudeErrorBody {
    #[allow(dead_code)]
    #[serde(rename = "type", default)]
    kind: String,
    message: String,
}

#[derive(Serialize)]
struct MessagesRequest {
    model: String,
    max_tokens: u32,
    stream: bool,
    system: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<ThinkingConfig>,
    messages: Vec<MessagesRequestMessage>,
}

#[derive(Serialize)]
struct ThinkingConfig {
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Serialize)]
struct MessagesRequestMessage {
    role: &'static str,
    content: String,
}

#[derive(Deserialize)]
struct ClaudeApiErrorEnvelope {
    error: ClaudeApiError,
}

#[derive(Deserialize)]
struct ClaudeApiError {
    #[allow(dead_code)]
    #[serde(rename = "type", default)]
    kind: String,
    message: String,
}

impl ClaudeProvider {
```

然后用 Edit 将 `consume_sse_event` 方法的收尾（`impl ClaudeProvider` 块的末尾）：

```rust
                "message_stop" => return Ok(true),
                _ => {}
            }
        }

        Ok(false)
    }
}
```

替换为：

```rust
                "message_stop" => return Ok(true),
                _ => {}
            }
        }

        Ok(false)
    }

    fn endpoint(&self) -> String {
        format!(
            "{}/v1/messages",
            self.config.base_url.trim_end_matches('/')
        )
    }

    fn request_body(&self, request: &TranslationRequest) -> MessagesRequest {
        MessagesRequest {
            model: self.config.model.clone(),
            max_tokens: 4096,
            stream: true,
            system: "你是一个专业翻译引擎。只输出译文，不要解释。".to_string(),
            thinking: if self.config.enable_thinking {
                Some(ThinkingConfig {
                    kind: "adaptive".to_string(),
                })
            } else {
                None
            },
            messages: vec![MessagesRequestMessage {
                role: "user",
                content: format!(
                    "请将以下文本翻译为{}：\n\n{}",
                    request.target_lang,
                    request.source_text()
                ),
            }],
        }
    }

    async fn parse_error_response(response: reqwest::Response) -> LlmError {
        let status = response.status();
        let retryable = status.as_u16() == 429 || status.is_server_error();
        let body = response.text().await.unwrap_or_default();
        let message = serde_json::from_str::<ClaudeApiErrorEnvelope>(&body)
            .map(|error| error.error.message)
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
            LlmError::Api {
                message,
                retryable: false,
            }
        }
    }
}

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

        let response = self
            .client
            .post(self.endpoint())
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&self.request_body(request))
            .send()
            .await
            .map_err(|error| LlmError::Http(error.to_string()))?;

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
                    let bytes = bytes.map_err(|error| LlmError::Http(error.to_string()))?;
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

- [ ] **步骤 5：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib llm::claude`
预期：`consume_sse_tests`（6）+ `request_body_tests`（2）共 8 个测试全部 PASS。

- [ ] **步骤 6：全量构建与测试确认无回归**

运行：`cd src-tauri && cargo build && cargo test`
预期：编译成功，无 unused 警告；全部测试 PASS。

- [ ] **步骤 7：Commit**

```bash
git add src-tauri/src/core/llm/claude.rs
git commit -m "feat(llm): ClaudeProvider 流式翻译实现"
```

---

## 任务 5：Provider 工厂接入 claude 分支

**文件：**
- 修改：`src-tauri/src/ui/web_popup.rs`

- [ ] **步骤 1：import 增加 ClaudeConfig/ClaudeProvider**

用 Edit 将：

```rust
        llm::{LlmProvider, MockLlmProvider, OpenAiCompatibleConfig, OpenAiCompatibleProvider},
```

替换为：

```rust
        llm::{
            ClaudeConfig, ClaudeProvider, LlmProvider, MockLlmProvider, OpenAiCompatibleConfig,
            OpenAiCompatibleProvider,
        },
```

- [ ] **步骤 2：工厂 match 增加 claude 分支**

用 Edit 将：

```rust
    let provider: Arc<dyn LlmProvider> = match config.provider.as_str() {
        "mock" => Arc::new(MockLlmProvider),
        _ => Arc::new(OpenAiCompatibleProvider::new(OpenAiCompatibleConfig::from(
            config.openai_compatible,
        ))),
    };
```

替换为：

```rust
    let provider: Arc<dyn LlmProvider> = match config.provider.as_str() {
        "mock" => Arc::new(MockLlmProvider),
        "claude" => Arc::new(ClaudeProvider::new(ClaudeConfig::from(config.claude))),
        _ => Arc::new(OpenAiCompatibleProvider::new(OpenAiCompatibleConfig::from(
            config.openai_compatible,
        ))),
    };
```

- [ ] **步骤 3：构建与测试确认**

运行：`cd src-tauri && cargo build && cargo test`
预期：编译成功；既有 web_popup 测试（`automatic_translation_source_text_is_cached_for_popup_refill`、`manual_translation_source_text_is_not_cached_for_popup_refill`）PASS，无回归。

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/ui/web_popup.rs
git commit -m "feat(translation): provider 工厂接入 claude 分支"
```

---

## 任务 6：前端 provider 下拉与 Claude 设置表单

**文件：**
- 修改：`frontend/index.html`
- 修改：`frontend/style.css`
- 修改：`frontend/main.js`

### 6.1 重构设置面板 HTML

- [ ] **步骤 1：替换 settingsPanel 内容**

用 Edit 将 `frontend/index.html` 中的整个设置面板：

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

替换为：

```html
      <div id="settingsPanel" class="settings-panel hidden">
        <label>
          Provider
          <select id="providerSelect">
            <option value="openai-compatible">OpenAI 兼容</option>
            <option value="claude">Claude</option>
            <option value="mock">Mock</option>
          </select>
        </label>
        <label>
          目标语言
          <input id="targetLangInput" type="text" placeholder="中文">
        </label>
        <p class="config-warning">API Key 本阶段会明文保存到本机配置文件，请只在可信设备上使用。</p>
        <div id="openaiForm" class="provider-form">
          <label>
            OpenAI API Key
            <input id="apiKeyInput" type="password" placeholder="sk-...">
          </label>
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
        <div id="claudeForm" class="provider-form hidden">
          <label>
            Claude API Key
            <input id="claudeApiKeyInput" type="password" placeholder="sk-ant-...">
          </label>
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
          <label class="checkbox-row">
            <input id="claudeEnableThinkingInput" type="checkbox">
            启用 Thinking（仅对支持的模型生效，Haiku 等需关闭）
          </label>
        </div>
        <button id="saveConfigBtn">保存配置</button>
        <div id="configStatus" class="config-status"></div>
      </div>
```

### 6.2 补充 select 与 checkbox 样式

- [ ] **步骤 2：追加 select 与 checkbox-row 样式**

用 Edit 将 `frontend/style.css` 中的：

```css
.settings-panel input:focus {
  border-color: #4a90d9;
}
```

替换为：

```css
.settings-panel input:focus {
  border-color: #4a90d9;
}

.settings-panel select {
  padding: 7px 8px;
  border: 1px solid #ddd;
  border-radius: 6px;
  font: inherit;
  outline: none;
  background: #fff;
}

.settings-panel select:focus {
  border-color: #4a90d9;
}

.settings-panel .checkbox-row {
  flex-direction: row;
  align-items: center;
  gap: 6px;
}

.settings-panel .checkbox-row input {
  width: auto;
  margin: 0;
}
```

### 6.3 扩展 main.js 配置读写与切换

- [ ] **步骤 3：增加新 DOM 引用**

用 Edit 将 `frontend/main.js` 中的：

```js
const saveConfigBtn = document.getElementById('saveConfigBtn');
const configStatus = document.getElementById('configStatus');
```

替换为：

```js
const saveConfigBtn = document.getElementById('saveConfigBtn');
const configStatus = document.getElementById('configStatus');
const providerSelect = document.getElementById('providerSelect');
const openaiForm = document.getElementById('openaiForm');
const claudeForm = document.getElementById('claudeForm');
const claudeApiKeyInput = document.getElementById('claudeApiKeyInput');
const claudeBaseUrlInput = document.getElementById('claudeBaseUrlInput');
const claudeModelInput = document.getElementById('claudeModelInput');
const claudeTimeoutInput = document.getElementById('claudeTimeoutInput');
const claudeEnableThinkingInput = document.getElementById('claudeEnableThinkingInput');
```

- [ ] **步骤 4：新增 showProviderForm 并重写 fillConfigForm**

用 Edit 将：

```js
function fillConfigForm(config) {
  targetLangInput.value = config.targetLang ?? '中文';
  apiKeyInput.value = config.openaiCompatible?.apiKey ?? '';
  baseUrlInput.value = config.openaiCompatible?.baseUrl ?? 'https://api.openai.com/v1';
  modelInput.value = config.openaiCompatible?.model ?? 'gpt-4o-mini';
  timeoutInput.value = String(config.openaiCompatible?.timeoutSeconds ?? 60);
}
```

替换为：

```js
function showProviderForm(provider) {
  openaiForm.classList.toggle('hidden', provider !== 'openai-compatible');
  claudeForm.classList.toggle('hidden', provider !== 'claude');
}

function fillConfigForm(config) {
  providerSelect.value = config.provider ?? 'openai-compatible';
  targetLangInput.value = config.targetLang ?? '中文';
  apiKeyInput.value = config.openaiCompatible?.apiKey ?? '';
  baseUrlInput.value = config.openaiCompatible?.baseUrl ?? 'https://api.openai.com/v1';
  modelInput.value = config.openaiCompatible?.model ?? 'gpt-4o-mini';
  timeoutInput.value = String(config.openaiCompatible?.timeoutSeconds ?? 60);
  claudeApiKeyInput.value = config.claude?.apiKey ?? '';
  claudeBaseUrlInput.value = config.claude?.baseUrl ?? 'https://api.anthropic.com';
  claudeModelInput.value = config.claude?.model ?? 'claude-haiku-4-5';
  claudeTimeoutInput.value = String(config.claude?.timeoutSeconds ?? 60);
  claudeEnableThinkingInput.checked = config.claude?.enableThinking ?? false;
  showProviderForm(providerSelect.value);
}
```

- [ ] **步骤 5：重写 readConfigForm**

用 Edit 将：

```js
function readConfigForm() {
  return {
    provider: 'openai-compatible',
    targetLang: targetLangInput.value.trim() || '中文',
    openaiCompatible: {
      apiKey: apiKeyInput.value.trim() || null,
      baseUrl: baseUrlInput.value.trim(),
      model: modelInput.value.trim(),
      timeoutSeconds: Number(timeoutInput.value),
    },
  };
}
```

替换为：

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

- [ ] **步骤 6：重写 validateConfig 为多 provider 校验**

用 Edit 将：

```js
function validateConfig(config) {
  let url;
  try {
    url = new URL(config.openaiCompatible.baseUrl);
  } catch {
    return 'Base URL 请输入有效的 http(s) 地址';
  }

  if (url.protocol !== 'http:' && url.protocol !== 'https:') {
    return 'Base URL 请输入有效的 http(s) 地址';
  }

  if (!config.openaiCompatible.model) {
    return 'Model 不能为空';
  }

  if (!Number.isInteger(config.openaiCompatible.timeoutSeconds)
      || config.openaiCompatible.timeoutSeconds < 1
      || config.openaiCompatible.timeoutSeconds > 600) {
    return 'Timeout 秒请输入 1-600 的整数';
  }

  return null;
}
```

替换为：

```js
function validateProviderFields({ baseUrl, model, timeoutSeconds }, label) {
  let url;
  try {
    url = new URL(baseUrl);
  } catch {
    return `${label} Base URL 请输入有效的 http(s) 地址`;
  }

  if (url.protocol !== 'http:' && url.protocol !== 'https:') {
    return `${label} Base URL 请输入有效的 http(s) 地址`;
  }

  if (!model) {
    return `${label} Model 不能为空`;
  }

  if (!Number.isInteger(timeoutSeconds)
      || timeoutSeconds < 1
      || timeoutSeconds > 600) {
    return `${label} Timeout 秒请输入 1-600 的整数`;
  }

  return null;
}

function validateConfig(config) {
  if (config.provider === 'openai-compatible') {
    return validateProviderFields(config.openaiCompatible, 'OpenAI');
  }
  if (config.provider === 'claude') {
    return validateProviderFields(config.claude, 'Claude');
  }
  return null;
}
```

- [ ] **步骤 7：绑定 provider 下拉切换事件**

用 Edit 将：

```js
saveConfigBtn.addEventListener('click', saveAppConfig);
```

替换为：

```js
saveConfigBtn.addEventListener('click', saveAppConfig);

providerSelect.addEventListener('change', () => {
  showProviderForm(providerSelect.value);
});
```

- [ ] **步骤 8：前端语法检查**

运行：`node --check frontend/main.js`
预期：无输出（语法正确）。

- [ ] **步骤 9：Commit**

```bash
git add frontend/index.html frontend/style.css frontend/main.js
git commit -m "feat(frontend): provider 下拉与 Claude 设置表单"
```

---

## 任务 7：文档同步

**文件：**
- 修改：`CLAUDE.md`
- 修改：`AGENTS.md`

按 CLAUDE.md 开发说明第 1 条，两文件须保持同步。两文件该段落内容一致，应用相同改动。

- [ ] **步骤 1：更新项目结构中的 llm 行（CLAUDE.md）**

用 Edit 将 `CLAUDE.md` 中的：

```
  src/core/llm/    LLM provider 抽象、mock、OpenAI-compatible 流式 provider
```

替换为：

```
  src/core/llm/    LLM provider 抽象、mock、OpenAI-compatible 与 Claude 流式 provider
```

- [ ] **步骤 2：更新配置存储段（CLAUDE.md）**

用 Edit 将 `CLAUDE.md` 中的：

```
- **配置存储**：当前设置面板将 OpenAI-compatible 配置保存到 Tauri app config dir 下的 `config.json`。API Key 在 MVP 阶段明文保存，后续产品化需迁移到系统 SecretStore。
```

替换为：

```
- **配置存储**：当前设置面板将所选 provider（openai-compatible / claude / mock）及其配置保存到 Tauri app config dir 下的 `config.json`，含 `openai_compatible` 与 `claude` 两组 provider 字段（`claude` 字段 `#[serde(default)]` 以兼容旧配置）。API Key 在 MVP 阶段明文保存，后续产品化需迁移到系统 SecretStore。
```

- [ ] **步骤 3：同步 AGENTS.md 的 llm 行**

用 Edit 将 `AGENTS.md` 中的：

```
  src/core/llm/    LLM provider 抽象、mock、OpenAI-compatible 流式 provider
```

替换为：

```
  src/core/llm/    LLM provider 抽象、mock、OpenAI-compatible 与 Claude 流式 provider
```

- [ ] **步骤 4：同步 AGENTS.md 的配置存储段**

用 Edit 将 `AGENTS.md` 中的：

```
- **配置存储**：当前设置面板将 OpenAI-compatible 配置保存到 Tauri app config dir 下的 `config.json`。API Key 在 MVP 阶段明文保存，后续产品化需迁移到系统 SecretStore。
```

替换为：

```
- **配置存储**：当前设置面板将所选 provider（openai-compatible / claude / mock）及其配置保存到 Tauri app config dir 下的 `config.json`，含 `openai_compatible` 与 `claude` 两组 provider 字段（`claude` 字段 `#[serde(default)]` 以兼容旧配置）。API Key 在 MVP 阶段明文保存，后续产品化需迁移到系统 SecretStore。
```

- [ ] **步骤 5：Commit**

```bash
git add CLAUDE.md AGENTS.md
git commit -m "docs: 同步 Claude provider 文档"
```

---

## 收尾验证

- [ ] **全量验证**

运行：
```bash
cd src-tauri && cargo test
cd src-tauri && cargo build
node --check frontend/main.js
```
预期：cargo test 全 PASS；cargo build 成功无错误；node --check 无输出。

- [ ] **人工验证（Tauri dev，规格 9.3 节）**

运行 `npm run tauri dev`，逐项确认：
1. 设置面板 provider 下拉可选 openai-compatible / claude / mock，切换时对应表单显隐正确。
2. 选 claude，填有效 API Key（默认 base_url/model/timeout），翻译成功并流式渲染。
3. 翻译中点取消按钮，Claude 流式响应被中断（复用取消/重试链路）。
4. 填错误 API Key，`Failed` 事件正确显示 401 错误信息。
5. 切回 openai-compatible / mock 翻译不回归。
6. 删除 `config.json` 中 `claude` 字段后重启，应用正常加载且 claude 字段为默认值（向后兼容）。

---

## 自检

**1. 规格覆盖度：**
- §3.1 新增组件 claude.rs / mod.rs / types.rs / web_popup.rs / index.html / main.js → 任务 1-6 全覆盖。
- §3.2 设计决策（独立 provider、base_url 可配默认 anthropic、默认模型 claude-haiku-4-5、thinking 默认不传可配透传 adaptive、max_tokens 固定 4096）→ 任务 4 `request_body` 与任务 2 默认常量全覆盖；不在代码层按模型过滤 thinking，错误交服务端返回（任务 4 `parse_error_response` 映射）。
- §4.1 请求 Headers（x-api-key / anthropic-version / content-type）与 Body → 任务 4 `stream_translate` 的 `.header(...)` 与 `MessagesRequest` 全覆盖。
- §4.2 SSE 事件解析 7 类事件 → 任务 3 `consume_sse_event` 的 match 分支全覆盖。
- §4.3 取消 → 任务 4 `tokio::select!` 复用 OpenAI 模式。
- §5 配置模型 ClaudeConfig/ClaudeAppConfig/AppConfig/env/From → 任务 1、任务 2 全覆盖。
- §6 Provider 工厂 → 任务 5。
- §7 错误处理（429/5xx→Http、401/403/400→Api retryable=false、JSON 解析失败→Parse、流内 error→Api、refusal→"翻译被拒绝"）→ 任务 3 `consume_sse_event`（error/refusal）+ 任务 4 `parse_error_response`（HTTP 状态映射）全覆盖。
- §8 前端集成 provider 下拉 + Claude 表单 + 切换 → 任务 6 全覆盖。
- §9 测试策略 → 任务 2（5 测试）、任务 3（6 测试）、任务 4（2 测试）+ 收尾验证命令全覆盖。
- §10 不向后兼容性 → 任务 2 `#[serde(default)]` + `app_config_deserializes_without_claude_field` 测试 + 收尾人工验证 6 覆盖。
- §11 文档同步 → 任务 7；plugins.md 本特性未新增插件，无需同步（已注明）。
- 无遗漏。

**2. 占位符扫描：** 无 "TODO/待定/类似任务 N/添加适当错误处理" 等；每个代码步骤均含完整代码块；命令与预期输出均明确。

**3. 类型一致性：**
- `ClaudeConfig` 字段（api_key/base_url/model/timeout_seconds/enable_thinking）在任务 1 定义，任务 2 `From<ClaudeAppConfig>` 与任务 4 `ClaudeProvider::new` 引用一致。
- `ClaudeAppConfig` 字段在任务 2 定义，任务 6 前端 `claude.{apiKey,baseUrl,model,timeoutSeconds,enableThinking}`（camelCase 由 `#[serde(rename_all = "camelCase")]` 保证）一致。
- `consume_sse_event(event: &str, on_delta: &mut (dyn FnMut(String) + Send)) -> Result<bool, LlmError>` 签名在任务 3 定义，任务 4 `stream_translate` 调用一致；与 OpenAI 的 `consume_sse_event` 同构。
- `LlmError` 变体（`MissingConfig`/`Http`/`Api{message,retryable}`/`Parse`）与 [provider.rs](../../../src-tauri/src/core/llm/provider.rs) 定义一致。
- 工厂分支 `"claude" => ClaudeProvider::new(ClaudeConfig::from(config.claude))` 与任务 2 `From` 转换、任务 1 `new` 签名一致。
- 前端 `providerSelect.value` 取值 `'openai-compatible'/'claude'/'mock'` 与后端 `match config.provider.as_str()` 分支一致。

无类型/签名不一致。

---

## 执行交接

计划已完成并保存到 `docs/superpowers/plans/2026-07-01-claude-provider.md`。两种执行方式：

**1. 子代理驱动（推荐）** - 每个任务调度一个新的子代理，任务间进行审查，快速迭代

**2. 内联执行** - 在当前会话中使用 executing-plans 执行任务，批量执行并设有检查点

选哪种方式？
