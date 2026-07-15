# 独立文字识别窗口 · OCR Debug 日志 · Vision detail=high 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [x]`）语法来跟踪进度。

**目标：** 在不改变 `Alt+S` 截图→OCR→自动翻译语义的前提下，交付独立「文字识别」窗口（截图/文件/剪贴板）、OCR 可诊断日志（含修复 redact 硬编码 info），以及 Vision 请求默认 `image_url.detail = "high"`。

**架构：** 方案 A——独立 Tauri `ocr` 窗口 + core/platform `recognize` 编排，与翻译弹窗解耦。截图路径通过 `CapturePurpose`（`Translate` | `RecognizeOnly`）在 `submit_capture_region` 处分叉：翻译继续 `start_translation_from_input`，纯识别 emit `ocr:recognize-result` 到 ocr 窗口。Vision/Windows 引擎与日志增强对两条路径共享。本轮不做一键翻译、识别历史、detail/温度 UI。

**技术栈：** Rust（Tauri 2、reqwest、serde、arboard 图像、image、log）、Vue 3 + TypeScript + Vite 多页、Vitest、cargo test

**规格来源：** `docs/superpowers/specs/2026-07-15-ocr-window-logging-quality-design.md`

---

## 与 spec 的实现澄清

1. **新快捷键 id：`ocr-recognize`，默认 `Alt+O`。** 与 `translate-screenshot`（`Alt+S`）并列。存量 `normalize_shortcuts` 仍将 **截图翻译** 历史键 `Alt+O`/`Alt+E` 迁到 `Alt+S`；**不得**把新键 `ocr-recognize` 的 `Alt+O` 迁走。
2. **`CapturePurpose` 放在 `AppState`：** `Translate`（现网）/ `RecognizeOnly`（纯识别）。`start_translation_from_ocr` 设 `Translate`；新入口 `start_ocr_capture` 设 `RecognizeOnly`。`submit_capture_region` 按 purpose 分叉。
3. **`OcrEngine` trait 签名本轮不改**（避免 Windows/Fake 大爆炸）。纯识别编排在 platform/ui 层计时并组装 `OcrRunMeta`；Vision 通过 `encode` 返回缩放信息 + HTTP status 填 meta；Windows 无 HTTP/`png_bytes` 则 `None`/`scaled=false`、送模尺寸=原图。
4. **文件选择：** 后端 command 用已有 `tauri-plugin-dialog` 的 open 对话框读文件字节（capability 补 `dialog:allow-open`）；前端不直连厂商 API。
5. **预览图：** `RecognizeImageResponse.previewPngBase64` 仅经 IPC 给 UI，**永不**写入日志。
6. **文案纠错：** `friendly_ocr_error` 中仍写「Alt+O」指截图翻译的句子改为「Alt+S」或中性「重新截图」（与现网快捷键一致）。

---

## 文件结构

| 文件 | 职责 |
|---|---|
| 修改 `src-tauri/src/core/ocr/vision_openai.rs` | `detail: "high"`；debug 日志（endpoint/prompt/耗时/status/usage）；单测断言 detail |
| 修改 `src-tauri/src/core/ocr/image_encode.rs` | `EncodePngInfo { png, source_w/h, sent_w/h, scaled }`；debug 缩放前后 |
| 创建 `src-tauri/src/core/ocr/meta.rs` | `OcrRunMeta` 可序列化结构 + 组装/日志摘要纯函数 |
| 修改 `src-tauri/src/core/ocr/mod.rs` | `mod meta`；导出 |
| 修改 `src-tauri/src/core/logging.rs` | `effective_redact_level()`：`log::max_level()` ≥ Debug → `"debug"` 否则 `"info"` |
| 修改 `src-tauri/src/core/ocr_translation.rs` | 用 `effective_redact_level()` 替代硬编码 `"info"`；info 记 meta 摘要字段 |
| 修改 `src-tauri/src/platform/windows/mod.rs` | `recognize_image_full`（整图→text+meta+preview）；裁剪纯识别入口 |
| 修改 `src-tauri/src/platform/unsupported.rs` | 签名对齐 |
| 修改 `src-tauri/src/platform/mod.rs` | 导出 |
| 修改 `src-tauri/src/app/state.rs` | `CapturePurpose` + get/set；注释中 Alt+O 指 capture 锁语义保持 |
| 修改 `src-tauri/src/ui/overlay.rs` | `submit_capture_region` 按 purpose 分叉 |
| 创建 `src-tauri/src/ui/ocr_window.rs` | `open_ocr_window`、`start_ocr_capture`、`recognize_image_bytes`、`recognize_clipboard_image`、`pick_and_recognize_image`；事件 payload |
| 修改 `src-tauri/src/ui/ocr_popup.rs` | 启动 capture 时设 `CapturePurpose::Translate`；友好文案 Alt+S |
| 修改 `src-tauri/src/ui/mod.rs` | `mod ocr_window` |
| 修改 `src-tauri/src/app/window.rs` | `OCR_LABEL` / `ensure_ocr_window` / `show_ocr_window`；`close_to_hide`；focus listener |
| 修改 `src-tauri/src/app/shortcuts.rs` | `ocr-recognize` Global + `OcrRecognize` action；`any_app_window_focused` 含 ocr |
| 修改 `src-tauri/src/app/tray.rs` | 菜单「文字识别」+ `TrayI18nHandles.ocr` |
| 修改 `src-tauri/src/ui/i18n.rs` | 托盘 `tray.ocr`；ocr 窗口标题键 |
| 修改 `src-tauri/src/core/config/types.rs` | `default_shortcuts` 增 `ocr-recognize`→`Alt+O`；`normalize` 勿迁移该键 |
| 修改 `src-tauri/src/lib.rs` | 注册 commands；setup `ensure_ocr_window` |
| 修改 `src-tauri/tauri.conf.json` | 可选：不预置 ocr 窗口（与 settings 一样代码创建） |
| 修改 `src-tauri/capabilities/default.json` | windows 加 `"ocr"`；`dialog:allow-open` |
| 修改 `frontend/vite.config.ts` | rollup input `ocr` |
| 创建 `frontend/ocr.html` | OCR 窗口 HTML 入口 |
| 创建 `frontend/src/ocr/main.ts` | Vue mount |
| 创建 `frontend/src/ocr/OcrWindow.vue` | 工具栏 + 双栏 + 元信息 + 状态机 |
| 修改 `frontend/src/settings/stores/settings.ts` | bindings 增 `ocr-recognize` |
| 修改 `frontend/src/settings/panels/ShortcutPanel.vue` | `GLOBAL_IDS` 加 `ocr-recognize` |
| 修改 `frontend/src/lib/config.test.ts` / `settings.test.ts` | 快捷键投影与默认 |
| 修改 `frontend/src/i18n/locales/zh-CN.json`、`en-US.json` | 快捷键/托盘/OCR 窗口文案 |
| 修改 `frontend/src/i18n` 其它已存在 locale（若键为强校验） | 补键或依赖 zh-CN 回退（按现有 i18n 规则） |
| 修改 `README.md` / 架构文档（编码收尾） | 能力一句 |
| 修改 spec 状态（编码收尾） | 复选/状态回填 |

