# 文字识别窗：PDF 首页 OCR + 会话级渠道切换 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 文字识别窗口支持打开 PDF（仅栅格化第 1 页后走现有 OCR），并支持会话级临时切换 OCR 渠道（不写配置、不改设置页启用状态）；剪贴板维持位图现状；`Alt+S` 翻译路径不受影响。

**架构：** 薄扩展现有识别链路。① `resolve_ocr_engine_for(services, override_id)`：有 id 则按实例映射且忽略 `enabled`，无 id 则沿用唯一启用规则。② OCR 窗 command 可选 `service_id`；截图纯识别经 `AppState.ocr_session_service_id` 槽传给 `RecognizeOnly` 提交路径。③ PDF：扩展文件过滤器 + 魔数/扩展名分支，WinRT `Windows.Data.Pdf` 渲染第 1 页为 `CapturedImage`，再进 `recognize_image_full`。

**技术栈：** Rust（Tauri 2、windows 0.58 WinRT Data.Pdf、image、serde）、Vue 3 + TypeScript + shadcn-vue Select、cargo test、vitest、vue-tsc

**规格来源：** `docs/superpowers/specs/2026-07-16-ocr-window-pdf-and-session-channel-design.md`

---

## 与 spec 的实现澄清

1. **不新建文档子系统**：PDF 只是打开文件路径上的分支，输出仍是 `CapturedImage`。
2. **`recognize_image_full` 第 4 参**：现为未使用的 `_model_hint: Option<String>`，本计划**改义**为 `service_id: Option<String>`（override），调用方传 `None` 或临时 id；`recognize_region`（翻译）**继续**内部 `resolve_ocr_engine` 仅 enabled，不读会话槽。
3. **会话槽**：`AppState.ocr_session_service_id: Arc<Mutex<Option<String>>>`，仅 `start_ocr_capture`（含 Alt+O 走 flow 时无 id → `None`）写入；`submit_capture_region(RecognizeOnly)` 读取后清除；`cancel_capture` 在 RecognizeOnly 时清除；`Translate` 路径永不读。
4. **临时切换不自动重跑**；loading 期间禁用渠道下拉与入口按钮。
5. **剪贴板**：只加可选 `service_id` 透传，不扩展 PDF/文件路径。
6. **页数提示**：`OcrRunMeta` 增加可选 `sourcePage` / `sourcePageCount`（仅 PDF 打开成功路径填充）；无则前端不展示。
7. **错误文案中文硬编码**，与现有 `friendly_ocr_error` 一致。

---

## 文件结构

| 文件 | 职责 |
|---|---|
| 修改 `src-tauri/src/core/ocr/mod.rs` | 新增 `OcrError` 变体（UnknownService / Pdf*）；`mod pdf_detect` |
| 创建 `src-tauri/src/core/ocr/pdf_detect.rs` | 纯函数：扩展名 + `%PDF` 魔数判断（无 WinRT） |
| 修改 `src-tauri/src/core/ocr/resolve.rs` | `resolve_ocr_engine_for`；`resolve_ocr_engine` 改为 `for(..., None)` 包装；by-id 单测 |
| 修改 `src-tauri/src/core/ocr/meta.rs` | `OcrRunMeta` 可选 `source_page` / `source_page_count` |
| 修改 `src-tauri/src/platform/windows/mod.rs` | `mod pdf`；`recognize_image_full` / `recognize_cropped_full` 用 `resolve_ocr_engine_for` + `service_id` |
| 创建 `src-tauri/src/platform/windows/pdf.rs` | WinRT 第 1 页 → `PdfFirstPage { image, page_count }` |
| 修改 `src-tauri/src/platform/unsupported.rs` | `render_pdf_first_page` 返回平台不支持；`recognize_*` 签名对齐 |
| 修改 `src-tauri/src/platform/mod.rs` | 导出 `render_pdf_first_page` / `PdfFirstPage` |
| 修改 `src-tauri/src/app/state.rs` | `ocr_session_service_id` 槽 + set/take/clear + 单测 |
| 修改 `src-tauri/src/ui/ocr_window.rs` | command 可选 `service_id`；PDF 分支；`start_ocr_capture` 写槽 |
| 修改 `src-tauri/src/ui/overlay.rs` | RecognizeOnly 读槽 resolve；finish/cancel 清槽 |
| 修改 `src-tauri/src/ui/ocr_popup.rs` | `friendly_ocr_error` 新变体文案 + 单测 |
| 修改 `src-tauri/Cargo.toml` | `windows` features 增加 `Data_Pdf`（及渲染所需 `Storage_Streams` 已有） |
| 修改 `frontend/src/ocr/types.ts` | meta 可选页字段；渠道选项类型 |
| 创建 `frontend/src/ocr/sessionChannel.ts` | 默认 id 选取纯函数 |
| 创建 `frontend/src/ocr/sessionChannel.test.ts` | vitest |
| 修改 `frontend/src/ocr/OcrWindow.vue` | 渠道 Select、传 `service_id`、config 监听、页数提示 |
| 修改 `README.md` / 架构短文（若有 OCR 窗段落） | 能力一句（编码收尾） |

**刻意不改：** 设置页 `enabled` 互斥、`Alt+S`/`recognize_region` 选引擎规则、剪贴板多格式、多页 PDF UI、vision 直传 PDF 字节、完整 i18n 键迁移。

**分层约束：** `core` **不**依赖 `platform` / WinRT。PDF 检测在 core；渲染在 platform。

---

### 任务 1：`OcrError` 新变体 + 友好文案

**文件：**
- 修改：`src-tauri/src/core/ocr/mod.rs`
- 修改：`src-tauri/src/ui/ocr_popup.rs`

- [x] **步骤 1：编写失败的测试**

