# 翻译执行控制设计（取消 + 重试 + Cancelled 事件）

## 背景与目标

里程碑 1 已完成翻译主链路：手动输入 / 划词 / OCR 三类输入 → `TranslationService` → `LlmProvider` 流式 → `translation:event` 前端渲染。但翻译一旦开始便无法中止，失败后只能手动重新输入重译，与 Bob 体验有差距。

本设计补齐翻译执行控制：用户可在流式翻译过程中**取消**（立即断流、停止计费），翻译**结束**（完成 / 失败 / 已取消）后可**重试**，并引入 `Cancelled` 事件让前端正确表达「已取消」状态。

核心原则：

- 取消是用户意图，不是错误；emit `Cancelled`，不进 `Failed`。
- 取消必须立即中断 provider 的网络流（drop reqwest stream、停止后续 token 拉取与计费），而非仅停止前端渲染。
- 重试复用现有 `start_translation_from_input` 主链路，不另起支线。
- provider trait 的取消信号对所有 provider 统一，后续 Claude provider 直接复用。

## 范围与非目标

**覆盖：**

- `TranslationEvent::Cancelled` 变体与序列化。
- `LlmProvider` trait 增加取消信号，mock / openai-compatible 两个 provider 同步改造。
- `TranslationService` 据取消状态 emit `Cancelled` / `Finished`。
- 取消翻译 command `cancel_translation`。
- 重试翻译 command `retry_translation`，复用最近一次翻译输入。
- 前端取消 / 重试按钮与 `cancelled` 状态渲染。

**非目标：**

- Anthropic / Claude 专用 provider（后续独立 spec，届时直接实现带取消的 trait）。
- 翻译历史记录、usage / token 统计。
- 自动超时重试、指数退避。
- 取消后的部分译文编辑 / 续译。
- 快捷键触发取消 / 重试（先做按钮交互，快捷键留后续）。

## 现状与约束

### 已有可复用能力

- `TranslationService::translate_with`（`core/translation/service.rs`）已统一 provider 流式输出到 `TranslationEvent`，是接入取消信号的天然位置。
- `AppState::try_begin_translation` / `finish_translation` 已提供 busy 单并发保护，重试可复用。
- `start_translation_from_input`（`ui/web_popup.rs`）是三类输入的统一入口，重试直接复用。
- 前端 `renderTranslationEvent` 已按 `type` 分支渲染，扩展 `cancelled` 分支即可。
- `started` 事件已携带 `sourceText`，前端已有 `currentSessionId` 会话过滤机制。

### 关键约束（决定设计）

- **`LlmProvider::stream_translate` 无取消信号**（`core/llm/provider.rs`）：当前签名为 `(&self, &TranslationRequest, &mut dyn FnMut(String)) -> Result<(), LlmError>`，OpenAI provider 的 `while let Some(bytes) = stream.next().await` 循环无法被外部中断。要做到立即断流，必须给 trait 加取消信号——这是破坏性改动，mock / openai-compatible 两个 provider 都要同步改。
- **mock provider 用同步 `thread::sleep`**（`core/llm/mock.rs`）：在 async 函数里阻塞 runtime，`tokio::select!` 无法中断阻塞的 `thread::sleep`。为支持取消，mock 须迁到 `tokio::time::sleep`，再用 `select!` 包裹。这是取消功能的必要改动。
- **翻译任务 spawn 后无 handle**（`ui/web_popup.rs`）：当前 `tauri::async_runtime::spawn` 后不保留 JoinHandle，无法从外部 abort；取消只能靠协作式信号——provider 在 `select!` 中响应 token。
- **`pending_source_text` 是 take 一次性消费**：重试需要持久的「最近输入」，不能复用 `pending_source_text`（那是划词 / OCR 原文回填 popup 的临时槽）。

## 架构总览

四层改动，每层单一职责：

```text
TranslationEvent   新增 Cancelled { session_id } 变体
LlmProvider trait  stream_translate 加 cancel: &CancellationToken 参数；provider 内 select! 响应取消
TranslationService translate_with 接收 cancel；provider 返回后据 is_cancelled 决定 emit Cancelled / Finished
AppState           新增 current_cancel_token + last_translation_input 两槽（存取 / 清空 / 幂等）
编排 (web_popup)    spawn 前存 token + last_input；新增 cancel_translation / retry_translation command
前端               新增 cancelBtn / retryBtn + cancelled 渲染 + 按钮状态机
```

## 取消链路设计

### LlmProvider trait 改造

`core/llm/provider.rs` 的 trait 签名增加取消信号：

