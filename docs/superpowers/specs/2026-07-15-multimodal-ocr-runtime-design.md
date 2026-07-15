# 多模态 OCR 运行时接通

- 日期：2026-07-15
- 状态：已实现
- 前置：`2026-07-15-services-drag-detail-ocr-design.md` 已完成 OCR **配置 UI + 持久化**；本规格接通 **截图运行时** 按配置调用引擎

## 1. 背景与目标

设置页「服务 → 文字识别」已可管理 `ocrServices[]`（默认 Windows 媒体 OCR + 可添加多模态视觉渠道），但截图链路仍 **硬编码** `Windows.Media.Ocr`，与配置无关。

本轮目标：把 **唯一启用** 的 OCR 实例接到 `Alt+S` 截图识别路径，使 OpenAI 兼容视觉模型能完成「只识别出文字」，再复用现有翻译批次。

**Done means**

1. 启用某一 OpenAI 兼容视觉 OCR 后，截图框选识别走该模型，输出纯文本进入现有翻译链路。
2. 启用 Windows 时行为与今日一致（系统 OCR）。
3. OCR 开关互斥且不允许全关；视觉失败 **不** 回退 Windows。
4. Claude 视觉本轮不可启用；脏配置运行时报明确错误。
5. 翻译多服务结果卡、协议与 `services[]` 零回归。

## 2. 范围

### 2.1 范围内

| 项 | 内容 |
|---|---|
| 引擎选择 | 从 `config.ocr_services` 解析唯一 `enabled` 实例并构造 `OcrEngine` |
| Windows | 继续 `WindowsOcrEngine`，仅当该实例 enabled |
| 视觉 OCR | 独立 `VisionOcrEngine`：OpenAI Chat Completions 多模态、**非流式** |
| 编排 | crop → recognize → `TranslationInput::OcrText` → 现有翻译入口 |
| 前端互斥 | 启用互斥、禁止全关、Claude 渠道不可启用 |
| 配置归一 | 后端 `normalized()` 保证运行时至多一个 enabled，空列表 seed Windows |
| 错误 | 扩展/映射 OCR 错误，复用弹窗 OCR 错误展示 |
| 测试 | 工厂选择、互斥、请求体形状、图像编码、错误映射、Windows 回归 |

### 2.2 范围外（YAGNI）

- Claude Messages 图像块 / 其它专用视觉协议
- 专用云 OCR（百度/腾讯/Azure 等）
- 多 OCR 并行、多识别结果卡
- 视觉「识别+翻译」合一
- OCR 结果 usage 展示、流式识别进度
- 修改 DXGI / overlay 框选交互
- 本地 PaddleOCR、语言包安装 UI

## 3. 已锁定产品决策

| # | 决策 |
|---|---|
| 1 | 架构：独立 `VisionOcrEngine` + `resolve_ocr_engine` 工厂（方案 A） |
| 2 | 同时 **只能启用一个** OCR 实例（开关互斥） |
| 3 | Windows 与视觉 **平等**；视觉失败 **不** 静默回退 Windows |
| 4 | **不允许全部关闭**；关唯一启用项时自动启用 Windows（列表无 Windows 则拒绝关闭） |
| 5 | 视觉职责：**只识别文字** → 再走已启用翻译服务 |
| 6 | 本轮协议：**仅 OpenAI 兼容视觉**（`openai_chat` 形态） |
| 7 | Claude 视觉：可展示配置，**不可启用**；运行时若仍 enabled → 明确错误 |
| 8 | Windows 允许关闭（撤销「永不可关」运行时语义；互斥 + 禁全关替代兜底） |

## 4. 架构

```
Alt+S
  → DXGI 抓帧 + overlay 框选
  → crop（物理像素）
  → resolve_ocr_engine(app_config.ocr_services)
       ├─ windows-media-ocr     → WindowsOcrEngine
       ├─ openai 兼容视觉 type  → VisionOcrEngine { endpoint, key, model, prompt }
       └─ claude-vision 等      → Err(UnsupportedOcrProtocol)
  → engine.recognize(cropped, hints)
  → TranslationInput::OcrText { text, image_id: None }
  → 现有 start_translation / 多服务批次（不变）
```

分层原则：