在 `mod.rs` 的 `error_tests` 中追加：

```rust
#[test]
fn pdf_and_unknown_service_variants_display() {
    let u = OcrError::UnknownService("svc-x".into());
    assert!(u.to_string().contains("svc-x") || u.to_string().contains("渠道"));

    let open = OcrError::PdfOpenFailed("bad".into());
    assert!(open.to_string().contains("PDF") || open.to_string().contains("打开"));

    let empty = OcrError::PdfEmptyDocument;
    assert!(empty.to_string().contains("页") || empty.to_string().contains("PDF"));

    let render = OcrError::PdfRenderFailed("x".into());
    assert!(render.to_string().contains("渲染") || render.to_string().contains("PDF"));
}
```

在 `ocr_popup.rs` 的 `tests` 追加（编译期需 match 穷尽，先写期望文案）：

```rust
#[test]
fn friendly_unknown_service_and_pdf_errors() {
    assert!(
        friendly_ocr_error(OcrTranslationError::Ocr(OcrError::UnknownService(
            "abc".into()
        )))
        .contains("渠道")
    );
    assert!(
        friendly_ocr_error(OcrTranslationError::Ocr(OcrError::PdfOpenFailed(
            "x".into()
        )))
        .contains("PDF")
    );
    assert!(
        friendly_ocr_error(OcrTranslationError::Ocr(OcrError::PdfEmptyDocument))
            .contains("页")
    );
    assert!(
        friendly_ocr_error(OcrTranslationError::Ocr(OcrError::PdfRenderFailed(
            "x".into()
        )))
        .contains("渲染")
    );
}
```

- [x] **步骤 2：运行测试验证失败**

```powershell
cd src-tauri; cargo test --lib ocr::error_tests::pdf_and_unknown_service_variants_display -- --nocapture
```

预期：FAIL（变体未定义）。

- [x] **步骤 3：最少实现**

在 `OcrError` 枚举中增加：

```rust
#[error("OCR 渠道不存在：{0}")]
UnknownService(String),
#[error("无法打开 PDF 文件：{0}")]
PdfOpenFailed(String),
#[error("PDF 中没有可识别的页面")]
PdfEmptyDocument,
#[error("PDF 页面渲染失败：{0}")]
PdfRenderFailed(String),
```

在 `friendly_ocr_error` 增加：

```rust
OcrTranslationError::Ocr(OcrError::UnknownService(_)) => {
    "OCR 识别失败：渠道已不存在，请重新选择。".to_string()
}
OcrTranslationError::Ocr(OcrError::PdfOpenFailed(_)) => {
    "OCR 识别失败：无法打开 PDF 文件。".to_string()
}
OcrTranslationError::Ocr(OcrError::PdfEmptyDocument) => {
    "OCR 识别失败：PDF 中没有可识别的页面。".to_string()
}
OcrTranslationError::Ocr(OcrError::PdfRenderFailed(_)) => {
    "OCR 识别失败：PDF 页面渲染失败。".to_string()
}
```

- [x] **步骤 4：运行测试验证通过**

```powershell
cd src-tauri; cargo test --lib ocr::error_tests::pdf_and_unknown_service_variants_display ocr_popup::tests::friendly_unknown_service_and_pdf_errors -- --nocapture
```

预期：PASS。

- [x] **步骤 5：Commit**

```bash
git add src-tauri/src/core/ocr/mod.rs src-tauri/src/ui/ocr_popup.rs
git commit -m "feat(ocr): 增加 PDF 与未知渠道错误变体及友好文案"
```

---

### 任务 2：`resolve_ocr_engine_for` + 单测

**文件：**
- 修改：`src-tauri/src/core/ocr/resolve.rs`
- 修改：`src-tauri/src/core/ocr/mod.rs`（导出）

- [x] **步骤 1：编写失败的测试**

在 `resolve.rs` 的 `tests` 中追加（复用已有 `svc` helper）：

```rust
#[test]
fn resolve_for_by_id_ignores_enabled() {
    let list = vec![
        svc("w", "windows-media-ocr", true, None),
        svc("v", "openai-vision", false, Some("sk-test")),
    ];
    let r = resolve_ocr_engine_for(&list, Some("v")).unwrap();
    match r {
        ResolvedOcrEngine::VisionOpenAiCompatible(c) => {
            assert_eq!(c.api_key, "sk-test");
            assert_eq!(c.model, "gpt-4o");
        }
        _ => panic!("expected vision by id"),
    }
}

#[test]
fn resolve_for_missing_id_is_unknown_service() {
    let list = vec![svc("w", "windows-media-ocr", true, None)];
    let err = resolve_ocr_engine_for(&list, Some("nope")).unwrap_err();
    assert!(matches!(err, OcrError::UnknownService(id) if id == "nope"));
}

#[test]
fn resolve_for_none_uses_enabled_only() {
    let list = vec![
        svc("w", "windows-media-ocr", false, None),
        svc("v", "openai-vision", true, Some("sk")),
    ];
    let r = resolve_ocr_engine_for(&list, None).unwrap();
    assert!(matches!(r, ResolvedOcrEngine::VisionOpenAiCompatible(_)));
}

#[test]
fn resolve_for_by_id_vision_missing_key_is_auth() {
    let list = vec![svc("v", "openai-vision", false, None)];
    let err = resolve_ocr_engine_for(&list, Some("v")).unwrap_err();
    assert!(matches!(err, OcrError::Auth(_)));
}

#[test]
fn resolve_ocr_engine_delegates_to_for_none() {
    let list = vec![svc("w", "windows-media-ocr", true, None)];
    assert_eq!(
        resolve_ocr_engine(&list).unwrap(),
        resolve_ocr_engine_for(&list, None).unwrap()
    );
}
```

