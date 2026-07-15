# 文字识别 · 重新识别 · Vision 请求诊断日志 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [x]`）语法来跟踪进度。

**目标：** 文字识别窗口支持对当前源图「重新识别」；Vision OCR 在 debug 下输出完整请求诊断（URL/脱敏头/参数摘要），永不写 API Key 明文与 base64 图正文。

**架构：** 路径 A——`AppState` 进程内单槽 `last_ocr_image` + `rerecognize_last_image` command。platform 的 `recognize_image_full` / `recognize_cropped_full` 成功时返回 `RecognizeImageFullResult { response, source_image }`，**ui 层**写缓存；翻译路径不写。Vision 请求体经纯函数 `sanitize_request_body_for_log` 脱敏后再 debug 输出。

**技术栈：** Rust（Tauri 2、serde_json、log、reqwest）、Vue 3 + TypeScript、cargo test、vue-tsc

**规格来源：** `docs/superpowers/specs/2026-07-16-ocr-rerecognize-request-logging-design.md`

---

## 与 spec 的实现澄清

1. **缓存仅内存单槽**：`Arc<Mutex<Option<CapturedImage>>>`，不落盘、不跨重启；成功覆盖、失败不清除。
2. **仅纯识别成功写缓存**：`recognize_clipboard_image` / `pick_and_recognize_image` / `submit_capture_region` 的 `RecognizeOnly` 成功分支 / `rerecognize_last_image` 成功后；**不写** `recognize_region` 翻译路径。
3. **platform 不持 AppState**：`RecognizeImageFullResult` 把 `source_image` 交回 ui；IPC 只序列化 `response`（`RecognizeImageResponse` 不变）。
4. **`clone_last_ocr_image` 非 take**：可多次重新识别同一张图。
5. **重新识别不占 capture 锁、不查 translation_busy**。
6. **文案中文硬编码**，本轮不做完整 i18n / 历史 / 一键翻译。
7. **debug 完整请求诊断；info 仍简要完成日志**。

---

## 文件结构

| 文件 | 职责 |
|---|---|
| 修改 `src-tauri/src/core/ocr/vision_openai.rs` | `sanitize_request_body_for_log`、`format_auth_header_for_log`；`recognize` 内 debug 请求诊断；响应 body 长度/usage；单测 |
| 修改 `src-tauri/src/core/ocr/meta.rs` | `RecognizeImageFullResult { response, source_image }`（不 Serialize 给前端） |
| 修改 `src-tauri/src/core/ocr/mod.rs` | 导出 `RecognizeImageFullResult` |
| 修改 `src-tauri/src/platform/windows/mod.rs` | `recognize_image_full` / `recognize_cropped_full` 返回 `RecognizeImageFullResult`；入口 clone 源图 |
| 修改 `src-tauri/src/platform/unsupported.rs` | 签名对齐 |
| 修改 `src-tauri/src/app/state.rs` | `last_ocr_image` + `set_last_ocr_image` / `clone_last_ocr_image` + 单测 |
| 修改 `src-tauri/src/ui/ocr_window.rs` | 成功路径写缓存；`rerecognize_last_image` command + 无缓存单测辅助 |
| 修改 `src-tauri/src/ui/overlay.rs` | `RecognizeOnly` 成功：`set_last_ocr_image` 后 emit `response` |
| 修改 `src-tauri/src/lib.rs` | 注册 `rerecognize_last_image` |
| 修改 `frontend/src/ocr/OcrWindow.vue` | 「重新识别」按钮 + `hasLastImage` + loading 禁用 |

**刻意不改：** `Alt+S` 语义、翻译 `recognize_region`、历史 SQLite、完整 i18n、多图缓存、`RecognizeImageResponse` 的 IPC 字段形状。

---

### 任务 1：Vision 请求体脱敏纯函数 + 单测

**文件：**
- 修改：`src-tauri/src/core/ocr/vision_openai.rs`

