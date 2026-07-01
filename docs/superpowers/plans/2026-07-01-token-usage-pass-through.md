# Token Usage 透传实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 把 LLM provider SSE 返回的 token usage 解析出来，经 `TranslationService` 透传到前端弹窗展示，并提供默认开启的采集开关。

**架构：** provider trait 回调由 `FnMut(String)` 类型化为 `FnMut(LlmStreamEvent)` 枚举（`Delta` / `Usage`）；三个 provider 解析自家 SSE usage 并回传；service 按配置开关 `collect_usage` 决定 `Finished.usage` 是否填充；前端在译文下方渲染 token 用量脚注；设置页加开关。

**技术栈：** Rust（edition 2021，async-trait，tokio，reqwest，serde）、原生静态前端（HTML/JS/CSS）。

**规格：** [docs/superpowers/specs/2026-07-01-token-usage-pass-through-design.md](../specs/2026-07-01-token-usage-pass-through-design.md)

---

## 文件结构

| 文件 | 职责 | 动作 |
|---|---|---|
| `src-tauri/src/core/translation/types.rs` | `TokenUsage` 类型；`Finished` 加 `usage` 字段 | 修改 |
| `src-tauri/src/core/llm/provider.rs` | `LlmStreamEvent` 枚举；trait 签名改 `on_event` | 修改 |
| `src-tauri/src/core/llm/mock.rs` | 流末发 `Usage` | 修改 |
| `src-tauri/src/core/llm/openai_compatible.rs` | 请求 `stream_options.include_usage`；解析末尾 chunk usage | 修改 |
| `src-tauri/src/core/llm/claude.rs` | 解析 `message_start` / `message_delta` usage | 修改 |
| `src-tauri/src/core/translation/service.rs` | `translate_with` 加 `collect_usage`；累积 usage 填 `Finished` | 修改 |
| `src-tauri/src/ui/web_popup.rs` | 调用点传 `config.collect_usage` | 修改 |
| `src-tauri/src/core/config/types.rs` | `collect_usage` 字段（默认 true）+ env | 修改 |
| `frontend/translate.html` | usage 脚注 DOM | 修改 |
| `frontend/translate.js` | finished 渲染 usage；状态切换清除 | 修改 |
| `frontend/translate.css` | usage 脚注样式 | 修改 |
| `frontend/settings.html` | 采集开关 checkbox | 修改 |
| `frontend/settings.js` | 读写 `collectUsage` | 修改 |

---

## 任务 1：TokenUsage 类型与 Finished 事件

**文件：**
- 修改：`src-tauri/src/core/translation/types.rs`

- [ ] **步骤 1：编写失败的测试**

在 `types.rs` 的 `#[cfg(test)] mod tests` 末尾追加：

```rust
    #[test]
    fn token_usage_serializes_camel_case() {
        let usage = TokenUsage {
            input_tokens: 27,
            output_tokens: 18,
        };
        let payload = serde_json::to_value(usage).expect("usage 应可序列化");
        assert_eq!(payload["inputTokens"], 27);
        assert_eq!(payload["outputTokens"], 18);
        assert!(payload.get("input_tokens").is_none());
    }

    #[test]
    fn finished_event_serializes_with_usage_when_present() {
        let event = TranslationEvent::Finished {
            session_id: TranslationSessionId("session-1".to_string()),
            full_text: "你好".to_string(),
            usage: Some(TokenUsage {
                input_tokens: 27,
                output_tokens: 18,
            }),
        };
        let payload = serde_json::to_value(event).expect("事件应可序列化");
        assert_eq!(payload["type"], "finished");
        assert_eq!(payload["fullText"], "你好");
        assert_eq!(payload["usage"]["inputTokens"], 27);
        assert_eq!(payload["usage"]["outputTokens"], 18);
    }

    #[test]
    fn finished_event_serializes_usage_null_when_absent() {
        let event = TranslationEvent::Finished {
            session_id: TranslationSessionId("session-1".to_string()),
            full_text: "你好".to_string(),
            usage: None,
        };
        let payload = serde_json::to_value(event).expect("事件应可序列化");
        assert!(payload["usage"].is_null());
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib token_usage && cargo test --lib finished_event`
预期：编译失败，`TokenUsage` 未定义、`Finished` 无 `usage` 字段。

- [ ] **步骤 3：实现 TokenUsage 与 Finished.usage**

