# Token Usage 透传设计规格

## 背景与动机

roadmap（[progressive-development-plan.md](../../roadmap/progressive-development-plan.md)）把 `usage/token 统计` 列为里程碑 1 未完成项。当前实现：

- `TranslationEvent::Finished` 只有 `full_text`，无 usage 字段（[types.rs](../../../src-tauri/src/core/translation/types.rs)）。
- `LlmProvider::stream_translate` 的回调签名为 `FnMut(String)`，只传文本增量，无通道回传 usage（[provider.rs](../../../src-tauri/src/core/llm/provider.rs)）。
- OpenAI provider 未请求 `stream_options.include_usage`、未解析 `usage`；Claude provider 未解析 `message_start.usage.input_tokens` 与 `message_delta.usage.output_tokens`。两家 SSE 实际都返回 token 数，被丢弃。
- 配置层无 usage 相关字段。

本组范围（已与「翻译模式/源语言」拆分）：**把 usage 从 provider SSE 解析出来，经 service 透传到前端弹窗展示，并提供配置开关**。翻译模式/源语言另开一组。

## 范围

- 数据模型：新增 `TokenUsage`，`Finished` 事件加 usage。
- provider 通道：`stream_translate` 回调由 `FnMut(String)` 改为 `FnMut(LlmStreamEvent)`，新增 `LlmStreamEvent` 枚举。
- 三个 provider（mock / openai / claude）解析 usage 并回传。
- 配置开关 `collect_usage`，默认开启，控制采集。
- service 编排：根据开关决定 `Finished.usage` 是否填充。
- 前端：弹窗展示 token 用量脚注；设置页加开关。
- 测试 + 文档同步。

## 设计

### 数据模型（types.rs）

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}
```

字段只保留 input + output（两家共同子集）。不保留 cache 字段——翻译场景短文本、几乎不命中 prompt cache，遵循 YAGNI；后续产品化阶段如需可再扩展。`total` 由前端求和，不在类型中冗余。

`Finished` 事件加 usage：

```rust
Finished {
    session_id: TranslationSessionId,
    full_text: String,
    usage: Option<TokenUsage>,   // 采集关闭或 provider 未返回时为 None
}
```

### provider trait 通道类型化（provider.rs）

```rust
pub enum LlmStreamEvent {
    Delta(String),
    Usage(TokenUsage),
}

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

选型理由：枚举化把 provider→service 通道类型化，与 `TranslationEvent` 枚举对称；mock provider 能自然发 usage 让单测覆盖完整链路；未来加 thinking 增量、工具调用等流事件零成本扩展。否决了「双回调」（回调膨胀）与「返回值」（扩展性差、且无法表达流中段 usage）。

### 配置开关（config/types.rs）

```rust
#[serde(default = "default_true")]
pub collect_usage: bool,
```

- 环境变量 `SHIZI_COLLECT_USAGE=true/false`，默认 `true`。
- `normalized()` 不修改该字段（布尔无需规整）。
- 沿用现有 `popup_precreate / overlay_precreate` 的 `#[serde(default)]` 模式，保证旧 config.json 缺字段可反序列化。
- 顶层全局开关，不按 provider 分。

**开关语义：控制采集，非展示。** 关闭时 `Finished.usage` 为 None；前端因 `usage` 为 `null` 不渲染脚注，天然兼容。provider 无条件解析 SSE usage（解析本身极轻），开关由 service 层统一拦截——provider 不感知开关，职责单一。

### service 编排（service.rs）

`translate_with` 加 `collect_usage: bool` 入参，service 保持无状态（单测可独立喂 true/false）。由 [web_popup.rs](../../../src-tauri/src/ui/web_popup.rs) 从 config 读出后传入。

```rust
pub async fn translate_with<F>(
    &self,
    request: TranslationRequest,
    collect_usage: bool,
    cancel: CancellationToken,
    mut emit: F,
) -> Result<(), TranslationError>
where F: FnMut(TranslationEvent) + Send,
```

回调内部：
- `LlmStreamEvent::Delta(t)` → 累积 `full_text` + emit `Delta`，沿用现有 Arc<Mutex<String>> 模式。
- `LlmStreamEvent::Usage(u)` → 若 `collect_usage`，写入 `Arc<Mutex<Option<TokenUsage>>>`；否则丢弃。
- 流结束：取消 → emit `Cancelled`；否则 emit `Finished { usage: usage.lock().clone() }`。