- [x] **步骤 2：运行测试验证失败**

```powershell
cd src-tauri; cargo test --lib ocr::resolve::tests::resolve_for_by_id_ignores_enabled -- --nocapture
```

预期：FAIL（`resolve_ocr_engine_for` 未定义）。

- [x] **步骤 3：最少实现**

```rust
/// OCR 引擎解析。
/// - `override_id = Some(id)`：按 id 查找实例，**不检查 enabled**；缺失 → UnknownService。
/// - `override_id = None`：仅 enabled（与历史行为一致）。
pub fn resolve_ocr_engine_for(
    services: &[OcrServiceInstanceConfig],
    override_id: Option<&str>,
) -> Result<ResolvedOcrEngine, OcrError> {
    if let Some(id) = override_id {
        let service = services
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| OcrError::UnknownService(id.to_string()))?;
        return map_service(service);
    }
    resolve_ocr_engine(services)
}

/// 从 ocr_services 解析唯一启用引擎……
pub fn resolve_ocr_engine(
    services: &[OcrServiceInstanceConfig],
) -> Result<ResolvedOcrEngine, OcrError> {
    // 保持现有 enabled 过滤实现不变（不要改成调用 for 造成递归）
    // ... existing body ...
}
```

注意：`resolve_ocr_engine_for(..., None)` 应调用现有 `resolve_ocr_engine`，**不要**反过来让 `resolve_ocr_engine` 调 `for` 再调自己造成无限递归。推荐结构：

```rust
pub fn resolve_ocr_engine(services: &[...]) -> Result<..., OcrError> {
    resolve_ocr_engine_for(services, None)
}

pub fn resolve_ocr_engine_for(...) -> Result<..., OcrError> {
    if let Some(id) = override_id {
        // by id + map_service
    }
    // 原 resolve_ocr_engine 的 enabled 过滤 body 内联在此
}
```

在 `mod.rs`：

```rust
pub use resolve::{resolve_ocr_engine, resolve_ocr_engine_for, ResolvedOcrEngine, VisionOcrConfig};
```

- [x] **步骤 4：运行测试验证通过**

```powershell
cd src-tauri; cargo test --lib ocr::resolve::tests -- --nocapture
```

预期：全部 PASS。

- [x] **步骤 5：Commit**

```bash
git add src-tauri/src/core/ocr/resolve.rs src-tauri/src/core/ocr/mod.rs
git commit -m "feat(ocr): resolve_ocr_engine_for 支持按 id 忽略 enabled"
```

---

### 任务 3：platform 纯识别接入 `service_id`

**文件：**
- 修改：`src-tauri/src/platform/windows/mod.rs`
- 修改：`src-tauri/src/platform/unsupported.rs`
- 修改：所有 `recognize_image_full(..., None)` 调用点（暂仍传 `None`，行为不变）

- [x] **步骤 1：改签名并接线**

将：

```rust
pub async fn recognize_image_full(
    image: CapturedImage,
    hints: OcrHints,
    ocr_services: &[OcrServiceInstanceConfig],
    _model_hint: Option<String>,
) -> Result<RecognizeImageFullResult, OcrError>
```

改为：

```rust
/// `service_id`：OCR 窗临时渠道；`None` 时仅用配置中 enabled 引擎。
pub async fn recognize_image_full(
    image: CapturedImage,
    hints: OcrHints,
    ocr_services: &[OcrServiceInstanceConfig],
    service_id: Option<String>,
) -> Result<RecognizeImageFullResult, OcrError> {
    // ...
    let resolved = resolve_ocr_engine_for(ocr_services, service_id.as_deref())?;
    // 其余 match 不变
}
```

`recognize_cropped_full` 增加同名参数并下传：

```rust
pub async fn recognize_cropped_full(
    frame: &CapturedImage,
    region: (u32, u32, u32, u32),
    hints: OcrHints,
    ocr_services: &[OcrServiceInstanceConfig],
    service_id: Option<String>,
) -> Result<RecognizeImageFullResult, OcrTranslationError> {
    let cropped = frame.crop(region.0, region.1, region.2, region.3)?;
    recognize_image_full(cropped, hints, ocr_services, service_id)
        .await
        .map_err(Into::into)
}
```

`recognize_region`（翻译）**继续** `resolve_ocr_engine(ocr_services)?`，**不**增加 service_id。

`unsupported.rs` 同步签名，仍返回 `UnsupportedPlatform`。

- [x] **步骤 2：编译与既有测试**

```powershell
cd src-tauri; cargo test --lib -- --nocapture
```

预期：PASS（调用点仍传 `None`）。

- [x] **步骤 3：Commit**

```bash
git add src-tauri/src/platform/windows/mod.rs src-tauri/src/platform/unsupported.rs
git commit -m "refactor(ocr): recognize_image_full 第 4 参改为 service_id override"
```

---

### 任务 4：AppState 会话槽 `ocr_session_service_id`

**文件：**
- 修改：`src-tauri/src/app/state.rs`

- [x] **步骤 1：编写失败的测试**

在 `state.rs` 的 `tests` 中追加：

```rust
#[test]
fn ocr_session_service_id_set_take_clears() {
    let state = app_state();
    assert_eq!(state.take_ocr_session_service_id().unwrap(), None);
    state
        .set_ocr_session_service_id(Some("vision-1".into()))
        .unwrap();
    assert_eq!(
        state.take_ocr_session_service_id().unwrap().as_deref(),
        Some("vision-1")
    );
    assert_eq!(state.take_ocr_session_service_id().unwrap(), None);
}

#[test]
fn ocr_session_service_id_clear_and_overwrite() {
    let state = app_state();
    state
        .set_ocr_session_service_id(Some("a".into()))
        .unwrap();
    state
        .set_ocr_session_service_id(Some("b".into()))
        .unwrap();
    assert_eq!(
        state.peek_ocr_session_service_id().unwrap().as_deref(),
        Some("b")
    );
    state.clear_ocr_session_service_id().unwrap();
    assert_eq!(state.peek_ocr_session_service_id().unwrap(), None);
}
```

