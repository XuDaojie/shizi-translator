# Claude 专用 Provider 设计规格

- 日期：2026-07-01
- 状态：已确认，待实现
- 关联：[translation-execution-control-design](./2026-07-01-translation-execution-control-design.md)（取消/重试已落地，本设计复用其 `CancellationToken` 与 `TranslationService.translate_with`）

## 1. 目的

为翻译链路新增 Anthropic / Claude 原生 Messages API provider，复用既有 `LlmProvider` trait，让用户可在设置面板选择 Claude 作为翻译后端。第一版优先覆盖划词翻译与 OCR 翻译复用的流式链路。

## 2. 范围

### 范围内

- 新增 `ClaudeProvider`，实现 `LlmProvider::stream_translate`，走 Claude 原生 Messages API（`POST /v1/messages`）的 SSE 流式响应
- 新增 `ClaudeAppConfig` 配置模型，`AppConfig` 增加 `claude` 字段；`from_env` + `normalized` + `From<ClaudeAppConfig> for ClaudeConfig`
- provider 工厂（`web_popup.rs`）增加 `"claude"` 分支
- 前端设置面板增加 provider 下拉（openai-compatible / claude / mock）与 Claude 表单
- 单元测试：SSE 事件解析纯函数、配置模型、`From` 转换

### 范围外（YAGNI）

- usage / token 统计（属「下一步」选项 2，单独 spec）
- 多轮对话 / 上下文记忆（当前翻译链路为单轮请求-响应）
- Prompt caching、compaction、task budget 等 beta 特性
- Tool use / vision / 文档输入（当前输入模型仅文本）
- Bedrock / Vertex / Foundry 等第三方平台客户端
- 自动 fallback（refusal 处理仅做错误映射，不做模型回退）

## 3. 架构

### 3.1 分层与新增组件

```
src-tauri/src/core/llm/
  claude.rs            新增：ClaudeProvider + ClaudeConfig + SSE 解析
  mod.rs               导出 ClaudeProvider / ClaudeConfig
src-tauri/src/core/config/types.rs   新增 ClaudeAppConfig + AppConfig.claude 字段
src-tauri/src/ui/web_popup.rs        工厂 match 增加 "claude" 分支
frontend/index.html                  设置面板：provider 下拉 + Claude 表单
frontend/main.js                     provider 切换逻辑 + 配置读写
```

`ClaudeProvider` 与 [OpenAiCompatibleProvider](../../../src-tauri/src/core/llm/openai_compatible.rs) 同构：持有 `reqwest::Client` + `ClaudeConfig`，`stream_translate` 用 `response.bytes_stream()` + `tokio::select!`（`cancel.cancelled()` 与 `stream.next()` 竞速）。

### 3.2 设计决策

| 决策 | 选择 | 理由 |
|---|---|---|
| 实现形态 | 独立 `ClaudeProvider`，非复用 OpenAI 兼容接口 | Claude 原生 SSE 结构、请求体、认证 header 与 OpenAI 不同；用户已明确要"专用 provider" |
| base_url | 可配，默认 `https://api.anthropic.com` | 与 OpenAI provider 一致；支持国内用户走反代/网关 |
| 默认模型 | `claude-haiku-4-5` | 用户选定；翻译为轻量任务，性价比优先 |
| thinking | 默认不传；`enable_thinking: bool` 可配，透传 `{type:"adaptive"}` | 翻译为轻量任务；复杂长文可由用户手动开启；Haiku 4.5 不支持 adaptive（传会 400），不在代码层按模型名过滤，错误交服务端返回 |
| max_tokens | 固定 4096 | 翻译输出长度上限；Messages API 必填字段 |

## 4. 数据流

### 4.1 请求

`POST {base_url}/v1/messages`

Headers：
- `x-api-key: <api_key>`
- `anthropic-version: 2023-06-01`
- `content-type: application/json`

Body：
```json
{
  "model": "claude-haiku-4-5",
  "max_tokens": 4096,
  "stream": true,
  "system": "你是一个专业翻译引擎。只输出译文，不要解释。",
  "messages": [
    { "role": "user", "content": "请将以下文本翻译为{target_lang}：\n\n{source_text}" }
  ]
}
```

`enable_thinking=true` 时追加 `thinking: { "type": "adaptive" }`。注意：`adaptive` 仅 Opus 4.6+/Sonnet 4.6+/Sonnet 5/Fable 5 等支持；**Haiku 4.5 不支持 adaptive，传入会返回 400**。因此 `enable_thinking` 默认 `false`，UI 标注"仅对支持的模型生效，Haiku 等模型需关闭"。实现时不在代码层按模型名硬过滤——保留配置透传，错误由服务端返回并经第 7 节映射为 `LlmError::Api`。

### 4.2 流式响应解析

Claude SSE 按 `\n\n` 分隔事件，每事件形如：
```
event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"..."}}
```

`ClaudeProvider::consume_sse_event` 处理规则：

| 事件类型 | 处理 |
|---|---|
| `message_start` | 忽略 |
| `content_block_start` | 忽略 |
| `content_block_delta`（`delta.type == "text_delta"`） | 提取 `delta.text`，非空则 `on_delta(text)` |
| `content_block_stop` | 忽略 |
| `message_delta` | 忽略（含 `stop_reason`，本版不消费 usage） |
| `message_stop` | 返回结束信号，流终止 |
| `ping` | 忽略 |
| `error` | 返回 `LlmError::Api` |

### 4.3 取消

