# 文字识别 · 重新识别 · Vision 请求诊断日志

- 日期：2026-07-16
- 状态：已确认设计，待实现
- 前置：`2026-07-15-ocr-window-logging-quality-design.md`（独立 OCR 窗、meta、detail=high 已实现）

## 1. 背景与问题

独立「文字识别」窗口上线后，使用中仍有两类缺口：

1. **缺少重新识别**：对同一张已成功识别的图，只能重新截图/重选文件/重读剪贴板，无法「原图再跑一遍」对照引擎波动或配置变更效果。
2. **请求可观测性仍不足**：Vision 路径 debug 仅有 endpoint / model / prompt_len / system 全文与完成摘要；**看不到**完整请求 URL 形态以外的参数结构（如 `max_tokens`、`stream`、`messages` 结构、`detail`、image_url 长度）、**请求头**（含脱敏后的 Authorization）。排查「到底发了什么」仍依赖读代码。

## 2. 目标与 Done means

### 2.1 目标

| # | 能力 |
|---|---|
| 1 | 文字识别窗口对**当前预览对应的源图**提供「重新识别」 |
| 2 | `logLevel=debug` 下 Vision OCR **完整请求诊断**（头 + 参数摘要 + 响应概要） |
| 3 | 继续遵守安全红线：永不写 API Key 明文、永不写完整 base64 / data URL 正文 |

### 2.2 Done means

1. 至少一次成功识别后，工具栏「重新识别」可用；点击后对同一源图再跑 OCR，成功覆盖文本/meta/预览（预览可与源一致），失败保留上次内容并显示错误。
2. 无缓存图时按钮禁用或隐藏；loading 时禁用。
3. debug 日志中可见：完整 POST URL、请求头（Authorization 脱敏）、请求体诊断字段（model / stream / max_tokens / system 全文 / user text / detail / image_url 形态与长度）、HTTP status、耗时；响应侧可记 body 长度或 usage（若有）；识别正文仍走 `effective_redact_level`。
4. info 级仍为简要完成日志，不刷整包参数。
5. 单测锁定：脱敏后 body/头日志不含完整 data URL 与明文 key；重新识别无缓存返回明确错误。
6. **本轮不做**：识别历史持久化、一键翻译、完整 i18n、多图缓存、跨重启缓存。

## 3. 已锁定产品决策

| # | 决策 |
|---|---|
| 1 | 重新识别 = 对**当前预览对应源图**再跑一遍 OCR（非重新框选） |
| 2 | 实现路径 A：后端进程内缓存最近成功图像 + `rerecognize_last_image` command |
| 3 | 缓存仅内存，不落盘、不跨重启 |
| 4 | 成功路径（`recognize_image_full` 出口）写入缓存；截图框选 / 文件 / 剪贴板成功均写入 |
| 5 | 失败时**不覆盖**已有缓存（无图失败仍无缓存；有上次成功图时可再点重新识别） |
| 6 | 日志级别：debug = 完整请求诊断；info = 保持简要 |
| 7 | Authorization 使用现有 `redact_api_key`；image_url 记 `data:image/png;base64,[len=N]` 形态，不写内容 |
| 8 | 文案仅中文硬编码，不做完整 i18n |
| 9 | Windows Media OCR：无 HTTP 参数；可继续记引擎/耗时/尺寸 |

## 4. 架构

### 4.1 重新识别

```
成功 recognize_image_full
        │
        ▼
AppState.last_ocr_image = Some(CapturedImage)  // 源图拷贝，非 base64 预览
        │
前端「重新识别」
        │
        ▼
invoke rerecognize_last_image
        │
        ├─ None → Err("没有可重新识别的图像，请先截图/打开文件/从剪贴板识别")
        └─ Some(img) → recognize_image_full(img.clone(), ...)
                              │
                              └─ 成功再覆盖 last_ocr_image + 返回 RecognizeImageResponse
```

- **缓存粒度**：单槽最近一张；新一次成功识别覆盖。
- **预览**：仍由 `RecognizeImageResponse.previewPngBase64` 生成；重新识别成功后 UI 与首次成功一致刷新。
- **线程安全**：与现有 `AppState` 字段一致，`Arc<Mutex<Option<CapturedImage>>>`。
- **翻译路径**：`recognize_region` / 截图翻译是否写入 `last_ocr_image`？
  - **锁定：仅纯识别编排 `recognize_image_full` 成功时写入**。截图翻译不污染识别窗「重新识别」缓存；识别窗内截图成功走 `recognize_cropped_full` → `recognize_image_full`，会写入。

### 4.2 Vision 请求诊断日志

在 `vision_openai::recognize` 发送前（debug）：

1. **URL**：`POST {endpoint}` 完整字符串（已有 endpoint 拼装）。
2. **Headers**（逐条或结构化一行）：
   - `Authorization: Bearer {redact_api_key(api_key)}`
   - `Content-Type: application/json`
3. **Body 诊断**（禁止 dump 原始含 base64 的 JSON 全文到日志）：
   - 从 `build_request_body` 结果抽取字段打日志；或 `sanitize_request_body_for_log(body) -> String/Value`：
     - 保留：`model`、`stream`、`max_tokens`、`messages[0].content`（system）、`messages[1].content` 中 text 部分、`image_url.detail`
     - `image_url.url` → 替换为 `data:image/png;base64,[len={n}]`（解析 data URL 长度；非 data URL 则记 scheme + len）
   - system prompt 全文（配置内容，非 secret）在 debug 保留（与现状一致）。
4. **响应**（debug/info）：
   - 已有 status + latency + redact 文本
   - debug 可增：response body 字节长度；若 JSON 含 `usage` 则记 usage 字段