- [x] **步骤 1：编写失败的测试**

在 `vision_openai.rs` 的 `#[cfg(test)] mod tests` 末尾新增：

```rust
#[test]
fn sanitize_request_body_redacts_data_url_keeps_structure() {
    let long_b64 = "A".repeat(200);
    let data_url = format!("data:image/png;base64,{long_b64}");
    let body = VisionOcrEngine::build_request_body("gpt-4o", "sys-prompt-full", &data_url);
    let sanitized = sanitize_request_body_for_log(&body);
    let s = sanitized.to_string();

    assert!(s.contains("[len="), "应含长度占位: {s}");
    assert!(!s.contains(&long_b64), "不得含原始 base64 片段");
    assert_eq!(sanitized["model"], "gpt-4o");
    assert_eq!(sanitized["stream"], false);
    assert_eq!(sanitized["max_tokens"], 2048);
    assert_eq!(sanitized["messages"][0]["content"], "sys-prompt-full");
    assert_eq!(sanitized["messages"][1]["content"][0]["text"], USER_HINT);
    assert_eq!(
        sanitized["messages"][1]["content"][1]["image_url"]["detail"],
        "high"
    );
    let url = sanitized["messages"][1]["content"][1]["image_url"]["url"]
        .as_str()
        .expect("url string");
    assert!(url.starts_with("data:image/png;base64,[len="));
    assert!(url.contains(&format!("[len={}]", long_b64.len())));
}

#[test]
fn sanitize_request_body_non_data_url_records_scheme_and_len() {
    let url = "https://example.com/img.png?token=secret";
    let body = VisionOcrEngine::build_request_body("m", "s", url);
    let sanitized = sanitize_request_body_for_log(&body);
    let out = sanitized["messages"][1]["content"][1]["image_url"]["url"]
        .as_str()
        .unwrap();
    assert!(out.contains("https"), "应含 scheme: {out}");
    assert!(out.contains(&format!("len={}", url.len())), "应含 len: {out}");
    assert!(!out.contains("secret"), "不得含 query 明文 token");
}

#[test]
fn format_auth_header_redacts_api_key() {
    let key = "sk-abcdef12345678";
    let header = format_auth_header_for_log(key);
    assert_eq!(header, format!("Bearer {}", crate::core::logging::redact_api_key(key)));
    assert!(!header.contains("abcdef12345678"));
    assert!(!header.contains(key));
}
```

- [x] **步骤 2：运行测试验证失败**

```powershell
cd src-tauri; cargo test --lib ocr::vision_openai::tests::sanitize_request_body_redacts_data_url_keeps_structure -- --nocapture
```

预期：FAIL（`sanitize_request_body_for_log` / `format_auth_header_for_log` 未定义）。

- [x] **步骤 3：最少实现纯函数**

在 `vision_openai.rs` 中、`VisionOcrEngine` impl 块之外（模块级 `pub(crate)`）新增：