在 `types.rs` 的 `TranslationSessionId` 定义之后、`TranslationRequest` 之前插入：

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}
```

修改 `TranslationEvent::Finished` 变体（当前只有 `session_id` + `full_text`）：

```rust
    Finished {
        session_id: TranslationSessionId,
        full_text: String,
        usage: Option<TokenUsage>,
    },
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib token_usage && cargo test --lib finished_event`
预期：3 个新测试 PASS。但 `service.rs` 中构造 `Finished { ... }` 处会编译失败——这是预期的，任务 5 修复。本任务先确认 types 测试通过即可（types 测试不依赖 service）。

> 说明：`cargo test --lib` 在此阶段会因 service.rs 编译失败而整体失败。改为单独验证 types 模块：`cd src-tauri && cargo test --lib translation::types`。若仍因 crate 整体编译失败，先临时在 service.rs 给 `Finished` 构造点补 `usage: None`，待任务 5 正式接入。这里采用后者——在步骤 3 后立即补 service.rs 的 `Finished` 构造点 `usage: None`，保证 crate 可编译。

补 [service.rs](../../src-tauri/src/core/translation/service.rs) 中 `Finished` 构造（当前位于 `translate_with` 内 `emit(TranslationEvent::Finished { session_id, full_text })`）：

```rust
            emit(TranslationEvent::Finished {
                session_id: request.session_id,
                full_text,
                usage: None,
            });
```

- [ ] **步骤 5：运行全量测试验证通过**

运行：`cd src-tauri && cargo test`
预期：全部 PASS（含原有 service 测试，因 usage 暂为 None）。

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/core/translation/types.rs src-tauri/src/core/translation/service.rs
git commit -m "feat(translation): TokenUsage 类型与 Finished 事件加 usage 字段"
```

---

## 任务 2：LlmStreamEvent 枚举与 trait 签名

**文件：**
- 修改：`src-tauri/src/core/llm/provider.rs`

- [ ] **步骤 1：实现 LlmStreamEvent 与新签名**

provider.rs 当前 trait 用 `on_delta: &mut (dyn FnMut(String) + Send)`。整体替换 trait 定义上方区域。

在 `LlmError` 定义之后、`LlmProvider` trait 之前插入：

```rust
use crate::core::translation::TokenUsage;

/// provider 向 service 输出的流事件。Delta 为文本增量，Usage 为 token 用量。
pub enum LlmStreamEvent {
    Delta(String),
    Usage(TokenUsage),
}
```

修改 trait 签名（把 `on_delta` 参数改为 `on_event`，类型改为 `LlmStreamEvent`）：

```rust
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    async fn stream_translate(
        &self,
        request: &TranslationRequest,
        on_event: &mut (dyn FnMut(LlmStreamEvent) + Send),
        cancel: &CancellationToken,
    ) -> Result<(), LlmError>;
}
```

注意：`use crate::core::translation::TranslationRequest;` 已存在于文件顶部，新增 `TokenUsage` import 合并到同一 use 语句：

```rust
use crate::core::translation::{TranslationRequest, TokenUsage};
```

- [ ] **步骤 2：运行构建验证（预期失败，三个 provider 未适配）**

运行：`cd src-tauri && cargo build`
预期：编译失败，报错指向 mock.rs / openai_compatible.rs / claude.rs 的 `stream_translate` 签名不匹配、回调类型不符。这是预期，后续任务逐个修复。

- [ ] **步骤 3：Commit（trait 签名变更，provider 未适配，处于中间不可编译状态）**

> 此处不单独 commit——会破坏构建。任务 3-5 修复全部 provider 后再统一 commit。本任务步骤到此为止，进入任务 3。

---

## 任务 3：Mock provider 发 Usage

**文件：**
- 修改：`src-tauri/src/core/llm/mock.rs`

- [ ] **步骤 1：编写失败的测试**

