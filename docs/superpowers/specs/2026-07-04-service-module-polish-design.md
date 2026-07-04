# 服务模块打磨：前后端配置同步与渠道开发中标记

- 日期：2026-07-04
- 状态：已完成
- 策略：config.json 作为 services 事实来源；前端设置页启动时拉取后端同步；旧格式 config.json 不迁移，services 为空时前端直接覆盖；未对接渠道标"开发中"并置灰启用

## 1. 背景与目标

上一轮（2026-07-03 服务协议抽象与多结果翻译）已建立 `services[]` 数组、批次翻译、多卡片渲染的架构，但实际运行时启用多个渠道，翻译弹窗仍只展示一个卡片。本轮诊断出 4 层根因并修复，同时补全"未对接渠道标记开发中"的需求。

根因诊断：

1. **前后端配置脱节**：前端设置页 [stores/settings.ts](../../../frontend/src/settings/stores/settings.ts) 只读写浏览器 localStorage（`app:settings:v1`），从不从后端 `config.json` 加载（`invokeGetAppConfig` 在 [lib/tauri.ts:14](../../../frontend/src/lib/tauri.ts#L14) 定义了却没有任何地方调用）；后端翻译链路 [ui/web_popup.rs](../../../src-tauri/src/ui/web_popup.rs) 只读 `config.json`；两者唯一桥梁是设置页底部的"保存"按钮。
2. **旧格式 config.json 残留**：当前 `C:\Users\xdj\AppData\Roaming\com.shizi.app\config.json` 是旧版本格式（`provider`/`openaiCompatible`/`claude` 字段，无 `services` 数组），反序列化后 `services` 为空（`#[serde(default)]`），`ConfigStore::load` 能成功解析旧格式（忽略未知字段），不走 `from_env()` 兜底。上一轮 spec 设计了迁移但未实现。
3. **协议 id 前后端不一致**：前端用 `openai_chat`/`claude_messages`（[types/config.ts:5](../../../frontend/src/types/config.ts#L5)），后端 `provider_for_service` 匹配 `openai-compatible`/`claude`/`mock`（[llm/protocol.rs:20](../../../src-tauri/src/core/llm/protocol.rs#L20)）。`openai_chat` 走默认分支碰巧工作，`claude_messages` 匹配不到 `"claude"` → Claude 被当 OpenAI 兼容发请求，必然失败。
4. **翻译弹窗图标匹配失效**：[translate.js:83](../../../frontend/public/translate.js#L83) 的 `ENGINE_META` key 是 `openai-compatible`/`claude`/`mock`（协议旧值），但 [translate.js:164](../../../frontend/public/translate.js#L164) 拿 `payload.serviceType`（渠道 id 如 `deepseek`/`zhipu`/`llm`）去匹配，永远匹配不到 → 图标永远 fallback 到 mock 的灰色 M。

目标：

1. 启用 N 个渠道，翻译弹窗出现 N 张卡片。
2. 前后端配置一致：设置页看到 = 后端实际用。
3. 协议 id 前后端统一，Claude 渠道不再误失败。
4. 翻译弹窗卡片图标按渠道区分。
5. 未对接渠道（`protocols` 为空）在添加 Dialog、服务列表、详情页明确标"开发中"，启用开关置灰。

## 2. 范围

### 范围内

- 后端 `provider_for_service` 协议 id 对齐前端（`openai_chat`/`claude_messages`/`mock`）。
- 后端 `from_env` 默认 protocol 改 `openai_chat`，`service_type` 改 `openai`。
- 前端设置页启动时调 `invokeGetAppConfig` 同步：后端 services 空 → 前端推后端覆盖；后端非空 → 按 id 合并。
- 前端 `ServicesPanel` 添加 Dialog / 服务列表 / 详情页三处"开发中"标记 + 启用置灰。
- 翻译弹窗 `ENGINE_META` 改按 `serviceType` 匹配，补渠道图标。

### 范围外（YAGNI）

- 不写旧格式 config.json 字段迁移（`openaiCompatible`/`claude` → `services`）。旧 config.json 由前端启动时 services 为空触发直接覆盖。当前 config.json 的 apiKey 都是 `null`，覆盖不丢数据。
- 不改"保存按钮"语义（仍需手动保存才持久化到后端）。
- 不实现未对接渠道的原生协议（gemini/deepl/google/baidu/youdao/tencent/volcengine/iflytek/moonshot/siliconflow 仍 `protocols: []`）。
- 不改 overlay/OCR 流程。
- 不加服务级单卡片重试。

## 3. 设计决策

| 决策 | 选择 | 理由 |
|---|---|---|
| 数据源 | config.json 是 services 事实来源 | 翻译弹窗读后端，后端为源保证弹窗 = 设置页 |
| 旧格式处理 | 不迁移，services 为空时前端直接覆盖后端 | 当前旧 config.json apiKey 为 null，覆盖不丢；避免写字段映射迁移逻辑 |
| 同步时机 | 设置页挂载时拉取 | 唯一入口，覆盖所有启动场景 |
| 合并策略 | 按 id 合并，后端核心字段覆盖前端，前端独有字段保留 | 保留用户自定义 prompts/keyStatus，核心字段以后端为准 |
| 协议 id | 统一为 `openai_chat`/`claude_messages`/`mock` | 前端已用 `openai_chat`/`claude_messages`，后端对齐 |
| 图标匹配 | 按 `serviceType`（渠道 id） | 区分 DeepSeek / 智谱 等同协议不同渠道 |
| 开发中判定 | `protocols.length === 0` | 不加新字段，从 ServiceMeta 推导 |
| 开发中行为 | 允许添加，启用置灰 | 与 OCR tab"开发中"风格一致；防止填 Key 误用 |

## 4. 后端改动

### 4.1 `llm/protocol.rs`

`provider_for_service` 的 match 改为：

- `"openai_chat"` → `OpenAiCompatibleProvider`
- `"claude_messages"` → `ClaudeProvider`
- `"mock"` → `MockLlmProvider`
- 其他 → 返回错误 `"未支持的协议：{protocol}"`

原 default 分支走 `OpenAiCompatibleProvider` 改为返回错误，避免静默误匹配（旧 `openai-compatible` 字面量不再被识别，倒推前端覆盖后 config.json 不会有此值）。

### 4.2 `config/types.rs`

- `DEFAULT_PROTOCOL` 改 `"openai_chat"`。
- `from_env()` 默认服务 `protocol` 用 `"openai_chat"`，`service_type` 用 `"openai"`（原 `"llm"`，便于弹窗按渠道 id 匹配图标）。
- `normalized()` 里 endpoint 默认分支的 `"claude"` 判断改为 `"claude_messages"`。
- `from_env` 的 `match protocol.as_str()` 分支 `"claude"`/`"mock"` 改为 `"claude_messages"`/`"mock"`（环境变量 `SHIZI_LLM_PROVIDER` 对齐）。

### 4.3 `config/store.rs` / `ui/config.rs`

不动。`get_app_config` 已存在，`save_app_config` 已存在。旧格式 config.json 反序列化后 services 空，由前端启动时覆盖，后端不写迁移逻辑。

### 4.4 `translation/batch.rs`

不动。filter enabled 逻辑正确。

## 5. 前端改动

### 5.1 `stores/settings.ts`

新增 `syncFromBackend()`：

1. 调 `invokeGetAppConfig()`。失败（Tauri 未就绪）则静默降级，用 localStorage，不阻塞设置页。
2. 若 `config.services` 为空 → `projectToAppConfig(state)` 推 `invokeSaveAppConfig` 覆盖后端。
3. 若非空 → 按 id 合并：
   - 后端字段（`enabled`/`apiKey`/`endpoint`/`model`/`protocol`）覆盖前端同 id 实例。
   - 前端独有字段（`prompts`/`keyStatus`/`chainOfThought`/`pulledModels`/`note`）保留。
   - 后端多出的实例：补进前端，独有字段用 `DEFAULT_PROMPTS` / 默认值。
   - 前端多出的实例：删除。
4. 合并完写回 localStorage，重置 baseline/dirty。

`defaultInstanceFor`：对 `protocols` 为空的服务，`protocol` 保持 fallback `'openai_chat'`（因启用置灰不会 enabled，不影响），加注释说明。

### 5.2 `SettingsPage.vue`

`onMounted` 调 `settings.syncFromBackend()`。

### 5.3 `panels/ServicesPanel.vue`

- 添加 Dialog：`svc.protocols.length === 0` 的渠道加 amber "开发中" badge（与 OCR tab 同风格）。
- 服务列表：`serviceById(inst.type)?.protocols.length === 0` 的实例，`SettingSwitch` `disabled` + tooltip "该渠道尚未对接，暂不可用"。
- 详情页顶部：`protocols.length === 0` 时加 amber 横幅 "该渠道尚未对接，暂不可用"。

判断依据是 `serviceById(inst.type)?.protocols.length === 0`，不加新字段。

### 5.4 `public/translate.js`

- `ENGINE_META` 改为按 `payload.serviceType`（渠道 id）匹配。
- 补 `openai`/`deepseek`/`zhipu`/`claude`/`mock` 五个彩色字母图标（纯内联 SVG，不引图标库，保持 translate 页纯静态）。
- 未匹配的渠道 fallback 到通用灰底图标（取 `serviceName` 首字）。
- 显示名仍用 `payload.serviceName`，保证 DeepSeek 和 智谱 AI 两张卡片名称可区分。

## 6. 数据流与边界

启动同步流程：

1. 前端从 localStorage 加载 state（含 `seedInstances` DeepSeek + 智谱 AI）。
2. `SettingsPage` `onMounted` → `syncFromBackend`。
3. 后端 services 空（旧格式 / 首次启动）→ 前端推后端覆盖 config.json 成新格式。
4. 后端非空 → 按 id 合并到 state，写回 localStorage。

保存流程（不变）：用户改配置 → 点保存 → `projectToAppConfig` → `invokeSaveAppConfig` → 后端写 config.json。

翻译流程（不变）：`start_translation` → 读 config.json → filter enabled → 每个服务发 Started 事件 → 弹窗按 `serviceInstanceId` 建卡。

边界：

- 用户加服务没保存 → 关闭/刷新后下次启动被合并逻辑删除（未保存丢失，符合"后端为源"语义）。
- `validateConfig` 失败 → `save()` return 不写后端，toast 明确指出哪个服务哪个字段。
- 旧 config.json 的 `openaiCompatible`/`claude` apiKey 为 `null`，前端覆盖不丢 Key。
- `syncFromBackend` 失败时静默降级，不阻塞设置页渲染。

## 7. 测试策略

### Rust 单测

- `provider_for_service`：`openai_chat` → `OpenAiCompatibleProvider`，`claude_messages` → `ClaudeProvider`，`mock` → `MockLlmProvider`，未知 → `Err`。
- `from_env`：默认 `protocol == "openai_chat"`，`service_type == "openai"`。
- 现有 batch/types 测试保持通过。

### 前端单测

- `syncFromBackend` 合并：后端空 → 推前端覆盖；后端非空 → 按 id 合并（后端字段覆盖、前端独有保留、后端多出补、前端多出删）。
- `protocols.length === 0` 判定 developing。

### 验证命令

```bash
npm run test
npm run typecheck
cd src-tauri && cargo test
cd src-tauri && cargo build
```

手动验证：

1. 启用 DeepSeek + 智谱 AI，点保存，翻译弹窗出现两张卡片（DeepSeek + 智谱 AI），图标不同。
2. 删除 config.json 模拟旧格式残留，启动设置页，services 为空 → 前端推后端覆盖，config.json 变新格式。
3. 添加 Dialog 里 gemini 等显示"开发中"badge；添加后启用开关置灰。
4. 启用 Claude 渠道（填 Key），翻译不再因协议误匹配失败。

## 8. 文档同步（收尾硬门禁）

实现完成后同步：

- README：配置同步机制、开发中渠道标记。
- `docs/roadmap/progressive-development-plan.md`：标记服务模块打磨进度。
- AGENTS.md / CLAUDE.md：前后端配置同步、协议 id 统一、开发中标记说明。

## 9. 风险

- **合并逻辑 id 不匹配**：前端 localStorage 实例 id 与后端 config.json id 应一致（保存时前端推后端）。若不一致会导致重复/丢失。合并逻辑需按 id 严格匹配，单测覆盖。
- **前端启动 invoke 失败**：Tauri 未就绪时 `syncFromBackend` 应静默降级（用 localStorage），不阻塞设置页。
- **`provider_for_service` 改为未知协议报错**：旧 config.json 若有 `protocol="openai-compatible"` 的服务，改后会报错。但前端覆盖后 config.json 是新 protocol，不会触发。边缘情况：用户手动改 config.json，此时该服务在翻译批次里会收到 Failed 事件。
- **图标 SVG 体积**：translate.js 补 5 个图标内联 SVG，体积可接受（每个 < 200 字节）。

## 10. 规格自检

- 无 TODO / 待定占位。
- 明确"不迁移旧格式，直接覆盖"，与用户决策一致。
- 明确协议 id 统一方向（后端对齐前端）。
- 明确开发中判定（`protocols.length === 0`）和行为（允许添加，启用置灰）。
- 明确合并逻辑（按 id，后端核心字段覆盖，前端独有保留）。
- 范围可由一个实现计划覆盖。