**刻意不改：** `TranslationProvider`、翻译历史 SQLite、Claude 视觉、`Alt+S` 产品语义、overlay 框选交互本身、一键翻译按钮。

---

## 任务 1：Vision `detail=high` + 单测

**文件：**
- 修改：`src-tauri/src/core/ocr/vision_openai.rs`

- [x] **步骤 1：编写失败的测试（扩展现有 `request_body_is_non_streaming_with_image_url` 或新测）**

在 `vision_openai.rs` 的 `#[cfg(test)] mod tests` 中新增：

```rust
#[test]
fn request_body_sets_image_url_detail_high() {
    let body = VisionOcrEngine::build_request_body(
        "gpt-4o",
        "sys",
        "data:image/png;base64,AAA",
    );
    assert_eq!(
        body["messages"][1]["content"][1]["image_url"]["detail"],
        "high"
    );
}
```

- [x] **步骤 2：运行测试验证失败**

```powershell
cd src-tauri; cargo test --lib ocr::vision_openai::tests::request_body_sets_image_url_detail_high -- --nocapture
```

预期：FAIL（`detail` 为 null/缺失）。

- [x] **步骤 3：最少实现**

在 `build_request_body` 的 `image_url` 对象中增加 `"detail": "high"`：

```rust
"image_url": {
    "url": data_url,
    "detail": "high"
}
```

同步更新旧测试 `request_body_is_non_streaming_with_image_url`：在断言 url 后加一行 detail 断言（或依赖新测即可）。

- [x] **步骤 4：运行测试验证通过**

```powershell
cd src-tauri; cargo test --lib ocr::vision_openai::tests -- --nocapture
```

预期：PASS。

- [x] **步骤 5：Commit**

```powershell
git add src-tauri/src/core/ocr/vision_openai.rs
git commit -m "feat(ocr): Vision 请求默认 image_url.detail=high"
```

---

## 任务 2：`effective_redact_level` + 修复 OCR 脱敏硬编码

**文件：**
- 修改：`src-tauri/src/core/logging.rs`
- 修改：`src-tauri/src/core/ocr_translation.rs`

- [x] **步骤 1：编写失败的测试**

在 `logging.rs` 的 tests 中增加（注意：`log::set_max_level` 是进程全局，测完尽量恢复，避免污染并行；若 crate 测试并行不稳，可用 `serial_test` 或仅测纯映射函数）：

```rust
/// 纯映射：给定 LevelFilter 返回 redact 用的 level 字符串。
pub fn redact_level_for_filter(filter: log::LevelFilter) -> &'static str {
    if filter >= log::LevelFilter::Debug {
        "debug"
    } else {
        "info"
    }
}

// tests:
#[test]
fn redact_level_for_filter_debug_and_trace_are_full() {
    assert_eq!(redact_level_for_filter(log::LevelFilter::Debug), "debug");
    assert_eq!(redact_level_for_filter(log::LevelFilter::Trace), "debug");
}

#[test]
fn redact_level_for_filter_info_and_below_are_summary() {
    assert_eq!(redact_level_for_filter(log::LevelFilter::Info), "info");
    assert_eq!(redact_level_for_filter(log::LevelFilter::Warn), "info");
    assert_eq!(redact_level_for_filter(log::LevelFilter::Error), "info");
    assert_eq!(redact_level_for_filter(log::LevelFilter::Off), "info");
}
```

`effective_redact_level()` 实现为：

```rust
pub fn effective_redact_level() -> &'static str {
    redact_level_for_filter(log::max_level())
}
```

- [x] **步骤 2：运行测试验证失败**

```powershell
cd src-tauri; cargo test --lib logging::tests::redact_level_for_filter -- --nocapture
```

预期：FAIL（函数不存在）。

- [x] **步骤 3：实现函数；修复 `ocr_translation.rs`**

将：

```rust
crate::core::logging::redact_text(&text, "info")
```

改为：

```rust
crate::core::logging::redact_text(&text, crate::core::logging::effective_redact_level())
```

并在识别成功日志处补充 info 级摘要（引擎名若可得写 result.engine；尺寸在 crop 后可知）：

