# 截图翻译：OCR 前先开弹窗（消除空窗）设计规格

- 日期：2026-08-03
- 规模：M
- 类型：交互时序 + 取消语义（Translate 路径）
- 状态：已实现

## 背景与动机

当前截图翻译（`Alt+S` / 托盘「截图翻译」/ 弹窗工具栏截图按钮）在框选提交后：

1. hide overlay  
2. **await OCR**（`recognize_region`）  
3. OCR 成功才 `show_translation_popup` + `start_translation_from_input`  

OCR 进行期间桌面无任何反馈。Windows 本地 OCR 往往可接受；云端 Vision OCR 常 1–5 秒，形成明显「空窗」，用户会以为快捷键失效。

根因不是「OCR 慢」，而是 **反馈断档**：主结果界面（翻译弹窗）出现得太晚。

## 目标

1. 框选提交后 **立刻** 打开翻译弹窗，进入「正在识别文字…」态。  
2. OCR 成功后无缝进入现有翻译流（`Started` / `Delta` / …）。  
3. OCR 失败时仍在同一弹窗展示友好错误（复用 `friendly_ocr_error`）。  
4. 识别中用户 **hide / destroy 弹窗** → **立刻取消本次 OCR**；结果不得再写回弹窗或拉起翻译。  
5. 新一轮截图翻译 **接管并取消** 进行中的 OCR（最新优先）。  
6. 识别中 StatusBar 提供「取消」，与关窗同语义。

## 非目标

- 不改 `CapturePurpose::RecognizeOnly`（纯文字识别窗）路径。  
- 不做独立 loading 小窗、主路径 toast、overlay 挂着识别。  
- 不把 OCR 伪装成 `translating`（避免文案与 cancel token 混淆）。  
- 不为 OCR 失败增加「重新识别」按钮或伪造翻译 `retry`（引导用户再 `Alt+S`）。  
- 不改 OCR 引擎本身、多屏 / DPI 已知问题。  
- 不引入应用内安装或 updater 相关能力。

## 现状约束

| 点 | 说明 |
|----|------|
| 时序入口 | [`submit_capture_region`](../../../src-tauri/src/ui/overlay.rs) 的 `CapturePurpose::Translate` 分支：先 OCR 再 show |
| 截图锁 | `capture_in_progress` 从抓帧持有到 submit 内 `finish_capture`；当前 OCR 期间仍占锁 |
| 翻译取消 | `translation_generation` + `current_cancel_token` + `begin_translation_overriding`（仅翻译阶段） |
| 弹窗关闭 | 系统关 / `CloseRequested` → **hide**（`attach_close_to_hide`）；工具栏关闭 → **destroy** |
| 截图前 hide | `start_translation_from_ocr` 会先 hide 弹窗再抓帧——此时尚无 OCR job，不得误 cancel |
| 前端状态 | StatusBar 已有 `loading` + 取消/重试动作；结果卡有 `pending` / `translating` 等 |

## 方案总览

采用 **方案 A：框选提交后立刻 show 翻译弹窗 + OCR 阶段独立取消**。

```text
框选提交
  → hide overlay
  → take 帧 + 物理矩形（校验）
  → finish_capture（尽早释放截图锁）
  → begin_ocr_overriding(token)   // 新：OCR 代次 + 取消
  → show_translation_popup        // 立刻
  → emit 识别中
  → await OCR（响应 cancel）
  → 已取消 → 静默返回
  → 失败 → 友好错误（不 start 翻译）
  → 成功 → start_translation_from_input（现有链路）
```

## 后端设计

### 1. `AppState`：OCR 管线状态

与翻译解耦，避免 OCR 阶段误占 `translation_busy`：

- `ocr_pipeline_generation: Arc<Mutex<u64>>`
- `ocr_cancel_token: Arc<Mutex<Option<CancellationToken>>>`

API（命名可微调，语义固定）：

| 方法 | 行为 |
|------|------|
| `begin_ocr_overriding(token) -> Result<u64, String>` | 若已有 OCR：cancel 旧 token；登记新 token；generation++；返回本代 |
| `finish_ocr_if_current(generation) -> Result<(), String>` | 仅 generation 仍当前时清 token；stale 则 no-op |
| `cancel_current_ocr() -> Result<(), String>` | take 并 cancel 当前 OCR token；幂等 |
| `is_ocr_generation_current(generation) -> bool` | OCR 完成后门闩，防取消后仍 start 翻译 |

**不** 在 OCR begin 时调用 `begin_translation_overriding`（翻译仍在 `start_translation_from_input` 内 begin）。

### 2. `submit_capture_region` Translate 分支改写

文件：[`src-tauri/src/ui/overlay.rs`](../../../src-tauri/src/ui/overlay.rs)

伪代码：

