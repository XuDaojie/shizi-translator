# 翻译执行控制实现计划（取消 + 重试 + Cancelled 事件）

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 让用户在流式翻译过程中可取消（立即断流、停止计费），结束后可重试，并引入 `Cancelled` 事件正确表达已取消状态。

**架构：** `LlmProvider` trait 加 `cancel: &CancellationToken` 参数，OpenAI/Mock provider 用 `tokio::select!` 响应取消提前返回；`TranslationService` 据取消状态 emit `Cancelled`/`Finished`；`AppState` 新增 `current_cancel_token` 与 `last_translation_input` 两槽；新增 `cancel_translation`/`retry_translation` command，重试复用 `start_translation_from_input`；前端加取消/重试按钮与 `cancelled` 渲染分支。

**技术栈：** Rust + Tauri 2 + `tokio-util`（`CancellationToken`）+ `tokio`（`time` feature，mock 迁移 `thread::sleep`→`tokio::time::sleep`）+ 原生静态 HTML/JS 前端（无构建）。

**规格依据：** `docs/superpowers/specs/2026-07-01-translation-execution-control-design.md`（commit a90bd35）

---

## 文件结构

**修改：**
- `src-tauri/Cargo.toml` — 加 `tokio-util`（sync feature），`tokio` 提到主依赖并加 `time` feature。
- `src-tauri/src/core/llm/provider.rs` — trait 加 `cancel: &CancellationToken` 参数。
- `src-tauri/src/core/llm/mock.rs` — `thread::sleep`→`tokio::time::sleep` + `select!` 包裹。
- `src-tauri/src/core/llm/openai_compatible.rs` — 流式循环 `select!` 包裹 `stream.next()` 与 `cancel.cancelled()`。
- `src-tauri/src/core/translation/types.rs` — `TranslationEvent` 加 `Cancelled` 变体 + 序列化测试。
- `src-tauri/src/core/translation/service.rs` — `translate_with` 接 `CancellationToken`，据 `is_cancelled` emit `Cancelled`/`Finished`。
- `src-tauri/src/app/state.rs` — `current_cancel_token` + `last_translation_input` 两槽及方法 + 测试。
- `src-tauri/src/ui/web_popup.rs` — spawn 前存 token + last_input；新增 `cancel_translation`/`retry_translation` command；适配 trait 新签名。
- `src-tauri/src/lib.rs` — 注册 `cancel_translation`/`retry_translation` 两个 command。
- `frontend/index.html` — `.action-bar` 加 cancelBtn / retryBtn。
- `frontend/main.js` — `cancelled` 渲染分支 + 按钮状态机 + 按钮事件。
- `frontend/style.css` — 按钮样式（按需，复用现有 `.action-bar button`）。

**新增：** 无。

**关键约束：** 任务 5 改 `LlmProvider` trait 签名后，mock（任务 6）、openai（任务 7）、service（任务 8）三处必须同 commit 完成，否则编译不过。故任务 5-8 合并为一个 commit 节点（计划内步骤 5 各自独立编写，最后统一编译 + 一次 commit）。

---

## 任务 1：`Cancelled` 事件变体（纯 Rust）

**文件：**
- 修改：`src-tauri/src/core/translation/types.rs`
- 测试：同文件 `#[cfg(test)] mod tests`

- [ ] **步骤 1：编写失败的测试**

在 `src-tauri/src/core/translation/types.rs` 的 `mod tests` 末尾追加：

```rust
    #[test]
    fn cancelled_event_serializes_with_frontend_field_names() {
        let event = TranslationEvent::Cancelled {
            session_id: TranslationSessionId("session-cancel-1".to_string()),
        };

        let payload = serde_json::to_value(event).expect("事件应可序列化");

        assert_eq!(payload["type"], "cancelled");
        assert_eq!(payload["sessionId"], "session-cancel-1");
        assert!(payload.get("session_id").is_none());
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib core::translation::types::tests::cancelled_event_serializes_with_frontend_field_names`
预期：FAIL，报错 `no variant named Cancelled`。

- [ ] **步骤 3：编写最少实现代码**

在 `src-tauri/src/core/translation/types.rs` 的 `TranslationEvent` 枚举内，`Failed` 变体后追加 `Cancelled` 变体：