```rust
/// 将请求体中的 image_url 替换为长度占位，供 debug 日志使用。禁止 dump 原始 base64。
pub(crate) fn sanitize_request_body_for_log(body: &serde_json::Value) -> serde_json::Value {
    let mut out = body.clone();
    if let Some(messages) = out.get_mut("messages").and_then(|m| m.as_array_mut()) {
        for msg in messages.iter_mut() {
            if let Some(content) = msg.get_mut("content").and_then(|c| c.as_array_mut()) {
                for part in content.iter_mut() {
                    if part.get("type").and_then(|t| t.as_str()) == Some("image_url") {
                        if let Some(url_val) = part
                            .pointer_mut("/image_url/url")
                            .filter(|v| v.is_string())
                        {
                            let original = url_val.as_str().unwrap_or("").to_string();
                            *url_val = serde_json::Value::String(sanitize_image_url_for_log(&original));
                        }
                    }
                }
            }
        }
    }
    out
}

fn sanitize_image_url_for_log(url: &str) -> String {
    const PREFIX: &str = "data:image/png;base64,";
    if let Some(rest) = url.strip_prefix(PREFIX) {
        return format!("{PREFIX}[len={}]", rest.len());
    }
    if let Some(rest) = url.strip_prefix("data:") {
        // 其它 data URL：保留 media type 前缀到第一个逗号后的 len
        if let Some((meta, payload)) = rest.split_once(',') {
            return format!("data:{meta},[len={}]", payload.len());
        }
        return format!("data:[len={}]", rest.len());
    }
    // 非 data URL：仅 scheme + 总长度，避免 query 明文
    let scheme = url.split(':').next().unwrap_or("unknown");
    format!("{scheme}:[len={}]", url.len())
}

/// Authorization 头日志：`Bearer {redact_api_key}`。
pub(crate) fn format_auth_header_for_log(api_key: &str) -> String {
    format!(
        "Bearer {}",
        crate::core::logging::redact_api_key(api_key)
    )
}
```

说明：若 `pointer_mut` 在当前 serde_json 版本不便，可改为逐层 `get_mut` 导航；保持语义一致即可。

- [x] **步骤 4：运行测试验证通过**

```powershell
cd src-tauri; cargo test --lib ocr::vision_openai::tests -- --nocapture
```

预期：全部 PASS（含既有 detail/high 测试）。

- [x] **步骤 5：Commit**

```bash
git add src-tauri/src/core/ocr/vision_openai.rs
git commit -m "test(ocr): 增加 Vision 请求体脱敏与 Authorization 日志单测"
```

（若实现与测试同 commit 更清晰，可用：`feat(ocr): Vision 请求体脱敏纯函数`）

---

### 任务 2：接入 recognize 的 debug 请求诊断日志

**文件：**
- 修改：`src-tauri/src/core/ocr/vision_openai.rs`（`OcrEngine::recognize`）

- [x] **步骤 1：替换发送前日志**

在 `recognize` 中，将现有简要 debug：

```rust
log::debug!(
    "Vision OCR 请求: endpoint={} model={} prompt_len={}",
    endpoint,
    self.config.model,
    system.chars().count()
);
log::debug!("Vision OCR system prompt: {system}");
let body = Self::build_request_body(&self.config.model, system, &data_url);
```

改为（**先 build body，再 debug 诊断**）：

```rust
let body = Self::build_request_body(&self.config.model, system, &data_url);
let endpoint_owned = endpoint.clone(); // endpoint() 返回 String，可直接用
log::debug!("Vision OCR 请求诊断: POST {endpoint}");
log::debug!(
    "Vision OCR 请求头: Authorization={}, Content-Type=application/json",
    format_auth_header_for_log(&self.config.api_key)
);
log::debug!(
    "Vision OCR 请求体: {}",
    sanitize_request_body_for_log(&body)
);
// 保留 system 全文 debug（配置非 secret，与现状一致）
log::debug!("Vision OCR system prompt: {system}");
```

注意：`.post(endpoint)` 需在 log 之后；`endpoint` 为 `String`，`post` 可 `post(&endpoint)` 或 `post(endpoint.clone())`。

- [x] **步骤 2：响应侧 debug 增强**

在成功 `parse_success_content` 前后，info 完成日志保持；debug 增加 body 长度与 usage：

```rust
let body_len = text.len();
log::debug!("Vision OCR 响应: status={status} body_len={body_len}");
if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
    if let Some(usage) = v.get("usage") {
        log::debug!("Vision OCR usage: {usage}");
    }
}
let content = Self::parse_success_content(&text)?;
log::info!(
    "Vision OCR 完成: status={} latency_ms={} text={}",
    status,
    start.elapsed().as_millis(),
    crate::core::logging::redact_text(
        &content,
        crate::core::logging::effective_redact_level()
    )
);
```