```rust
CapturePurpose::Translate => {
    // 帧已 take；选区已校验
    let _ = state.finish_capture(); // 尽早释放，OCR 不占 capture

    let cancel = CancellationToken::new();
    let generation = state.begin_ocr_overriding(cancel.clone())?;

    let _ = show_translation_popup(&app, &config);
    emit_ocr_recognizing(&app); // 见事件节

    let result = recognize_region_cancellable(
        &frame, region, &config.ocr_services, &cancel,
    ).await;

    if cancel.is_cancelled() || !state.is_ocr_generation_current(generation) {
        let _ = state.finish_ocr_if_current(generation);
        return Ok(()); // 静默：不 error、不翻译
    }

    let _ = state.finish_ocr_if_current(generation);

    match result {
        Ok(None) => show_translation_error(...), // 契约违反兜底
        Ok(Some(input)) => {
            if let Err(e) = start_translation_from_input(input, app.clone(), state.inner()) {
                show_translation_error(&app, e);
            }
        }
        Err(e) => show_translation_error(&app, friendly_ocr_error(e)),
    }
}
```

要点：

- **show 在 OCR 之前**；失败路径仍可能 `show_translation_error` 内再 show（幂等）。  
- 取消路径 **禁止** `show_translation_error` / `start_translation_from_input`。  
- `finish_capture` 在 OCR 前调用：允许用户在识别中再按 `Alt+S`；新一轮 `try_begin_capture` 成功后，提交时应 `begin_ocr_overriding` 取消上一 OCR。

### 3. OCR 可取消

- Vision / HTTP OCR：在请求循环或 `select!` 上响应 `CancellationToken`（与翻译 provider 同模式；能 drop 请求体则 drop）。  
- Windows 本地 OCR：若底层同步且短时，至少在 **调用前后** 检查 token；完成后若已 cancel 则丢弃结果（门闩 `is_ocr_generation_current`）。  
- 不必为本地 OCR 做线程 abort；**结果丢弃** 即可满足关窗语义。

若 `recognize_region` 签名扩展不便，可在 `overlay` 层：

```text
tokio::select! {
  _ = cancel.cancelled() => /* 当作取消 */,
  r = recognize_region(...) => r,
}
```

取消时不把 `OcrError` 当用户错误。

### 4. 关窗 / 取消入口

| 入口 | 行为 |
|------|------|
| 弹窗 `CloseRequested` → hide | 在现有 hide 钩子中调用 `cancel_current_ocr()`（仅 OCR 进行中时有效） |
| 弹窗 `Destroyed` | 同上 `cancel_current_ocr()` |
| 新 command 或复用 | 前端 StatusBar「取消」在识别中调用 `cancel_ocr`（或扩展现有 `cancel_translation` 文档语义为「取消当前活动」——**推荐独立 `cancel_ocr`**，避免与翻译 token 混淆） |
| 新一轮 `begin_ocr_overriding` | 自动 cancel 旧 OCR |

**截图前主动 hide**（`start_translation_from_ocr`）：此时通常无 OCR job；`cancel_current_ocr` 幂等空操作。若上一轮 OCR 仍在跑且用户又开截图：capture 开始时 **也可** `cancel_current_ocr()`，避免旧识别结果在框选中途写回——推荐在 `start_translation_from_ocr` 成功 `try_begin_capture` 后调用一次，语义更干净。

### 5. 事件（前端识别中态）

**推荐**：扩展 `translation:event` 载荷，增加识别阶段类型，避免第二套 listen 基础设施。

序列化示例（camelCase，与现有一致）：

```json
{ "type": "ocrStarted" }
```

```json
{ "type": "ocrFailed", "message": "…", "retryable": false }
```

或失败继续走现有 `Failed` + `show_translation_error`（已有 `retryable: false`），仅新增 **`ocrStarted`** 即可驱动「识别中」UI。

约定：

- `ocrStarted`：show 弹窗后、OCR await 前 emit（可 best-effort 再发一次防冷启动丢事件，YAGNI 下先单次；冷启动依赖弹窗 listen 就绪——若窗口已 precreate 通常足够；若仍丢，后续用 pending 位补）。  
- 成功：不 emit `ocrFinished`；直接 `start_translation_from_input` → 现有 `Started`。  
- 取消：不 emit `Cancelled`（用户已关窗时无展示对象）；若弹窗仍在且点 StatusBar 取消，可 emit 轻量恢复 ready，或保持「已取消」——**推荐**：弹窗仍可见时 StatusBar 回到 `ready`，原文保持空。  
- 失败：`show_translation_error` 现有路径。

若扩展 `TranslationEvent` 枚举成本高，允许独立事件名 `screenshot-translate:ocr-started`；前端多 listen 一次。优先扩展现有事件以保持单通道。