在 mock.rs 末尾追加测试模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::llm::LlmStreamEvent;
    use crate::core::translation::{
        TokenUsage, TranslationInput, TranslationRequest, TranslationSessionId,
    };

    fn request() -> TranslationRequest {
        TranslationRequest {
            session_id: TranslationSessionId("test".to_string()),
            input: TranslationInput::ManualText("hello world".to_string()),
            target_lang: "中文".to_string(),
        }
    }

    #[tokio::test]
    async fn mock_emits_usage_at_end() {
        let provider = MockLlmProvider;
        let cancel = CancellationToken::new();
        let mut events = Vec::new();
        provider
            .stream_translate(
                &request(),
                &mut |ev: LlmStreamEvent| events.push(ev),
                &cancel,
            )
            .await
            .expect("mock 应成功");

        let usage = events.iter().find_map(|ev| match ev {
            LlmStreamEvent::Usage(u) => Some(u.clone()),
            _ => None,
        });
        assert!(usage.is_some(), "mock 应在流末发 Usage 事件");
        let usage = usage.unwrap();
        assert_eq!(usage, TokenUsage { input_tokens: 2, output_tokens: 2 });
    }

    #[tokio::test]
    async fn mock_emits_delta_before_usage() {
        let provider = MockLlmProvider;
        let cancel = CancellationToken::new();
        let mut events = Vec::new();
        provider
            .stream_translate(
                &request(),
                &mut |ev: LlmStreamEvent| events.push(ev),
                &cancel,
            )
            .await
            .expect("mock 应成功");

        // 最后一个事件应为 Usage
        matches!(events.last(), Some(LlmStreamEvent::Usage(_)));
        // 至少有一个 Delta
        assert!(events.iter().any(|ev| matches!(ev, LlmStreamEvent::Delta(_))));
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib mock`
预期：编译失败（mock 的 `stream_translate` 仍用旧 `on_delta` 签名）。

- [ ] **步骤 3：实现 mock provider**

整体替换 mock.rs 的 `impl LlmProvider for MockLlmProvider`：

```rust
#[async_trait::async_trait]
impl LlmProvider for MockLlmProvider {
    async fn stream_translate(
        &self,
        request: &TranslationRequest,
        on_event: &mut (dyn FnMut(LlmStreamEvent) + Send),
        cancel: &CancellationToken,
    ) -> Result<(), LlmError> {
        let chunks = [
            "[Mock 翻译] ".to_string(),
            request.source_text().to_string(),
            " -> ".to_string(),
            request.target_lang.clone(),
        ];

        for chunk in chunks {
            on_event(LlmStreamEvent::Delta(chunk));
            tokio::select! {
                _ = cancel.cancelled() => return Ok(()),
                _ = tokio::time::sleep(Duration::from_millis(180)) => {}
            }
        }

        // 固定假 usage，供单测覆盖 usage 全链路
        on_event(LlmStreamEvent::Usage(TokenUsage {
            input_tokens: 2,
            output_tokens: 2,
        }));

        Ok(())
    }
}
```

mock.rs 顶部 import 调整：把 `use crate::core::{llm::{LlmError, LlmProvider}, translation::TranslationRequest};` 改为：

```rust
use crate::core::{
    llm::{LlmError, LlmProvider, LlmStreamEvent},
    translation::{TokenUsage, TranslationRequest},
};
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib mock`
预期：2 个新测试 PASS。

- [ ] **步骤 5：Commit（暂不 commit，待 openai/claude 适配后统一提交）**

> 进入任务 4。

---

## 任务 4：OpenAI provider 解析 usage

**文件：**
- 修改：`src-tauri/src/core/llm/openai_compatible.rs`

- [ ] **步骤 1：编写失败的测试**

在 openai_compatible.rs 的 `#[cfg(test)] mod tests`（若不存在则新建）末尾追加：

```rust
    use super::*;
    use crate::core::llm::LlmStreamEvent;
    use crate::core::translation::{
        TokenUsage, TranslationInput, TranslationRequest, TranslationSessionId,
    };

    fn request() -> TranslationRequest {
        TranslationRequest {
            session_id: TranslationSessionId("test".to_string()),
            input: TranslationInput::ManualText("hi".to_string()),
            target_lang: "中文".to_string(),
        }
    }

    #[test]
    fn consume_sse_event_extracts_usage_from_final_chunk() {
        // OpenAI 流式 usage 出现在最后一个 chunk，choices 为空
        let event = "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":27,\"completion_tokens\":18}}";
        let mut events: Vec<LlmStreamEvent> = Vec::new();
        let done = OpenAiCompatibleProvider::consume_sse_event(event, &mut |ev| {
            events.push(ev);
        })
        .unwrap();
        assert!(!done);
        let usage = events.iter().find_map(|ev| match ev {
            LlmStreamEvent::Usage(u) => Some(u.clone()),
            _ => None,
        });
        assert_eq!(
            usage,
            Some(TokenUsage {
                input_tokens: 27,
                output_tokens: 18
            })
        );
    }

    #[test]
    fn request_body_includes_stream_options_include_usage() {
        let config = OpenAiCompatibleConfig {
            api_key: Some("sk-x".to_string()),
            base_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-4o-mini".to_string(),
            timeout_seconds: 60,
        };
        let provider = OpenAiCompatibleProvider::new(config);
        let body = provider.request_body(&request());
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["streamOptions"]["includeUsage"], true);
    }
```

> 注意：`request_body` 当前是私有方法。测试在同模块内可直接调用，无需改可见性。

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib openai_compatible`
预期：编译失败（`consume_sse_event` 签名仍是 `FnMut(String)`；`ChatCompletionChunk` 无 `usage` 字段；请求体无 `stream_options`）。

- [ ] **步骤 3：实现请求体 stream_options**

修改 `ChatCompletionRequest` 结构与 `request_body`：

```rust
#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    stream: bool,
    stream_options: StreamOptions,
    messages: Vec<ChatMessage>,
}

#[derive(Serialize)]
struct StreamOptions {
    include_usage: bool,
}
```

`request_body` 内补字段：

```rust
    fn request_body(&self, request: &TranslationRequest) -> ChatCompletionRequest {
        ChatCompletionRequest {
            model: self.config.model.clone(),
            stream: true,
            stream_options: StreamOptions { include_usage: true },
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: "你是一个专业翻译引擎。只输出译文，不要解释。".to_string(),
                },
                ChatMessage {
                    role: "user",
                    content: format!(
                        "请将以下文本翻译为{}：\n\n{}",
                        request.target_lang,
                        request.source_text()
                    ),
                },
            ],
        }
    }