复用与 OpenAI provider 相同的模式：
```rust
loop {
    tokio::select! {
        _ = cancel.cancelled() => return Ok(()),
        bytes = stream.next() => { /* 追加 buffer，按 \n\n 切事件 */ }
    }
}
```

## 5. 配置模型

### 5.1 ClaudeConfig（运行时）

```rust
#[derive(Debug, Clone)]
pub struct ClaudeConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub timeout_seconds: u64,
    pub enable_thinking: bool,
}
```

### 5.2 ClaudeAppConfig（持久化）

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeAppConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub timeout_seconds: u64,
    pub enable_thinking: bool,
}
```

默认值：
- `base_url`: `https://api.anthropic.com`
- `model`: `claude-haiku-4-5`
- `timeout_seconds`: `60`
- `enable_thinking`: `false`

### 5.3 AppConfig 扩展

```rust
pub struct AppConfig {
    pub provider: String,                 // "openai-compatible" | "claude" | "mock"
    pub target_lang: String,
    pub openai_compatible: OpenAiCompatibleAppConfig,
    pub claude: ClaudeAppConfig,          // 新增
}
```

`AppConfig::from_env` 增 `claude: ClaudeAppConfig::from_env()`；`normalized` 增 `self.claude = self.claude.normalized()`。

### 5.4 环境变量

| 变量 | 用途 | 默认 |
|---|---|---|
| `SHIZI_CLAUDE_API_KEY` | API Key | 无 |
| `SHIZI_CLAUDE_BASE_URL` | 网关地址 | `https://api.anthropic.com` |
| `SHIZI_CLAUDE_MODEL` | 模型 ID | `claude-haiku-4-5` |
| `SHIZI_CLAUDE_TIMEOUT_SECS` | 超时秒数 | `60` |
| `SHIZI_CLAUDE_ENABLE_THINKING` | `true`/`false` | `false` |

`From<ClaudeAppConfig> for ClaudeConfig` 与 OpenAI 的 `From` 转换同构。

## 6. Provider 工厂

[web_popup.rs:73](../../../src-tauri/src/ui/web_popup.rs#L73) 的 `match config.provider.as_str()`：

```rust
let provider: Arc<dyn LlmProvider> = match config.provider.as_str() {
    "mock" => Arc::new(MockLlmProvider),
    "claude" => Arc::new(ClaudeProvider::new(ClaudeConfig::from(config.claude))),
    _ => Arc::new(OpenAiCompatibleProvider::new(
        OpenAiCompatibleConfig::from(config.openai_compatible),
    )),
};
```

`DEFAULT_PROVIDER` 保持 `"openai-compatible"`（不改变默认后端，避免影响既有用户）。

## 7. 错误处理

HTTP 状态码 → `LlmError` 映射（与 OpenAI provider 一致）：

| 状态 | 映射 | retryable |
|---|---|---|
| 429 / 5xx | `LlmError::Http(message)` | true |
| 401 / 403 / 400 | `LlmError::Api { message, retryable: false }` | false |
| 流式 JSON 解析失败 | `LlmError::Parse(error)` | false |
| 流内 `error` 事件 | `LlmError::Api { message, retryable: false }` | false |
| `stop_reason: "refusal"` | `LlmError::Api { message: "翻译被拒绝", retryable: false }` | false |

错误响应体解析：Claude 错误格式 `{"type":"error","error":{"type":"...","message":"..."}}`，新增对应反序列化结构。

## 8. 前端集成

### 8.1 设置面板

`frontend/index.html` 设置面板增加：
- provider 下拉：openai-compatible / claude / mock
- provider 切换时显示对应表单（openai-compatible 表单已存在；新增 claude 表单）
- Claude 表单字段：API Key、Base URL、Model、Timeout、Enable Thinking（checkbox，标注"仅对支持的模型生效"）

### 8.2 交互

`frontend/main.js`：
- 复用现有 `get_app_config` / `save_app_config`，扩展读写 `claude` 字段
- provider 下拉 `change` 事件切换表单可见性
- 核心逻辑不进前端，仅表单与配置读写

## 9. 测试策略

### 9.1 单元测试（Rust）

- `ClaudeProvider::consume_sse_event`：
  - `content_block_delta` 提取 text
  - `message_stop` 返回结束
  - `ping` 忽略
  - `error` 事件 → `LlmError::Api`
  - 多事件混合
- `ClaudeAppConfig::from_env` / `normalized`：默认值、空串处理、env 覆盖
- `From<ClaudeAppConfig> for ClaudeConfig`：字段映射

### 9.2 验证命令

```bash
cd src-tauri && cargo test
cd src-tauri && cargo build
node --check frontend/main.js
```

### 9.3 人工验证（Tauri dev）

- 设置面板选 claude，填有效 API Key，翻译成功并流式渲染
- 取消按钮中断 Claude 流式响应（复用取消/重试链路）
- 错误 API Key → `Failed` 事件正确显示
- 切回 openai-compatible / mock 不回归

## 10. 不向后兼容性

`AppConfig` 增字段，旧 `config.json` 缺 `claude` 字段时 serde 反序列化失败。处理方式：给 `ClaudeAppConfig` 字段加 `#[serde(default)]` 或对整个 `claude` 字段加 `#[serde(default)]`，确保旧配置文件可平滑升级。需在实现时验证。

## 11. 文档同步

- `CLAUDE.md` 与 `AGENTS.md`：前后端通信、provider 列表、配置说明同步
- `plugins.md`：本特性未新增插件/技能，无需同步