```rust
use tokio_util::sync::CancellationToken;

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

**取消不是错误**：provider 检测到取消时提前 `return Ok(())`，由 `TranslationService` 负责 emit `Cancelled`。这样 provider 只关心「流是否被中断」，不关心「中断后发什么事件」，职责清晰。

### OpenAI provider 改造

`core/llm/openai_compatible.rs` 的流式循环改为 `tokio::select!` 包裹 `stream.next()` 与 `cancel.cancelled()`：

```rust
loop {
    tokio::select! {
        _ = cancel.cancelled() => return Ok(()),
        bytes = stream.next() => {
            let Some(bytes) = bytes else { break };
            let bytes = bytes.map_err(|error| LlmError::Http(error.to_string()))?;
            // ... 原有 buffer 累积 + consume_sse_event 逻辑不变
        }
    }
}
```

cancel 触发时 select 跳出，函数返回 `Ok(())`；`stream`（reqwest `bytes_stream`）在函数返回时被 drop，底层连接断开、停止后续 token 拉取与计费。SSE 解析逻辑（`consume_sse_event`）不变。

> 实现者注意：`stream.next()` 的 future 在 select 分支里每次重新 poll，需保证 `stream` 是可变且 `StreamExt` 在作用域（现状已满足）。select 中 `bytes = stream.next()` 的 future 在未选中时被丢弃，下次循环重建，语义正确。

### Mock provider 改造

`core/llm/mock.rs` 当前用 `thread::sleep(Duration::from_millis(180))` 阻塞。改为 `tokio::time::sleep` 并用 `select!` 包裹：

```rust
for chunk in chunks {
    on_delta(chunk);
    tokio::select! {
        _ = cancel.cancelled() => return Ok(()),
        _ = tokio::time::sleep(Duration::from_millis(180)) => {}
    }
}
```

迁移 `thread::sleep` → `tokio::time::sleep` 同时修复了「mock 阻塞 async runtime」的既有隐患。`use std::thread` 若不再需要则移除。

### TranslationService 改造

`core/translation/service.rs` 的 `translate_with` 接收 `cancel: CancellationToken`，provider 返回后据取消状态分流：

```rust
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
    // ... 累积 delta 逻辑不变
    self.provider
        .stream_translate(&request, &mut |chunk| { /* ... */ }, &cancel)
        .await?;

    let full_text = full_text.lock().map(|t| t.clone()).unwrap_or_default();

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
```

`TranslationError` 不新增取消变体——取消走 `Ok` 路径，`Cancelled` 事件由 service 内部 emit。

### AppState cancel token 槽

`app/state.rs` 新增字段与方法：

```rust
use tokio_util::sync::CancellationToken;

// 结构体新增字段（在 translation_busy 后）
current_cancel_token: Arc<Mutex<Option<CancellationToken>>>,
```

方法（幂等、与现有 busy / capture 锁风格一致）：

```rust
/// 存入当前翻译的取消信号。begin 时调用，覆盖前次（前次应已清空）。
pub fn set_current_cancel_token(&self, token: CancellationToken) -> Result<(), String> {
    let mut slot = self.current_cancel_token.lock().map_err(|_| "取消信号状态锁已损坏".to_string())?;
    *slot = Some(token);
    Ok(())
}

/// 取出并触发当前翻译的取消信号。幂等：无 token 或已结束返回 Ok 无操作。
pub fn cancel_current_translation(&self) -> Result<(), String> {
    let token = {
        let mut slot = self.current_cancel_token.lock().map_err(|_| "取消信号状态锁已损坏".to_string())?;
        slot.take()
    };
    if let Some(token) = token {
        token.cancel();
    }
    Ok(())
}

/// 清空当前 cancel token（翻译自然结束时调用）。幂等。
pub fn clear_current_cancel_token(&self) -> Result<(), String> {
    let mut slot = self.current_cancel_token.lock().map_err(|_| "取消信号状态锁已损坏".to_string())?;
    *slot = None;
    Ok(())
}
```

`AppState::new` 初始化 `current_cancel_token: Arc::new(Mutex::new(None))`。

### 编排与 cancel_translation command

`ui/web_popup.rs` 的 `start_translation_from_input` 改造（在 `try_begin_translation` 成功后、spawn 前）：

```rust
let cancel_token = CancellationToken::new();
state.set_current_cancel_token(cancel_token.clone())?;
state.set_last_translation_input(request.input.clone())?;  // 见重试链路