```

- [ ] **步骤 4：实现 chunk usage 解析与回调类型化**

修改 `ChatCompletionChunk` 加 `usage` 字段：

```rust
#[derive(Deserialize)]
struct ChatCompletionChunk {
    choices: Option<Vec<ChatChoice>>,
    usage: Option<ChatUsage>,
    error: Option<ApiError>,
}

#[derive(Deserialize)]
struct ChatUsage {
    prompt_tokens: u64,
    completion_tokens: u64,
}
```

修改 `consume_sse_event` 签名与逻辑（回调由 `FnMut(String)` 改为 `FnMut(LlmStreamEvent)`）：

```rust
    fn consume_sse_event(
        event: &str,
        on_event: &mut (dyn FnMut(LlmStreamEvent) + Send),
    ) -> Result<bool, LlmError> {
        for line in event.lines() {
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data.is_empty() {
                continue;
            }
            if data == "[DONE]" {
                return Ok(true);
            }

            let chunk = serde_json::from_str::<ChatCompletionChunk>(data)
                .map_err(|error| LlmError::Parse(error.to_string()))?;

            if let Some(error) = chunk.error {
                return Err(LlmError::Api {
                    message: error.message,
                    retryable: false,
                });
            }

            if let Some(usage) = chunk.usage {
                on_event(LlmStreamEvent::Usage(TokenUsage {
                    input_tokens: usage.prompt_tokens,
                    output_tokens: usage.completion_tokens,
                }));
            }

            if let Some(choices) = chunk.choices {
                for choice in choices {
                    if let Some(content) = choice.delta.and_then(|delta| delta.content) {
                        if !content.is_empty() {
                            on_event(LlmStreamEvent::Delta(content));
                        }
                    }
                }
            }
        }

        Ok(false)
    }
```

修改 `impl LlmProvider` 的 `stream_translate`：把 `on_delta` 参数名改为 `on_event`，类型改为 `dyn FnMut(LlmStreamEvent) + Send`，内部 `Self::consume_sse_event(&event, on_event)?` 调用不变（参数名跟随）。两处 `consume_sse_event(&event, on_delta)` 与 `consume_sse_event(&buffer, on_delta)` 改为 `on_event`。

openai_compatible.rs 顶部 import 补 `LlmStreamEvent`、`TokenUsage`：

```rust
use crate::core::{
    llm::{LlmError, LlmProvider, LlmStreamEvent},
    translation::{TokenUsage, TranslationRequest},
};
```

- [ ] **步骤 5：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib openai_compatible`
预期：2 个新测试 PASS。

- [ ] **步骤 6：Commit（暂不 commit，待 claude 适配后统一提交）**

> 进入任务 5。

---

## 任务 5：Claude provider 解析 usage

**文件：**
- 修改：`src-tauri/src/core/llm/claude.rs`

- [ ] **步骤 1：编写失败的测试**

在 claude.rs 的 `#[cfg(test)] mod tests` 末尾追加：

```rust
    use crate::core::llm::LlmStreamEvent;
    use crate::core::translation::TokenUsage;

    #[test]
    fn consume_sse_event_extracts_input_usage_from_message_start() {
        let event = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-haiku-4-5\",\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":27,\"output_tokens\":1}}}";
        let mut events: Vec<LlmStreamEvent> = Vec::new();
        ClaudeProvider::consume_sse_event(event, &mut None, &mut |ev| {
            events.push(ev);
        })
        .unwrap();
        let usage = events.iter().find_map(|ev| match ev {
            LlmStreamEvent::Usage(u) => Some(u.clone()),
            _ => None,
        });
        assert_eq!(
            usage,
            Some(TokenUsage {
                input_tokens: 27,
                output_tokens: 1
            })
        );
    }

    #[test]
    fn consume_sse_event_extracts_output_usage_from_message_delta() {
        // 先经 message_start 设置 input_tokens=27
        let start = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-haiku-4-5\",\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":27,\"output_tokens\":1}}}";
        let delta = "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":18}}";
        let mut input_tokens: Option<u64> = None;
        let mut events: Vec<LlmStreamEvent> = Vec::new();
        ClaudeProvider::consume_sse_event(start, &mut input_tokens, &mut |ev| {
            events.push(ev);
        })
        .unwrap();
        ClaudeProvider::consume_sse_event(delta, &mut input_tokens, &mut |ev| {
            events.push(ev);
        })
        .unwrap();
        // message_delta 应回传 input=27(沿用) + output=18
        let last_usage = events.iter().rev().find_map(|ev| match ev {
            LlmStreamEvent::Usage(u) => Some(u.clone()),
            _ => None,
        });
        assert_eq!(
            last_usage,
            Some(TokenUsage {
                input_tokens: 27,
                output_tokens: 18
            })
        );
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib claude`
预期：编译失败（`consume_sse_event` 签名仍是 `(event, on_delta)`；`ClaudeSseEvent` 无 usage 字段）。

- [ ] **步骤 3：实现 Claude usage 解析**