```rust
log::info!(
    "OCR 翻译入口: engine={} text={}",
    result.engine,
    crate::core::logging::redact_text(&text, crate::core::logging::effective_redact_level())
);
```

（`result` 在 trim 前已有；保持 empty 检查逻辑不变。）

- [x] **步骤 4：运行测试**

```powershell
cd src-tauri; cargo test --lib logging::tests; cargo test --lib ocr_translation::tests
```

预期：PASS。

- [x] **步骤 5：Commit**

```powershell
git add src-tauri/src/core/logging.rs src-tauri/src/core/ocr_translation.rs
git commit -m "fix(ocr): 按运行时日志级别脱敏识别正文，不再硬编码 info"
```

---

## 任务 3：`OcrRunMeta` + `EncodePngInfo`

**文件：**
- 创建：`src-tauri/src/core/ocr/meta.rs`
- 修改：`src-tauri/src/core/ocr/mod.rs`
- 修改：`src-tauri/src/core/ocr/image_encode.rs`
- 修改：`src-tauri/src/core/ocr/vision_openai.rs`（改用 EncodePngInfo，填日志）

- [x] **步骤 1：编写失败的测试（meta + encode）**

`meta.rs`：

```rust
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrRunMeta {
    pub engine: String,
    pub model: Option<String>,
    pub source_width: u32,
    pub source_height: u32,
    pub sent_width: u32,
    pub sent_height: u32,
    pub png_bytes: Option<u64>,
    pub latency_ms: u64,
    pub http_status: Option<u16>,
    pub scaled: bool,
}

impl OcrRunMeta {
    pub fn info_summary(&self) -> String {
        format!(
            "engine={} model={:?} src={}x{} sent={}x{} png={:?} latency_ms={} http={:?} scaled={}",
            self.engine,
            self.model,
            self.source_width,
            self.source_height,
            self.sent_width,
            self.sent_height,
            self.png_bytes,
            self.latency_ms,
            self.http_status,
            self.scaled
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_summary_contains_core_fields() {
        let m = OcrRunMeta {
            engine: "openai-vision".into(),
            model: Some("gpt-4o".into()),
            source_width: 100,
            source_height: 50,
            sent_width: 100,
            sent_height: 50,
            png_bytes: Some(1234),
            latency_ms: 42,
            http_status: Some(200),
            scaled: false,
        };
        let s = m.info_summary();
        assert!(s.contains("openai-vision"));
        assert!(s.contains("100x50"));
        assert!(s.contains("latency_ms=42"));
    }

    #[test]
    fn serializes_camel_case() {
        let m = OcrRunMeta {
            engine: "windows-media-ocr".into(),
            model: None,
            source_width: 1,
            source_height: 1,
            sent_width: 1,
            sent_height: 1,
            png_bytes: None,
            latency_ms: 0,
            http_status: None,
            scaled: false,
        };
        let v = serde_json::to_value(&m).unwrap();
        assert!(v.get("sourceWidth").is_some());
        assert!(v.get("latencyMs").is_some());
        assert!(v.get("httpStatus").is_some());
    }
}
```

`image_encode.rs` 新增类型与 API：

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodePngInfo {
    pub png: Vec<u8>,
    pub source_width: u32,
    pub source_height: u32,
    pub sent_width: u32,
    pub sent_height: u32,
    pub scaled: bool,
}

pub fn encode_captured_image_png_info(image: &CapturedImage) -> Result<EncodePngInfo, OcrError> {
    // 原 encode 逻辑；返回完整 info
}