- [x] **步骤 2：运行测试验证失败**

```powershell
cd src-tauri; cargo test --lib app::state::tests::ocr_session_service_id_set_take_clears -- --nocapture
```

预期：FAIL。

- [x] **步骤 3：最少实现**

在 `AppState` 结构体增加字段：

```rust
// OCR 窗截图纯识别：临时渠道 id。仅 RecognizeOnly 路径读取；不落盘。
ocr_session_service_id: Arc<Mutex<Option<String>>>,
```

`new` / 初始化：`Arc::new(Mutex::new(None))`。

方法：

```rust
pub fn set_ocr_session_service_id(&self, id: Option<String>) -> Result<(), String> {
    let mut g = self
        .ocr_session_service_id
        .lock()
        .map_err(|_| "OCR 会话渠道锁已损坏".to_string())?;
    *g = id.filter(|s| !s.is_empty());
    Ok(())
}

pub fn peek_ocr_session_service_id(&self) -> Result<Option<String>, String> {
    let g = self
        .ocr_session_service_id
        .lock()
        .map_err(|_| "OCR 会话渠道锁已损坏".to_string())?;
    Ok(g.clone())
}

pub fn take_ocr_session_service_id(&self) -> Result<Option<String>, String> {
    let mut g = self
        .ocr_session_service_id
        .lock()
        .map_err(|_| "OCR 会话渠道锁已损坏".to_string())?;
    Ok(g.take())
}

pub fn clear_ocr_session_service_id(&self) -> Result<(), String> {
    self.set_ocr_session_service_id(None)
}
```

- [x] **步骤 4：运行测试验证通过**

```powershell
cd src-tauri; cargo test --lib app::state::tests::ocr_session_service_id -- --nocapture
```

预期：PASS。

- [x] **步骤 5：Commit**

```bash
git add src-tauri/src/app/state.rs
git commit -m "feat(ocr): AppState 增加 OCR 会话临时渠道槽"
```

---

### 任务 5：OCR commands 透传 `service_id` + 写会话槽

**文件：**
- 修改：`src-tauri/src/ui/ocr_window.rs`
- 修改：`src-tauri/src/ui/overlay.rs`
- 修改：`src-tauri/src/app/shortcuts.rs`（若直接调 `start_ocr_capture_flow`：无 id → 槽写 `None`）

- [x] **步骤 1：扩展 command 签名**

```rust
#[tauri::command]
pub async fn start_ocr_capture(
    app: AppHandle,
    state: State<'_, AppState>,
    service_id: Option<String>,
) -> Result<(), String> {
    state.set_ocr_session_service_id(service_id)?;
    start_ocr_capture_flow(app, state.inner().clone()).await;
    Ok(())
}

#[tauri::command]
pub async fn recognize_clipboard_image(
    state: State<'_, AppState>,
    service_id: Option<String>,
) -> Result<RecognizeImageResponse, String> {
    // ...
    let full = recognize_image_full(
        image,
        OcrHints::default(),
        &config.ocr_services,
        service_id,
    )
    .await
    .map_err(|e| friendly_ocr_error(OcrTranslationError::from(e)))?;
    // set_last_ocr_image 不变
    Ok(full.response)
}

#[tauri::command]
pub async fn pick_and_recognize_image(
    app: AppHandle,
    state: State<'_, AppState>,
    service_id: Option<String>,
) -> Result<Option<RecognizeImageResponse>, String> {
    // 文件选择与解码暂不变；recognize_image_full 传入 service_id
    let full = recognize_image_full(
        image,
        OcrHints::default(),
        &config.ocr_services,
        service_id,
    )
    .await
    // ...
}

#[tauri::command]
pub async fn rerecognize_last_image(
    state: State<'_, AppState>,
    service_id: Option<String>,
) -> Result<RecognizeImageResponse, String> {
    // ...
    let full = recognize_image_full(..., service_id).await...;
}
```

快捷键 `Alt+O` 调用 `start_ocr_capture_flow` 时，在 flow 入口**之前**不写 id，或显式 `set_ocr_session_service_id(None)`，避免残留上次 OCR 窗截图的临时 id。

推荐在 `start_ocr_capture_flow` 开头**不**清槽（由 command 负责写入）；shortcuts 路径在调用 flow 前：

```rust
let _ = state.set_ocr_session_service_id(None);
crate::ui::ocr_window::start_ocr_capture_flow(app_handle, state).await;
```

- [x] **步骤 2：overlay RecognizeOnly 读槽**

```rust
CapturePurpose::RecognizeOnly => {
    // take：避免泄漏到下次；cancel 路径另行 clear
    let service_id = state.take_ocr_session_service_id().unwrap_or(None);
    let result = recognize_cropped_full(
        &frame,
        region,
        OcrHints::default(),
        &config.ocr_services,
        service_id,
    )
    .await;
    // finish_capture + emit 不变
}
CapturePurpose::Translate => {
    // 绝不 take/peek ocr_session_service_id
    // recognize_region 不变
}
```

`cancel_capture`：

```rust
let purpose = state.capture_purpose();
// ... existing take_pending / finish / hide ...
if purpose == CapturePurpose::RecognizeOnly {
    let _ = state.clear_ocr_session_service_id();
    // show_ocr_window 不变
}
```

- [x] **步骤 3：单测「Translate 不依赖槽」**（逻辑级，无 Tauri）

若难以测 overlay 整函数，至少保证：