```rust
    Failed {
        session_id: TranslationSessionId,
        message: String,
        retryable: bool,
    },
    Cancelled {
        session_id: TranslationSessionId,
    },
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib core::translation::types`
预期：PASS，新增 1 个测试 + 现有测试全过。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/core/translation/types.rs
git commit -m "feat(translation): TranslationEvent 新增 Cancelled 变体"
```

---

## 任务 2：`AppState` cancel token 槽（纯 Rust）

**文件：**
- 修改：`src-tauri/src/app/state.rs`
- 测试：同文件 `mod tests`

- [ ] **步骤 1：编写失败的测试**

在 `src-tauri/src/app/state.rs` 的 `mod tests` 末尾追加：

```rust
    #[test]
    fn cancel_token_triggers_on_cancel_current_translation() {
        let state = app_state();
        let token = tokio_util::sync::CancellationToken::new();
        state.set_current_cancel_token(token.clone()).expect("写入 cancel token");

        state.cancel_current_translation().expect("触发取消");

        assert!(token.is_cancelled(), "token 应被触发");
    }

    #[test]
    fn cancel_current_translation_is_idempotent_when_no_token() {
        let state = app_state();
        // 无 token 时取消应返回 Ok 无操作
        state.cancel_current_translation().expect("无 token 取消应幂等");
    }

    #[test]
    fn cancel_current_translation_is_idempotent_after_take() {
        let state = app_state();
        let token = tokio_util::sync::CancellationToken::new();
        state.set_current_cancel_token(token.clone()).expect("写入 cancel token");

        state.cancel_current_translation().expect("第一次取消触发");
        // token 已被 take，再次取消应幂等无操作
        state.cancel_current_translation().expect("重复取消应幂等");

        assert!(token.is_cancelled());
    }

    #[test]
    fn clear_current_cancel_token_is_idempotent() {
        let state = app_state();
        let token = tokio_util::sync::CancellationToken::new();
        state.set_current_cancel_token(token).expect("写入 cancel token");

        state.clear_current_cancel_token().expect("第一次清空");
        state.clear_current_cancel_token().expect("幂等清空");
        // 清空后取消应幂等（无 token）
        state.cancel_current_translation().expect("清空后取消应幂等");
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib app::state`
预期：FAIL，报错 `no method named set_current_cancel_token`。

- [ ] **步骤 3：编写最少实现代码**

在 `src-tauri/src/app/state.rs` 顶部 use 区追加（保留现有 use）：

```rust
use tokio_util::sync::CancellationToken;
```

`AppState` 结构体在 `pending_capture` 字段后追加：

```rust
    // 当前翻译的取消信号。begin 时存入，翻译自然结束 clear、用户取消 cancel。
    // cancel 取出并触发；幂等：无 token 或已清空返回 Ok 无操作。
    current_cancel_token: Arc<Mutex<Option<CancellationToken>>>,
```

`AppState::new` 的初始化在 `pending_capture` 后追加：

```rust
            current_cancel_token: Arc::new(Mutex::new(None)),
```

`impl AppState` 内（`take_pending_capture` 方法后）追加：

```rust
    /// 存入当前翻译的取消信号。begin 时调用。
    pub fn set_current_cancel_token(&self, token: CancellationToken) -> Result<(), String> {
        let mut slot = self
            .current_cancel_token
            .lock()
            .map_err(|_| "取消信号状态锁已损坏".to_string())?;
        *slot = Some(token);
        Ok(())
    }

    /// 取出并触发当前翻译的取消信号。幂等：无 token 或已结束返回 Ok 无操作。
    pub fn cancel_current_translation(&self) -> Result<(), String> {
        let token = {
            let mut slot = self
                .current_cancel_token
                .lock()
                .map_err(|_| "取消信号状态锁已损坏".to_string())?;
            slot.take()
        };
        if let Some(token) = token {
            token.cancel();
        }
        Ok(())
    }

    /// 清空当前 cancel token（翻译自然结束时调用）。幂等。
    pub fn clear_current_cancel_token(&self) -> Result<(), String> {
        let mut slot = self
            .current_cancel_token
            .lock()
            .map_err(|_| "取消信号状态锁已损坏".to_string())?;
        *slot = None;
        Ok(())
    }
```

- [ ] **步骤 4：运行测试验证通过**

> 注意：此任务用到 `tokio_util`，若 `Cargo.toml` 尚未加依赖会编译失败。若失败，先执行任务 4 步骤 1 加依赖再回来运行测试。

运行：`cd src-tauri && cargo test --lib app::state`
预期：PASS，新增 4 个测试 + 现有测试全过。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/app/state.rs
git commit -m "feat(state): AppState 暂存翻译取消信号 token"
```

---

## 任务 3：`AppState` last_translation_input 槽（纯 Rust）

**文件：**
- 修改：`src-tauri/src/app/state.rs`
- 测试：同文件 `mod tests`

- [ ] **步骤 1：编写失败的测试**

在 `src-tauri/src/app/state.rs` 的 `mod tests` 末尾追加：

```rust
    #[test]
    fn last_translation_input_round_trips() {
        use crate::core::translation::TranslationInput;
        let state = app_state();
        let input = TranslationInput::ManualText("hello".to_string());

        state.set_last_translation_input(input.clone()).expect("写入重试输入");

        let taken = state.take_last_translation_input().expect("取出重试输入");
        assert_eq!(taken, Some(input));

        // 取出后应清空
        let again = state.take_last_translation_input().expect("再次取出");
        assert_eq!(again, None);
    }

    #[test]
    fn last_translation_input_overwrites_previous() {
        use crate::core::translation::TranslationInput;
        let state = app_state();
        let first = TranslationInput::SelectedText("first".to_string());
        let second = TranslationInput::SelectedText("second".to_string());

        state.set_last_translation_input(first).expect("写入第一个");
        state.set_last_translation_input(second.clone()).expect("覆盖第二个");

        let taken = state.take_last_translation_input().expect("取出");
        assert_eq!(taken, Some(second));
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib app::state::tests::last_translation_input`
预期：FAIL，报错 `no method named set_last_translation_input`。

- [ ] **步骤 3：编写最少实现代码**

在 `src-tauri/src/app/state.rs` 顶部 use 区追加（与现有 use 合并，避免重复）：

```rust
use crate::core::translation::TranslationInput;
```

`AppState` 结构体在 `current_cancel_token` 字段后追加：

```rust
    // 最近一次成功开始的翻译输入，供重试复用。begin 成功后存入，retry 时 take。
    last_translation_input: Arc<Mutex<Option<TranslationInput>>>,
```

`AppState::new` 的初始化在 `current_cancel_token` 后追加：

```rust
            last_translation_input: Arc::new(Mutex::new(None)),
```

`impl AppState` 内（`clear_current_cancel_token` 方法后）追加：

```rust
    /// 记录最近一次成功开始的翻译输入，供重试复用。begin 成功后调用。
    pub fn set_last_translation_input(&self, input: TranslationInput) -> Result<(), String> {
        let mut slot = self
            .last_translation_input
            .lock()
            .map_err(|_| "重试输入状态锁已损坏".to_string())?;
        *slot = Some(input);
        Ok(())
    }

    /// 取出最近一次翻译输入。无则返回 None。retry 时调用，取出即清空。
    pub fn take_last_translation_input(&self) -> Result<Option<TranslationInput>, String> {
        let mut slot = self
            .last_translation_input
            .lock()
            .map_err(|_| "重试输入状态锁已损坏".to_string())?;
        Ok(slot.take())
    }
```

> 注意：`TranslationInput` 需实现 `PartialEq` 才能在测试中 `assert_eq!`。若 `TranslationInput` 未派生 `PartialEq`，在 `types.rs` 的 `#[derive(...)]` 加 `PartialEq`（`Clone` 已有）。派生 `PartialEq` 不影响 `Serialize` 行为，安全。若需此改动，在步骤 3 先改 `types.rs` 再改 `state.rs`。

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib app::state`
预期：PASS，新增 2 个测试 + 现有测试全过。

若 `TranslationInput` 缺 `PartialEq` 导致编译失败：在 `src-tauri/src/core/translation/types.rs` 的 `TranslationInput` 的 `#[derive(Debug, Clone, Serialize)]` 改为 `#[derive(Debug, Clone, PartialEq, Serialize)]`，重跑测试。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/app/state.rs src-tauri/src/core/translation/types.rs
git commit -m "feat(state): AppState 暂存最近翻译输入供重试"
```

---

## 任务 4：加 `tokio-util` 与 `tokio` time 依赖

> 此任务为任务 2/6/7 的前置依赖。若任务 2 已因缺依赖失败，先做此任务再回任务 2。

**文件：**
- 修改：`src-tauri/Cargo.toml`

- [ ] **步骤 1：加依赖**

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 末尾（`thiserror = "2"` 后）追加：

```toml
tokio = { version = "1", features = ["time"] }
tokio-util = { version = "0.7", features = ["sync"] }
```

> 说明：`tokio` 当前仅在 `[dev-dependencies]`（features `macros, rt`）。主依赖加 `tokio` 的 `time` feature 供 mock 用 `tokio::time::sleep`；`tokio-util` 的 `sync` feature 提供 `CancellationToken`。Tauri 2 已依赖 tokio runtime，主依赖声明不会重复引入。
> 实现者注意：精确版本与 feature 名以本地 `cargo build` 报错为准微调。

- [ ] **步骤 2：编译验证**

运行：`cd src-tauri && cargo build`
预期：编译通过（依赖下载可能耗时）。

- [ ] **步骤 3：Commit**

```bash
git add src-tauri/Cargo.toml
git commit -m "chore(deps): 加 tokio-util(CancellationToken) 与 tokio time feature"
```

---

## 任务 5-8：LlmProvider trait + 两 provider + service 改造（原子 commit）

> **关键约束：** 任务 5 改 trait 签名后，mock（任务 6）、openai（任务 7）、service（任务 8）必须全部改完才能编译通过。因此这四个任务的「编写代码」步骤各自独立完成，但「编译 + commit」在任务 8 末尾统一进行。

### 任务 5：LlmProvider trait 加取消参数

**文件：**
- 修改：`src-tauri/src/core/llm/provider.rs`

- [ ] **步骤 1：改 trait 签名**

在 `src-tauri/src/core/llm/provider.rs` 顶部 use 区追加：

```rust
use tokio_util::sync::CancellationToken;
```

将 `LlmProvider` trait 的 `stream_translate` 签名改为：

```rust
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    async fn stream_translate(
        &self,
        request: &TranslationRequest,
        on_delta: &mut (dyn FnMut(String) + Send),
        cancel: &CancellationToken,
    ) -> Result<(), LlmError>;
}
```

> 此时整个 crate 编译失败（mock / openai / service 三处调用未适配），属预期。先不编译，继续任务 6/7/8。

### 任务 6：Mock provider 迁移 tokio::time::sleep + select! 包裹

**文件：**
- 修改：`src-tauri/src/core/llm/mock.rs`

- [ ] **步骤 1：改 mock 实现**

将 `src-tauri/src/core/llm/mock.rs` 全文改为：

```rust
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::core::{
    llm::{LlmError, LlmProvider},
    translation::TranslationRequest,
};