tauri::async_runtime::spawn(async move {
    let result = translation_service
        .translate_with(request, cancel_token.clone(), |event| {
            let _ = emit_translation_event(&app_handle, event);
        })
        .await;
    // ... 现有 Failed 处理不变
    let _ = state_for_task.clear_current_cancel_token();
    let _ = state_for_task.finish_translation();
});
```

新增 command：

```rust
#[tauri::command]
pub async fn cancel_translation(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.cancel_current_translation()
}
```

`cancel_current_translation` 幂等：翻译自然结束后 token 已被 `clear_current_cancel_token` 清空，重复调用返回 `Ok` 无操作。

> busy 关系：取消不主动 `finish_translation`——翻译任务自身的 spawn 收尾会调 `finish_translation`。cancel 只触发信号，让 provider 尽快返回，service emit `Cancelled`，spawn 末尾统一收尾。避免 cancel 路径与自然结束路径重复 `finish_translation`。

## 重试链路设计

### AppState last_translation_input 槽

`app/state.rs` 新增字段与方法：

```rust
use crate::core::translation::TranslationInput;

// 结构体新增字段（在 current_cancel_token 后）
last_translation_input: Arc<Mutex<Option<TranslationInput>>>,
```

方法：

```rust
/// 记录最近一次成功开始的翻译输入，供重试复用。begin 成功后调用。
pub fn set_last_translation_input(&self, input: TranslationInput) -> Result<(), String> {
    let mut slot = self.last_translation_input.lock().map_err(|_| "重试输入状态锁已损坏".to_string())?;
    *slot = Some(input);
    Ok(())
}

/// 取出最近一次翻译输入。无则返回 None。
pub fn take_last_translation_input(&self) -> Result<Option<TranslationInput>, String> {
    let mut slot = self.last_translation_input.lock().map_err(|_| "重试输入状态锁已损坏".to_string())?;
    Ok(slot.take())
}
```

`AppState::new` 初始化 `last_translation_input: Arc::new(Mutex::new(None))`。

> take 而非 peek：重试后清空，避免「重试 → 失败 → 再次重试」时持有过期输入；但重试内部会重新 `set_last_translation_input`，所以连续重试仍可用（见 retry command）。

### retry_translation command

`ui/web_popup.rs` 新增：

```rust
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

`start_translation_from_input` 内部会重新 `try_begin_translation`（busy 预检）、生成新 `session_id`、新建 `CancellationToken`、覆盖 `last_translation_input`。所以重试 = 用同输入 + 当前配置重新走一次完整翻译链路。

### 重试契约

- **结束后才可重试**：前端 busy 中禁用 `retryBtn`；后端 `try_begin_translation` 兜底拒绝并发（若用户绕过前端禁用）。
- **不区分结束类型**：`Finished` / `Failed` / `Cancelled` 后均可重试，只要 `last_translation_input` 存在。`retryable` 字段仅作为 UI 提示参考，不强制阻止重试——失败是否可重试交给用户判断（例如配置错误重试无意义，但用户想重试就允许）。
- **重试用当前配置**：重试时重新读 `config_store`，因此改完设置后重试会生效（与 Pot 行为一致）。
- **重试仅指「重新翻译上一次的输入」，不重试「触发动作」**：`retry_translation` 复用 `last_translation_input`，不重新执行划词（`Alt+T`）或截图（`Alt+O`）。前置失败（`show_translation_error` 路径，如划词复制失败、OCR 前置失败、busy 拒绝）不经过 `start_translation_from_input`，**不更新** `last_translation_input`——这类 `failed` 后点重试会重试上一次成功翻译的输入（若有），或返回「没有可重试的翻译」。这是有意为之：重试翻译 ≠ 重试触发。前端 `failed` 分支统一显示 retryBtn，但前置失败的 retry 语义即「重试上次翻译」，由后端 last_input 是否存在决定成败。

## Cancelled 事件与序列化

`core/translation/types.rs` 的 `TranslationEvent` 新增变体：

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "type")]
pub enum TranslationEvent {
    Started { /* ... */ },
    Delta { /* ... */ },
    Finished { /* ... */ },
    Failed { /* ... */ },
    Cancelled {
        session_id: TranslationSessionId,
    },
}
```

序列化为 `{ "type": "cancelled", "sessionId": "..." }`，与现有事件驼峰命名一致。

## 前端交互

### index.html

`frontend/index.html` 的 `.action-bar` 新增两个按钮（与 translateBtn / settingsBtn / clearBtn 同区）：

```html
<div class="action-bar">
  <button id="translateBtn">翻译</button>
  <button id="cancelBtn" hidden>取消</button>
  <button id="retryBtn" hidden>重试</button>
  <button id="settingsBtn">设置</button>
  <button id="clearBtn">清空</button>
