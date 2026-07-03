# 服务协议抽象与多结果翻译设计

- 日期：2026-07-03
- 状态：已确认，待实现
- 策略：服务实例默认展示 DeepSeek / 智谱 AI 但关闭；当前新增渠道统一走 OpenAI Chat；保留协议适配器抽象

## 1. 背景与目标

当前设置页已经有服务实例列表、启用开关和拖拽排序，但保存到后端时仍投影成单 provider 配置，翻译弹窗也只渲染一个结果框。用户希望服务列表里的开关直接决定翻译弹窗下方的结果框数量，并且结果框顺序严格等于服务列表顺序。

同时，服务商可能支持多种协议。本版不一次性实现所有协议，只把未接入渠道先按 OpenAI Chat 协议接通；已经接好的 provider 保持现状。为后续 DeepL 原生协议、百度签名协议、腾讯云 TC3、Claude Messages 等扩展留下清晰的适配器边界。

目标：

1. 首次初始化服务列表只默认展示 DeepSeek 和智谱 AI 两个实例，默认关闭，不自动参与翻译。
2. 所有服务编辑页都展示端点 URL；当前协议端点可编辑，官方原生端点可只读展示。
3. 启用的服务实例按列表顺序触发翻译，弹窗按同样顺序渲染多个结果框。
4. 后端引入协议适配器抽象，但本阶段只新增/复用必要代码，不为未实现协议写空 provider。

## 2. 范围

### 范围内

- 前端设置状态新增 `protocol` 概念，服务实例可选择/保存协议，当前默认 `openai_chat`。
- 服务元数据补端点信息：OpenAI Chat 默认端点、官方原生端点、端点是否可编辑。
- 初始化服务实例从 OpenAI + Claude 改为 DeepSeek + 智谱 AI，且 `enabled=false`。
- 后端 `AppConfig` 支持持久化多服务实例，保留旧单 provider 字段用于兼容迁移。
- 后端翻译入口按启用实例顺序调度多个翻译任务，并给事件附带服务实例信息。
- 翻译弹窗按 `serviceInstanceId` 渲染多个结果框，单卡片内仍复用现有流式、失败、取消、重试 UI。

### 范围外（YAGNI）

- 不实现 DeepL / 百度 / 有道 / 腾讯 / 火山机器翻译的原生签名协议。
- 不做复杂协议 UI（只需要当前协议选择和端点可见）。
- 不做服务级自动 fallback、竞速翻译、结果合并或质量打分。
- 不新增数据库或额外持久化层，继续使用现有 Tauri config JSON。
- 不改 overlay / OCR 识别流程；OCR 只复用新的翻译批次入口。

## 3. 设计决策

| 决策 | 选择 | 理由 |
|---|---|---|
| 默认列表 | DeepSeek、智谱 AI，各一条实例，默认关闭 | 用户明确要求“默认展示”，不是默认服务，也不是默认启用 |
| 当前新增协议 | `openai_chat` | 覆盖 DeepSeek、智谱、Kimi、硅基流动、Gemini、讯飞星火等兼容入口，改动最小 |
| 已接入能力 | Claude native provider 保持现状 | 符合“已经对接好的保持现状”，不回退现有功能 |
| 抽象层级 | `ServiceInstance` + `ProtocolAdapter` + `TranslationBatch` | 足够承载后续多协议，不提前实现空 provider |
| 展示顺序 | `services[]` 数组顺序 | 复用现有拖拽重排能力，少一套排序字段 |
| 开关规则 | 未配置必填项时禁止开启或保存时提示 | 不因“没有专属 provider”置灰；本版协议能力由 adapter 决定 |

## 4. 服务与协议模型

### 4.1 前端设置模型

`ServiceInstance` 增加：

```ts
protocol: ServiceProtocolId
```

`ServiceProtocolId` 第一版：

```ts
type ServiceProtocolId = 'openai_chat' | 'claude_messages'
```

- `openai_chat`：走 OpenAI Chat Completions 兼容协议。
- `claude_messages`：保留现有 Claude provider，不作为新增渠道默认值。

`ServiceMeta` 增加：

```ts
protocols: ServiceProtocolMeta[]
```

`ServiceProtocolMeta`：

```ts
type ServiceProtocolMeta = {
  id: ServiceProtocolId
  label: string
  defaultEndpoint: string
  defaultModel: string
  editableEndpoint: boolean
  status: 'available' | 'planned'
}
```

服务编辑页按当前协议显示 endpoint 输入框。若该服务有计划中的原生协议，则额外展示只读“官方原生端点”，用于用户判断服务商真实入口，但本版不会调用。

### 4.2 后端配置模型

`AppConfig` 新增：

```rust
pub services: Vec<ServiceInstanceConfig>
```

`ServiceInstanceConfig`：

```rust
pub struct ServiceInstanceConfig {
    pub id: String,
    pub service_type: String,
    pub name: String,
    pub enabled: bool,
    pub protocol: String,
    pub api_key: Option<String>,
    pub endpoint: String,
    pub model: String,
    pub timeout_seconds: u64,
}
```