- [x] **步骤 3：运行既有测试**

```powershell
cd src-tauri; cargo test --lib ocr::vision_openai -- --nocapture
```

预期：PASS（日志无断言，纯函数测仍绿）。

- [x] **步骤 4：Commit**

```bash
git add src-tauri/src/core/ocr/vision_openai.rs
git commit -m "feat(ocr): Vision debug 输出完整请求诊断与响应概要"
```

---

### 任务 3：`RecognizeImageFullResult` + platform 返回类型改造

**文件：**
- 修改：`src-tauri/src/core/ocr/meta.rs`
- 修改：`src-tauri/src/core/ocr/mod.rs`
- 修改：`src-tauri/src/platform/windows/mod.rs`
- 修改：`src-tauri/src/platform/unsupported.rs`
- 修改：`src-tauri/src/ui/ocr_window.rs`（暂只取 `.response`，缓存下一任务）
- 修改：`src-tauri/src/ui/overlay.rs`（暂只取 `.response`）

- [x] **步骤 1：编写失败的测试（meta 类型）**

在 `meta.rs` 的 `#[cfg(test)]` 中新增（或新建简单构造测）：

```rust
#[test]
fn full_result_holds_response_and_source_image() {
    use crate::core::capture::{CapturedImage, CapturedImageFormat};
    let img = CapturedImage {
        bytes: vec![0; 4],
        width: 1,
        height: 1,
        format: CapturedImageFormat::Bgra8,
    };
    let response = RecognizeImageResponse {
        text: "hi".into(),
        meta: OcrRunMeta {
            engine: "mock".into(),
            model: None,
            source_width: 1,
            source_height: 1,
            sent_width: 1,
            sent_height: 1,
            png_bytes: None,
            latency_ms: 0,
            http_status: None,
            scaled: false,
        },
        preview_png_base64: "eA==".into(),
    };
    let full = RecognizeImageFullResult {
        response: response.clone(),
        source_image: img.clone(),
    };
    assert_eq!(full.response.text, "hi");
    assert_eq!(full.source_image.width, 1);
    // 确认 RecognizeImageResponse 仍可独立序列化（IPC 形状不变）
    let v = serde_json::to_value(&full.response).unwrap();
    assert!(v.get("previewPngBase64").is_some());
    assert!(v.get("source_image").is_none());
}
```

注意：`RecognizeImageResponse` 当前未 derive `Clone`——若未实现，先给 `RecognizeImageResponse` 加 `Clone`（与 `OcrRunMeta` 一致），或测试里不 `response.clone()`。

- [x] **步骤 2：运行测试验证失败**

```powershell
cd src-tauri; cargo test --lib ocr::meta::tests::full_result_holds_response_and_source_image -- --nocapture
```

预期：FAIL（类型未定义）。

- [x] **步骤 3：定义类型并改 platform 签名**

`meta.rs` 新增（**不要** `Serialize` 整个 FullResult，避免误把图走 IPC）：

```rust
/// platform 纯识别成功结果：IPC 用 response；source_image 供 ui 写 last_ocr_image。
#[derive(Debug, Clone)]
pub struct RecognizeImageFullResult {
    pub response: RecognizeImageResponse,
    /// 源图拷贝，勿写入日志
    pub source_image: crate::core::capture::CapturedImage,
}
```

若 `RecognizeImageResponse` 无 `Clone`，加上 `#[derive(..., Clone)]`。

`mod.rs` 导出：

```rust
pub use meta::{OcrRunMeta, RecognizeImageFullResult, RecognizeImageResponse};
```

`windows/mod.rs` 的 `recognize_image_full`：