/// 兼容旧调用点
pub fn encode_captured_image_png(image: &CapturedImage) -> Result<Vec<u8>, OcrError> {
    Ok(encode_captured_image_png_info(image)?.png)
}
```

测试：缩放 case 断言 `scaled == true` 且 `sent_width <= 2048`。

- [x] **步骤 2：运行测试验证失败**

```powershell
cd src-tauri; cargo test --lib ocr::meta -- --nocapture
```

预期：FAIL。

- [x] **步骤 3：实现 meta 模块、改 encode、改 vision 使用 info**

`vision_openai::recognize`：

```rust
let start = std::time::Instant::now();
let encoded = encode_captured_image_png_info(&image)?;
log::debug!(
    "Vision OCR 编码: src={}x{} sent={}x{} scaled={} png_bytes={}",
    encoded.source_width, encoded.source_height,
    encoded.sent_width, encoded.sent_height,
    encoded.scaled, encoded.png.len()
);
// ... build body, send ...
log::debug!(
    "Vision OCR 请求: endpoint={} model={} prompt_len={}",
    self.endpoint(), // 无 query；勿 log Authorization
    self.config.model,
    system.chars().count()
);
// debug 可打 system prompt 全文（配置内容，非 key）
log::debug!("Vision OCR system prompt: {system}");
// 成功后：
log::info!(
    "Vision OCR 完成: status={} latency_ms={} text={}",
    status,
    start.elapsed().as_millis(),
    crate::core::logging::redact_text(&content, crate::core::logging::effective_redact_level())
);
```

**注意：** 本任务不强制 Vision 返回 meta 给调用方；任务 4 在 platform 层组装。若愿减少重复计时，可后续在 Vision 内暴露 helper；YAGNI 先 platform 计时 + encode 再跑一次仅用于 meta 时禁止——**platform 层应对 Vision 路径调用一次 encode 信息**：推荐在 platform `recognize_image_full` 中先 `encode_captured_image_png_info` 取 meta 字段，但 Vision 内部会再 encode 一次。为避免双 encode，任务 4 可让 Vision 使用已有 `encode` 一次即可，meta 的 sent/png/scaled 从「仅 Vision 路径」在 `recognize` 前后由 image 尺寸 + 再 encode info 估算：

**实现约定（避免双次缩放偏差）：**  
platform 在调用 `engine.recognize` **之前** 对入参 image 调一次 `encode_captured_image_png_info` 只为填 meta（浪费一次 CPU）。更优：把 encode 留在 Vision 内，platform 对 Windows 用原图尺寸，对 Vision 用：

- 调用前记录 `source_* = image.width/height`
- `sent_*` / `png_bytes` / `scaled`：调用前用 `encode_captured_image_png_info` **仅 meta**（接受双 encode）或扩展 trait（本轮不选）。

**本计划选择：接受 Vision 路径双 encode（实现简单）**；meta 在 platform 组装，http_status 对 Vision 成功固定记 `Some(200)`，失败路径在 Err 时 UI 可能无 status——失败时仍尽量 log。若需失败 HTTP status，可在 map_http_error 前 log。UI 成功路径 `http_status: Some(200)` 即可；更精确可后续改。

更干净的折中：在 `recognize` 成功后 platform 不知道 HTTP。**任务 3 只交付 EncodePngInfo + OcrRunMeta 类型与 Vision 日志；http_status 在成功时由 platform 填 `None` 或 Vision 日志已有 status。**  
为满足 spec UI 字段：在 `vision_openai` 增加：

```rust
// 线程不共享状态；改为 Recognize 后由调用方不知道 status。
```

**最终约定：** 新增 `VisionOcrEngine` 不改 trait；platform 组装 meta 时：

| 字段 | Windows | Vision |
|---|---|---|
| engine | result.engine | result.engine |
| model | None | Some(cfg.model) |
| source_* | image | image |
| sent_*/scaled/png_bytes | =source / false / None | 来自 `encode_captured_image_png_info` |
| latency_ms | Instant 包一层 | 同 |
| http_status | None | 成功 `Some(200)`；失败走 Err 无 meta UI 或 partial meta |

失败时 UI 仍可显示已采集 meta（尺寸等）——command 返回 `Result` 时 `Err(String)` 即可，可选后续 `RecognizeError` 结构；本轮失败只字符串 + 前端保留上次 meta。

- [x] **步骤 4：测试通过 + Commit**

```powershell
cd src-tauri; cargo test --lib ocr::
git add src-tauri/src/core/ocr/
git commit -m "feat(ocr): 增加 OcrRunMeta 与 PNG 编码尺寸元信息"
```

---

## 任务 4：platform 纯识别编排 `recognize_image_full`

**文件：**
- 修改：`src-tauri/src/platform/windows/mod.rs`
- 修改：`src-tauri/src/platform/unsupported.rs`
- 修改：`src-tauri/src/platform/mod.rs`（若需 re-export）

- [x] **步骤 1：定义返回类型（可放 `core/ocr/meta.rs`）**

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognizeImageResponse {
    pub text: String,
    pub meta: OcrRunMeta,
    /// 供 UI 预览的 PNG base64（无 data: 前缀）；勿写入日志
    pub preview_png_base64: String,
}
```

预览生成：对任意 `CapturedImage` 用 `encode_captured_image_png`（**不做 2048 缩放的预览**更佳——可新增 `encode_preview_png` 最长边 1280 或不缩放）。YAGNI：直接用 `encode_captured_image_png` 的 PNG（已可能缩放）作预览亦可；**预览应反映用户所见原图** → 用 **不缩放** 编码：

```rust
// image_encode.rs
pub fn encode_png_unscaled(image: &CapturedImage) -> Result<Vec<u8>, OcrError>
```

或 `maybe_scale` 参数 `max_long_edge: Option<u32>`。任务 3 若未做，本任务补 `encode_png_unscaled`。

- [x] **步骤 2：实现 `recognize_image_full`**

```rust
pub async fn recognize_image_full(
    image: CapturedImage,
    hints: OcrHints,
    ocr_services: &[OcrServiceInstanceConfig],
    model_hint: Option<String>, // Vision 时从 resolve 配置取
) -> Result<RecognizeImageResponse, OcrError> {
    let start = Instant::now();
    let source_width = image.width;
    let source_height = image.height;
    let encode_meta = encode_captured_image_png_info(&image).ok(); // Windows 也可生成预览
    let preview_png = encode_png_unscaled(&image)?;
    let preview_b64 = base64::engine::general_purpose::STANDARD.encode(&preview_png);

    let resolved = resolve_ocr_engine(ocr_services)?;
    let (result, model, vision_encode) = match resolved {
        ResolvedOcrEngine::WindowsMedia => {
            let r = WindowsOcrEngine.recognize(image, hints).await?;
            (r, None, None)
        }
        ResolvedOcrEngine::VisionOpenAiCompatible(cfg) => {
            let model = Some(cfg.model.clone());
            let enc = encode_captured_image_png_info(&image)?; // 与 vision 内部双 encode，可接受
            let engine = VisionOcrEngine::new(cfg)?;
            let r = engine.recognize(image, hints).await?;
            (r, model, Some(enc))
        }
    };

    let text = result.text.trim().to_string();
    if text.is_empty() {
        return Err(OcrError::EmptyResult);
    }

    let (sent_w, sent_h, png_bytes, scaled) = if let Some(enc) = vision_encode {
        (enc.sent_width, enc.sent_height, Some(enc.png.len() as u64), enc.scaled)
    } else {
        (source_width, source_height, None, false)
    };

    let meta = OcrRunMeta {
        engine: result.engine.clone(),
        model,
        source_width,
        source_height,
        sent_width: sent_w,
        sent_height: sent_h,
        png_bytes,
        latency_ms: start.elapsed().as_millis() as u64,
        http_status: if vision_encode.is_some() { Some(200) } else { None },
        scaled,
    };

    log::info!("OCR 纯识别: {}", meta.info_summary());
    log::info!(
        "OCR 纯识别文本: {}",
        crate::core::logging::redact_text(&text, crate::core::logging::effective_redact_level())
    );

    Ok(RecognizeImageResponse {
        text,
        meta,
        preview_png_base64: preview_b64,
    })
}
```