```rust
#[test]
fn translate_path_does_not_require_session_slot() {
    // resolve_ocr_engine 在槽有脏数据时仍只看 enabled——本任务已由 resolve 单测覆盖
    // 此处测 take 后 Translate 不调用 take：文档化 + 代码审
    assert!(true);
}
```

更有价值：在 `state` 测试中确认 `take` 语义即可；overlay 代码审检查 `Translate` 分支无 `ocr_session` 字样。

```powershell
cd src-tauri; cargo test --lib -- --nocapture
rg "ocr_session" src-tauri/src/ui/overlay.rs
```

`rg` 应仅出现在 RecognizeOnly / cancel 分支。

- [x] **步骤 4：Commit**

```bash
git add src-tauri/src/ui/ocr_window.rs src-tauri/src/ui/overlay.rs src-tauri/src/app/shortcuts.rs
git commit -m "feat(ocr): OCR 窗 command 透传 service_id 且会话槽仅 RecognizeOnly 读取"
```

---

### 任务 6：前端会话渠道下拉 + 传参

**文件：**
- 创建：`frontend/src/ocr/sessionChannel.ts`
- 创建：`frontend/src/ocr/sessionChannel.test.ts`
- 修改：`frontend/src/ocr/OcrWindow.vue`
- 修改：`frontend/src/ocr/types.ts`（可选类型）

- [x] **步骤 1：编写失败的 vitest**

`frontend/src/ocr/sessionChannel.ts`（先写测试再实现）：

```ts
// sessionChannel.test.ts
import { describe, expect, it } from 'vitest'
import { pickDefaultOcrServiceId, buildOcrChannelOptions } from './sessionChannel'
import type { OcrServiceInstanceConfig } from '@/types/config'

function svc(
  id: string,
  enabled: boolean,
  name = id,
  serviceType = 'windows-media-ocr',
  model = '',
): OcrServiceInstanceConfig {
  return {
    id,
    serviceType,
    name,
    enabled,
    apiKey: null,
    endpoint: '',
    model,
    preferredLang: '',
    ocrPrompt: '',
  }
}

describe('pickDefaultOcrServiceId', () => {
  it('优先 enabled', () => {
    expect(
      pickDefaultOcrServiceId([svc('a', false), svc('b', true), svc('c', false)]),
    ).toBe('b')
  })

  it('无 enabled 取第一项', () => {
    expect(pickDefaultOcrServiceId([svc('a', false), svc('b', false)])).toBe('a')
  })

  it('空列表返回 null', () => {
    expect(pickDefaultOcrServiceId([])).toBeNull()
  })
})

describe('buildOcrChannelOptions', () => {
  it('列出全部实例且含摘要', () => {
    const opts = buildOcrChannelOptions([
      svc('w', true, 'Windows', 'windows-media-ocr', ''),
      svc('v', false, 'GPT', 'openai-vision', 'gpt-4o'),
    ])
    expect(opts).toHaveLength(2)
    expect(opts[0]).toMatchObject({ value: 'w', label: 'Windows' })
    expect(opts[1].description).toContain('gpt-4o')
  })
})
```

- [x] **步骤 2：运行测试验证失败**

```powershell
npm run test -- frontend/src/ocr/sessionChannel.test.ts
```

预期：FAIL（模块不存在）。

- [x] **步骤 3：实现纯函数**

```ts
// frontend/src/ocr/sessionChannel.ts
import type { OcrServiceInstanceConfig } from '@/types/config'

export function pickDefaultOcrServiceId(
  services: OcrServiceInstanceConfig[],
): string | null {
  const enabled = services.find((s) => s.enabled)
  if (enabled) return enabled.id
  return services[0]?.id ?? null
}

export function buildOcrChannelOptions(
  services: OcrServiceInstanceConfig[],
): { value: string; label: string; description?: string }[] {
  return services.map((s) => {
    const parts = [s.serviceType, s.model].filter((x) => x && x.trim().length > 0)
    return {
      value: s.id,
      label: s.name || s.id,
      description: parts.length ? parts.join(' · ') : undefined,
    }
  })
}

/** 配置变更后：当前 id 仍存在则保留，否则回落默认 */
export function reconcileSelectedOcrServiceId(
  services: OcrServiceInstanceConfig[],
  currentId: string | null,
): string | null {
  if (currentId && services.some((s) => s.id === currentId)) return currentId
  return pickDefaultOcrServiceId(services)
}
```

可选：为 `reconcile` 补一条 vitest。

- [x] **步骤 4：改造 `OcrWindow.vue`**

要点：

```ts
import { Select } from '@/components/ui/select'
import type { AppConfig, OcrServiceInstanceConfig } from '@/types/config'
import {
  buildOcrChannelOptions,
  pickDefaultOcrServiceId,
  reconcileSelectedOcrServiceId,
} from './sessionChannel'

const ocrServices = ref<OcrServiceInstanceConfig[]>([])
const selectedOcrServiceId = ref<string | null>(null)

const channelOptions = computed(() => buildOcrChannelOptions(ocrServices.value))

async function loadOcrServices(): Promise<void> {
  const apis = getTauriApis()
  if (!apis) return
  try {
    const cfg = await apis.invoke<AppConfig>('get_app_config')
    ocrServices.value = cfg.ocrServices ?? []
    selectedOcrServiceId.value = reconcileSelectedOcrServiceId(
      ocrServices.value,
      selectedOcrServiceId.value,
    )
  } catch (e) {
    logger.warn('加载 OCR 服务列表失败', String(e))
  }
}

// 所有识别 invoke 增加：
// { serviceId: selectedOcrServiceId.value ?? undefined }
// 注意 Tauri 2 参数名：Rust `service_id` ↔ 前端 camelCase `serviceId`（serde rename 默认）
// 本项目 command 参数若未 rename，确认序列化：Tauri 默认对命令参数用 serde；
// 查阅现有 command——多数用 snake_case 参数名，前端 invoke 第二参用对象键与 Rust 参数名一致。
// **锁定：前端传 `{ service_id: selectedOcrServiceId.value ?? null }`，键名与 Rust 参数一致。**

await apis.invoke('start_ocr_capture', {
  service_id: selectedOcrServiceId.value,
})
await apis.invoke('pick_and_recognize_image', {
  service_id: selectedOcrServiceId.value,
})
// clipboard / rerecognize 同理
```