```rust
pub async fn recognize_image_full(
    image: CapturedImage,
    hints: OcrHints,
    ocr_services: &[crate::core::config::types::OcrServiceInstanceConfig],
    _model_hint: Option<String>,
) -> Result<RecognizeImageFullResult, OcrError> {
    let start = Instant::now();
    let source_image = image.clone(); // 入口缓存用
    let source_width = image.width;
    let source_height = image.height;
    // ... 其余逻辑不变，仍 move image 进 engine ...
    Ok(RecognizeImageFullResult {
        response: RecognizeImageResponse {
            text,
            meta,
            preview_png_base64: preview_b64,
        },
        source_image,
    })
}
```

`recognize_cropped_full`：

```rust
) -> Result<RecognizeImageFullResult, OcrTranslationError> {
    let cropped = frame.crop(...)?;
    // ...
    recognize_image_full(cropped, hints, ocr_services, None)
        .await
        .map_err(Into::into)
}
```

`unsupported.rs` 同步返回类型（仍 `Err(...)`）。

更新 platform 内既有测试：期望类型仍为 `Err`，签名匹配即可。

- [x] **步骤 4：更新 ui 调用方（只透传 response）**

`ocr_window.rs`：

```rust
let full = recognize_image_full(...).await.map_err(...)?;
Ok(full.response)
// pick: Ok(Some(full.response))
```

`overlay.rs` RecognizeOnly：

```rust
Ok(full) => {
    let _ = show_ocr_window(&app);
    if let Err(e) = app.emit("ocr:recognize-result", &full.response) {
        ...
    }
}
```

- [x] **步骤 5：运行测试**

```powershell
cd src-tauri; cargo test --lib -- --nocapture
```

预期：PASS（编译通过 + 相关测绿）。

- [x] **步骤 6：Commit**

```bash
git add src-tauri/src/core/ocr/meta.rs src-tauri/src/core/ocr/mod.rs \
  src-tauri/src/platform/windows/mod.rs src-tauri/src/platform/unsupported.rs \
  src-tauri/src/ui/ocr_window.rs src-tauri/src/ui/overlay.rs
git commit -m "refactor(ocr): recognize_image_full 返回 FullResult 携带源图"
```

---

### 任务 4：AppState `last_ocr_image` 缓存

**文件：**
- 修改：`src-tauri/src/app/state.rs`

- [x] **步骤 1：编写失败的测试**

在 `state.rs` 的 `#[cfg(test)]` 中、仿照 `last_translation_input_*` 新增：

```rust
#[test]
fn last_ocr_image_none_when_empty() {
    let state = app_state();
    let got = state.clone_last_ocr_image().expect("clone");
    assert!(got.is_none());
}

#[test]
fn last_ocr_image_set_and_clone_round_trip() {
    use crate::core::capture::{CapturedImage, CapturedImageFormat};
    let state = app_state();
    let img = CapturedImage {
        bytes: vec![1, 2, 3, 4],
        width: 1,
        height: 1,
        format: CapturedImageFormat::Bgra8,
    };
    state.set_last_ocr_image(img.clone()).expect("set");
    let a = state.clone_last_ocr_image().expect("clone1");
    let b = state.clone_last_ocr_image().expect("clone2");
    assert_eq!(a, Some(img.clone()));
    assert_eq!(b, Some(img)); // clone 非 take，可多次
}

#[test]
fn last_ocr_image_overwrite_keeps_latest() {
    use crate::core::capture::{CapturedImage, CapturedImageFormat};
    let state = app_state();
    let first = CapturedImage {
        bytes: vec![0; 4],
        width: 1,
        height: 1,
        format: CapturedImageFormat::Bgra8,
    };
    let second = CapturedImage {
        bytes: vec![9; 8],
        width: 2,
        height: 1,
        format: CapturedImageFormat::Rgba8,
    };
    state.set_last_ocr_image(first).expect("first");
    state.set_last_ocr_image(second.clone()).expect("second");
    assert_eq!(state.clone_last_ocr_image().unwrap(), Some(second));
}
```

- [x] **步骤 2：运行测试验证失败**

```powershell
cd src-tauri; cargo test --lib app::state::tests::last_ocr_image_none_when_empty -- --nocapture
```