- 截图 / OCR / 翻译编排仍在 Rust 核心层。
- UI 只配置与展示错误，不发起视觉 HTTP。
- 翻译 `TranslationProvider` **不感知** 输入来自 Windows 还是视觉 OCR。
- **不** 把图像塞进现有流式翻译 provider（避免污染 session/多卡/stream/usage）。

### 4.1 模块落点（建议）

| 位置 | 职责 |
|---|---|
| `core/ocr/mod.rs` | trait、`OcrError` 扩展、公共类型 |
| `core/ocr/resolve.rs`（或 `mod` 内） | `resolve_ocr_engine` / 从 `OcrServiceInstanceConfig` 映射 |
| `core/ocr/vision_openai.rs` | `VisionOcrEngine`：编码图、组包、HTTP、解析文本 |
| `platform/windows/ocr.rs` | 现有 Windows 实现不变 |
| `ui/ocr_popup.rs`（及 platform workflow） | 读 config → resolve → `recognize_cropped_for_translation` |
| `core/config/types.rs` | `normalized()` OCR 互斥与 seed |
| `frontend/.../settings` store + panel | 开关互斥、禁全关、Claude 不可启用 |
| i18n | 新错误/状态文案 |

## 5. 引擎选择与配置归一

### 5.1 运行时 resolve 规则

1. 过滤 `ocr_services` 中 `enabled == true`。
2. **0 个**：`OcrError`（或配置层错误）「没有可用的文字识别服务」——正常路径不应出现（由归一化与 UI 保证）。
3. **1 个**：按 `service_type` 构造引擎。
4. **>1 个**（脏配置）：取列表 **第一个** enabled，并 `log::warn`；不崩溃。

### 5.2 type → 引擎映射

| `service_type` | 引擎 | 本轮 |
|---|---|---|
| `windows-media-ocr` | `WindowsOcrEngine` | ✅ |
| `openai-vision` | `VisionOcrEngine` | ✅ |
| `gemini-vision` | `VisionOcrEngine` | ✅ |
| `zhipu-vl` | `VisionOcrEngine` | ✅ |
| `siliconflow-vision` | `VisionOcrEngine` | ✅ |
| `moonshot-vision` | `VisionOcrEngine` | ✅ |
| `openai-compatible-vision` | `VisionOcrEngine` | ✅ |
| `claude-vision` | 不构造引擎 | ❌ → `UnsupportedOcrProtocol` |
| 未知 type | 同上 | ❌ |

`VisionOcrEngine` 配置字段来自实例：`endpoint`、`api_key`、`model`、`ocr_prompt`（空则默认提示词）。

### 5.3 后端 `normalized()`

在现有 `AppConfig::normalized` 中增加 OCR 规则：

1. `ocr_services` 为空 → seed 单条 Windows（`enabled: true`），与前端 seed 对齐。
2. 若 **没有任何** enabled → 强制列表中 Windows 实例 `enabled = true`；若无 Windows 行则插入默认 Windows。
3. 若 **多个** enabled → 仅保留 **第一个** enabled，其余 `enabled = false`。
4. 不删除用户已添加的视觉实例；不改 apiKey/model 等字段。

保存路径（`save_app_config`）与加载路径均走 `normalized`，保证磁盘与运行时一致。

### 5.4 前端互斥（设置页）

- `toggleOcrEnabled(id, true)`：目标 `enabled=true`，**其余全部** `enabled=false`。
- `toggleOcrEnabled(id, false)`：
  - 若目标不是当前唯一 enabled → 仅关目标；
  - 若目标是唯一 enabled → **自动启用 Windows**（`windows-media-ocr` 类型那条）；若列表中无 Windows → **拒绝关闭**并 toast。
- Claude 视觉（`protocolId === 'claude_messages'` 或 type `claude-vision`）：开关 **disabled**，旁注本版本不支持；无法点启用。
- 删除视觉实例：若删的是当前唯一 enabled，删除后启用 Windows（与关开关同策略）。
- 去掉/改写「仅配置预留、运行时固定 Windows」类文案，改为说明「当前启用的一项用于截图识别」。

## 6. VisionOcrEngine（OpenAI 兼容）

### 6.1 图像处理