挂载：

```ts
onMounted(() => {
  void setWindowTitle()
  void setupListeners()
  void loadOcrServices()
  // listen app-config:changed → loadOcrServices（不自动重跑 OCR）
})
```

模板顶栏：在按钮组后、右侧引擎摘要前插入：

```vue
<div class="w-44 shrink-0">
  <Select
    :model-value="selectedOcrServiceId ?? undefined"
    :options="channelOptions"
    :disabled="isLoading"
    placeholder="识别渠道"
    class="h-8 text-xs"
    @update:model-value="(v) => (selectedOcrServiceId = v)"
  />
</div>
```

**禁止**在切换渠道时 `invokeSaveAppConfig` / 改 enabled。

- [x] **步骤 5：类型检查与单测**

```powershell
npm run test -- frontend/src/ocr/sessionChannel.test.ts
npm run typecheck
```

预期：PASS。

- [x] **步骤 6：Commit**

```bash
git add frontend/src/ocr/sessionChannel.ts frontend/src/ocr/sessionChannel.test.ts frontend/src/ocr/OcrWindow.vue frontend/src/ocr/types.ts
git commit -m "feat(ocr): 文字识别窗会话级渠道下拉并透传 service_id"
```

---

### 任务 7：PDF 检测纯函数 + 单测

**文件：**
- 创建：`src-tauri/src/core/ocr/pdf_detect.rs`
- 修改：`src-tauri/src/core/ocr/mod.rs`

- [x] **步骤 1：编写失败的测试**

```rust
// pdf_detect.rs
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn path_extension_pdf_case_insensitive() {
        assert!(is_pdf_path(Path::new("a.PDF")));
        assert!(is_pdf_path(Path::new("x/y/z.pdf")));
        assert!(!is_pdf_path(Path::new("a.png")));
        assert!(!is_pdf_path(Path::new("pdf")));
    }

    #[test]
    fn magic_percent_pdf() {
        assert!(is_pdf_bytes(b"%PDF-1.4\n..."));
        assert!(!is_pdf_bytes(b"\x89PNG\r\n"));
        assert!(!is_pdf_bytes(b""));
        assert!(!is_pdf_bytes(b"%PD"));
    }

    #[test]
    fn looks_like_pdf_or_of_path_and_magic() {
        assert!(looks_like_pdf(Some(Path::new("doc.pdf")), b"not-magic"));
        assert!(looks_like_pdf(Some(Path::new("doc.bin")), b"%PDF-1.7"));
        assert!(!looks_like_pdf(Some(Path::new("a.png")), b"\x89PNG"));
    }
}
```

- [x] **步骤 2：运行测试验证失败**

```powershell
cd src-tauri; cargo test --lib ocr::pdf_detect -- --nocapture
```

预期：FAIL。

- [x] **步骤 3：实现**

```rust
use std::path::Path;

pub fn is_pdf_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
}

pub fn is_pdf_bytes(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && &bytes[..4] == b"%PDF"
}

pub fn looks_like_pdf(path: Option<&Path>, bytes: &[u8]) -> bool {
    path.map(is_pdf_path).unwrap_or(false) || is_pdf_bytes(bytes)
}
```

`mod.rs`：`pub mod pdf_detect;`

- [x] **步骤 4：测试通过 + Commit**

```powershell
cd src-tauri; cargo test --lib ocr::pdf_detect -- --nocapture
```

```bash
git add src-tauri/src/core/ocr/pdf_detect.rs src-tauri/src/core/ocr/mod.rs
git commit -m "feat(ocr): PDF 路径与魔数检测纯函数"
```

---

### 任务 8：WinRT 渲染 PDF 第 1 页

**文件：**
- 修改：`src-tauri/Cargo.toml`（`Data_Pdf` feature）
- 创建：`src-tauri/src/platform/windows/pdf.rs`
- 修改：`src-tauri/src/platform/windows/mod.rs`
- 修改：`src-tauri/src/platform/unsupported.rs`
- 修改：`src-tauri/src/platform/mod.rs`

- [x] **步骤 1：Cargo feature**

在 `src-tauri/Cargo.toml` 的 `windows` features 数组加入：

```toml
"Data_Pdf",
```

（`Storage_Streams`、`Foundation`、`Graphics_Imaging` 已存在则可复用解码。）

- [x] **步骤 2：定义 API + unsupported**

```rust
// 可放 core 旁或 platform 公共类型
pub struct PdfFirstPage {
    pub image: CapturedImage,
    pub page_count: u32,
}

pub fn render_pdf_first_page(bytes: &[u8]) -> Result<PdfFirstPage, OcrError>;
```

`unsupported`：

```rust
pub fn render_pdf_first_page(_bytes: &[u8]) -> Result<PdfFirstPage, OcrError> {
    Err(OcrError::UnsupportedPlatform) // 或 PdfOpenFailed("当前平台暂不支持 PDF 识别")
}
```

友好文案：若用 `UnsupportedPlatform`，可在 pick 路径 PDF 分支映射为「当前平台暂不支持 PDF 识别」；推荐 **PDF 专用**：

```rust
Err(OcrError::PdfOpenFailed("当前平台暂不支持 PDF 识别".into()))
```