`consume_sse_event` 当前是无状态关联函数。按规格约束「不引入实例状态、由调用点持有 input_tokens」，改为接收 `&mut Option<u64>`。

修改 `ClaudeSseEvent` 与相关反序列化结构，加 usage 字段：

```rust
#[derive(Deserialize)]
#[allow(dead_code)]
struct ClaudeSseEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<EventDelta>,
    error: Option<ClaudeEventError>,
    message: Option<ClaudeMessageStart>,
    usage: Option<ClaudeUsage>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct ClaudeMessageStart {
    usage: Option<ClaudeUsage>,
}

#[derive(Deserialize)]
struct ClaudeUsage {
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
}
```

修改 `consume_sse_event` 签名与逻辑：

```rust
    pub fn consume_sse_event(
        event: &str,
        input_tokens: &mut Option<u64>,
        on_event: &mut (dyn FnMut(LlmStreamEvent) + Send),
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

            if event_type == "message_stop" {
                return Ok(true);
            }

            if event_type == "message_start" {
                if let Some(usage) = parsed.message.as_ref().and_then(|m| m.usage.as_ref()) {
                    if let Some(input) = usage.input_tokens {
                        *input_tokens = Some(input);
                    }
                    let input = input_tokens.unwrap_or(0);
                    let output = usage.output_tokens.unwrap_or(0);
                    on_event(LlmStreamEvent::Usage(TokenUsage {
                        input_tokens: input,
                        output_tokens: output,
                    }));
                }
            }

            if event_type == "message_delta" {
                if let Some(usage) = parsed.usage.as_ref() {
                    let input = input_tokens.unwrap_or(0);
                    let output = usage.output_tokens.unwrap_or(0);
                    on_event(LlmStreamEvent::Usage(TokenUsage {
                        input_tokens: input,
                        output_tokens: output,
                    }));
                }
            }

            if event_type == "content_block_delta" {
                if let Some(delta) = &parsed.delta {
                    if delta.delta_type == "text_delta" {
                        if let Some(text) = &delta.text {
                            if !text.is_empty() {
                                on_event(LlmStreamEvent::Delta(text.clone()));
                            }
                        }
                    }
                }
            }
        }

        Ok(false)
    }
```

修改 `impl LlmProvider::stream_translate`：参数 `on_delta` 改为 `on_event: &mut (dyn FnMut(LlmStreamEvent) + Send)`；在循环外初始化 `let mut input_tokens: Option<u64> = None;`；三处 `Self::consume_sse_event(&event, on_event)` 改为 `Self::consume_sse_event(&event, &mut input_tokens, on_event)`。

claude.rs 顶部 import 补 `LlmStreamEvent`、`TokenUsage`：

```rust
use crate::core::{
    llm::{LlmError, LlmProvider, LlmStreamEvent},
    translation::{TokenUsage, TranslationRequest},
};
```

- [ ] **步骤 4：修复既有 claude 测试**