pub struct MockLlmProvider;

#[async_trait::async_trait]
impl LlmProvider for MockLlmProvider {
    async fn stream_translate(
        &self,
        request: &TranslationRequest,
        on_delta: &mut (dyn FnMut(String) + Send),
        cancel: &CancellationToken,
    ) -> Result<(), LlmError> {
        let chunks = [
            "[Mock 翻译] ".to_string(),
            request.source_text().to_string(),
            " -> ".to_string(),
            request.target_lang.clone(),
        ];

        for chunk in chunks {
            on_delta(chunk);
            // 迁移 thread::sleep -> tokio::time::sleep（不再阻塞 async runtime），
            // 并用 select! 包裹，cancel 触发时提前返回 Ok（取消非错误）。
            tokio::select! {
                _ = cancel.cancelled() => return Ok(()),
                _ = tokio::time::sleep(Duration::from_millis(180)) => {}
            }
        }

        Ok(())
    }
}
```

> 说明：删除了 `use std::thread`（不再用 `thread::sleep`）。`std::time::Duration` 保留。select! 中 `cancel.cancelled()` 是 future，未选中时被 drop，语义正确。

### 任务 7：OpenAI provider select! 包裹流式循环

**文件：**
- 修改：`src-tauri/src/core/llm/openai_compatible.rs`

- [ ] **步骤 1：改流式循环**

在 `src-tauri/src/core/llm/openai_compatible.rs` 顶部 use 区，`use crate::core::{...}` 块内追加 `CancellationToken`（实际来自 `tokio_util`，故在 use 区加）：

```rust
use tokio_util::sync::CancellationToken;
```

将 `impl LlmProvider for OpenAiCompatibleProvider` 的 `stream_translate` 内的流式循环（当前为 `while let Some(bytes) = stream.next().await { ... }`）改为 `loop { tokio::select! { ... } }`：

把这段：

```rust
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        while let Some(bytes) = stream.next().await {
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

        if !buffer.trim().is_empty() {
            Self::consume_sse_event(&buffer, on_delta)?;
        }

        Ok(())
```

改为：

```rust
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        loop {
            tokio::select! {
                // cancel 触发：提前返回 Ok。stream 在函数返回时被 drop，
                // 底层 reqwest 连接断开，停止后续 token 拉取与计费。
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
```

> 注意：`stream_translate` 的 trait 签名已加 `cancel: &CancellationToken`（任务 5），此处函数签名须同步加该参数：
>
> ```rust
>     async fn stream_translate(
>         &self,
>         request: &TranslationRequest,
>         on_delta: &mut (dyn FnMut(String) + Send),
>         cancel: &CancellationToken,
>     ) -> Result<(), LlmError> {
> ```

### 任务 8：TranslationService 接 CancellationToken

**文件：**
- 修改：`src-tauri/src/core/translation/service.rs`
- 测试：同文件 `mod tests`（新增）

- [ ] **步骤 1：编写失败的测试**

在 `src-tauri/src/core/translation/service.rs` 末尾追加测试模块（若已有 `mod tests` 则并入）：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::llm::LlmProvider;
    use crate::core::translation::{TranslationInput, TranslationRequest, TranslationSessionId};
    use std::sync::{Arc, Mutex};
    use tokio_util::sync::CancellationToken;

    /// fake provider：产出 3 个 chunk，每段 sleep 50ms，可在中途取消。
    struct CancelAwareFakeProvider {
        deltas_emitted: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl LlmProvider for CancelAwareFakeProvider {
        async fn stream_translate(
            &self,
            _request: &TranslationRequest,
            on_delta: &mut (dyn FnMut(String) + Send),
            cancel: &CancellationToken,
        ) -> Result<(), LlmError> {
            let chunks = ["a", "b", "c"];
            for chunk in chunks {
                tokio::select! {
                    _ = cancel.cancelled() => return Ok(()),
                    _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {}
                }
                on_delta(chunk.to_string());
                self.deltas_emitted.lock().unwrap().push(chunk.to_string());
            }
            Ok(())
        }
    }

    fn request() -> TranslationRequest {
        TranslationRequest {
            session_id: TranslationSessionId("test-session".to_string()),
            input: TranslationInput::ManualText("hi".to_string()),
            target_lang: "中文".to_string(),
        }
    }

    #[tokio::test]
    async fn emits_cancelled_when_cancelled_before_completion() {
        let emitted = Arc::new(Mutex::new(Vec::new()));
        let provider = CancelAwareFakeProvider {
            deltas_emitted: emitted.clone(),
        };
        let service = TranslationService::new(Arc::new(provider));
        let cancel = CancellationToken::new();
        let cancel_for_task = cancel.clone();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_for_task = events.clone();

        let handle = tokio::spawn(async move {
            service
                .translate_with(request(), cancel_for_task, |event| {
                    events_for_task.lock().unwrap().push(event);
                })
                .await
        });

        // 让 provider 产出至少一个 chunk 后取消
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;
        cancel.cancel();

        handle.await.expect("task 未 panic").expect("应返回 Ok");

        let events = events.lock().unwrap();
        let types: Vec<&str> = events.iter().map(|e| match e {
            TranslationEvent::Started { .. } => "started",
            TranslationEvent::Delta { .. } => "delta",
            TranslationEvent::Finished { .. } => "finished",
            TranslationEvent::Failed { .. } => "failed",
            TranslationEvent::Cancelled { .. } => "cancelled",
        }).collect();

        assert!(types.contains(&"cancelled"), "应 emit Cancelled: {:?}", types);
        assert!(!types.contains(&"finished"), "取消时不应 emit Finished");
    }

    #[tokio::test]
    async fn emits_finished_when_not_cancelled() {
        let emitted = Arc::new(Mutex::new(Vec::new()));
        let provider = CancelAwareFakeProvider {
            deltas_emitted: emitted.clone(),
        };
        let service = TranslationService::new(Arc::new(provider));
        let cancel = CancellationToken::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_for_task = events.clone();

        service
            .translate_with(request(), cancel, |event| {
                events_for_task.lock().unwrap().push(event);
            })
            .await
            .expect("应返回 Ok");

        let events = events.lock().unwrap();
        let types: Vec<&str> = events.iter().map(|e| match e {
            TranslationEvent::Started { .. } => "started",
            TranslationEvent::Delta { .. } => "delta",
            TranslationEvent::Finished { .. } => "finished",
            TranslationEvent::Failed { .. } => "failed",
            TranslationEvent::Cancelled { .. } => "cancelled",
        }).collect();

        assert!(types.contains(&"finished"), "未取消应 emit Finished: {:?}", types);
        assert!(!types.contains(&"cancelled"), "未取消不应 emit Cancelled");
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib core::translation::service`
预期：FAIL，报错 `translate_with` 参数不匹配（trait 未适配 / service 未改）。

- [ ] **步骤 3：编写最少实现代码**

将 `src-tauri/src/core/translation/service.rs` 全文改为：

```rust
use std::{sync::Arc, sync::Mutex};

use crate::core::llm::{LlmError, LlmProvider};
use tokio_util::sync::CancellationToken;

use super::{TranslationEvent, TranslationRequest};

#[derive(Debug, thiserror::Error)]
pub enum TranslationError {
    #[error(transparent)]
    Llm(#[from] LlmError),
}

impl TranslationError {
    pub fn retryable(&self) -> bool {
        match self {
            Self::Llm(error) => error.retryable(),
        }
    }
}

#[derive(Clone)]
pub struct TranslationService {
    provider: Arc<dyn LlmProvider>,
}

impl TranslationService {
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        Self { provider }
    }

    pub async fn translate_with<F>(
        &self,
        request: TranslationRequest,
        cancel: CancellationToken,
        mut emit: F,
    ) -> Result<(), TranslationError>
    where
        F: FnMut(TranslationEvent) + Send,
    {
        let full_text = Arc::new(Mutex::new(String::new()));
        let delta_text = full_text.clone();
        let delta_session_id = request.session_id.clone();

        self.provider
            .stream_translate(&request, &mut |chunk| {
                if let Ok(mut text) = delta_text.lock() {
                    text.push_str(&chunk);
                }
                emit(TranslationEvent::Delta {
                    session_id: delta_session_id.clone(),
                    text: chunk,
                });
            }, &cancel)
            .await?;

        let full_text = full_text
            .lock()
            .map(|text| text.clone())
            .unwrap_or_default();

        // provider 提前返回后据取消状态分流：取消走 Cancelled（非错误），否则 Finished。
        if cancel.is_cancelled() {
            emit(TranslationEvent::Cancelled {
                session_id: request.session_id,
            });
        } else {
            emit(TranslationEvent::Finished {
                session_id: request.session_id,
                full_text,
            });
        }

        Ok(())
    }
}
```

- [ ] **步骤 4：编译 + 测试（任务 5-8 统一验证）**

运行：`cd src-tauri && cargo build`
预期：编译通过（trait/mock/openai/service 四处已全部适配）。若报错集中在签名不匹配，按报错微调。

运行：`cd src-tauri && cargo test --lib core::translation`
预期：PASS，新增 service 取消测试 2 个 + types Cancelled 测试 + 现有测试全过。

- [ ] **步骤 5：Commit（任务 5-8 合并提交）**

```bash
git add src-tauri/src/core/llm/provider.rs src-tauri/src/core/llm/mock.rs src-tauri/src/core/llm/openai_compatible.rs src-tauri/src/core/translation/service.rs
git commit -m "feat(translation): LlmProvider trait 加取消信号并据状态 emit Cancelled/Finished"
```

---

## 任务 9：编排接入取消信号 + last_input（web_popup）

**文件：**
- 修改：`src-tauri/src/ui/web_popup.rs`

> 测试说明：`start_translation_from_input` 真实触发翻译（spawn + 网络），无法在单测里干净验证其副作用。它与 `AppState` 的契约（set last_input / set token / clear token）已由任务 2/3 的 AppState 单测覆盖；本任务的集成正确性由任务 16 全量 `cargo test` + 人工验证保证。故本任务不新增单测，专注适配 trait 新签名。

- [ ] **步骤 1：编写实现代码**

在 `src-tauri/src/ui/web_popup.rs` 顶部 use 区，加 `CancellationToken`（在现有 `use tauri::Emitter;` 后追加一行，其余 use 保持不变）：

```rust
use tauri::Emitter;
use tokio_util::sync::CancellationToken;
```

> 注意：现有 use 块已含 `TranslationInput`（`crate::core::translation::{TranslationEvent, TranslationInput, TranslationRequest, TranslationService, TranslationSessionId}`），无需补。

将 `start_translation_from_input` 函数内 `state.try_begin_translation()?;` 后、`show_window(&app);` 前的段落：

```rust
    state.try_begin_translation()?;
    if let Err(error) =
        cache_automatic_source_text_for_popup(&request.input, request.source_text(), state)
    {
        let _ = state.finish_translation();
        return Err(error);
    }
```

改为（插入存 token + last_input，每条失败路径都回滚 token）：

```rust
    state.try_begin_translation()?;

    let cancel_token = CancellationToken::new();
    if let Err(error) = state.set_current_cancel_token(cancel_token.clone()) {
        let _ = state.finish_translation();
        return Err(error);
    }
    if let Err(error) = state.set_last_translation_input(request.input.clone()) {
        let _ = state.clear_current_cancel_token();
        let _ = state.finish_translation();
        return Err(error);
    }
    if let Err(error) =
        cache_automatic_source_text_for_popup(&request.input, request.source_text(), state)
    {
        let _ = state.clear_current_cancel_token();
        let _ = state.finish_translation();
        return Err(error);
    }
```

将 spawn 块内 `translate_with(request, |event| {...})` 调用改为三参，并在收尾加 `clear_current_cancel_token`。原 spawn 块：

```rust
    tauri::async_runtime::spawn(async move {
        let failed_session_id = request.session_id.clone();
        let result = translation_service
            .translate_with(request, |event| {
                let _ = emit_translation_event(&app_handle, event);
            })
            .await;

        if let Err(error) = result {
            let retryable = error.retryable();
            let _ = emit_translation_event(
                &app_handle,
                TranslationEvent::Failed {
                    session_id: failed_session_id,
                    message: error.to_string(),
                    retryable,
                },
            );
        }
        let _ = state_for_task.finish_translation();
    });
```

改为：

```rust
    tauri::async_runtime::spawn(async move {
        let failed_session_id = request.session_id.clone();
        let result = translation_service
            .translate_with(request, cancel_token, |event| {
                let _ = emit_translation_event(&app_handle, event);
            })
            .await;

        if let Err(error) = result {
            let retryable = error.retryable();
            let _ = emit_translation_event(
                &app_handle,
                TranslationEvent::Failed {
                    session_id: failed_session_id,
                    message: error.to_string(),
                    retryable,
                },
            );
        }
        let _ = state_for_task.clear_current_cancel_token();
        let _ = state_for_task.finish_translation();
    });
```

> **move 语义要点**：`cancel_token` 在 `set_current_cancel_token(cancel_token.clone())` 时传的是 clone，原 `cancel_token` 所有权未被移走，故后续 spawn 的 `async move` 闭包可继续捕获并 move `cancel_token`。`state_for_task`、`translation_service`、`request`、`app_handle`、`cancel_token` 五者均被 move 进闭包。若编译报 `cancel_token` 已被 move，检查是否在某处误用了 `cancel_token`（非 clone）作参数——只允许 `.clone()` 调用消耗 clone。

- [ ] **步骤 2：编译**

运行：`cd src-tauri && cargo build`
预期：编译通过。`TranslationInput` 已派生 `Clone`（见 types.rs），`request.input.clone()` 合法。

- [ ] **步骤 3：运行现有 web_popup 测试确认不回归**

运行：`cd src-tauri && cargo test --lib ui::web_popup`
预期：PASS（现有 `cache_automatic_source_text` 测试不回归）。

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/ui/web_popup.rs
git commit -m "feat(translation): 编排接入取消信号并记录最近输入供重试"
```

---

## 任务 10：cancel_translation / retry_translation command

**文件：**
- 修改：`src-tauri/src/ui/web_popup.rs`

- [ ] **步骤 1：加两个 command**

在 `src-tauri/src/ui/web_popup.rs` 的 `start_translation` command 后追加：

```rust
#[tauri::command]
pub async fn cancel_translation(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.cancel_current_translation()
}

#[tauri::command]
pub async fn retry_translation(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let input = state
        .take_last_translation_input()?
        .ok_or_else(|| "没有可重试的翻译".to_string())?;
    start_translation_from_input(input, app, state.inner())
}
```

- [ ] **步骤 2：编译**

运行：`cd src-tauri && cargo build`
预期：编译通过。

- [ ] **步骤 3：Commit（与任务 11 注册合并提交）**

跳过独立 commit，进入任务 11。

---

## 任务 11：注册两个 command

**文件：**
- 修改：`src-tauri/src/lib.rs`

- [ ] **步骤 1：改 lib.rs**

在 `src-tauri/src/lib.rs` 顶部 use 区，`ui::web_popup::{start_translation, take_pending_source_text}` 改为：

```rust
use ui::{
    config::{get_app_config, save_app_config},
    overlay::{
        cancel_capture, get_capture_frame_bytes, get_capture_frame_meta, show_overlay,
        submit_capture_region,
    },
    web_popup::{
        cancel_translation, retry_translation, start_translation, take_pending_source_text,
    },
};
```

`invoke_handler` 的 `generate_handler!` 列表加两项（在 `start_translation` 后）：

```rust
        .invoke_handler(tauri::generate_handler![
            start_translation,
            cancel_translation,
            retry_translation,
            take_pending_source_text,
            get_app_config,
            save_app_config,
            get_capture_frame_meta,
            get_capture_frame_bytes,
            submit_capture_region,
            cancel_capture,
            show_overlay,
        ])
```

- [ ] **步骤 2：编译**

运行：`cd src-tauri && cargo build`
预期：编译通过。

- [ ] **步骤 3：Commit（任务 10+11 合并）**

```bash
git add src-tauri/src/ui/web_popup.rs src-tauri/src/lib.rs
git commit -m "feat(translation): 注册 cancel_translation/retry_translation 命令"
```

---

## 任务 12：全量编译 + 测试（后端收尾）

**文件：** 无（验证）

- [ ] **步骤 1：全量编译与测试**

运行：`cd src-tauri && cargo build`
预期：PASS。

运行：`cd src-tauri && cargo test`
预期：所有单元测试 PASS（含任务 1/2/3/8 新增）。

- [ ] **步骤 2：人工核对 trait 调用方**

确认 `start_translation_from_input` 内 `translate_with(request, cancel_token, |event| {...})` 三参调用正确；`ocr_popup.rs` / `overlay.rs` 未直接调 `translate_with`（它们走 `start_translation_from_input`），故无需改动。

---

## 任务 13：前端 cancelBtn / retryBtn（index.html）

**文件：**
- 修改：`frontend/index.html`

- [ ] **步骤 1：加按钮**

在 `frontend/index.html` 的 `.action-bar` 内，`translateBtn` 后、`settingsBtn` 前插入取消与重试按钮：

将：

```html
      <div class="action-bar">
        <button id="translateBtn">翻译</button>
        <button id="settingsBtn">设置</button>
        <button id="clearBtn">清空</button>
      </div>
```

改为：

```html
      <div class="action-bar">
        <button id="translateBtn">翻译</button>
        <button id="cancelBtn" hidden>取消</button>
        <button id="retryBtn" hidden>重试</button>
        <button id="settingsBtn">设置</button>
        <button id="clearBtn">清空</button>
      </div>
```

- [ ] **步骤 2：Commit**

```bash
git add frontend/index.html
git commit -m "feat(frontend): 加取消与重试按钮"
```

---

## 任务 14：前端按钮状态机与 cancelled 渲染（main.js）

**文件：**
- 修改：`frontend/main.js`

- [ ] **步骤 1：加按钮引用与状态机**

在 `frontend/main.js` 顶部元素引用区，`clearBtn` 后追加：

```js
const cancelBtn = document.getElementById('cancelBtn');
const retryBtn = document.getElementById('retryBtn');
```

将现有 `setTranslating(value)` 函数替换为统一的按钮状态设置函数 `setActionButtons`，并保留 `setTranslating` 兼容（或直接替换所有调用点）：

找到：

```js
function setTranslating(value) {
  isTranslating = value;
  translateBtn.disabled = value;
  clearBtn.disabled = value;
  translateBtn.textContent = value ? '翻译中...' : '翻译';
}
```

改为：

```js
function setActionButtons({ translating, canRetry }) {
  isTranslating = translating;
  translateBtn.disabled = translating;
  clearBtn.disabled = translating;
  translateBtn.textContent = translating ? '翻译中...' : '翻译';
  cancelBtn.hidden = !translating;
  retryBtn.hidden = !canRetry;
  retryBtn.disabled = translating;
}
```

将文件中所有 `setTranslating(true)` 替换为 `setActionButtons({ translating: true, canRetry: false })`，`setTranslating(false)` 替换为 `setActionButtons({ translating: false, canRetry: true })`。涉及位置：

- `translateBtn.click` 处理器内：`setTranslating(true);`（翻译中）
- `translateBtn.click` catch 内：`setTranslating(false);`（失败，可重试）
- `clearBtn.click` 内：`setTranslating(false);` → 改为 `setActionButtons({ translating: false, canRetry: false });`（清空后无重试目标，先隐藏 retry；待后续翻译才有）

> 注意：`clearBtn.click` 清空后 `currentSessionId = null`，retryBtn 应隐藏（无重试目标）。故用 `canRetry: false`。

- [ ] **步骤 2：renderTranslationEvent 加 cancelled 分支 + 调整按钮**

在 `renderTranslationEvent` 的 `switch` 内，`failed` 分支后追加 `cancelled` 分支，并调整各分支按钮显隐。

找到 `case 'started':` 块，确认其内调用改为 `setActionButtons({ translating: true, canRetry: false });`（替换原 `setTranslating(true)`）。

找到 `case 'finished':` 块，将 `setTranslating(false);` 改为 `setActionButtons({ translating: false, canRetry: true });`。

找到 `case 'failed':` 块，将 `setTranslating(false);` 改为 `setActionButtons({ translating: false, canRetry: true });`。

在 `case 'failed':` 块后、`default:` 前追加：

```js
    case 'cancelled':
      if (!shouldHandleSessionEvent(payload)) return;
      // 保留已渲染的部分译文，追加灰色「已取消」标记
      outputText.textContent += '\n[已取消]';
      outputText.style.color = '#999';
      currentSessionId = null;
      setActionButtons({ translating: false, canRetry: true });
      break;
```

- [ ] **步骤 3：加按钮事件**

在 `frontend/main.js` 的 `clearBtn.addEventListener('click', ...)` 后追加：

```js
cancelBtn.addEventListener('click', async () => {
  if (!invoke) {
    return;
  }
  try {
    await invoke('cancel_translation');
  } catch (error) {
    outputText.textContent = String(error);
    outputText.style.color = '#b42318';
  }
});

retryBtn.addEventListener('click', async () => {
  if (isTranslating) {
    return;
  }
  if (!invoke) {
    outputText.textContent = 'Tauri API 未就绪，请在桌面应用中运行';
    outputText.style.color = '#b42318';
    return;
  }
  outputText.textContent = '翻译中...';
  outputText.style.color = '#999';
  setActionButtons({ translating: true, canRetry: false });
  try {
    await invoke('retry_translation');
  } catch (error) {
    outputText.textContent = String(error);
    outputText.style.color = '#b42318';
    currentSessionId = null;
    setActionButtons({ translating: false, canRetry: true });
  }
});
```

- [ ] **步骤 4：前端语法检查**

运行：`node --check frontend/main.js`
预期：无输出（语法正确）。

- [ ] **步骤 5：Commit**

```bash
git add frontend/main.js
git commit -m "feat(frontend): 取消/重试按钮状态机与 cancelled 渲染"
```

---

## 任务 15：按钮样式（style.css）

**文件：**
- 修改：`frontend/style.css`

> 现有样式（已读确认）：`.action-bar button { flex:1; padding:8px; border:none; border-radius:6px; ... }` 等比布局；`#translateBtn` 蓝（#4a90d9）、`#settingsBtn`/`#clearBtn` 灰。cancelBtn/retryBtn 作为 `.action-bar button` 子项自动继承等比布局与基础样式，但 `hidden` 属性需兜底，且两按钮需各自色调。

- [ ] **步骤 1：加样式**

在 `frontend/style.css` 的 `#clearBtn { ... }` 规则后追加：

```css
#cancelBtn {
  background: #f0f0f0;
  color: #555;
}

#retryBtn {
  background: #38bdf8;
  color: #fff;
}
```

并在文件顶部 `* { ... }` 规则后或 `.hidden` 规则旁追加 `hidden` 属性兜底（确保 flex 容器内 `hidden` 按钮真隐藏——`.action-bar button { flex:1 }` 会让 hidden 项仍占位除非显式 display:none）：

```css
[hidden] {
  display: none !important;
}
```

- [ ] **步骤 2：Commit**

```bash
git add frontend/style.css
git commit -m "style(frontend): 取消/重试按钮色调与 hidden 兜底"
```

---

## 任务 16：整体编译 + 全量测试 + 人工验证

**文件：** 无（验证）

- [ ] **步骤 1：全量编译与测试**

运行：`cd src-tauri && cargo build`
预期：PASS。

运行：`cd src-tauri && cargo test`
预期：所有单元测试 PASS。

运行：`node --check frontend/main.js`
预期：无输出。

- [ ] **步骤 2：人工验证（`npm run tauri dev`，用 mock provider）**

启动（设 mock）：`$env:SHIZI_LLM_PROVIDER='mock'; npm run tauri dev`

逐项确认：

- 手动输入文本翻译 → 翻译中显示「取消」按钮，完成后显示「重试」。
- 翻译中点取消 → 立即停止 + 输出区追加「[已取消]」+ retryBtn 可用。
- 取消后点重试 → 用同原文重新翻译，流式正常。
- 翻译完成后点重试 → 重新翻译。
- 重复点取消 → 无报错、无重复事件。
- busy 中 retryBtn 禁用（灰色不可点）。
- `Alt+T` 划词翻译不回归（选中文本 → 翻译 → 可取消 → 可重试）。
- `Alt+O` 截图 OCR 翻译不回归（框选识别 → 翻译 → 可取消 → 可重试）。

- [ ] **步骤 3：可选——真实 OpenAI provider 取消验证**

配置真实 OpenAI-compatible API Key，输入长文本翻译中点取消，观察：

- 译文立即停止增长（断流生效）。
- 输出区「[已取消]」。
- retryBtn 可用且能重试。

- [ ] **步骤 4：同步文档**

更新 `docs/architecture/screenshot-ocr-architecture.md`（若涉及翻译链路描述）或 `docs/roadmap/progressive-development-plan.md` 里程碑 1「暂未完成」段落，标注 `Cancelled` 事件、取消/重试已完成。具体：

在 `docs/roadmap/progressive-development-plan.md` 的「暂未完成 / 后续演进」段落，将 `Cancelled 事件、usage/token 统计、取消/重试交互。` 中 `Cancelled` 与 `取消/重试` 标记为已完成（保留 usage/token 统计为未完成）。

- [ ] **步骤 5：Commit**

```bash
git add docs/roadmap/progressive-development-plan.md
git commit -m "docs(translation): 标注取消/重试/Cancelled 事件已落地"
```

---

## 风险与已知限制

- **取消响应延迟**：`select!` 在 `stream.next()` chunk 边界响应。极端情况多输出一个 chunk 再停。可接受。
- **取消与自然结束竞争**：provider 即将返回时点取消，`is_cancelled` 可能未置位 → emit `Finished`；已置位 → emit `Cancelled`。两者自洽。
- **mock 阻塞迁移**：`thread::sleep`→`tokio::time::sleep` 改变调度行为（不再阻塞 runtime worker），属正面修复。现有 mock 测试若有依赖时序需复核（当前 mock 无独立时序测试）。
- **provider trait 破坏性改动**：trait 签名变更影响 mock/openai/service 三处，任务 5-8 原子 commit 保证编译性。
- **重试不重试触发动作**：retry 复用 `last_translation_input`，不重新执行划词/截图。前置失败（`show_translation_error`）不更新 last_input，故前置失败后 retry 重试上次翻译的输入或返回「没有可重试的翻译」。
- **tokio 主依赖新增**：主依赖加 `tokio`（time feature）可能与 Tauri 已带的 tokio 版本统一，无冲突；若 `cargo build` 报版本冲突，按报错对齐版本。

---

## 自检结论

- **规格覆盖度**：
  - `LlmProvider` trait 加 `CancellationToken` → 任务 5。
  - OpenAI provider `select!` 包裹 → 任务 7。
  - Mock provider 迁移 + `select!` → 任务 6。
  - `TranslationService` 据 `is_cancelled` emit Cancelled/Finished → 任务 8。
  - `AppState` cancel token 槽 → 任务 2。
  - `AppState` last_translation_input 槽 → 任务 3。
  - `cancel_translation` command → 任务 10。
  - `retry_translation` command → 任务 10。
  - `Cancelled` 事件变体 → 任务 1。
  - 编排接入取消信号 + last_input → 任务 9。
  - 注册 command → 任务 11。
  - 前端 cancelBtn/retryBtn → 任务 13。
  - 前端 cancelled 渲染 + 状态机 → 任务 14。
  - `tokio-util`/`tokio` time 依赖 → 任务 4。
  - `TranslationInput` 加 `PartialEq`（测试需要）→ 任务 3 步骤 3 附带。
  - 手动验证（含 Alt+T/Alt+O 回归）→ 任务 16。
  - 文档同步 → 任务 16 步骤 4。
- **占位符扫描**：无 TODO/待定；`tokio-util`/`tokio` 版本与 feature 标注「以编译为准微调」属合理工程注记，非占位；style.css 任务标注「按需」并提供具体代码与跳过条件。
- **类型一致性**：`stream_translate(&self, request, on_delta, cancel: &CancellationToken)` 贯穿任务 5（trait 定义）/6（mock）/7（openai）/8（service 调用）一致；`translate_with(request, cancel: CancellationToken, emit)` 贯穿任务 8（定义）/9（编排调用）一致；`AppState::{set_current_cancel_token, cancel_current_translation, clear_current_cancel_token, set_last_translation_input, take_last_translation_input}` 贯穿任务 2/3（定义）/9（调用）/10（command 调用）一致；`Cancelled { session_id }` 贯穿任务 1（定义）/8（emit）/14（前端 `case 'cancelled'`）一致；`cancel_translation`/`retry_translation` command 名贯穿任务 10（定义）/11（注册）/14（前端 invoke）一致。