### 各 provider 实现

**OpenAI provider**（[openai_compatible.rs](../../../src-tauri/src/core/llm/openai_compatible.rs)）：
- 请求体加 `stream_options: { include_usage: true }`（OpenAI 规范：流式 usage 出现在最后一个 chunk，`choices` 为空数组）。
- `ChatCompletionChunk` 反序列化加 `usage: Option<Usage>`；`Usage { input_tokens, output_tokens }`（注意 OpenAI 字段名为 `prompt_tokens / completion_tokens`，反序列化时 `rename`）。
- 命中时 `on_event(LlmStreamEvent::Usage(...))`。
- `[DONE]` 与 usage 解析互不影响。

**Claude provider**（[claude.rs](../../../src-tauri/src/core/llm/claude.rs)）：
- `message_start` 事件带 `message.usage.input_tokens` → `Usage(input, 0)`（input 已知，output 尚未产生）。
- `message_delta` 事件带 `usage.output_tokens`（累积值）→ `Usage(input, output)`。input 沿用 `message_start` 的值，需 provider 内部持有。
- 在现有 `ClaudeSseEvent` 反序列化上加 `usage` 字段。
- **`consume_sse_event` 处理方式**：当前它是无状态关联函数（仅取 `event` 字符串 + 回调）。Claude 的 usage 解析需要跨事件持有 `input_tokens`（`message_start` 给 input，`message_delta` 给 output 累积值）。方案：`stream_translate` 内联维护 `Option<u64> input_tokens`，把 `consume_sse_event` 改为接收 `&mut Option<u64>`（或返回解析出的 usage 让调用方累积），命中 usage 事件时 `on_event(Usage(...))`。不引入 provider 实例字段，保持 provider 无状态、可复用。具体签名在实现计划阶段定，但「不引入实例状态、由调用点持有 input_tokens」是本规格的约束。

**Mock provider**（[mock.rs](../../../src-tauri/src/core/llm/mock.rs)）：
- 流末发一个固定 `Usage { input_tokens: <按源文本长度估算>, output_tokens: <按译文长度估算> }`，让单测覆盖 usage 全链路。

### 前端展示（translate.js / translate.css）

- `finished` 分支：若 `payload.usage` 非空，在输出区下方渲染一行轻量脚注，格式 `27 → 18 tokens`（input → output）。
- 与来源徽章对称：`showUsageFooter(usage)` / `hideUsageFooter()`；翻译结束显示，新翻译 / 取消 / 失败 / 清空时清除。
- 采集关闭时 `payload.usage` 为 `null`，前端不渲染脚注。
- 样式低饱和、小字号，不抢译文焦点。

### 设置页（settings.html / settings.js）

- 加 checkbox「采集 token 用量（显示翻译 token 消耗）」，绑定 `collectUsage`。
- `save_app_config` / `get_app_config` 读写该字段，沿用现有 provider 字段存取模式。
- 默认勾选。

## 测试（TDD）

- `types.rs`：`Finished` 带 usage 的序列化字段名（`usage` / `inputTokens` / `outputTokens`）。
- `service.rs`：
  - mock provider 发 Usage 后，`Finished.usage` 为 `Some`。
  - `collect_usage=false` 时 `Finished.usage` 为 `None`（即使 provider 发了 Usage）。
  - 取消时不 emit `Finished`。
- `openai_compatible.rs`：
  - 构造带 `usage` 的最后一个 chunk SSE，断言 `on_event(Usage(...))` 被调用。
  - 请求体包含 `stream_options.include_usage`。
- `claude.rs`：`message_start` / `message_delta` 的 usage 分别被解析并回传。
- 前端 `node --check frontend/translate.js`。

## 验证命令

```bash
cd src-tauri && cargo test
cd src-tauri && cargo build
node --check frontend/translate.js
```

## 文档同步（收尾硬门禁）

- README「当前能力」补 token 用量展示与采集开关；「当前限制」无相关项需移除（usage 本就不在限制列表，仅补能力）。
- roadmap 第 83 行 `usage/token 统计` 标记 ✅。
- 本组 plan 复选框回填。

## 不在本组范围

- 翻译模式（翻译/润色/解释/总结）与源语言字段——另开一组。
- `OcrText.image_id` 半成品字段清理——属输入模型完善组，不在本组。
- 翻译历史记录、token 用量累计统计——产品化阶段。