1. 输入 `CapturedImage`：`Bgra8` / `Rgba8`（与现截图 crop 一致）；`Png` 若出现可直接用或统一再编码策略在实现时二选一并测。
2. 可选缩放：最长边超过 **2048** 时等比缩小，降低 token。
3. 编码为 **PNG** 字节，再 Base64。
4. `data_url = "data:image/png;base64," + base64`。
5. **禁止** 将完整 base64 写入 info 级日志；debug 最多记宽高与字节长度。

### 6.2 HTTP 请求

- 方法：`POST`
- URL：与现有 OpenAI 兼容翻译侧 **同一套** base_url 规范化逻辑（去尾 `/`，拼 `chat/completions`）。
- Header：`Authorization: Bearer {api_key}`，`Content-Type: application/json`。
- Body（概念形状，字段 snake_case）：

```json
{
  "model": "<instance.model>",
  "stream": false,
  "max_tokens": 2048,
  "messages": [
    {
      "role": "system",
      "content": "<ocr_prompt or DEFAULT_OCR_PROMPT>"
    },
    {
      "role": "user",
      "content": [
        { "type": "text", "text": "请识别图中全部文字。" },
        {
          "type": "image_url",
          "image_url": { "url": "data:image/png;base64,..." }
        }
      ]
    }
  ]
}
```

- 超时：独立常量（建议 60s），不绑翻译流式长连接语义。
- **非流式**：一次读完整 JSON，不走 SSE。

### 6.3 默认提示词

与前端 `DEFAULT_OCR_PROMPT` 对齐（实现时以 tokens 单源为准，后端可常量化同一文案）：

- 要求：只输出图中文字；保持原有换行与阅读顺序；不要解释、不要 markdown 代码围栏、不要前后缀。

用户自定义 `ocrPrompt` 非空时完全替换 system content。

### 6.4 响应解析

1. HTTP 非 2xx：解析 error.message（若可），否则截断 body；映射为可展示 OCR 错误。
2. 2xx：取 `choices[0].message.content`：
   - string → 使用；
   - 若部分厂商返回 content 数组，拼接其中 `type==text` 的 text（兼容性最小实现）。
3. `trim` 后空 → `OcrError::EmptyResult`。
4. 成功 → `OcrResult { text, lines: vec![], engine: "<service_type 或 vision-openai-compatible>" }`。
5. 本轮不填 word/line 几何信息。

### 6.5 与翻译 OpenAI provider 的关系

- **可复用**：HTTP client 构建、base_url 规范化、错误 body 解析辅助（若已有纯函数可抽）。
- **不可复用**：流式 SSE、`TranslationRequest`、`ChatMessage { content: String }` 现结构（需 vision 专用请求 DTO，避免破坏翻译序列化）。

## 7. 编排数据流

`recognize_cropped_for_translation` 保持：

```text
frame + region + &dyn OcrEngine + hints → Option<TranslationInput>
```

调用方变更：

1. 读取 `AppState` / `ConfigStore` 当前 `AppConfig`。
2. `resolve_ocr_engine(&config.ocr_services)?` 得到具体引擎（或 `Box<dyn OcrEngine>`）。
3. 调用 `recognize_cropped_for_translation(..., engine, hints)`。
4. 成功后进入现有 OCR→翻译入口（弹窗、source badge「来自 OCR」、历史 sourceType 等不变）。

取消语义、capture 锁、overlay 四命令 **不变**。

## 8. 错误处理

### 8.1 `OcrError` 扩展（建议）

在现有变体基础上增加（命名以实现为准）：

| 变体 | 场景 |
|---|---|
| `NoEngineConfigured` | 无可用 enabled（兜底） |
| `UnsupportedProtocol(String)` | claude-vision / 未知 type |
| `Auth(String)` | 缺 Key、401/403 |
| `Api { message, retryable }` | 厂商业务错误 |
| `Http(String)` | 网络/超时/5xx |
| 现有 | `EmptyResult` / `ImageTooLarge` / `LanguageUnavailable` / … |

展示层将新变体映射为带阶段前缀的用户文案（对齐 `ocr-error-display` 既有模式），**视觉失败不触发 Windows 重试**。

### 8.2 前端

- 复用 OCR 错误卡片/状态栏路径；必要时增加 message key。
- 不因视觉 OCR 新增翻译结果卡。
- 缺 Key：引导「设置 → 文字识别」填写。