旧字段 `provider/openaiCompatible/claude` 暂时保留：

- 读取旧配置时，如果 `services` 缺失，按旧 provider 生成一条兼容实例。
- 保存新配置时同时写 `services`；旧字段可继续写默认投影，避免老代码路径崩溃。

## 5. 官方端点梳理

当前协议端点是 `openai_chat` 的 base URL，后端 adapter 会拼 `/chat/completions`。官方文档直接给完整 `/chat/completions` 的，配置里保存其 base URL。

| 服务 | OpenAI Chat base URL | 默认模型建议 | 来源 |
|---|---|---|---|
| OpenAI | `https://api.openai.com/v1` | `gpt-4o-mini` | [OpenAI Chat API](https://platform.openai.com/docs/api-reference/chat/create) |
| DeepSeek | `https://api.deepseek.com` | `deepseek-chat` | [DeepSeek API Docs](https://api-docs.deepseek.com/) |
| 智谱 AI | `https://open.bigmodel.cn/api/paas/v4` | `glm-4-flash` | [智谱快速开始](https://docs.bigmodel.cn/cn/api/introduction) |
| Kimi / Moonshot | `https://api.moonshot.cn/v1` | `moonshot-v1-8k` | [Kimi Chat API](https://platform.kimi.com/docs/api/chat) |
| 硅基流动 | `https://api.siliconflow.cn/v1` | `Qwen/Qwen2.5-7B-Instruct` | [SiliconFlow OpenAI Chat](https://docs.siliconflow.cn/cn/api-reference/chat-completions/chat-completions) |
| Gemini | `https://generativelanguage.googleapis.com/v1beta/openai/` | `gemini-1.5-flash` | [Gemini OpenAI compatibility](https://ai.google.dev/gemini-api/docs/openai) |
| 讯飞星火 | `https://spark-api-open.xf-yun.com/v1` | `generalv3.5` | [讯飞星火 HTTP 文档](https://www.xfyun.cn/doc/spark/HTTP%E8%B0%83%E7%94%A8%E6%96%87%E6%A1%A3.html) |

以下服务本版不直接走原生协议，但编辑页要展示官方原生端点（只读或标注“待接入”）：

| 服务 | 官方原生端点 | 后续协议 |
|---|---|---|
| DeepL | `https://api.deepl.com/v2/translate` / `https://api-free.deepl.com/v2/translate` | `deepl_translate` |
| 百度翻译 | `https://fanyi-api.baidu.com/api/trans/vip/translate` | `baidu_translate` |
| 有道翻译 | `https://openapi.youdao.com/api` | `youdao_translate` |
| 腾讯翻译君 | `https://tmt.tencentcloudapi.com` | `tencent_tmt` |
| 火山翻译 | 火山开放 API 网关 + 机器翻译 Action | `volcengine_translate` |

## 6. 后端架构

### 6.1 ProtocolAdapter

新增轻量 adapter 工厂，不新增复杂 trait 层级：

```rust
fn provider_for_service(
    service: &ServiceInstanceConfig,
) -> Result<Arc<dyn LlmProvider>, String>
```

第一版分支：

- `openai_chat`：构造 `OpenAiCompatibleProvider`，字段来自服务实例。
- `claude_messages`：构造 `ClaudeProvider`，字段来自服务实例。
- 其他协议：返回“当前协议暂未接入”错误，前端应避免开启。

后续新增原生协议时，只扩展这个工厂和对应 provider，不改服务列表与弹窗事件结构。

### 6.2 TranslationBatch

`start_translation_from_input` 改为批次编排：

1. 读取 config。
2. 过滤 `enabled` 服务，保持原数组顺序。
3. 对每个启用服务创建子 session id：`{batch_id}:{service_id}`。
4. 每个服务 emit `Started`，并发执行翻译任务。
5. 任一服务失败只更新自己的结果卡，不影响其他服务。
6. 所有服务结束后释放全局翻译锁。

若没有启用服务，返回“请先在服务列表启用至少一个已配置服务”。

### 6.3 取消与重试

- 取消：一个批次共享父 `CancellationToken`，取消会中断全部服务。
- 重试：保存最后一次 `TranslationInput`，按当前启用服务列表重新创建批次。
- 不做单卡片重试；以后有需求再加服务级 command。

## 7. 翻译事件

`TranslationEvent` 每个变体增加服务信息：

```rust
service_instance_id: String,
service_name: String,
service_type: String,
protocol: String,
```

`Started` 仍携带 `sourceText/sourceType`，前端只用第一条 `Started` 回填原文。结果卡按 `serviceInstanceId` 创建/更新。

事件示例：

```json
{
  "type": "delta",
  "sessionId": "batch-1:svc-1",
  "serviceInstanceId": "svc-1",
  "serviceName": "DeepSeek",
  "serviceType": "deepseek",
  "protocol": "openai_chat",
  "text": "你好"
}
```

## 8. 设置页交互

### 8.1 服务列表

- 初始服务列表：DeepSeek、智谱 AI。
- 两个实例默认 `enabled=false`。
- 左侧开关点击时校验当前协议必填字段：
  - 需要 Key 且 `apiKey` 为空：不允许打开，提示“请先填写 API Key”。
  - `endpoint` 为空：不允许打开。
  - `model` 为空：不允许打开。
  - `protocol` 不可用：不允许打开。

### 8.2 服务编辑页

- Endpoint 对所有服务始终可见。
- 当前协议 endpoint 使用输入框展示，内置默认值作为初始值。
- 官方原生 endpoint 使用只读文本展示，状态标为“待接入”。
- `custom` 服务默认 `openai_chat`，endpoint 为空，用户必须填写。

### 8.3 添加服务

添加服务时按 `ServiceMeta.protocols` 选第一个 `available` 协议作为默认协议。没有可用协议的服务仍可添加，但默认关闭，并在详情页标注“当前协议待接入”。

## 9. 翻译弹窗

弹窗把单个 `resultCard` 改为结果容器：

```html
<section id="resultsList"></section>
```

每个结果卡状态：

```js
{
  serviceInstanceId,
  serviceName,
  serviceType,
  protocol,
  text,
  usage,
  status: 'idle' | 'loading' | 'finished' | 'failed' | 'cancelled'
}
```

渲染规则：

- `Started`：创建卡片，状态 loading，清空文本。
- `Delta`：按 `serviceInstanceId` 追加文本。
- `Finished`：写 fullText、usage、状态 finished。
- `Failed`：只把对应卡片标红，不影响其他卡片。
- `Cancelled`：对应卡片状态 cancelled。
- 卡片顺序：首次 `Started` 到达时按后端启用服务顺序批量初始化，避免并发返回顺序影响排版。

## 10. 错误处理

| 场景 | 处理 |
|---|---|
| 无启用服务 | `start_translation` 返回错误，弹窗 toast |
| 服务缺 Key | 设置页禁止开启；后端仍做二次校验，返回该服务 Failed |
| Endpoint 非法 | 保存前校验 URL；运行时 HTTP 错误显示在对应卡片 |
| 单服务失败 | 只影响该服务卡片 |
| 全部失败 | 状态栏显示“翻译失败”，允许重试 |
| 部分成功 | 状态栏显示“部分完成”，成功卡可复制 |

## 11. 测试策略

### Rust 单元测试

- `AppConfig` 反序列化：旧配置缺 `services` 可升级。
- `ServiceInstanceConfig::normalized`：空 endpoint/model/key 处理。
- adapter 工厂：`openai_chat` / `claude_messages` / 未知协议。
- 翻译事件序列化：服务字段 camelCase。
- 批次编排：启用服务按数组顺序生成事件；单服务失败不阻断其他服务；取消发出 cancelled。

### 前端测试

- `projectToAppConfig` 或新投影函数：保留多服务数组，不再只取默认服务。
- 服务开关：缺 Key / 缺 endpoint / 缺 model 时禁止打开。
- 默认设置：首次初始化只有 DeepSeek 和智谱 AI，且都关闭。

### 验证命令

```bash
npm run test
npm run typecheck
cd src-tauri && cargo test
cd src-tauri && cargo build
```

手动验证：

1. 首次打开设置页，服务列表只显示 DeepSeek / 智谱 AI，开关关闭。
2. 填写 DeepSeek Key 并开启，翻译弹窗出现一个 DeepSeek 结果框。
3. 再开启智谱 AI，弹窗出现两个结果框，顺序为 DeepSeek → 智谱 AI。
4. 拖拽调整服务顺序后，结果框顺序随之变化。
5. 一个服务 Key 错误时，只对应卡片失败，其他服务继续输出。
6. 取消按钮能取消当前批次全部服务。

## 12. 文档同步（收尾硬门禁）

实现完成后同步：

- README：服务列表、默认展示、OpenAI Chat 协议、多结果弹窗。
- docs/roadmap/progressive-development-plan.md：标记多服务翻译能力进度。
- AGENTS.md / CLAUDE.md：配置模型、翻译弹窗事件和多服务调度说明同步。

## 13. 风险

- **并发顺序不稳定**：卡片不能按事件到达顺序插入，必须用启用服务列表预初始化。
- **OpenAI Chat 兼容差异**：部分服务不支持 `stream_options.include_usage`。实现时 adapter 需要允许按服务关闭 usage 参数，避免兼容服务 400。
- **旧配置迁移**：用户已有 OpenAI/Claude 配置不能丢。读取旧字段生成服务实例，保存后写新 `services`。
- **抽象过度**：本版只建 adapter 工厂和协议元数据，不写未使用 trait 方法或空 provider。

## 14. 规格自检

- 无 TODO / 待定占位。
- 明确了“默认展示但关闭”，没有把 DeepSeek / 智谱 AI 设为默认服务。
- 明确当前新增渠道统一 `openai_chat`，同时保留 `protocol` 扩展点。
- 明确 endpoint 对所有服务可见，并区分当前协议 endpoint 与只读原生 endpoint。
- 范围可由一个实现计划覆盖。