裁剪路径：

```rust
pub async fn recognize_cropped_full(
    frame: &CapturedImage,
    region: (u32, u32, u32, u32),
    hints: OcrHints,
    ocr_services: &[...],
) -> Result<RecognizeImageResponse, OcrTranslationError> {
    let cropped = frame.crop(region.0, region.1, region.2, region.3)?;
    log::debug!("OCR 裁剪物理矩形: x={} y={} w={} h={}", region.0, region.1, region.2, region.3);
    recognize_image_full(cropped, hints, ocr_services, None)
        .await
        .map_err(Into::into)
}
```

- [x] **步骤 3：unsupported 返回 `UnsupportedPlatform`**

- [x] **步骤 4：编译 + 现有测试**

```powershell
cd src-tauri; cargo test --lib
```

- [x] **步骤 5：Commit**

```powershell
git add src-tauri/src/platform src-tauri/src/core/ocr
git commit -m "feat(ocr): 增加纯识别编排 recognize_image_full 与预览编码"
```

---

## 任务 5：配置快捷键 `ocr-recognize` 默认 `Alt+O`

**文件：**
- 修改：`src-tauri/src/core/config/types.rs`

- [x] **步骤 1：失败测试**

```rust
#[test]
fn default_shortcuts_include_ocr_recognize_alt_o() {
    let config = AppConfig::default();
    assert_eq!(
        config.shortcuts.get("ocr-recognize").map(String::as_str),
        Some("Alt+O")
    );
}

#[test]
fn normalize_keeps_ocr_recognize_alt_o_while_migrating_screenshot() {
    let mut config = AppConfig::default();
    config.shortcuts.insert("translate-screenshot".into(), "Alt+O".into());
    config.shortcuts.insert("ocr-recognize".into(), "Alt+O".into());
    // 先制造冲突非法态仅测 normalize 映射：normalize 只改 screenshot 历史键
    let n = config.normalized();
    assert_eq!(n.shortcuts.get("translate-screenshot").unwrap(), "Alt+S");
    assert_eq!(n.shortcuts.get("ocr-recognize").unwrap(), "Alt+O");
}
```

说明：若 `normalized` 后 `configured_shortcuts` 会因重复键报错，那是保存/注册时的事；normalize 本身应保留两键值。用户若同时保留两者同为 Alt+O，注册阶段 `configured_shortcuts` 报重复——可接受。默认配置中 screenshot=`Alt+S`、ocr=`Alt+O`，不冲突。

- [x] **步骤 2：改 `default_shortcuts`**

```rust
("ocr-recognize".to_string(), "Alt+O".to_string()),
```

**不要**在 `normalize_shortcuts` 的 match 里把 `ocr-recognize` 的 Alt+O 改掉。截图翻译迁移保持：

```rust
("translate-screenshot", "Alt+O" | "Alt+E") => "Alt+S".to_string(),
```

- [x] **步骤 3：测试 + Commit**

```powershell
cd src-tauri; cargo test --lib config::types
git add src-tauri/src/core/config/types.rs
git commit -m "feat(config): 新增 ocr-recognize 快捷键默认 Alt+O"
```

---

## 任务 6：快捷键注册与动作 `OcrRecognize`

**文件：**
- 修改：`src-tauri/src/app/shortcuts.rs`
- 修改：`src-tauri/src/ui/ocr_window.rs`（可先 stub `open_ocr_window` + `start_ocr_capture`，任务 8/9 补全）

- [x] **步骤 1：扩展枚举与映射**

```rust
// ShortcutAction
OcrRecognize,

// kind_for_id
"ocr-recognize" => ShortcutKind::Global,

// action_for_id
"ocr-recognize" => Some(ShortcutAction::OcrRecognize),

// label_for_id
"ocr-recognize" => "文字识别",
```

`any_app_window_focused`：

```rust
for label in ["main", SETTINGS_LABEL, "ocr"] {
```

（`OCR_LABEL` 常量放 `window.rs` 后在此引用。）

- [x] **步骤 2：handler**

```rust
Some(ShortcutAction::OcrRecognize) => {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = crate::ui::ocr_window::open_ocr_window(&app_handle) {
            log::warn!("打开文字识别窗口失败: {e}");
        }
        // 产品：快捷键打开窗口；是否立即截图？
        // spec：入口为打开识别能力；建议打开窗口即可，截图由窗口内按钮或再按需。
        // 为贴近「快捷键触发识别」，采用：show 窗口 + 自动 start_ocr_capture（与 Alt+S 对称）。
        // **锁定：快捷键 = 打开窗口并启动截图框选（RecognizeOnly）**；托盘仅打开窗口。
        let state = app_handle.state::<AppState>().inner().clone();
        crate::ui::ocr_window::start_ocr_capture(app_handle, state).await;
    });
}
```

**产品锁定（实现必须遵守）：**

| 入口 | 行为 |
|---|---|
| 快捷键 `ocr-recognize` | `show` OCR 窗口 + 启动截图框选（RecognizeOnly） |
| 托盘「文字识别」 | 仅 `show` OCR 窗口 |
| 窗口内「截图」按钮 | `start_ocr_capture` |

- [x] **步骤 3：单测 classify**