## 9. 配置与兼容

- `ocrServices` / `ocr_services` 字段形状 **不强制** 变更；本轮是行为接通。
- 旧配置：仅 Windows 或视觉全关 → `normalized` 保证 Windows enabled。
- 旧配置：多视觉同时 enabled（若用户手改 json）→ 只留第一个。
- 文档：README「配置预留、运行时固定 Windows」改为「按当前启用的 OCR 实例识别；支持 Windows 与 OpenAI 兼容视觉」。

## 10. 测试与验收

### 10.1 自动化

| 测试 | 断言 |
|---|---|
| resolve：仅 Windows enabled | 得到 Windows 引擎标识 |
| resolve：仅 openai-vision enabled | 得到 Vision 配置（endpoint/model） |
| resolve：claude enabled | `UnsupportedProtocol` |
| resolve：多个 enabled | 取第一个 + 不 panic |
| normalized：全关 | 强制 Windows on |
| normalized：双开 | 只留第一个 |
| Vision 请求体 | `stream==false`；content 含 image_url data URL；system=prompt |
| 图像 | BGRA 小图 → PNG 可解码；超长边缩放 |
| 响应 | 正常 content → text；空 content → EmptyResult；401 → Auth |
| 前端 store | 互斥；关唯一 → Windows on；Claude 不能 enable |
| 回归 | `recognize_cropped_for_translation` + Windows 单测仍过 |

### 10.2 手工（WebView2）

1. 默认：仅 Windows → Alt+S 行为与现网一致。
2. 添加 OpenAI 兼容视觉，填 Key/模型，启用 → 截图识别出字并翻译。
3. 关视觉 → 自动回 Windows 且可识别。
4. 尝试启用 Claude 视觉 → UI 阻止。
5. 错误 Key → 可读错误，不静默变 Windows。
6. 翻译多服务批次与设置保存无回归。

## 11. 实现顺序（供 plan 拆任务）

1. 后端 `OcrError` 扩展 + `normalized` OCR 规则 + 单测  
2. `resolve_ocr_engine` + 单测  
3. 图像 PNG/缩放工具 + 单测  
4. `VisionOcrEngine` 请求/解析 + 单测（mock HTTP）  
5. `ocr_popup` / workflow 接 resolve  
6. 前端互斥 / 禁 Claude 启用 / 文案  
7. i18n、README/架构文档回填  
8. `cargo test` / `npm run test` / `typecheck` + 手工点验清单  

## 12. 风险与边界

| 风险 | 处理 |
|---|---|
| 厂商 vision 请求体微差 | 以 OpenAI 官方形状为准；失败透传 message |
| 大图 token/耗时 | 最长边 2048；超时 60s；错误可操作 |
| 用户以为多开 OCR | UI 互斥 + 详情说明「仅当前启用项生效」 |
| Claude 配置残留 enabled | 归一化不自动改 type；resolve 报 Unsupported；UI 禁启用 |
| 把 vision 塞进翻译 provider | **禁止**；保持独立引擎 |

## 13. 规格自检

- [x] 无阻塞性 TODO/待定（Claude 明确下轮；缩放阈值写死 2048）
- [x] 与前置 OCR 配置 spec 衔接：本轮只接通运行时
- [x] 互斥 / 禁全关 / 不回退 / 仅 OpenAI 兼容 四处一致
- [x] 范围可单 plan 覆盖，未混入专用 OCR 或多引擎并行
- [x] 翻译链路边界清晰：只消费 `OcrText` 字符串

## 14. 参考

- 前置 spec：`docs/superpowers/specs/2026-07-15-services-drag-detail-ocr-design.md`
- 架构：`docs/architecture/screenshot-ocr-architecture.md`
- 现状：`src-tauri/src/core/ocr/mod.rs`、`ocr_translation.rs`、`platform/windows/ocr.rs`、`ui/ocr_popup.rs`
- 前端配置：`frontend/src/settings/types.ts`、`tokens.ts`、settings store / ServicesPanel
- 翻译 OpenAI：`src-tauri/src/core/llm/openai_compatible.rs`（仅复用规范化/错误解析思路，不扩展为 OCR）