- [x] **步骤 3：Windows 实现要点（`platform/windows/pdf.rs`）**

实现约束（实现时按此清单，WinRT 细节可微调但行为不变）：

1. 空字节 / 非 `%PDF` → `PdfOpenFailed`。
2. 将 `bytes` 写入 `InMemoryRandomAccessStream`（`DataWriter` + `StoreAsync` + `FlushAsync`，或等价）。
3. `PdfDocument::LoadFromStreamAsync(&stream)`；失败 → `PdfOpenFailed`。
4. `page_count = doc.PageCount()`；`0` → `PdfEmptyDocument`。
5. `page = doc.GetPage(0)`。
6. 计算目标渲染尺寸：以页的 `Size` 为逻辑尺寸，按 **96 DPI 基准 × 2（约 192 DPI）** 或限制最长边 ≤ `2048`（与 `VISION_MAX_LONG_EDGE` 对齐，可 `use crate::core::ocr::image_encode::VISION_MAX_LONG_EDGE`）。
7. `PdfPageRenderOptions` 设置 `DestinationWidth` / `DestinationHeight`（若 API 可用）。
8. `page.RenderToStreamAsync` 到新的 memory stream（默认输出可被 `BitmapDecoder` 打开的编码；若得到 BGRA buffer 更佳）。
9. 解码为 **RGBA8 或 BGRA8** `CapturedImage`（与 `load_image_file_bytes` / Windows OCR 兼容；优先 BGRA8 若解码器直接给出，否则 RGBA8）。
10. 成功返回 `PdfFirstPage { image, page_count }`；渲染失败 → `PdfRenderFailed`。
11. **禁止** log PDF 全文 / 像素 dump；可 `log::info!("PDF 首页渲染: pages={} size={}x{}", ...)`。

同步包装：在 `spawn_blocking` 中用 `block_on` 或 WinRT 同步等待（与现有 `windows` OCR 风格一致）。若 async 更干净，可 `pub async fn`，由 `pick_and_recognize_image` `.await`。

推荐签名：

```rust
pub async fn render_pdf_first_page(bytes: &[u8]) -> Result<PdfFirstPage, OcrError>
```

`platform/mod.rs` 导出。

- [x] **步骤 4：Windows 单测（有 fixture）**

在 `pdf.rs`：

```rust
#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// 最小单页 PDF（无文本内容亦可；只要 PageCount>=1 且可渲染）。
    fn minimal_one_page_pdf() -> Vec<u8> {
        // 使用仓库内 fixture 文件更稳：tests/fixtures/minimal.pdf
        // 或内嵌已知合法最小 PDF 字节
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/minimal-one-page.pdf"
        ))
        .expect("fixture")
    }

    #[tokio::test]
    async fn render_first_page_nonzero_bitmap() {
        let page = render_pdf_first_page(&minimal_one_page_pdf())
            .await
            .expect("render");
        assert!(page.page_count >= 1);
        assert!(page.image.width >= 1 && page.image.height >= 1);
        assert!(!page.image.bytes.is_empty());
        assert_eq!(page.image.bytes.len() as u32, page.image.width * page.image.height * 4);
    }

    #[tokio::test]
    async fn render_garbage_is_open_failed() {
        let err = render_pdf_first_page(b"not a pdf").await.unwrap_err();
        assert!(matches!(
            err,
            OcrError::PdfOpenFailed(_) | OcrError::ImageConversionFailed(_)
        ));
    }
}
```

**Fixture 准备：** 在任务中用任意合法单页 PDF 二进制放入 `src-tauri/tests/fixtures/minimal-one-page.pdf`（可从本机用 Word/打印到 PDF 导出一页空白，或使用开源最小 PDF）。**不要**用损坏文件。多页 PDF 手动验收即可，单测只需单页。

若 CI 无 WinRT PDF 能力导致失败，用 `#[ignore]` 并在注释写明「本机 Windows 手动 `cargo test -- --ignored`」——**优先不 ignore**，因开发机为 Windows。

- [x] **步骤 5：运行测试**

```powershell
cd src-tauri; cargo test --lib pdf -- --nocapture
```

预期：PASS（Windows）。

- [x] **步骤 6：Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/platform src-tauri/tests/fixtures/minimal-one-page.pdf
git commit -m "feat(ocr): WinRT 渲染 PDF 第 1 页为 CapturedImage"
```

---

### 任务 9：`pick_and_recognize_image` 接 PDF + meta 页数

**文件：**
- 修改：`src-tauri/src/core/ocr/meta.rs`
- 修改：`src-tauri/src/ui/ocr_window.rs`
- 修改：`frontend/src/ocr/types.ts`
- 修改：`frontend/src/ocr/OcrWindow.vue`（页数展示）

- [x] **步骤 1：meta 可选字段**

```rust
// OcrRunMeta
#[serde(skip_serializing_if = "Option::is_none")]
pub source_page: Option<u32>,
#[serde(skip_serializing_if = "Option::is_none")]
pub source_page_count: Option<u32>,
```

所有现有 `OcrRunMeta { ... }` 构造处补 `source_page: None, source_page_count: None`（编译器会指引）。

前端 `OcrRunMeta`：

```ts
sourcePage?: number | null
sourcePageCount?: number | null
```

- [x] **步骤 2：pick 分支**

```rust
.add_filter("图片与 PDF", &["png", "jpg", "jpeg", "webp", "bmp", "pdf"])
// 可另加 .add_filter("PDF", &["pdf"])

let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
log::info!("OCR 文件读入: path={}", path.display());