```rust
#[test]
fn classifies_ocr_recognize_shortcut() {
    let config = config_with(&[("ocr-recognize", "Alt+O")]);
    let shortcut = "Alt+O".parse::<Shortcut>().unwrap();
    assert_eq!(
        classify_shortcut(&shortcut, &config),
        Some(ShortcutAction::OcrRecognize)
    );
}
```

- [x] **步骤 4：测试 + Commit**（若 ocr_window 未就绪，handler 可暂 `todo` 编译不过——应同时建最小 stub）

最小 stub：

```rust
// ui/ocr_window.rs
pub fn open_ocr_window(app: &AppHandle) -> Result<(), String> {
    crate::app::window::show_ocr_window(app)
}
pub async fn start_ocr_capture(app: AppHandle, state: AppState) {
    // 任务 9 完整实现；此处可 log::warn 未实现 —— 不允许：本任务结束前至少能 show 窗。
}
```

```powershell
cd src-tauri; cargo test --lib app::shortcuts
git add src-tauri/src/app/shortcuts.rs src-tauri/src/ui/ocr_window.rs src-tauri/src/ui/mod.rs
git commit -m "feat(shortcuts): 注册 ocr-recognize 全局快捷键动作"
```

---

## 任务 7：`CapturePurpose` + `submit_capture_region` 分叉

**文件：**
- 修改：`src-tauri/src/app/state.rs`
- 修改：`src-tauri/src/ui/overlay.rs`
- 修改：`src-tauri/src/ui/ocr_popup.rs`

- [x] **步骤 1：State API**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CapturePurpose {
    #[default]
    Translate,
    RecognizeOnly,
}

// AppState 字段
capture_purpose: Arc<Mutex<CapturePurpose>>,

pub fn set_capture_purpose(&self, purpose: CapturePurpose) -> Result<(), String> { ... }
pub fn capture_purpose(&self) -> CapturePurpose { ... } // 锁毒化回退 Translate
```

`start_translation_from_ocr` 在 `try_begin_capture` 成功后：

```rust
let _ = state.set_capture_purpose(CapturePurpose::Translate);
```

- [x] **步骤 2：分叉 submit**

在 `submit_capture_region` recognize 完成后：

```rust
let purpose = state.capture_purpose();
match purpose {
    CapturePurpose::Translate => {
        // 现有：show_translation_popup + start_translation_from_input
        let result = recognize_region(...).await;
        // 现逻辑
    }
    CapturePurpose::RecognizeOnly => {
        let result = recognize_cropped_full(...).await;
        let _ = state.finish_capture();
        match result {
            Ok(payload) => {
                let _ = crate::app::window::show_ocr_window(&app);
                let _ = app.emit("ocr:recognize-result", &payload);
            }
            Err(error) => {
                let msg = crate::ui::ocr_popup::friendly_ocr_error(error);
                let _ = app.emit("ocr:recognize-failed", msg);
                // 可选：也 show ocr 窗
                let _ = crate::app::window::show_ocr_window(&app);
            }
        }
    }
}
```

取消截图：不发 failed 风暴；ocr 窗保持 idle。

- [x] **步骤 3：修正 friendly 文案中的 Alt+O → 中性或 Alt+S**

- [x] **步骤 4：`cargo test --lib` + Commit**

```powershell
git commit -m "feat(ocr): CapturePurpose 分叉纯识别与截图翻译路径"
```

---

## 任务 8：OCR 窗口壳（Tauri window + capabilities）

**文件：**
- 修改：`src-tauri/src/app/window.rs`
- 修改：`src-tauri/capabilities/default.json`
- 修改：`src-tauri/src/lib.rs`（setup ensure）

- [x] **步骤 1：window API**

```rust
pub const OCR_LABEL: &str = "ocr";
pub const OCR_URL: &str = "ocr.html";

pub fn ensure_ocr_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(w) = app.get_webview_window(OCR_LABEL) {
        return Ok(w);
    }
    let window = WebviewWindowBuilder::new(app, OCR_LABEL, WebviewUrl::App(OCR_URL.into()))
        .title("Shizi 文字识别")
        .inner_size(960.0, 640.0)
        .min_inner_size(720.0, 480.0)
        .resizable(true)
        .center()
        .visible(false)
        .build()
        .map_err(|e| format!("创建文字识别窗口失败: {e}"))?;
    close_to_hide(&window);
    attach_app_shortcut_focus_listener(&window, app);
    Ok(window)
}

pub fn show_ocr_window(app: &AppHandle) -> Result<(), String> {
    let window = ensure_ocr_window(app)?;
    window.show().map_err(|e| e.to_string())?;
    window.unminimize().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}