### 6. Command

| Command | 用途 |
|---------|------|
| `cancel_ocr` | 识别中取消；幂等 |

权限：在 capabilities 中为 popup 侧授权（与 `cancel_translation` 同级窗口权限范围）。

## 前端设计

文件：[`frontend/src/popup/TranslationPopup.vue`](../../../frontend/src/popup/TranslationPopup.vue) 及 `useTranslationEvents`（若事件走 translation:event）。

### 识别中态

- 收到 `ocrStarted`：  
  - `setStatus('popup.status.recognizing', true, { 取消 → invoke cancel_ocr })`  
  - 原文清空或保持空（新一轮截图翻译应清旧结果卡到 pending，与新 batch 一致；可在 `ocrStarted` 时 reset 卡片为 pending / 清空译文）  
- 收到翻译 `Started`：沿用现有「翻译中」+ `cancel_translation`。  
- OCR 失败 `Failed`：现有失败态；`retryable: false` 不显示翻译重试（已有约定则保持）。  
- 用户点取消（识别中）：`cancel_ocr`；StatusBar → ready。  
- **不** 在识别中显示翻译重试。

### 文案

| Key | 中文默认 |
|-----|----------|
| `popup.status.recognizing` | 正在识别文字… |

i18n 包同步（`interface` 语言资源中增加 key）。

### 关闭与 destroy

后端 hide/destroy 钩子负责 cancel；前端 unmount 时 **可选** 再调 `cancel_ocr` 作双保险（destroy 路径）。hide 不 unmount WebView 时依赖后端钩子。

## 错误与边界

| 场景 | 行为 |
|------|------|
| OCR 空文本 | `friendly_ocr_error(EmptyResult)`，弹窗已开 |
| 用户识别中 hide | cancel OCR；不再 show / 不再翻译 |
| 用户识别中 destroy | 同上 |
| 识别中再 Alt+S | 新 capture；旧 OCR 在 begin_capture 或 begin_ocr 时 cancel |
| 识别中点取消但弹窗仍开 | cancel；StatusBar ready；可再截图 |
| 本地 OCR 极快 | 「识别中」可能一闪——可接受 |
| 取消与失败竞态 | generation 门闩：stale 结果丢弃 |
| 框选取消 | 现有 `cancel_capture`；不进入 OCR 管线 |

## 测试计划

### 后端单测

- `begin_ocr_overriding` 取消旧 token、generation 递增。  
- `finish_ocr_if_current` 对 stale generation no-op。  
- `cancel_current_ocr` 幂等。  
- 逻辑测试（可抽纯函数或 mock OCR）：cancel 后不调用 start 翻译；失败走 friendly 文案路径契约。

### 集成 / 手工

1. Alt+S → 框选 → **立即** 见弹窗「正在识别文字…」。  
2. 云端 OCR 慢时 loading 持续，成功后进翻译。  
3. 识别中 hide → 不自动再弹、无翻译请求。  
4. 识别中工具栏关闭 destroy → 同上。  
5. 识别中 StatusBar 取消 → ready，无翻译。  
6. 识别中再 Alt+S 完成新框选 → 旧 OCR 无效，新结果驱动翻译。  
7. OCR 失败（空区）→ 弹窗内友好错误，无误导重试。  
8. 纯文字识别窗截图路径回归不变。

## 文档同步（实现收尾门禁）

- [ ] 本 spec 勾选完成项（实现后）  
- [ ] [`docs/agent/architecture-notes.md`](../../agent/architecture-notes.md) 补充：截图翻译 Translate 路径「先 show 再 OCR」与 OCR cancel  
- [ ] 若 roadmap 有对应项则勾选；无则不硬加  

## 实现范围摘要

| 层 | 文件（预期） | 改动 |
|----|----------------|------|
| 状态 | `src-tauri/src/app/state.rs` | OCR generation + cancel token |
| 编排 | `src-tauri/src/ui/overlay.rs` | Translate 时序重排 |
| 弹窗生命周期 | `popup_window` / `window` hide·destroy 钩子 | `cancel_current_ocr` |
| Command | `lib.rs` + capabilities | `cancel_ocr` |
| 事件 | translation 事件类型或旁路事件 | `ocrStarted` |
| 前端 | popup + i18n | 识别中 StatusBar + 取消 |
| 测试 | state 单测 + 必要前端单测 | 取消 / 代次 |

## 决议记录

| 项 | 决议 |
|----|------|
| 方案 | A：先开翻译弹窗再 OCR |
| 关窗 | hide/destroy 立刻取消 OCR（选项 1） |
| 识别中文案 | 「正在识别文字…」 |
| 纯识别路径 | 不变 |
| plan 文档 | M 档默认不写独立 plan；实现可在本对话内联推进 |