let (image, pdf_pages): (CapturedImage, Option<u32>) =
    if crate::core::ocr::pdf_detect::looks_like_pdf(Some(&path), &bytes) {
        let rendered = crate::platform::render_pdf_first_page(&bytes)
            .await
            .map_err(|e| friendly_ocr_error(OcrTranslationError::from(e)))?;
        log::info!(
            "PDF 首页: pages={} {}x{}",
            rendered.page_count,
            rendered.image.width,
            rendered.image.height
        );
        (rendered.image, Some(rendered.page_count))
    } else {
        let image = load_image_file_bytes(&bytes).map_err(|e| e.to_string())?;
        (image, None)
    };

// recognize_image_full(... service_id) ...
let mut response = full.response;
if let Some(n) = pdf_pages {
    response.meta.source_page = Some(1);
    response.meta.source_page_count = Some(n);
}
// set_last_ocr_image(full.source_image) —— 注意 source_image 是栅格图，不是 PDF
```

渲染失败：**不**写 `last_ocr_image`。

图片路径回归：现有 `load_image_file_bytes` 测试保持。

- [x] **步骤 3：前端页数提示**

在预览标题或 footer 增加：

```vue
<span v-if="meta?.sourcePage && meta?.sourcePageCount" class="text-xs text-muted-foreground">
  已识别第 {{ meta.sourcePage }} 页（共 {{ meta.sourcePageCount }} 页）
</span>
```

- [x] **步骤 4：测试**

```powershell
cd src-tauri; cargo test --lib -- --nocapture
npm run typecheck
npm run test -- frontend/src/ocr/sessionChannel.test.ts
```

- [x] **步骤 5：Commit**

```bash
git add src-tauri/src/core/ocr/meta.rs src-tauri/src/ui/ocr_window.rs frontend/src/ocr/types.ts frontend/src/ocr/OcrWindow.vue
git commit -m "feat(ocr): 打开文件支持 PDF 首页识别并展示页数"
```

---

### 任务 10：回归、文档与手动验收清单

**文件：**
- 修改：`README.md`（当前能力：OCR 窗 PDF 首页 + 临时渠道一句）
- 若存在：`docs/architecture/*ocr*` 短补
- 规格状态：编码完成后将 spec 状态改为「已实现」（本任务编码阶段做；计划阶段只列项）

- [x] **步骤 1：全量自动验证**

```powershell
cd src-tauri; cargo test --lib
cd src-tauri; cargo build
npm run test
npm run typecheck
```

预期：全部通过。

- [x] **步骤 2：手动验收清单（开发者勾选）**

> 自动验证已通过（`cargo test --lib` / `cargo build` / `npm run test` / `npm run typecheck`）。下表供本机 GUI 验收时勾选，编码阶段不必跑通 GUI。

| # | 步骤 | 期望 | 开发者 |
|---|---|---|---|
| 1 | OCR 窗打开多页 PDF | 仅第 1 页预览/正文；有页数则显示「第 1 页 / 共 N 页」 | [ ] |
| 2 | 打开 png/jpg | 与改前一致 | [ ] |
| 3 | 下拉选未启用视觉渠道后识别 | 成功或按 Key/协议正确报错；设置页 enabled 未变 | [ ] |
| 4 | 识别中尝试切换渠道 | 下拉 disabled | [ ] |
| 5 | 切换渠道不自动重跑 | 正文不变直至再次识别 | [ ] |
| 6 | 关闭 OCR 窗再开 | 下拉回到设置页启用项 | [ ] |
| 7 | Alt+S 截图翻译 | 仍用 enabled 引擎，不受 OCR 窗临时选择影响 | [ ] |
| 8 | 剪贴板位图 | 仍可用；无 PDF 扩展 | [ ] |
| 9 | 损坏 PDF | 中文错误条，无脏预览 | [ ] |
| 10 | 重新识别 | 使用当前下拉渠道 | [ ] |

- [x] **步骤 3：文档 commit**

```bash
git add README.md docs/
git commit -m "docs(ocr): 补充 PDF 首页与会话渠道能力说明"
```

---

## 自检（计划 vs 规格）

| 规格条目 | 对应任务 |
|---|---|
| PDF 打开 + 第 1 页栅格化 | 7, 8, 9 |
| 剪贴板维持位图 | 5（仅透传 id）、刻意不改 clipboard 读图 |
| 临时渠道 UI 全部 ocrServices | 6 |
| 不写 config / 不改 enabled | 6（禁止 save）；手动验收 3 |
| 本窗四入口用临时 id | 5, 6 |
| Alt+S 不读槽 | 3（recognize_region）、5（Translate 分支） |
| resolve by id 忽略 enabled | 2 |
| 会话槽 RecognizeOnly | 4, 5 |
| loading 禁用下拉 | 6 |
| 错误文案 | 1, 9 |
| 单测 resolve / PDF / 槽 | 2, 4, 7, 8 |
| 页数提示 | 9 |

**占位符扫描：** 无 TODO/待定；WinRT 实现步骤已列行为约束与错误映射。

**类型一致性：**
- Rust 参数名 `service_id: Option<String>`
- 前端 invoke 键 `service_id`
- `resolve_ocr_engine_for(services, Option<&str>)`
- `PdfFirstPage { image, page_count }`
- meta：`source_page` / `source_page_count` ↔ `sourcePage` / `sourcePageCount`

**实现顺序（与 spec §12 对齐）：** 任务 1→2→3→4→5→6→7→8→9→10。

---

## 风险备忘

| 风险 | 计划内缓解 |
|---|---|
| WinRT PDF feature/API 差异 | 任务 8 行为清单 + fixture；失败映射 Pdf* 错误 |
| 会话槽泄漏到 Translate | 任务 5 分支隔离 + rg 检查 |
| 大页内存 | 渲染最长边 ≤ 2048 |
| 配置删除当前 id | 任务 6 `reconcileSelectedOcrServiceId` |