预期：FAIL（方法不存在）。

- [x] **步骤 3：实现字段与方法**

在 `AppState` 结构体中增加字段（注释说明用途）：

```rust
// 最近一次纯识别成功的源图（进程内单槽），供 OCR 窗「重新识别」。
// 仅 recognize_image_full 成功路径由 ui 写入；翻译路径不写；不落盘。
last_ocr_image: Arc<Mutex<Option<CapturedImage>>>,
```

`new` 中初始化：`last_ocr_image: Arc::new(Mutex::new(None)),`

方法（放在 `set_last_translation_input` 附近）：

```rust
pub fn set_last_ocr_image(&self, image: CapturedImage) -> Result<(), String> {
    let mut slot = self
        .last_ocr_image
        .lock()
        .map_err(|_| "OCR 图像缓存锁已损坏".to_string())?;
    *slot = Some(image);
    Ok(())
}

/// 克隆缓存图供重新识别；无缓存返回 `Ok(None)`（非 take）。
pub fn clone_last_ocr_image(&self) -> Result<Option<CapturedImage>, String> {
    let slot = self
        .last_ocr_image
        .lock()
        .map_err(|_| "OCR 图像缓存锁已损坏".to_string())?;
    Ok(slot.clone())
}
```

- [x] **步骤 4：运行测试验证通过**

```powershell
cd src-tauri; cargo test --lib app::state::tests::last_ocr_image -- --nocapture
```

预期：PASS。

- [x] **步骤 5：Commit**

```bash
git add src-tauri/src/app/state.rs
git commit -m "feat(app): AppState 增加 last_ocr_image 纯识别源图缓存"
```

---

### 任务 5：ui 写缓存 + `rerecognize_last_image` command

**文件：**
- 修改：`src-tauri/src/ui/ocr_window.rs`
- 修改：`src-tauri/src/ui/overlay.rs`
- 修改：`src-tauri/src/lib.rs`

- [x] **步骤 1：编写失败的测试（无缓存错误文案）**

在 `ocr_window.rs` 的 `#[cfg(test)]` 中新增对错误常量/辅助的断言：

```rust
#[test]
fn rerecognize_no_image_error_message() {
    assert_eq!(
        RERECOGNIZE_NO_IMAGE_MSG,
        "没有可重新识别的图像，请先截图、打开文件或从剪贴板识别。"
    );
}
```

并在模块级定义：

```rust
pub(crate) const RERECOGNIZE_NO_IMAGE_MSG: &str =
    "没有可重新识别的图像，请先截图、打开文件或从剪贴板识别。";
```

（先写测试引用常量 → 再加常量，或同一步骤内完成。）

另可在 `state` 已有测覆盖 empty；此处锁定用户可见文案与 spec 一致。

- [x] **步骤 2：实现 command 与写缓存**

**常量 + command**（`ocr_window.rs`）：

```rust
pub(crate) const RERECOGNIZE_NO_IMAGE_MSG: &str =
    "没有可重新识别的图像，请先截图、打开文件或从剪贴板识别。";

/// 对最近一次纯识别成功的源图再跑一遍 OCR。
#[tauri::command]
pub async fn rerecognize_last_image(
    state: State<'_, AppState>,
) -> Result<RecognizeImageResponse, String> {
    let image = state
        .clone_last_ocr_image()?
        .ok_or_else(|| RERECOGNIZE_NO_IMAGE_MSG.to_string())?;
    log::info!(
        "OCR 重新识别: {}x{}",
        image.width,
        image.height
    );
    let config = state.config_store.get().map_err(|e| e.to_string())?;
    let full = recognize_image_full(image, OcrHints::default(), &config.ocr_services, None)
        .await
        .map_err(|e| friendly_ocr_error(OcrTranslationError::from(e)))?;
    // 成功再覆盖缓存（同图 clone；失败不进入此处，保留旧缓存）
    if let Err(e) = state.set_last_ocr_image(full.source_image) {
        log::warn!("写入 last_ocr_image 失败: {e}");
    }
    Ok(full.response)
}
```