```

- [x] **步骤 2：capabilities**

```json
"windows": ["main", "settings", "screenshot-overlay", "ocr"],
"permissions": [
  ...
  "dialog:allow-open"
]
```

- [x] **步骤 3：setup 中 `ensure_ocr_window`（与 settings 一样预创建 hidden）**

- [x] **步骤 4：Commit**

```powershell
git commit -m "feat(window): 增加独立文字识别 ocr 窗口壳"
```

---

## 任务 9：OCR commands（文件/剪贴板/截图启动）

**文件：**
- 修改：`src-tauri/src/ui/ocr_window.rs`
- 修改：`src-tauri/src/lib.rs` invoke_handler
- 修改：`src-tauri/src/core/selection/clipboard.rs` 或新建 `core/selection/clipboard_image.rs`

- [x] **步骤 1：剪贴板读图**

```rust
// 使用 arboard::Clipboard::get_image()
// ImageData { width, height, bytes } 为 RGBA
pub fn read_clipboard_image() -> Result<Option<CapturedImage>, String> {
    let mut cb = Clipboard::new().map_err(|e| e.to_string())?;
    match cb.get_image() {
        Ok(img) => Ok(Some(CapturedImage {
            bytes: img.bytes.into_owned(),
            width: img.width as u32,
            height: img.height as u32,
            format: CapturedImageFormat::Rgba8,
        })),
        Err(arboard::Error::ContentNotAvailable) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
```

单测：无系统剪贴板环境可只测 `CapturedImage` 组装纯函数。

- [x] **步骤 2：文件解码**

```rust
pub fn load_image_file_bytes(bytes: &[u8]) -> Result<CapturedImage, OcrError> {
    let dyn_img = image::load_from_memory(bytes)
        .map_err(|e| OcrError::ImageConversionFailed(e.to_string()))?;
    let rgba = dyn_img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Ok(CapturedImage {
        bytes: rgba.into_raw(),
        width: w,
        height: h,
        format: CapturedImageFormat::Rgba8,
    })
}
```

- [x] **步骤 3：commands**

```rust
#[tauri::command]
pub fn open_ocr_window(app: AppHandle) -> Result<(), String> {
    show_ocr_window(&app)
}

#[tauri::command]
pub async fn start_ocr_capture(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    // 与 start_translation_from_ocr 类似，但不检查 translation_busy
    // try_begin_capture → capture_screen → set purpose RecognizeOnly → open_overlay
    ...
    Ok(())
}

#[tauri::command]
pub async fn recognize_clipboard_image(
    state: State<'_, AppState>,
) -> Result<RecognizeImageResponse, String> {
    let image = read_clipboard_image()
        .map_err(|e| e)?
        .ok_or_else(|| "剪贴板中没有图片".to_string())?;
    let config = state.config_store.get().map_err(|e| e.to_string())?;
    recognize_image_full(image, OcrHints::default(), &config.ocr_services, None)
        .await
        .map_err(|e| friendly_ocr_error(OcrTranslationError::from(e)))
}

#[tauri::command]
pub async fn pick_and_recognize_image(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<RecognizeImageResponse>, String> {
    use tauri_plugin_dialog::DialogExt;
    let path = app.dialog().file()
        .add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp"])
        .blocking_pick_file();
    let Some(path) = path else { return Ok(None); }; // 用户取消
    let path = path.into_path().map_err(|e| e.to_string())?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let image = load_image_file_bytes(&bytes).map_err(|e| e.to_string())?;
    let config = state.config_store.get().map_err(|e| e.to_string())?;
    let resp = recognize_image_full(image, OcrHints::default(), &config.ocr_services, None)
        .await
        .map_err(|e| friendly_ocr_error(OcrTranslationError::from(e)))?;
    Ok(Some(resp))
}
```

`blocking_pick_file` 在 async command 中应 `spawn_blocking` 或使用 dialog 的异步 API，避免卡 executor：

```rust
let app2 = app.clone();
let path = tauri::async_runtime::spawn_blocking(move || {
    app2.dialog().file()
        .add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp"])
        .blocking_pick_file()
}).await.map_err(|e| e.to_string())?;
```

- [x] **步骤 4：注册 handler + Commit**

```powershell
git commit -m "feat(ocr): 文字识别 commands（截图/文件/剪贴板）"
```

---

## 任务 10：托盘菜单 + i18n 键（后端托盘）

**文件：**
- 修改：`src-tauri/src/app/tray.rs`
- 修改：`src-tauri/src/ui/i18n.rs`
- 修改：`frontend/src/i18n/locales/zh-CN.json`、`en-US.json`（及内置包若需）

- [x] **步骤 1：托盘项**

```rust
let ocr_item = MenuItem::with_id(app, "ocr", "文字识别", true, None::<&str>)?;
let menu = Menu::with_items(app, &[&translate_item, &ocr_item, &settings_item, &quit_item])?;

// on_menu_event
"ocr" => { let _ = show_ocr_window(app); }

// TrayI18nHandles 增加 ocr: MenuItem
```

- [x] **步骤 2：i18n apply**

```rust
handles.ocr.set_text(&messages["tray.ocr"])?;
// window title key: window.ocrTitle
```

内置 messages（`core/i18n` 的 zh-CN/en-US 嵌入或前端 JSON——托盘走后端 resolve_messages）：

确认 `core/i18n` 内置 JSON 路径，在 **后端内置语言包** 与 **frontend locales** 同步加：

```json
"tray.ocr": "文字识别",
"window.ocrTitle": "Shizi 文字识别"
```

en-US：`"Text Recognition"` / `"Shizi OCR"`。

- [x] **步骤 3：Commit**

```powershell
git commit -m "feat(tray): 托盘增加文字识别入口并接入 i18n"
```

---

## 任务 11：前端快捷键设置项

**文件：**
- 修改：`frontend/src/settings/stores/settings.ts`
- 修改：`frontend/src/settings/panels/ShortcutPanel.vue`
- 修改：`frontend/src/lib/config.test.ts`、`settings.test.ts`（如有默认 bindings 断言）
- 修改：`frontend/src/i18n/locales/zh-CN.json`、`en-US.json`

- [x] **步骤 1：默认 binding**

在 `createDefaultSettings` 的 `shortcut.bindings` 中，于 `translate-screenshot` 后插入：

```ts
{
  id: 'ocr-recognize',
  label: '文字识别',
  description: '打开文字识别窗口并框选屏幕区域识别文字。',
  keys: 'Alt+O',
},
```

`ShortcutPanel.vue`：

```ts
const GLOBAL_IDS = new Set([
  'translate-selection',
  'translate-clipboard',
  'translate-screenshot',
  'ocr-recognize',
])
```

i18n：

```json
"settings.shortcut.ocr-recognize.label": "文字识别",
"settings.shortcut.ocr-recognize.description": "打开文字识别并框选区域识别文字（不自动翻译）。"
```

- [x] **步骤 2：测试**

```powershell
cd <repo>; npm run test -- --run src/settings/stores/settings.test.ts src/lib/config.test.ts
```

更新任何硬编码「bindings 长度/id 列表」的断言。

- [x] **步骤 3：Commit**

```powershell
git commit -m "feat(settings): 全局快捷键增加文字识别 ocr-recognize"
```

---

## 任务 12：前端 OCR 窗口页面

**文件：**
- 修改：`frontend/vite.config.ts`（input.ocr）
- 创建：`frontend/ocr.html`
- 创建：`frontend/src/ocr/main.ts`
- 创建：`frontend/src/ocr/OcrWindow.vue`
- 可选：`frontend/src/ocr/types.ts`
- i18n 键：`ocr.*`

- [x] **步骤 1：Vite 入口**

```ts
input: {
  settings: resolve(frontendDir, 'settings.html'),
  translate: resolve(frontendDir, 'translate.html'),
  ocr: resolve(frontendDir, 'ocr.html'),
},
```

`ocr.html` 同 settings 结构，script → `/src/ocr/main.ts`。

- [x] **步骤 2：状态机**

```ts
type Status = 'idle' | 'loading' | 'success' | 'error'
// 字段：previewUrl (data:image/png;base64,...), text, meta, errorMessage, engineSummary
```

- [x] **步骤 3：UI 布局（Tailwind，与 settings 风格接近）**

- 顶栏：`截图` | `打开文件` | `从剪贴板`；右侧引擎摘要（`meta.engine` + `meta.model`）
- 主区 grid 两列：左 `<img>` 预览；右 textarea/readonly + `复制`
- 底栏 meta：原图 WxH、送模 WxH、PNG bytes、耗时、HTTP、engine、model
- loading 时三按钮 disabled

- [x] **步骤 4：接线**

```ts
invoke('start_ocr_capture')
invoke('pick_and_recognize_image') // null = 取消
invoke('recognize_clipboard_image')
listen('ocr:recognize-result', handler)
listen('ocr:recognize-failed', handler)
// 复制：navigator.clipboard.writeText(text)
// 语言：listen interface-language:changed + setTitle(t('window.ocrTitle'))
```

取消文件/剪贴板无图：toast 或底栏轻提示，status 保持 idle（或短暂 error 文案）。

- [x] **步骤 5：typecheck**

```powershell
npm run typecheck
npm run build
```

- [x] **步骤 6：Commit**

```powershell
git commit -m "feat(ocr): 文字识别窗口前端页面（截图/文件/剪贴板）"
```

---

## 任务 13：回归与文档收尾

**文件：**
- `README.md`（当前能力一句）
- `docs/architecture/screenshot-ocr-architecture.md`（若存在：补纯识别分叉）
- `docs/superpowers/specs/2026-07-15-ocr-window-logging-quality-design.md` 状态 → 已实现
- 本 plan 复选框在执行时勾选

- [x] **步骤 1：自动化**

```powershell
cd src-tauri; cargo test
cd ..; npm run test; npm run typecheck; npm run build
```

- [x] **步骤 2：手动验收清单（执行者勾选）**

1. 托盘「文字识别」→ 仅开窗口  
2. `Alt+O` → 开窗口 + overlay 框选 → 文本与 meta  
3. 窗口内文件 / 剪贴板路径  
4. `logLevel=debug`：日志含 prompt、全文、尺寸；**无** API Key、无 base64 图  
5. Vision 请求体 `detail=high`（单测已锁）  
6. `Alt+S` 仍 OCR→翻译弹窗，不进 OCR 窗  
7. 关闭 OCR 窗为 hide，托盘再开状态可继续  

- [x] **步骤 3：文档 commit**

```powershell
git commit -m "docs: 同步文字识别窗口与 OCR 日志质量说明"
```

---

## 自检（写作时已核对）

| Spec 需求 | 任务 |
|---|---|
| 独立 OCR 窗口布局 | 8, 12 |
| 截图/文件/剪贴板 | 7, 9, 12 |
| 快捷键 + 托盘 | 5, 6, 10, 11 |
| Debug/info 日志 + redact 修复 | 2, 3, 4 |
| `detail=high` | 1 |
| `Alt+S` 语义不变 | 7（purpose 分叉） |
| 无一键翻译/历史 | 刻意不做 |
| 单测 detail/meta/快捷键 | 1–5 |
| 前端状态与 typecheck | 11–12 |

**占位符扫描：** 无「TODO/待定」步骤；dialog 异步、`CapturePurpose`、快捷键行为已锁定。

**类型一致性：**

- 快捷键 id 全程 `ocr-recognize`
- 事件名 `ocr:recognize-result` / `ocr:recognize-failed`
- 窗口 label `ocr` / URL `ocr.html`
- 响应类型 `RecognizeImageResponse` + `OcrRunMeta`（camelCase）
- `CapturePurpose::{Translate, RecognizeOnly}`

---

## 风险备忘（执行时）

1. **默认 `Alt+O` 与历史心智：** 截图翻译已是 `Alt+S`；若用户手动把 screenshot 改回 `Alt+O`，与 ocr-recognize 冲突由现有重复键检测拦截。  
2. **Vision 双 encode：** 可接受；若耗时明显再优化为单次 encode 注入引擎。  
3. **dialog blocking：** 必须 `spawn_blocking`。  
4. **大图 base64 IPC：** 预览可再缩最长边 1920；若卡顿再加。