辅助函数建议放 `vision_openai.rs` 或 `logging.rs`（纯函数便于单测）。

### 4.3 前端

`OcrWindow.vue`：

- 工具栏增加「重新识别」按钮（中文）。
- `canRerecognize`：本地可跟踪「曾成功过」；或 invoke 前由后端判无图错误。推荐：成功 `applySuccess` 后 `hasLastImage=true`；新会话初始 false。后端仍为权威。
- loading 时三入口 + 重新识别均 disabled。
- 调用：`invoke('rerecognize_last_image')`，成功 `applySuccess`，失败 `applyError`（保留预览/文本）。

## 5. 数据与接口

### 5.1 AppState

```rust
// 概念字段
last_ocr_image: Arc<Mutex<Option<CapturedImage>>>,

pub fn set_last_ocr_image(&self, image: CapturedImage) -> Result<(), String>;
pub fn clone_last_ocr_image(&self) -> Result<Option<CapturedImage>, String>;
```

可选：`clear_last_ocr_image` 本轮可不做。

### 5.2 Command

```rust
#[tauri::command]
pub async fn rerecognize_last_image(
    state: State<'_, AppState>,
) -> Result<RecognizeImageResponse, String>
```

- 读配置 `ocr_services`，`OcrHints::default()`。
- 错误：`friendly_ocr_error` 包装。

### 5.3 写入点

`recognize_image_full` 在 `Ok(RecognizeImageResponse)` 返回前：

- 调用方传入的 `image` 需在 recognize 消费后仍可缓存——注意 `image` 可能已 move 进 engine。
- **实现约定**：在函数入口 `let image_for_cache = image.clone()`（或 encode 预览前 clone），成功后 `state` 不在 platform 层——platform 无 AppState。

**写入职责归属（锁定）：**

- **不在** `platform::recognize_image_full` 内写 AppState（保持 platform 无 UI 状态）。
- 在 **ui 层** 所有成功返回 `RecognizeImageResponse` 的路径统一 `set_last_ocr_image`：
  - `recognize_clipboard_image`
  - `pick_and_recognize_image`（Some）
  - `submit_capture_region` 的 `RecognizeOnly` 成功分支（emit 前）
  - `rerecognize_last_image` 成功后可再次 set（同一图 clone，可选优化跳过）

为支持重新识别，缓存必须是 **源图 `CapturedImage`**，不是预览 PNG base64。

**问题**：ui 层在成功时只有 `RecognizeImageResponse`，没有 `CapturedImage`。

**解决（锁定）：**

1. 在 `recognize_image_full` 入口 clone `image` 为 `to_cache`，成功时把 `to_cache` 通过扩展返回或副作用交回？  
2. 更干净：新增返回类型或并行 API：

```rust
pub struct RecognizeImageOutcome {
    pub response: RecognizeImageResponse,
    pub source_image: CapturedImage, // 供调用方缓存；勿写入日志
}
```

或 `recognize_image_full` 返回 `(RecognizeImageResponse, CapturedImage)`。

**选定：** `recognize_image_full` / `recognize_cropped_full` 成功时额外返回源图供 ui 缓存：

```rust
// 方案：RecognizeImageResponse 不变（仍给前端 IPC）
// 内部或包装：
pub struct RecognizeImageFullResult {
    pub response: RecognizeImageResponse,
    pub source_image: CapturedImage,
}
```

ui command 只把 `response` 序列化给前端；`source_image` 进 AppState。

`recognize_cropped_full` 同理返回 crop 后源图。

## 6. 错误处理

| 场景 | 行为 |
|---|---|
| 无 last image | `Err("没有可重新识别的图像，请先截图、打开文件或从剪贴板识别。")` |
| 引擎失败 | 同现有 friendly 文案；不清除 last_ocr_image |
| capture 锁占用 | 重新识别不走 overlay，**不**占 capture 锁 |
| 翻译进行中 | 重新识别**不**检查 `translation_busy`（与纯识别一致） |

## 7. 测试

| 项 | 说明 |
|---|---|
| `sanitize_request_body_for_log` | 含长 data URL 时输出含 `[len=`，不含原始 base64 片段 |
| Authorization 日志辅助 | 脱敏后不含完整 key |
| `rerecognize` 无缓存 | 返回错误字符串匹配 |
| 可选 | 有缓存时调用 mock engine 的集成测（若现有 Fake 不便可仅 unit） |
| 前端 | typecheck；可选 vitest 不强制 |

## 8. 风险与非目标

**风险**

- 大图 `CapturedImage` 内存驻留：单槽可接受；多屏 4K 裁剪后通常可控。
- 日志过长：system prompt 全文仅 debug；body 必须 sanitize。

**非目标**

- 识别历史 SQLite / 多槽缓存  
- 一键翻译  
- 完整 i18n  
- info 级打印完整参数  
- 修改 `Alt+S` 语义  

## 9. 实现落点（文件提示）

| 区域 | 文件 |
|---|---|
| 缓存 | `app/state.rs` |
| 返回包装 | `core/ocr/meta.rs` 或 platform 签名 |
| 编排 | `platform/windows/mod.rs`、`unsupported.rs` |
| Command | `ui/ocr_window.rs`、`lib.rs` |
| Overlay 成功 | `ui/overlay.rs` RecognizeOnly 分支写缓存 |
| 日志 | `core/ocr/vision_openai.rs`、可选 `logging.rs` |
| 前端 | `frontend/src/ocr/OcrWindow.vue` |

## 10. 与上一版规格关系

本规格是 `2026-07-15-ocr-window-logging-quality` 的增量：不重做窗口壳与三种输入，仅加「重新识别」与 Vision **请求级**诊断日志。