</div>
```

初始 `hidden`，由 `main.js` 状态机控制显隐。

### main.js 状态机

`renderTranslationEvent` 扩展 `cancelled` 分支，并调整按钮显隐：

```js
function setActionButtons({ translating, canRetry }) {
  translateBtn.disabled = translating;
  clearBtn.disabled = translating;
  translateBtn.textContent = translating ? '翻译中...' : '翻译';
  cancelBtn.hidden = !translating;
  retryBtn.hidden = !canRetry;
  retryBtn.disabled = translating;
}
```

事件分支：

- `started`：`setActionButtons({ translating: true, canRetry: false })`；缓存 `lastSourceText`（已有 `inputText.value = payload.sourceText`）。
- `delta`：累加（不变）。
- `finished`：`setActionButtons({ translating: false, canRetry: true })`；保留译文。
- `failed`：`setActionButtons({ translating: false, canRetry: true })`；保留错误文案。
- `cancelled`：保留已渲染的部分译文，追加灰色「已取消」标记；`setActionButtons({ translating: false, canRetry: true })`。

按钮事件：

- `cancelBtn.click` → `invoke('cancel_translation')`；乐观禁用 cancelBtn（后端会 emit `cancelled`）。
- `retryBtn.click` → `invoke('retry_translation')`（若 invoke 报错，回显错误文案）。
- `clearBtn.click`：清空时也 `invoke('cancel_translation')` 兜底（若 busy 中残留——实际 clearBtn busy 中已禁用，此为防御）。

### style.css

`frontend/style.css` 为 cancelBtn / retryBtn 补按钮样式（复用现有 button 样式，可加次要色调区分 cancel 为中性灰、retry 为强调色）。若无独立样式需求，复用现有 `.action-bar button` 即可，仅在需要时新增。

## 错误处理

- **取消不是错误**：走 `Ok` 路径，emit `Cancelled`，不进 `Failed`。
- **retry 无 last_input**：返回「没有可重试的翻译」（前端禁用已防，后端兜底）。
- **cancel token 幂等**：翻译自然结束后 token 已清空，重复 cancel 返回 Ok 无操作。
- **取消与自然结束竞争**：若用户在 provider 即将返回时点取消，`is_cancelled` 可能未及时置位 → emit `Finished`。可接受：用户看到完整译文，无副作用。若已置位 → emit `Cancelled`，部分译文保留。两种结果都自洽。
- **重试时 busy**：`try_begin_translation` 拒绝并返回「正在翻译中，请稍后再试」（与现有行为一致）。

## 测试策略

### 纯 Rust 单元测试

- `TranslationEvent::Cancelled` 序列化为 `{ "type": "cancelled", "sessionId": "..." }`。
- `AppState` cancel token：`set_current_cancel_token` → `cancel_current_translation` 触发 → `is_cancelled() == true`；`cancel_current_translation` 幂等（无 token 返回 Ok）；`clear_current_cancel_token` 幂等。
- `AppState` `last_translation_input`：set → take 返回 input，再次 take 返回 None。
- Mock provider + fake cancel：先 `set_current_cancel_token`，`cancel_current_translation` 后 `stream_translate` 在首个 sleep 前或中提前返回 `Ok(())`（用 `tokio::time::timeout` 包裹断言不超时且不产出全部 chunk）。
- TranslationService：cancel 已触发时 `translate_with` emit `Cancelled` 且不 emit `Finished`；未 cancel 时 emit `Finished`。（用 fake provider + 收集 emit 事件）
- retry：fake provider 计数，`retry_translation` 后 provider 被再次调用（验证复用输入）。

### 手动验证

`npm run tauri dev` 逐项确认：

- 长文本翻译中点取消 → 立即停止 + 输出区「已取消」+ retryBtn 可用。
- 取消后点重试 → 用同原文重新翻译，流式正常。
- 翻译完成后点重试 → 重新翻译（可用 mock 验证）。
- 翻译失败后点重试 → 重新翻译。
- busy 中 retryBtn 禁用。
- 重复点取消 → 无报错、无重复事件。
- `Alt+T` 划词翻译不回归（取消 / 重试按钮状态正确）。
- `Alt+O` 截图 OCR 翻译不回归（OCR 输入也能取消 / 重试）。

## 依赖

`src-tauri/Cargo.toml` 加 `tokio-util`，启用 `sync` feature（`CancellationToken` 所在）：

```toml
tokio-util = { version = "0.7", features = ["sync"] }
```

> `tokio-util` 的精确版本与 feature 名以本地 `cargo build` 报错为准微调。`tokio` 主依赖现状已含 `time` feature（mock 改造用 `tokio::time::sleep`），若未含则补 `time`。

## 受影响文件清单

**修改：**

- `src-tauri/Cargo.toml` — 加 `tokio-util`（sync feature），必要时补 tokio time feature。
- `src-tauri/src/core/llm/provider.rs` — trait 加 `cancel: &CancellationToken` 参数。
- `src-tauri/src/core/llm/mock.rs` — `thread::sleep` → `tokio::time::sleep` + `select!` 包裹。
- `src-tauri/src/core/llm/openai_compatible.rs` — 流式循环 `select!` 包裹 `stream.next()` 与 `cancel.cancelled()`。
- `src-tauri/src/core/translation/types.rs` — `TranslationEvent` 加 `Cancelled` 变体 + 序列化测试。
- `src-tauri/src/core/translation/service.rs` — `translate_with` 接 `CancellationToken`，据 `is_cancelled` emit `Cancelled` / `Finished`。
- `src-tauri/src/app/state.rs` — `current_cancel_token` + `last_translation_input` 两槽及方法 + 测试。
- `src-tauri/src/ui/web_popup.rs` — spawn 前存 token + last_input；新增 `cancel_translation` / `retry_translation` command；调用点（`start_translation_from_input`、`ocr_popup` / `overlay` 调用方）适配 trait 新签名。
- `src-tauri/src/lib.rs` — 注册 `cancel_translation` / `retry_translation` 两个 command。
- `frontend/index.html` — `.action-bar` 加 cancelBtn / retryBtn。
- `frontend/main.js` — `cancelled` 渲染分支 + 按钮状态机 + 按钮事件。
- `frontend/style.css` — 按钮样式（按需）。

**新增：** 无。

**capabilities：** `cancel_translation` / `retry_translation` 是本地 invoke command，权限由 `core:default` 覆盖，`capabilities/default.json` 无需改动。

## 风险与已知限制

- **取消响应延迟**：`select!` 在 `stream.next()` 已就绪的 chunk 边界响应取消。极端情况下用户点取消时正好有一个 chunk 即将就绪，会多输出一个 chunk 再停止。可接受——单 chunk 延迟、不影响「立即断流」体感。
- **取消与自然结束竞争**：见错误处理章节，两种结果自洽，不强制保证唯一。
- **重试不恢复取消状态**：重试是新 session，不复用旧 cancel token；旧 token 若残留由 `clear_current_cancel_token` 在 spawn 末尾清理。
- **mock 阻塞迁移**：`thread::sleep` → `tokio::time::sleep` 改变了 mock 的调度行为（不再阻塞 runtime worker），属正面修复，但需确认现有 mock 相关测试（若有依赖时序的）仍通过。
- **provider trait 破坏性改动**：所有 `LlmProvider` 实现与调用方都要同步改。当前仅 mock / openai-compatible 两处实现 + service 一处调用，影响面可控。

## 验收标准

- 翻译过程中点取消，立即停止 token 输出，输出区显示「已取消」标记，retryBtn 可用。
- 取消不产生 `Failed` 事件，只产生 `Cancelled` 事件。
- 重复点取消无报错、无重复事件。
- 翻译完成 / 失败 / 取消后，retryBtn 可用且能用同原文重新翻译。
- busy 中 retryBtn 禁用；后端 busy 预检兜底。
- mock / openai-compatible 两个 provider 在取消信号下都能提前返回 Ok。
- `Alt+T` 划词、`Alt+O` 截图 OCR 翻译链路不回归，且都支持取消 / 重试。
- `cargo test` 全过（含新增取消 / 重试 / Cancelled 单测）。

## 自检结论

- **占位符扫描**：无 TODO / 待定；`tokio-util` 版本与 feature 名标注「以编译为准微调」属合理工程注记，非占位。
- **内部一致性**：trait 签名 `stream_translate(..., cancel: &CancellationToken)` 贯穿 provider / mock / openai / service 一致；`translate_with(request, cancel, emit)` 调用方一致；`AppState` 两槽方法名 `set_/take_/clear_/cancel_` 贯穿编排一致；`Cancelled` 事件 type tag `cancelled` 与前端 `case 'cancelled'` 一致。
- **范围检查**：聚焦取消 + 重试 + Cancelled 三件事，一个实现计划可覆盖；Claude provider 明确划为后续独立 spec。
- **模糊性检查**：取消后部分译文保留与否已明确（保留 + 灰色标记）；retry 是否区分失败类型已明确（不区分，retryable 仅作提示）；cancel 是否主动 finish_translation 已明确（不主动，由 spawn 末尾统一收尾）。