既有 5 个测试调用 `consume_sse_event` 的旧签名 `(event, &mut |t| texts.push(t))`，需改为 `(event, &mut input_tokens, &mut |ev| ...)`。逐个精确改法如下（参考 [claude.rs:273-334](../../src-tauri/src/core/llm/claude.rs#L273-L334) 现有代码）：

**测试 1 `consume_sse_event_extracts_text_delta`**（断言 `texts == vec!["你好"]`）：

```rust
        let mut texts = Vec::new();
        let mut input_tokens: Option<u64> = None;
        let done = ClaudeProvider::consume_sse_event(event, &mut input_tokens, &mut |ev| {
            if let LlmStreamEvent::Delta(t) = ev { texts.push(t); }
        }).unwrap();
        assert!(!done);
        assert_eq!(texts, vec!["你好"]);
```

**测试 2 `consume_sse_event_message_stop_returns_done`**（断言 done=true、texts 空）：

```rust
        let mut texts = Vec::new();
        let mut input_tokens: Option<u64> = None;
        let done = ClaudeProvider::consume_sse_event(event, &mut input_tokens, &mut |ev| {
            if let LlmStreamEvent::Delta(t) = ev { texts.push(t); }
        }).unwrap();
        assert!(done);
        assert!(texts.is_empty());
```

**测试 3 `consume_sse_event_ignores_ping`**（断言 done=false、texts 空）：套用测试 2 同样的 `input_tokens` + 回调改写，断言不变。

**测试 4 `consume_sse_event_error_returns_api_error`**（断言 `Err(LlmError::Api { retryable: false, .. })`）：

```rust
        let mut input_tokens: Option<u64> = None;
        let result = ClaudeProvider::consume_sse_event(event, &mut input_tokens, &mut |ev| {
            if let LlmStreamEvent::Delta(t) = ev { texts.push(t); }
        });
        match result {
            Err(LlmError::Api { retryable: false, .. }) => {}
            other => panic!("预期 Api(retryable=false)，得到：{other:?}"),
        }
```
> 该测试内 `texts` 变量原用于收集，仍保留 `let mut texts = Vec::new();`。

**测试 5 `consume_sse_event_multiple_events_mixed`**（断言 `texts == vec!["Hello", " World"]`）：套用测试 1 的 `input_tokens` + 回调改写，断言不变。

**测试 6 `stream_translate_requires_api_key`**（`#[tokio::test]`，调用 `provider.stream_translate(&request, &mut |_| {}, &cancel)`）：回调签名从 `|_|` 改为接收 `LlmStreamEvent`，即 `&mut |_| {}` 不变（仍忽略参数），无需改动——但需确认编译通过。其余不变。

- [ ] **步骤 5：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib claude`
预期：2 个新测试 + 4 个改后既有测试全部 PASS。

- [ ] **步骤 6：Commit（trait + 三个 provider 统一提交）**

运行：`cd src-tauri && cargo build` 确认整个 crate 编译通过。

```bash
git add src-tauri/src/core/llm/provider.rs src-tauri/src/core/llm/mock.rs src-tauri/src/core/llm/openai_compatible.rs src-tauri/src/core/llm/claude.rs
git commit -m "feat(llm): LlmStreamEvent 枚举化 + 三 provider 解析 usage 并回传"
```

---

## 任务 6：config collect_usage 字段

**文件：**
- 修改：`src-tauri/src/core/config/types.rs`

- [ ] **步骤 1：编写失败的测试**

在 types.rs 的 `#[cfg(test)] mod tests` 末尾追加：

```rust
    #[test]
    fn app_config_defaults_collect_usage_true() {
        let config = AppConfig::from_env();
        assert!(config.collect_usage, "collect_usage 默认应为 true");
    }

    #[test]
    fn app_config_serializes_collect_usage_camel_case() {
        let config = AppConfig::from_env();
        let json = serde_json::to_string(&config).expect("序列化");
        assert!(json.contains("\"collectUsage\":true"), "应输出 camelCase 字段 collectUsage: {json}");
    }

    #[test]
    fn app_config_deserializes_collect_usage_default_when_missing() {
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
        let config: AppConfig = serde_json::from_str::<AppConfig>(json)
            .expect("缺少 collect_usage 字段应可反序列化")
            .normalized();
        assert!(config.collect_usage);
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib config::types`
预期：编译失败，`AppConfig` 无 `collect_usage` 字段。

- [ ] **步骤 3：实现 collect_usage 字段**

在 `AppConfig` 结构体的 `overlay_precreate` 后追加：

```rust
    #[serde(default = "default_true")]
    pub collect_usage: bool,
```

`from_env` 内（当前 `overlay_precreate: true,` 之后）追加：

```rust
            collect_usage: env::var("SHIZI_COLLECT_USAGE")
                .map(|value| value.eq_ignore_ascii_case("true"))
                .unwrap_or(true),
```

`normalized()` 不修改该字段（布尔无需规整）。

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib config::types`
预期：3 个新测试 + 既有 config 测试全部 PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/core/config/types.rs
git commit -m "feat(config): collect_usage 字段默认开启 + env 支持"
```

---

## 任务 7：service 累积 usage 并填 Finished

**文件：**
- 修改：`src-tauri/src/core/translation/service.rs`

- [ ] **步骤 1：编写失败的测试**

service.rs 既有 `CancelAwareFakeProvider` 已适配新 trait（任务 5 后），但需确认其回调签名。在测试模块新增一个发 Usage 的 fake provider 与两个测试：

```rust
    struct UsageFakeProvider;

    #[async_trait::async_trait]
    impl LlmProvider for UsageFakeProvider {
        async fn stream_translate(
            &self,
            _request: &TranslationRequest,
            on_event: &mut (dyn FnMut(crate::core::llm::LlmStreamEvent) + Send),
            _cancel: &CancellationToken,
        ) -> Result<(), LlmError> {
            on_event(crate::core::llm::LlmStreamEvent::Delta("你好".to_string()));
            on_event(crate::core::llm::LlmStreamEvent::Usage(
                crate::core::translation::TokenUsage {
                    input_tokens: 27,
                    output_tokens: 18,
                },
            ));
            Ok(())
        }
    }

    #[tokio::test]
    async fn finished_carries_usage_when_collect_enabled() {
        let service = TranslationService::new(Arc::new(UsageFakeProvider));
        let cancel = CancellationToken::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_for_task = events.clone();

        service
            .translate_with(
                request(),
                true,
                cancel,
                |event| events_for_task.lock().unwrap().push(event),
            )
            .await
            .expect("应返回 Ok");

        let events = events.lock().unwrap();
        let usage = events.iter().find_map(|e| match e {
            TranslationEvent::Finished { usage, .. } => usage.clone(),
            _ => None,
        });
        assert_eq!(
            usage,
            Some(crate::core::translation::TokenUsage {
                input_tokens: 27,
                output_tokens: 18
            })
        );
    }

    #[tokio::test]
    async fn finished_usage_none_when_collect_disabled() {
        let service = TranslationService::new(Arc::new(UsageFakeProvider));
        let cancel = CancellationToken::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_for_task = events.clone();

        service
            .translate_with(
                request(),
                false,
                cancel,
                |event| events_for_task.lock().unwrap().push(event),
            )
            .await
            .expect("应返回 Ok");

        let events = events.lock().unwrap();
        let usage = events.iter().find_map(|e| match e {
            TranslationEvent::Finished { usage, .. } => usage.clone(),
            _ => None,
        });
        assert_eq!(usage, None);
    }
```

> 注：`request()` helper 已存在于既有测试模块，复用。既有 `CancelAwareFakeProvider` 的 `stream_translate` 在任务 5 后已改为 `on_event` 签名——若未改，本步骤一并将其内部 `on_delta(chunk.to_string())` 改为 `on_event(crate::core::llm::LlmStreamEvent::Delta(chunk.to_string()))`，并改参数名 `on_delta` → `on_event`。

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib translation::service`
预期：编译失败，`translate_with` 无 `collect_usage` 参数。

- [ ] **步骤 3：实现 service 改造**

整体替换 `translate_with` 实现：

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
        let full_text = Arc::new(Mutex::new(String::new()));
        let usage: Arc<Mutex<Option<TokenUsage>>> = Arc::new(Mutex::new(None));

        let delta_text = full_text.clone();
        let usage_slot = usage.clone();
        let delta_session_id = request.session_id.clone();

        self.provider
            .stream_translate(&request, &mut |event| {
                match event {
                    crate::core::llm::LlmStreamEvent::Delta(chunk) => {
                        if let Ok(mut text) = delta_text.lock() {
                            text.push_str(&chunk);
                        }
                        emit(TranslationEvent::Delta {
                            session_id: delta_session_id.clone(),
                            text: chunk,
                        });
                    }
                    crate::core::llm::LlmStreamEvent::Usage(u) => {
                        if collect_usage {
                            if let Ok(mut slot) = usage_slot.lock() {
                                *slot = Some(u);
                            }
                        }
                    }
                }
            }, &cancel)
            .await?;

        let full_text = full_text
            .lock()
            .map(|text| text.clone())
            .unwrap_or_default();

        if cancel.is_cancelled() {
            emit(TranslationEvent::Cancelled {
                session_id: request.session_id,
            });
        } else {
            let usage = usage
                .lock()
                .map(|slot| slot.clone())
                .unwrap_or(None);
            emit(TranslationEvent::Finished {
                session_id: request.session_id,
                full_text,
                usage,
            });
        }

        Ok(())
    }
```

service.rs 顶部 import 补 `TokenUsage`：

```rust
use super::{TokenUsage, TranslationEvent, TranslationRequest};
```

- [ ] **步骤 4：修复既有 service 测试调用**

既有两个测试 `emits_cancelled_when_cancelled_before_completion` / `emits_finished_when_not_cancelled` 调用 `translate_with(request(), cancel_for_task, |event| ...)`，需补 `collect_usage` 参数。两处改为 `translate_with(request(), true, cancel_for_task, |event| ...)`。

既有 `emits_finished_when_not_cancelled` 可补断言：`Finished.usage` 为 None（因 fake provider 不发 Usage）。可选，不强制。

- [ ] **步骤 5：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib translation::service`
预期：2 个新测试 + 2 个改后既有测试全部 PASS。

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/core/translation/service.rs
git commit -m "feat(translation): translate_with 按 collect_usage 累积 usage 填 Finished"
```

---

## 任务 8：web_popup 调用点传 collect_usage

**文件：**
- 修改：`src-tauri/src/ui/web_popup.rs`

- [ ] **步骤 1：修改调用点**

[web_popup.rs:136](../../src-tauri/src/ui/web_popup.rs#L136) 当前：

```rust
            .translate_with(request, cancel_token, |event| {
```

改为：

```rust
            .translate_with(request, config.collect_usage, cancel_token, |event| {
```

`config` 变量在该函数作用域内已存在（`let config = state.config_store.get()...`）。

- [ ] **步骤 2：运行构建验证**

运行：`cd src-tauri && cargo build`
预期：编译通过。

- [ ] **步骤 3：Commit**

```bash
git add src-tauri/src/ui/web_popup.rs
git commit -m "feat(ui): translate_with 调用点传入 collect_usage"
```

---

## 任务 9：前端弹窗 usage 脚注

**文件：**
- 修改：`frontend/translate.html`、`frontend/translate.js`、`frontend/translate.css`

- [ ] **步骤 1：translate.html 加 usage 脚注 DOM**

在 [translate.html:26](../../frontend/translate.html#L26) `<div id="outputText" ...>` 之后追加：

```html
      <div id="usageFooter" class="usage-footer hidden"></div>
```

- [ ] **步骤 2：translate.js 渲染与清除 usage**

在 [translate.js](../../frontend/translate.js) 顶部元素获取区（`outputText` 获取附近）追加：

```js
const usageFooter = document.getElementById('usageFooter');

function showUsageFooter(usage) {
  if (!usage) {
    hideUsageFooter();
    return;
  }
  usageFooter.textContent = `${usage.inputTokens} → ${usage.outputTokens} tokens`;
  usageFooter.classList.remove('hidden');
}

function hideUsageFooter() {
  usageFooter.classList.add('hidden');
  usageFooter.textContent = '';
}
```

修改 `finished` 分支（[translate.js:90-98](../../frontend/translate.js#L90-L98)），在 `hideSourceBadge()` 之后、`setActionButtons(...)` 之前加：

```js
      showUsageFooter(payload.usage);
```

在 `started` 分支（`outputText.textContent = '';` 附近）加 `hideUsageFooter();`。
在 `failed` 分支（`hideSourceBadge()` 附近）加 `hideUsageFooter();`。
在 `cancelled` 分支（`hideSourceBadge()` 附近）加 `hideUsageFooter();`。

> clearBtn 清空逻辑若存在也应调用 `hideUsageFooter()`——检查 clearBtn handler，在清空 outputText 处一并加。

- [ ] **步骤 3：translate.css 加样式**

在 [translate.css](../../frontend/translate.css) 末尾追加（参照 `.source-badge` 的低饱和小字号风格，具体值按现有徽章样式对齐调整）：

```css
.usage-footer {
  margin-top: 4px;
  font-size: 11px;
  color: #999;
  text-align: right;
}

.usage-footer.hidden {
  display: none;
}
```

- [ ] **步骤 4：前端语法检查**

运行：`node --check frontend/translate.js`
预期：无输出（语法正确）。

- [ ] **步骤 5：Commit**

```bash
git add frontend/translate.html frontend/translate.js frontend/translate.css
git commit -m "feat(frontend): 翻译弹窗 token 用量脚注展示"
```

---

## 任务 10：设置页采集开关

**文件：**
- 修改：`frontend/settings.html`、`frontend/settings.js`

- [ ] **步骤 1：settings.html 加 checkbox**

在 [settings.html](../../frontend/settings.html) 中 `overlayPrecreateInput` checkbox 附近（窗口策略区块），追加：

```html
      <label>
        <input type="checkbox" id="collectUsageInput" />
        采集 token 用量（显示翻译 token 消耗）
      </label>
```

- [ ] **步骤 2：settings.js 读写字段**

[settings.js:15](../../frontend/settings.js#L15) `overlayPrecreateInput` 获取之后追加：

```js
const collectUsageInput = document.getElementById('collectUsageInput');
```

[settings.js:45](../../frontend/settings.js#L45) `overlayPrecreateInput.checked = ...` 之后追加：

```js
  collectUsageInput.checked = config.collectUsage ?? true;
```

[settings.js:67](../../frontend/settings.js#L67) `overlayPrecreate: ...` 之后追加：

```js
    collectUsage: collectUsageInput.checked,
```

- [ ] **步骤 3：前端语法检查**

运行：`node --check frontend/settings.js`
预期：无输出。

- [ ] **步骤 4：Commit**

```bash
git add frontend/settings.html frontend/settings.js
git commit -m "feat(frontend): 设置页采集 token 用量开关"
```

---

## 任务 11：全量验证与文档同步

**文件：**
- 修改：`README.md`、`docs/roadmap/progressive-development-plan.md`、本计划文件复选框

- [ ] **步骤 1：全量测试与构建**

运行：
```bash
cd src-tauri && cargo test
cd src-tauri && cargo build
node --check frontend/translate.js
node --check frontend/settings.js
```
预期：全部通过。

- [ ] **步骤 2：README 当前能力补 token 用量**

[README.md](../../README.md)「当前能力」列表中「翻译取消与重试」之后追加一条：

```markdown
- Token 用量展示：流式翻译结束时在译文下方显示 input → output token 数；可在设置页关闭采集。
```

- [ ] **步骤 3：roadmap 标记 usage 完成**

[progressive-development-plan.md:83](../../roadmap/progressive-development-plan.md#L83) 当前 `- ~~`Cancelled` 事件~~ ✅、usage/token 统计、~~取消/重试交互~~ ✅。`，把 `usage/token 统计` 改为 `~~usage/token 统计~~ ✅`。

- [ ] **步骤 4：回填本计划复选框**

逐任务勾选本文件所有 `- [ ]` 为 `- [x]`。

- [ ] **步骤 5：Commit**

```bash
git add README.md docs/roadmap/progressive-development-plan.md docs/superpowers/plans/2026-07-01-token-usage-pass-through.md
git commit -m "docs: 同步 token usage 透传到 README/roadmap/plan"
```

---

## 任务 12：收尾

- [ ] **步骤 1：执行 finishing-a-development-branch 流程**

调用 superpowers:finishing-a-development-branch 决定合并/PR/清理。按协作规范第 2 条，进入 finish 前确认文档已同步（任务 11 已完成）。