**剪贴板成功写缓存：**

```rust
pub async fn recognize_clipboard_image(
    state: State<'_, AppState>,
) -> Result<RecognizeImageResponse, String> {
    // ... 读图、读 config 不变 ...
    let full = recognize_image_full(image, OcrHints::default(), &config.ocr_services, None)
        .await
        .map_err(|e| friendly_ocr_error(OcrTranslationError::from(e)))?;
    if let Err(e) = state.set_last_ocr_image(full.source_image) {
        log::warn!("写入 last_ocr_image 失败: {e}");
    }
    Ok(full.response)
}
```

**文件成功写缓存（取消仍 `Ok(None)` 不写）：**

```rust
let full = recognize_image_full(...).await.map_err(...)?;
if let Err(e) = state.set_last_ocr_image(full.source_image) {
    log::warn!("写入 last_ocr_image 失败: {e}");
}
Ok(Some(full.response))
```

**overlay RecognizeOnly 成功：**

```rust
Ok(full) => {
    if let Err(e) = state.set_last_ocr_image(full.source_image) {
        log::warn!("写入 last_ocr_image 失败: {e}");
    }
    let _ = crate::app::window::show_ocr_window(&app);
    if let Err(e) = app.emit("ocr:recognize-result", &full.response) {
        log::warn!("emit ocr:recognize-result 失败: {e}");
    }
}
```

失败分支**不**调用 `set_last_ocr_image`。

**lib.rs：**

```rust
use ui::ocr_window::{
    open_ocr_window, pick_and_recognize_image, recognize_clipboard_image,
    rerecognize_last_image, start_ocr_capture,
};
// generate_handler! 中增加：
rerecognize_last_image,
```

- [x] **步骤 3：运行测试**

```powershell
cd src-tauri; cargo test --lib -- --nocapture
```

预期：PASS。可选再跑：

```powershell
cd src-tauri; cargo test --lib ui::ocr_window -- --nocapture
```

- [x] **步骤 4：Commit**

```bash
git add src-tauri/src/ui/ocr_window.rs src-tauri/src/ui/overlay.rs src-tauri/src/lib.rs
git commit -m "feat(ocr): 纯识别成功缓存源图并支持 rerecognize_last_image"
```

---

### 任务 6：前端「重新识别」按钮

**文件：**
- 修改：`frontend/src/ocr/OcrWindow.vue`

- [x] **步骤 1：状态与处理函数**

在 script 中增加：

```ts
const hasLastImage = ref(false)

// applySuccess 内成功后：
hasLastImage.value = true

// applyError 保持：不改 hasLastImage（失败保留上次可重试）

const canRerecognize = computed(() => hasLastImage.value && !isLoading.value)

async function onRerecognize(): Promise<void> {
  const apis = getTauriApis()
  if (!apis) {
    applyError('Tauri API 未就绪')
    return
  }
  status.value = 'loading'
  errorMessage.value = ''
  copyHint.value = ''
  try {
    const result = await apis.invoke<RecognizeImageResponse>('rerecognize_last_image')
    applySuccess(result)
  } catch (e) {
    applyError(String(e))
  }
}
```

截图路径成功走事件 `ocr:recognize-result` → 已有 `applySuccess` → 自动 `hasLastImage=true`。

- [x] **步骤 2：工具栏 UI**

在「从剪贴板」按钮后增加：

```vue
<Button
  variant="outline"
  size="sm"
  :disabled="!canRerecognize"
  @click="onRerecognize"
>
  重新识别
</Button>
```

说明：无缓存时 `canRerecognize` 为 false（禁用）；loading 时亦禁用。三入口仍 `:disabled="isLoading"`。

- [x] **步骤 3：类型检查**

```powershell
npm run typecheck
```

预期：无新增错误。

- [x] **步骤 4：Commit**

```bash
git add frontend/src/ocr/OcrWindow.vue
git commit -m "feat(ocr): 文字识别窗口增加重新识别按钮"
```

---

### 任务 7：全量验证与文档回填准备

**文件：**
- 可选修改：spec 状态字段（编码收尾时）、README 一句能力（协作规范第 2 条，**在 finish 阶段**做；本任务先跑验证）

- [x] **步骤 1：后端测试**

```powershell
cd src-tauri; cargo test --lib -- --nocapture
```

预期：全部 PASS。

- [x] **步骤 2：前端**

```powershell
npm run typecheck
npm run test
```

预期：PASS（无强制新 vitest；既有不破）。

- [x] **步骤 3：手动验收清单（开发者 / 代理在 dev 下勾选）**

| # | 步骤 | 期望 |
|---|---|---|
| 1 | 打开文字识别窗，无操作看「重新识别」 | 禁用 |
| 2 | 剪贴板/文件/截图成功一次 | 文本+meta+预览；按钮可用 |
| 3 | 点「重新识别」 | loading → 成功覆盖结果；预览可刷新 |
| 4 | 故意失败（断网/错 Key）后看缓存 | 错误条显示；预览/旧文本保留；仍可再点重新识别 |
| 5 | `logLevel=debug` 跑 Vision 一次 | 日志含 POST URL、脱敏 Authorization、sanitize 后 body（`[len=`）、响应 body_len/usage；**无**明文 key、无长 base64 |
| 6 | `logLevel=info` | 无整包请求体诊断；有完成摘要 |
| 7 | Alt+S 截图翻译 | 不依赖 OCR 窗缓存；翻译正常 |

- [x] **步骤 4：若验证中有编译修复，单独 commit；无则跳过**

编码全部完成后，按协作规范同步 README/spec 状态，再走 `finishing-a-development-branch`（**本 plan 阶段不执行 finish**）。

---

## 自检（对照 spec）

| Spec 需求 | 任务 |
|---|---|
| 重新识别当前源图 | 任务 4–6 |
| debug 完整请求诊断 | 任务 1–2 |
| 永不写 Key/base64 明文 | 任务 1 单测锁定 + 任务 2 使用 sanitize |
| 无缓存明确错误 | 任务 5 `RERECOGNIZE_NO_IMAGE_MSG` |
| loading/无缓存禁用按钮 | 任务 6 `canRerecognize` |
| 仅纯识别写缓存 | 任务 5 写入点；overlay 仅 RecognizeOnly |
| FullResult platform→ui | 任务 3 |
| 失败不覆盖缓存 | 任务 5 仅 Ok 路径 set |
| 中文硬编码 / 无历史 / 无一键翻译 | 任务 6 中文按钮；未安排其它任务 |
| Windows Media 无 HTTP 参数 | 任务 2 仅改 vision_openai |

**占位符扫描：** 无 TODO/待定步骤；代码块均为可粘贴实现。

**类型一致性：**
- `RecognizeImageFullResult { response, source_image }`
- `set_last_ocr_image` / `clone_last_ocr_image`
- `rerecognize_last_image` → `RecognizeImageResponse`
- 前端 invoke 名 `rerecognize_last_image`
- 常量 `RERECOGNIZE_NO_IMAGE_MSG` 与 spec 文案一致

---

## 风险与实现注意

1. **大图 clone**：入口 `image.clone()` + 缓存再持一份；单槽可接受。避免在日志中打印 `CapturedImage`/`preview_b64`。
2. **set 失败仅 warn**：缓存写失败不应让已成功的识别对前端失败。
3. **serde_json pointer_mut**：若编译器/版本问题，改用手写嵌套 `get_mut`。
4. **unsupported 平台**：签名改完即可；Windows 开发机跑功能验证。
