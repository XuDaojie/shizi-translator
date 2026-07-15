# 多模态 OCR 运行时接通 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 将设置页已有 `ocrServices` 接到截图 OCR 运行时，使唯一启用的 Windows 或 OpenAI 兼容视觉引擎完成纯文字识别后进入现有翻译链路。

**架构：** 方案 A——独立 `VisionOcrEngine`（OpenAI Chat Completions 多模态、非流式）+ `resolve_ocr_engine` 工厂返回选择枚举；调用方（platform/UI）按枚举构造具体引擎，**不**污染 `TranslationProvider`。后端 `AppConfig::normalized` 保证 OCR 互斥与禁全关；前端 store 开关互斥 + Claude 禁启用。crop → recognize → `TranslationInput::OcrText` → 现有批次翻译不变。

**技术栈：** Rust（Tauri 2、reqwest、serde_json、image+png、base64、tokio）、Vue 3 + TypeScript + Vitest、cargo test

**规格来源：** `docs/superpowers/specs/2026-07-15-multimodal-ocr-runtime-design.md`

---

## 文件结构

| 文件 | 职责 |
|---|---|
| 修改 `src-tauri/src/core/ocr/mod.rs` | 扩展 `OcrError`；`mod resolve` / `mod image_encode` / `mod vision_openai`；导出公共类型 |
| 创建 `src-tauri/src/core/ocr/resolve.rs` | `ResolvedOcrEngine` 枚举 + `resolve_ocr_engine` + 类型映射 |
| 创建 `src-tauri/src/core/ocr/image_encode.rs` | BGRA/RGBA → 可选缩放 → PNG 字节 + data URL |
| 创建 `src-tauri/src/core/ocr/vision_openai.rs` | `VisionOcrEngine`：组包、HTTP、解析、错误映射 |
| 修改 `src-tauri/src/core/config/types.rs` | `AppConfig::normalized` OCR 互斥/seed；默认 Windows 工厂辅助 |
| 修改 `src-tauri/src/platform/windows/mod.rs` | `recognize_region` 接收已解析引擎选择或 `dyn OcrEngine` |
| 修改 `src-tauri/src/platform/unsupported.rs` | 签名对齐（仍返回 Unsupported） |
| 修改 `src-tauri/src/ui/overlay.rs` | 读 config → resolve → 调用 recognize |
| 修改 `src-tauri/src/ui/ocr_popup.rs` | `friendly_ocr_error` 映射新变体 |
| 修改 `src-tauri/Cargo.toml` | 增加 `image`（png）、`base64` |
| 修改 `frontend/src/settings/stores/settings.ts` | 互斥开关、禁全关、删唯一启用回 Windows、Claude 拒启用；去掉「Windows 永开」 |
| 修改 `frontend/src/settings/stores/settings.test.ts` | 覆盖互斥/禁全关/Claude/merge |
| 修改 `frontend/src/settings/tokens.ts` | Windows `canDisable: true`；文案 detail；Claude 标记不可运行时启用 |
| 修改 `frontend/src/settings/types.ts` | 可选 `runtimeSupported?: boolean`（Claude = false） |
| 修改 `frontend/src/settings/panels/ServicesPanel.vue` | 开关 UI、Claude disabled、去掉 configReserved 误导文案 |
| 修改 `frontend/src/i18n/locales/zh-CN.json`、`en-US.json`（及其它已有 locale 键若存在） | 新文案键 |
| 修改 `README.md` | OCR 运行时说明一句 |
| 修改 `docs/architecture/screenshot-ocr-architecture.md` | 补运行时选引擎说明（简短） |
| 修改 `docs/superpowers/specs/2026-07-15-multimodal-ocr-runtime-design.md` | 收尾状态回填（编码阶段） |

**刻意不改：** 翻译 `TranslationProvider` / 流式协议、DXGI/overlay 框选交互、Claude Messages 视觉协议、多 OCR 并行。

**分层约束：** `core` **不**依赖 `platform`。`resolve_ocr_engine` 返回 `ResolvedOcrEngine` 枚举（`WindowsMedia` | `VisionOpenAiCompatible(VisionOcrConfig)`），Windows 具体类型由 `platform/windows` 在调用点 `Box`/`&` 构造。

---

## 任务 1：扩展 `OcrError` + 单测

**文件：**
- 修改：`src-tauri/src/core/ocr/mod.rs`
- 修改：`src-tauri/src/ui/ocr_popup.rs`（本任务只加 match 臂占位，完整文案在任务 7 可再调；**本任务必须让编译通过**）

- [ ] **步骤 1：编写失败的测试（在 `ocr/mod.rs` 底部或临时 `#[cfg(test)]`）**

在 `mod.rs` 末尾增加：

```rust
#[cfg(test)]
mod error_tests {
    use super::OcrError;

    #[test]
    fn new_variants_display_messages() {
        let no = OcrError::NoEngineConfigured;
        assert!(no.to_string().contains("没有可用") || no.to_string().contains("文字识别"));

        let unsup = OcrError::UnsupportedProtocol("claude-vision".into());
        assert!(unsup.to_string().contains("claude-vision"));

        let auth = OcrError::Auth("missing key".into());
        assert!(auth.to_string().contains("missing key") || auth.to_string().contains("认证"));

        let api = OcrError::Api {
            message: "rate limit".into(),
            retryable: true,
        };
        assert!(api.to_string().contains("rate limit"));

        let http = OcrError::Http("timeout".into());
        assert!(http.to_string().contains("timeout") || http.to_string().contains("网络"));
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

```powershell
cd src-tauri; cargo test --lib ocr::error_tests -- --nocapture
```

预期：FAIL（变体不存在）。

- [ ] **步骤 3：扩展 `OcrError`**

在现有变体后增加（保持 `PartialEq, Eq`）：

```rust
#[error("没有可用的文字识别服务")]
NoEngineConfigured,
#[error("不支持的 OCR 协议：{0}")]
UnsupportedProtocol(String),
#[error("OCR 认证失败：{0}")]
Auth(String),
#[error("OCR 服务错误：{message}")]
Api { message: String, retryable: bool },
#[error("OCR 网络错误：{0}")]
Http(String),
```

`Api` 的 `retryable` 参与 `PartialEq` 即可；本轮 UI 可不区分重试按钮。

- [ ] **步骤 4：更新 `friendly_ocr_error` 穷尽 match**

在 `ocr_popup.rs` 的 `friendly_ocr_error` 增加：

```rust
OcrTranslationError::Ocr(OcrError::NoEngineConfigured) => {
    "OCR 识别失败：没有可用的文字识别服务。请在「设置 → 服务 → 文字识别」启用一项。".to_string()
}
OcrTranslationError::Ocr(OcrError::UnsupportedProtocol(ref p)) => {
    format!("OCR 识别失败：当前版本不支持该识别协议（{p}）。请改用 Windows 媒体 OCR 或 OpenAI 兼容视觉。")
}
OcrTranslationError::Ocr(OcrError::Auth(ref d)) => {
    format!("OCR 识别失败：认证失败（{d}）。请在「设置 → 文字识别」检查 API Key。")
}
OcrTranslationError::Ocr(OcrError::Api { ref message, .. }) => {
    format!("OCR 识别失败：{message}")
}
OcrTranslationError::Ocr(OcrError::Http(ref d)) => {
    format!("OCR 识别失败：网络错误（{d}）")
}
```

同步在 `ocr_popup` 测试模块为新变体各加一条 `friendly_error_maps_*` 断言（至少 `NoEngineConfigured`、`UnsupportedProtocol`、`Auth`）。

- [ ] **步骤 5：运行测试验证通过**

```powershell
cd src-tauri; cargo test --lib ocr::error_tests; cargo test --lib ui::ocr_popup
```

预期：PASS。

- [ ] **步骤 6：Commit**

```powershell
git add src-tauri/src/core/ocr/mod.rs src-tauri/src/ui/ocr_popup.rs
git commit -m "feat(ocr): 扩展 OcrError 支持视觉引擎错误变体"
```

---

## 任务 2：`AppConfig::normalized` OCR 规则 + 单测（TDD）

**文件：**
- 修改：`src-tauri/src/core/config/types.rs`

**规则（规格 5.3）：**

1. `ocr_services` 为空 → seed 单条 Windows（`enabled: true`）
2. 没有任何 enabled → 强制已有 Windows 行 `enabled = true`；若无 Windows 行则插入默认 Windows
3. 多个 enabled → 仅保留**列表顺序第一个** enabled，其余 `enabled = false`
4. 不删除视觉实例；不改 apiKey/model 等字段

**辅助函数（建议私有）：**

```rust
fn default_windows_ocr_service() -> OcrServiceInstanceConfig {
    OcrServiceInstanceConfig {
        id: "windows-media-ocr".into(), // 稳定 id 便于前端 merge；若已有实例用其 id
        service_type: "windows-media-ocr".into(),
        name: "Windows 媒体 OCR".into(),
        enabled: true,
        api_key: None,
        endpoint: String::new(),
        model: String::new(),
        preferred_lang: String::new(),
        ocr_prompt: String::new(),
    }
}

fn normalize_ocr_services(mut list: Vec<OcrServiceInstanceConfig>) -> Vec<OcrServiceInstanceConfig> {
    // 1. 空 → seed
    // 2. 逐条已由 .normalized() 处理字段
    // 3. 多 enabled → 只留第一个
    // 4. 零 enabled → 找 windows-media-ocr 打开，否则 insert default_windows
    list
}
```

注意：前端 seed 的 Windows 实例 id 是随机 `newInstanceId()`，后端 seed 用固定 `"windows-media-ocr"` 仅在**磁盘为空**时出现；merge 按 id 对齐，空后端时前端保留本地 id——与现行为一致。

- [ ] **步骤 1：编写失败的测试**

在 `types.rs` 测试模块追加：

```rust
#[test]
fn normalized_seeds_windows_ocr_when_empty() {
    let config = AppConfig::default(); // 走 normalized
    assert_eq!(config.ocr_services.len(), 1);
    assert_eq!(config.ocr_services[0].service_type, "windows-media-ocr");
    assert!(config.ocr_services[0].enabled);
}

#[test]
fn normalized_enables_windows_when_all_ocr_disabled() {
    let mut config = AppConfig::default();
    config.ocr_services = vec![
        OcrServiceInstanceConfig {
            id: "win".into(),
            service_type: "windows-media-ocr".into(),
            name: "W".into(),
            enabled: false,
            api_key: None,
            endpoint: String::new(),
            model: String::new(),
            preferred_lang: String::new(),
            ocr_prompt: String::new(),
        },
        OcrServiceInstanceConfig {
            id: "v".into(),
            service_type: "openai-vision".into(),
            name: "V".into(),
            enabled: false,
            api_key: Some("sk".into()),
            endpoint: "https://api.openai.com/v1".into(),
            model: "gpt-4o".into(),
            preferred_lang: String::new(),
            ocr_prompt: String::new(),
        },
    ];
    let n = config.normalized();
    assert!(n.ocr_services.iter().find(|s| s.id == "win").unwrap().enabled);
    assert!(!n.ocr_services.iter().find(|s| s.id == "v").unwrap().enabled);
}

#[test]
fn normalized_keeps_only_first_enabled_ocr() {
    let mut config = AppConfig::default();
    config.ocr_services = vec![
        OcrServiceInstanceConfig {
            id: "v1".into(),
            service_type: "openai-vision".into(),
            name: "V1".into(),
            enabled: true,
            api_key: Some("sk".into()),
            endpoint: "https://a".into(),
            model: "m".into(),
            preferred_lang: String::new(),
            ocr_prompt: String::new(),
        },
        OcrServiceInstanceConfig {
            id: "win".into(),
            service_type: "windows-media-ocr".into(),
            name: "W".into(),
            enabled: true,
            api_key: None,
            endpoint: String::new(),
            model: String::new(),
            preferred_lang: String::new(),
            ocr_prompt: String::new(),
        },
    ];
    let n = config.normalized();
    assert!(n.ocr_services[0].enabled);
    assert!(!n.ocr_services[1].enabled);
}

#[test]
fn normalized_inserts_windows_when_all_disabled_and_no_windows_row() {
    let mut config = AppConfig::default();
    config.ocr_services = vec![OcrServiceInstanceConfig {
        id: "v".into(),
        service_type: "openai-vision".into(),
        name: "V".into(),
        enabled: false,
        api_key: None,
        endpoint: "https://a".into(),
        model: "m".into(),
        preferred_lang: String::new(),
        ocr_prompt: String::new(),
    }];
    let n = config.normalized();
    assert!(n.ocr_services.iter().any(|s| s.service_type == "windows-media-ocr" && s.enabled));
}
```

- [ ] **步骤 2：运行测试验证失败**

```powershell
cd src-tauri; cargo test --lib config::types::tests::normalized_seeds_windows -- --nocapture
```

预期：FAIL（当前 default 空列表或 Windows 不强制）。

- [ ] **步骤 3：实现 `normalize_ocr_services` 并接入 `AppConfig::normalized`**

在现有 `ocr_services` map `normalized()` 之后调用：

```rust
self.ocr_services = normalize_ocr_services(self.ocr_services);
```

- [ ] **步骤 4：修正既有测试**

`ocr_services_default_empty_and_deserializes_missing_as_empty`：

- `AppConfig::default()` 断言改为 **seed 后 1 条 Windows**
- 反序列化缺字段仍为 empty **原始值**；若测试要的是「缺字段 deserialize 为空」，保持对 `parsed`（未 normalized）的断言，或对 `.normalized()` 单独断言 seed

示例：

```rust
#[test]
fn ocr_services_default_seeds_windows_and_missing_field_deserializes_empty() {
    let config = AppConfig::default();
    assert_eq!(config.ocr_services.len(), 1);
    assert_eq!(config.ocr_services[0].service_type, "windows-media-ocr");

    let json = r#"{"targetLang":"zh-CN","services":[]}"#;
    let parsed: AppConfig = serde_json::from_str(json).expect("parse");
    assert!(parsed.ocr_services.is_empty()); // 未 normalized
    assert_eq!(parsed.normalized().ocr_services.len(), 1);
}
```

- [ ] **步骤 5：运行测试验证通过**

```powershell
cd src-tauri; cargo test --lib config::types
```

预期：PASS。

- [ ] **步骤 6：Commit**

```powershell
git add src-tauri/src/core/config/types.rs
git commit -m "feat(config): OCR 配置归一化互斥与禁全关 seed Windows"
```

---

## 任务 3：`resolve_ocr_engine` + 单测（TDD）

**文件：**
- 创建：`src-tauri/src/core/ocr/resolve.rs`
- 修改：`src-tauri/src/core/ocr/mod.rs`（`pub mod resolve;` + re-export）

**API：**

```rust
use crate::core::config::OcrServiceInstanceConfig;
use super::OcrError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisionOcrConfig {
    pub service_type: String,
    pub endpoint: String,
    pub api_key: String,
    pub model: String,
    /// 空则引擎使用 DEFAULT_OCR_PROMPT
    pub ocr_prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedOcrEngine {
    WindowsMedia,
    VisionOpenAiCompatible(VisionOcrConfig),
}

/// 从 ocr_services 解析唯一启用引擎。调用前配置应已 normalized；
/// 仍对脏配置兜底：0 → NoEngineConfigured；>1 → 取第一个 + log::warn。
pub fn resolve_ocr_engine(
    services: &[OcrServiceInstanceConfig],
) -> Result<ResolvedOcrEngine, OcrError> {
    todo!()
}

fn is_openai_compatible_vision(service_type: &str) -> bool {
    matches!(
        service_type,
        "openai-vision"
            | "gemini-vision"
            | "zhipu-vl"
            | "siliconflow-vision"
            | "moonshot-vision"
            | "openai-compatible-vision"
    )
}
```

映射规则（规格 5.2）：

| type | 结果 |
|---|---|
| `windows-media-ocr` | `WindowsMedia` |
| 上表 openai 兼容 | `VisionOpenAiCompatible`；缺 key → `Auth` |
| `claude-vision` | `UnsupportedProtocol` |
| 未知 | `UnsupportedProtocol` |

缺 Key：`api_key` 为 `None` 或 trim 后空 → `OcrError::Auth("请填写 API Key".into())`。

- [ ] **步骤 1：编写失败的测试（`resolve.rs` 内 `#[cfg(test)]`）**

```rust
fn svc(id: &str, ty: &str, enabled: bool, key: Option<&str>) -> OcrServiceInstanceConfig {
    OcrServiceInstanceConfig {
        id: id.into(),
        service_type: ty.into(),
        name: id.into(),
        enabled,
        api_key: key.map(|s| s.into()),
        endpoint: "https://api.openai.com/v1".into(),
        model: "gpt-4o".into(),
        preferred_lang: String::new(),
        ocr_prompt: "自定义".into(),
    }
}

#[test]
fn resolve_windows_only() {
    let r = resolve_ocr_engine(&[svc("w", "windows-media-ocr", true, None)]).unwrap();
    assert_eq!(r, ResolvedOcrEngine::WindowsMedia);
}

#[test]
fn resolve_openai_vision() {
    let r = resolve_ocr_engine(&[svc("v", "openai-vision", true, Some("sk-test"))]).unwrap();
    match r {
        ResolvedOcrEngine::VisionOpenAiCompatible(c) => {
            assert_eq!(c.model, "gpt-4o");
            assert_eq!(c.api_key, "sk-test");
            assert_eq!(c.ocr_prompt, "自定义");
            assert_eq!(c.service_type, "openai-vision");
        }
        _ => panic!("expected vision"),
    }
}

#[test]
fn resolve_claude_unsupported() {
    let err = resolve_ocr_engine(&[svc("c", "claude-vision", true, Some("sk"))]).unwrap_err();
    assert!(matches!(err, OcrError::UnsupportedProtocol(_)));
}

#[test]
fn resolve_multiple_enabled_takes_first() {
    let list = vec![
        svc("v", "openai-vision", true, Some("sk")),
        svc("w", "windows-media-ocr", true, None),
    ];
    let r = resolve_ocr_engine(&list).unwrap();
    assert!(matches!(r, ResolvedOcrEngine::VisionOpenAiCompatible(_)));
}

#[test]
fn resolve_none_enabled_errors() {
    let err = resolve_ocr_engine(&[svc("w", "windows-media-ocr", false, None)]).unwrap_err();
    assert_eq!(err, OcrError::NoEngineConfigured);
}

#[test]
fn resolve_vision_missing_key_is_auth() {
    let err = resolve_ocr_engine(&[svc("v", "openai-vision", true, None)]).unwrap_err();
    assert!(matches!(err, OcrError::Auth(_)));
}
```

- [ ] **步骤 2：运行测试验证失败**

```powershell
cd src-tauri; cargo test --lib ocr::resolve -- --nocapture
```

预期：FAIL（模块不存在）。

- [ ] **步骤 3：实现 `resolve_ocr_engine`**

- [ ] **步骤 4：运行测试验证通过**

```powershell
cd src-tauri; cargo test --lib ocr::resolve
```

预期：PASS。

- [ ] **步骤 5：Commit**

```powershell
git add src-tauri/src/core/ocr/mod.rs src-tauri/src/core/ocr/resolve.rs
git commit -m "feat(ocr): 实现 resolve_ocr_engine 工厂选择 Windows/视觉"
```

---

## 任务 4：图像 PNG 编码与缩放 + 单测（TDD）

**文件：**
- 修改：`src-tauri/Cargo.toml`（依赖）
- 创建：`src-tauri/src/core/ocr/image_encode.rs`
- 修改：`src-tauri/src/core/ocr/mod.rs`

**依赖：**

```toml
image = { version = "0.25", default-features = false, features = ["png"] }
base64 = "0.22"
```

**API：**

```rust
use crate::core::capture::{CapturedImage, CapturedImageFormat};
use super::OcrError;

pub const VISION_MAX_LONG_EDGE: u32 = 2048;

/// 将截图编码为 PNG 字节；最长边 > 2048 时等比缩小。
pub fn encode_captured_image_png(image: &CapturedImage) -> Result<Vec<u8>, OcrError> { ... }

/// `data:image/png;base64,...`
pub fn png_to_data_url(png: &[u8]) -> String { ... }
```

实现要点：

1. `Bgra8`：转 RGBA 再 `image::RgbaImage`
2. `Rgba8`：直接
3. `Png`：若输入已是 PNG，可原样返回字节（或再 decode 后统一缩放路径）；**优先** decode → 缩放 → re-encode，保证最长边规则
4. 缩放：`max(w,h) > 2048` 时 `new_w/h = round(dim * 2048 / long)`
5. 错误 → `OcrError::ImageConversionFailed`
6. **禁止**在 info 日志打印 base64；debug 可记 `width/height/png_len`

- [ ] **步骤 1：添加依赖并写失败测试**

```rust
#[test]
fn encodes_bgra_1x1_to_valid_png() {
    let img = CapturedImage {
        bytes: vec![0, 0, 255, 255], // B,G,R,A → 红
        width: 1,
        height: 1,
        format: CapturedImageFormat::Bgra8,
    };
    let png = encode_captured_image_png(&img).expect("png");
    assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
    let url = png_to_data_url(&png);
    assert!(url.starts_with("data:image/png;base64,"));
}

#[test]
fn scales_down_when_long_edge_exceeds_2048() {
    // 构造 3000x10 的 RGBA 小内存图
    let w = 3000u32;
    let h = 10u32;
    let img = CapturedImage {
        bytes: vec![0u8; (w * h * 4) as usize],
        width: w,
        height: h,
        format: CapturedImageFormat::Rgba8,
    };
    let png = encode_captured_image_png(&img).unwrap();
    let decoded = image::load_from_memory(&png).unwrap();
    assert!(decoded.width() <= VISION_MAX_LONG_EDGE);
    assert!(decoded.height() <= VISION_MAX_LONG_EDGE);
}
```

解码校验需 `image` 的 `png` decode（`load_from_memory` 默认需要 png feature，已启用）。

- [ ] **步骤 2：运行测试验证失败 → 实现 → 通过**

```powershell
cd src-tauri; cargo test --lib ocr::image_encode
```

- [ ] **步骤 3：Commit**

```powershell
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/core/ocr/mod.rs src-tauri/src/core/ocr/image_encode.rs
git commit -m "feat(ocr): 视觉 OCR 图像 PNG 编码与最长边缩放"
```

---

## 任务 5：`VisionOcrEngine` 请求体/响应解析 + 单测（TDD）

**文件：**
- 创建：`src-tauri/src/core/ocr/vision_openai.rs`
- 修改：`src-tauri/src/core/ocr/mod.rs`

**常量：**

```rust
/// 与 frontend/src/settings/tokens.ts DEFAULT_OCR_PROMPT 对齐
pub const DEFAULT_OCR_PROMPT: &str = "提取图中全部文字，保持阅读顺序";
pub const VISION_OCR_TIMEOUT_SECS: u64 = 60;
pub const VISION_OCR_MAX_TOKENS: u32 = 2048;
const USER_HINT: &str = "请识别图中全部文字。";
```

**结构：**

```rust
pub struct VisionOcrEngine {
    config: VisionOcrConfig,
    client: reqwest::Client,
}

impl VisionOcrEngine {
    pub fn new(config: VisionOcrConfig) -> Result<Self, OcrError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(VISION_OCR_TIMEOUT_SECS))
            .build()
            .map_err(|e| OcrError::Http(e.to_string()))?;
        Ok(Self { config, client })
    }

    fn endpoint(&self) -> String {
        format!(
            "{}/chat/completions",
            self.config.endpoint.trim_end_matches('/')
        )
    }

    /// 纯函数：可单测
    pub(crate) fn build_request_body(model: &str, system: &str, data_url: &str) -> serde_json::Value { ... }

    /// 纯函数：解析 2xx JSON → 文本
    pub(crate) fn parse_success_content(body: &str) -> Result<String, OcrError> { ... }

    /// 纯函数：HTTP 状态 + body → OcrError
    pub(crate) fn map_http_error(status: u16, body: &str) -> OcrError { ... }
}
```

**请求体形状（规格 6.2）：**

```json
{
  "model": "...",
  "stream": false,
  "max_tokens": 2048,
  "messages": [
    { "role": "system", "content": "<prompt>" },
    {
      "role": "user",
      "content": [
        { "type": "text", "text": "请识别图中全部文字。" },
        { "type": "image_url", "image_url": { "url": "data:image/png;base64,..." } }
      ]
    }
  ]
}
```

system = `ocr_prompt` 非空则用其，否则 `DEFAULT_OCR_PROMPT`。

**响应：**

1. 非 2xx：401/403 → `Auth`；其余优先解析 `error.message`，否则截断 body → `Api` 或 `Http`
2. 2xx：`choices[0].message.content` string；若 array 则拼接 `type=="text"` 的 text
3. trim 空 → `EmptyResult`
4. 成功 `OcrResult { text, lines: vec![], engine: config.service_type.clone() }`

**`impl OcrEngine`：** encode → data_url → POST JSON → parse。`hints` 本轮忽略（视觉 prompt 已定）。

**错误解析可参考** `OpenAiCompatibleProvider::message_from_error_body` 思路，但**复制最小逻辑到 vision 模块**，不要改翻译 provider 签名。

- [ ] **步骤 1：编写失败的纯函数测试**

```rust
#[test]
fn request_body_is_non_streaming_with_image_url() {
    let body = VisionOcrEngine::build_request_body(
        "gpt-4o",
        "提取图中全部文字，保持阅读顺序",
        "data:image/png;base64,AAA",
    );
    assert_eq!(body["stream"], false);
    assert_eq!(body["max_tokens"], 2048);
    assert_eq!(body["model"], "gpt-4o");
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][0]["content"], "提取图中全部文字，保持阅读顺序");
    let user_content = &body["messages"][1]["content"];
    assert_eq!(user_content[0]["type"], "text");
    assert_eq!(user_content[1]["type"], "image_url");
    assert_eq!(user_content[1]["image_url"]["url"], "data:image/png;base64,AAA");
}

#[test]
fn parse_success_string_content() {
    let raw = r#"{"choices":[{"message":{"content":"  Hello  "}}]}"#;
    assert_eq!(VisionOcrEngine::parse_success_content(raw).unwrap(), "Hello");
}

#[test]
fn parse_success_array_content() {
    let raw = r#"{"choices":[{"message":{"content":[
      {"type":"text","text":"A"},
      {"type":"text","text":"B"}
    ]}}]}"#;
    assert_eq!(VisionOcrEngine::parse_success_content(raw).unwrap(), "AB");
}

#[test]
fn parse_empty_content_is_empty_result() {
    let raw = r#"{"choices":[{"message":{"content":"   "}}]}"#;
    assert!(matches!(
        VisionOcrEngine::parse_success_content(raw),
        Err(OcrError::EmptyResult)
    ));
}

#[test]
fn map_401_to_auth() {
    let err = VisionOcrEngine::map_http_error(401, r#"{"error":{"message":"bad key"}}"#);
    assert!(matches!(err, OcrError::Auth(_)));
}
```

- [ ] **步骤 2：实现纯函数 → 测试通过**

- [ ] **步骤 3：实现 `recognize` HTTP 路径**

伪代码：

```rust
async fn recognize(&self, image: CapturedImage, _hints: OcrHints) -> Result<OcrResult, OcrError> {
    let png = encode_captured_image_png(&image)?;
    log::debug!(
        "Vision OCR 编码: {}x{} png_bytes={}",
        image.width, image.height, png.len()
    );
    let data_url = png_to_data_url(&png);
    let system = if self.config.ocr_prompt.trim().is_empty() {
        DEFAULT_OCR_PROMPT
    } else {
        self.config.ocr_prompt.as_str()
    };
    let body = Self::build_request_body(&self.config.model, system, &data_url);
    let resp = self
        .client
        .post(self.endpoint())
        .header("Authorization", format!("Bearer {}", self.config.api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| OcrError::Http(e.to_string()))?;
    let status = resp.status().as_u16();
    let text = resp.text().await.map_err(|e| OcrError::Http(e.to_string()))?;
    if !(200..300).contains(&status) {
        return Err(Self::map_http_error(status, &text));
    }
    let content = Self::parse_success_content(&text)?;
    Ok(OcrResult {
        text: content,
        lines: vec![],
        engine: self.config.service_type.clone(),
    })
}
```

本轮**不**强制 wiremock；HTTP 路径靠纯函数 + 手工验收覆盖。

- [ ] **步骤 4：`cargo test --lib ocr::vision_openai` 通过**

- [ ] **步骤 5：Commit**

```powershell
git add src-tauri/src/core/ocr/mod.rs src-tauri/src/core/ocr/vision_openai.rs
git commit -m "feat(ocr): 实现 OpenAI 兼容 VisionOcrEngine 非流式识别"
```

---

## 任务 6：运行时接线（overlay + platform）

**文件：**
- 修改：`src-tauri/src/platform/windows/mod.rs`
- 修改：`src-tauri/src/platform/unsupported.rs`
- 修改：`src-tauri/src/ui/overlay.rs`

**签名变更（推荐）：**

```rust
// platform/windows/mod.rs
use crate::core::ocr::{
    resolve::{resolve_ocr_engine, ResolvedOcrEngine, VisionOcrConfig},
    vision_openai::VisionOcrEngine,
    OcrHints,
};
use ocr::WindowsOcrEngine;

pub async fn recognize_region(
    frame: &CapturedImage,
    region: (u32, u32, u32, u32),
    hints: OcrHints,
    ocr_services: &[crate::core::config::OcrServiceInstanceConfig],
) -> Result<Option<TranslationInput>, OcrTranslationError> {
    let resolved = resolve_ocr_engine(ocr_services)?;
    match resolved {
        ResolvedOcrEngine::WindowsMedia => {
            recognize_cropped_for_translation(frame, region, &WindowsOcrEngine, hints).await
        }
        ResolvedOcrEngine::VisionOpenAiCompatible(cfg) => {
            let engine = VisionOcrEngine::new(cfg).map_err(OcrTranslationError::from)?;
            recognize_cropped_for_translation(frame, region, &engine, hints).await
        }
    }
}
```

`unsupported.rs` 同步增加 `_ocr_services` 参数，行为不变。

**overlay `submit_capture_region`：**

```rust
let config = state.config_store.get().map_err(|e| e.to_string())?;
// 可选：再 .normalized() 一次（get 若已 normalized 可省略）
let result = recognize_region(&frame, region, OcrHints::default(), &config.ocr_services).await;
```

注意：当前 `submit` 在 recognize **之后**才读 config 用于 show popup；改为 recognize **前**读 config（一份即可复用 show）。

- [ ] **步骤 1：改签名并编译**

```powershell
cd src-tauri; cargo test --lib
```

修复所有调用点（`recognize_region` 仅 overlay + unsupported 测试）。

- [ ] **步骤 2：更新 unsupported 测试传 `&[]` 或假列表**

- [ ] **步骤 3：确认 `recognize_cropped_for_translation` 回归测试仍绿**

```powershell
cd src-tauri; cargo test --lib ocr_translation
```

- [ ] **步骤 4：Commit**

```powershell
git add src-tauri/src/platform/windows/mod.rs src-tauri/src/platform/unsupported.rs src-tauri/src/ui/overlay.rs
git commit -m "feat(ocr): 截图路径按 ocrServices 解析引擎"
```

---

## 任务 7：前端 OCR 互斥 / 禁全关 / Claude 不可启用（TDD）

**文件：**
- 修改：`frontend/src/settings/types.ts`（`OcrServiceMeta.runtimeSupported?: boolean`，默认 true）
- 修改：`frontend/src/settings/tokens.ts`
- 修改：`frontend/src/settings/stores/settings.ts`
- 修改：`frontend/src/settings/stores/settings.test.ts`
- 修改：`frontend/src/settings/panels/ServicesPanel.vue`

### 7.1 tokens

- Windows：`canDisable: true`（允许在另有启用项时关闭）
- Windows `detail`：改为「截图识别使用当前启用的文字识别服务；与视觉渠道互斥，仅一项生效。」
- Claude：`runtimeSupported: false`（或 `canDisable: false` + 专用语义）；**不可启用**
- 其它视觉：`runtimeSupported: true`

### 7.2 store 逻辑

替换 `ensureWindowsOcr`「Windows 永开」为 `normalizeOcrList`：

```ts
/** 保证含 Windows 行；零 enabled → 开 Windows；多 enabled → 只留第一个 */
function normalizeOcrList(list: OcrServiceInstance[]): OcrServiceInstance[] {
  let next = [...list]
  if (!next.some((s) => s.type === 'windows-media-ocr')) {
    next = [...seedOcrInstances(), ...next]
  }
  const enabledIdxs = next
    .map((s, i) => (s.enabled ? i : -1))
    .filter((i) => i >= 0)
  if (enabledIdxs.length === 0) {
    return next.map((s) =>
      s.type === 'windows-media-ocr' ? { ...s, enabled: true } : { ...s, enabled: false },
    )
  }
  if (enabledIdxs.length > 1) {
    const keep = enabledIdxs[0]
    return next.map((s, i) => ({ ...s, enabled: i === keep }))
  }
  return next
}
```

`setOcrEnabled(id, enabled)`：

```ts
setOcrEnabled(instanceId: string, enabled: boolean): void {
  const inst = state.ocrServices.find((s) => s.id === instanceId)
  if (!inst) return
  const meta = ocrServiceById(inst.type)
  if (enabled && meta?.runtimeSupported === false) {
    // Claude：拒绝启用（调用方 toast）
    return
  }
  if (enabled) {
    state.ocrServices = state.ocrServices.map((s) => ({
      ...s,
      enabled: s.id === instanceId,
    }))
    return
  }
  // 关闭
  const enabledCount = state.ocrServices.filter((s) => s.enabled).length
  const isOnly = inst.enabled && enabledCount === 1
  if (!isOnly) {
    inst.enabled = false
    return
  }
  // 关唯一项 → 自动启用 Windows
  if (inst.type === 'windows-media-ocr') {
    // 唯一且是 Windows：拒绝关闭（normalize 也会拉回）
    inst.enabled = true
    return
  }
  inst.enabled = false
  const win = state.ocrServices.find((s) => s.type === 'windows-media-ocr')
  if (win) win.enabled = true
  else state.ocrServices = normalizeOcrList(state.ocrServices)
}
```

`removeOcrService`：删除后若无任何 enabled → `normalizeOcrList` 开 Windows。

`mergeBackendIntoOcrServices`：

- **去掉**「Windows 强制 enabled=true」
- merge 后调用 `normalizeOcrList`
- Claude 若后端 enabled=true：前端可保留字段，但 `setOcrEnabled`/UI 禁启用；保存前可选强制 `enabled=false`（推荐在 `normalizeOcrList` 或 save 投影时：`runtimeSupported===false` 不得 enabled；若它是唯一 enabled，改开 Windows）

```ts
// normalize 末尾：
// 若当前唯一 enabled 为 runtimeSupported===false → 关它并开 Windows
```

### 7.3 测试（改写旧用例）

替换 `Windows 不可删不可关；视觉可关可删且不互斥`：

```ts
it('OCR 开关互斥；关唯一视觉自动回 Windows', () => {
  const s = useSettings()
  const winId = s.state.ocrServices[0].id
  const v = s.addOcrService('openai-vision')
  s.setOcrEnabled(v.id, true)
  expect(s.state.ocrServices.find((x) => x.id === v.id)!.enabled).toBe(true)
  expect(s.state.ocrServices.find((x) => x.id === winId)!.enabled).toBe(false)

  s.setOcrEnabled(v.id, false)
  expect(s.state.ocrServices.find((x) => x.id === v.id)!.enabled).toBe(false)
  expect(s.state.ocrServices.find((x) => x.id === winId)!.enabled).toBe(true)
})

it('不能关闭唯一的 Windows', () => {
  const s = useSettings()
  const winId = s.state.ocrServices[0].id
  s.setOcrEnabled(winId, false)
  expect(s.state.ocrServices.find((x) => x.id === winId)!.enabled).toBe(true)
})

it('Claude 视觉不可启用', () => {
  const s = useSettings()
  const c = s.addOcrService('claude-vision')
  s.setOcrEnabled(c.id, true)
  expect(s.state.ocrServices.find((x) => x.id === c.id)!.enabled).toBe(false)
  expect(s.state.ocrServices.some((x) => x.enabled && x.type === 'windows-media-ocr')).toBe(true)
})

it('删除唯一启用的视觉后回 Windows', () => {
  const s = useSettings()
  const v = s.addOcrService('openai-vision')
  s.setOcrEnabled(v.id, true)
  s.removeOcrService(v.id)
  expect(s.state.ocrServices.every((x) => x.type !== 'openai-vision' || !x.enabled)).toBe(true)
  expect(s.state.ocrServices.find((x) => x.type === 'windows-media-ocr')!.enabled).toBe(true)
})
```

改写 `Windows 合并时强制 enabled=true` → **merge 尊重后端 enabled；全关后 normalize 开 Windows**：

```ts
it('merge 后全关则 normalize 强制 Windows on', () => {
  const local = [makeLocalOcr({ id: 'win', type: 'windows-media-ocr', enabled: true, ... })]
  const backend = [makeBackendOcr({ id: 'win', serviceType: 'windows-media-ocr', enabled: false, ... })]
  const result = normalizeOcrList(mergeBackendIntoOcrServices(local, backend))
  // 若 merge 后全关，normalize 打开 Windows
  expect(result.find((s) => s.type === 'windows-media-ocr')!.enabled).toBe(true)
})
```

若 `mergeBackendIntoOcrServices` 内部已 normalize，测试直接 assert merge 结果。

- [ ] **步骤 1：先改测试为新语义 → 运行失败**

```powershell
npm run test -- frontend/src/settings/stores/settings.test.ts
```

- [ ] **步骤 2：实现 store + tokens**

- [ ] **步骤 3：ServicesPanel UI**

1. 列表开关：去掉 `canDisable === false` 永开分支；统一：

```vue
<SettingSwitch
  :model-value="inst.enabled"
  :disabled="isOcrSwitchDisabled(inst)"
  :title="ocrSwitchTitle(inst)"
  @update:model-value="(v) => onOcrToggle(inst, v)"
/>
```

```ts
function isOcrSwitchDisabled(inst: OcrServiceInstance): boolean {
  const meta = ocrServiceById(inst.type)
  if (meta?.runtimeSupported === false) return true
  // 唯一 Windows 且 enabled：允许显示 on，但关闭会被 store 拒绝；也可 disabled
  return false
}

function onOcrToggle(inst: OcrServiceInstance, enabled: boolean): void {
  if (enabled && ocrServiceById(inst.type)?.runtimeSupported === false) {
    toast.error(t(msgKey('settings.toast.ocrUnsupported')), t(msgKey('settings.toast.ocrClaudeUnsupported')))
    return
  }
  if (!enabled) {
    const only = inst.enabled && props.state.ocrServices.filter((s) => s.enabled).length === 1
    if (only && inst.type === 'windows-media-ocr') {
      toast.error(t(msgKey('settings.toast.ocrCannotDisableLast')))
      return
    }
  }
  settings.setOcrEnabled(inst.id, enabled)
}
```

2. 详情：Claude 旁注「本版本截图识别不支持 Claude 视觉，请使用 OpenAI 兼容渠道或 Windows。」

3. 去掉/替换 system 与 vision 底部 `settings.ocr.configReserved` amber 条为：

- system：`settings.ocr.runtimeActiveHint` = 「当前启用的文字识别服务用于截图框选识别。」
- vision：同样 hint；若 `runtimeSupported===false` 用警告条

4. Windows 详情 `canDisable` 行：文案改为「可与视觉渠道互斥切换；不允许全部关闭。」

- [ ] **步骤 4：测试与 typecheck**

```powershell
npm run test -- frontend/src/settings/stores/settings.test.ts
npm run typecheck
```

- [ ] **步骤 5：Commit**

```powershell
git add frontend/src/settings/types.ts frontend/src/settings/tokens.ts frontend/src/settings/stores/settings.ts frontend/src/settings/stores/settings.test.ts frontend/src/settings/panels/ServicesPanel.vue
git commit -m "feat(settings): OCR 开关互斥、禁全关与 Claude 不可启用"
```

---

## 任务 8：i18n + README + 架构文档

**文件：**
- `frontend/src/i18n/locales/zh-CN.json`
- `frontend/src/i18n/locales/en-US.json`
- 若其它 locale 含 `settings.ocr.configReserved`，同步改键或保留 fallback
- `README.md`（约第 74 行）
- `docs/architecture/screenshot-ocr-architecture.md`（简短补「引擎选择」一节）

**中文键示例：**

```json
"settings.ocr.runtimeActiveHint": "截图识别使用当前启用的一项文字识别服务（Windows 或 OpenAI 兼容视觉）。",
"settings.ocr.claudeUnsupported": "本版本不支持 Claude 视觉识别，请改用 Windows 媒体 OCR 或 OpenAI 兼容视觉渠道。",
"settings.toast.ocrUnsupported": "无法启用",
"settings.toast.ocrClaudeUnsupported": "本版本不支持 Claude 视觉 OCR。",
"settings.toast.ocrCannotDisableLast": "至少保留一项文字识别服务。",
"settings.description.addOcrService": "仅多模态视觉渠道；与翻译实例独立。截图识别使用当前启用的一项 OCR。",
"settings.tooltip.ocrAlwaysEnabled": "至少保留一项启用的文字识别服务",
"settings.description.canDisable": "可与视觉渠道互斥切换；不允许全部关闭。"
```

英文对等翻译。删除或停用 `settings.ocr.configReserved` 的展示（键可留以免旧包报错）。

README 改：

```markdown
- 文字识别：设置页「服务 → 文字识别」管理 OCR 实例；截图识别使用**当前唯一启用**的引擎（Windows.Media.Ocr 或 OpenAI 兼容视觉模型，只识别文字后再走翻译批次）。Claude 视觉本版本不可启用。
```

- [ ] **步骤 1：改文案与文档**

- [ ] **步骤 2：验证**

```powershell
npm run typecheck
npm run test
```

- [ ] **步骤 3：Commit**

```powershell
git add frontend/src/i18n/locales/zh-CN.json frontend/src/i18n/locales/en-US.json README.md docs/architecture/screenshot-ocr-architecture.md
git commit -m "docs(ocr): 同步 OCR 运行时文案与 README"
```

---

## 任务 9：全量验证与规格回填

- [ ] **步骤 1：后端全测**

```powershell
cd src-tauri; cargo test
```

预期：全部 PASS（忽略带 `#[ignore]` 的 Windows 真机 OCR）。

- [ ] **步骤 2：前端**

```powershell
npm run test
npm run typecheck
```

预期：PASS。

- [ ] **步骤 3：手工验收清单（WebView2 / `npm run tauri dev`）**

| # | 步骤 | 期望 |
|---|---|---|
| 1 | 默认仅 Windows，Alt+S 框选 | 与现网一致，系统 OCR |
| 2 | 添加 OpenAI 兼容视觉，填 Key/模型并启用 | 互斥关 Windows；截图走视觉，出字后多服务翻译 |
| 3 | 关视觉 | 自动回 Windows，可识别 |
| 4 | 尝试启用 Claude 视觉 | UI 阻止 + toast |
| 5 | 错误 Key | 可读 OCR 错误，**不**静默变 Windows |
| 6 | 翻译多服务批次与设置保存 | 无回归 |

- [ ] **步骤 4：规格状态回填**

`docs/superpowers/specs/2026-07-15-multimodal-ocr-runtime-design.md` 状态改为「已实现」；计划本文件任务复选框在编码过程中勾选。

- [ ] **步骤 5：文档 commit（若有未提交回填）**

```powershell
git add docs/superpowers/specs/2026-07-15-multimodal-ocr-runtime-design.md docs/superpowers/plans/2026-07-15-multimodal-ocr-runtime.md
git commit -m "docs(ocr): 回填多模态 OCR 运行时实现状态"
```

---

## 自检

### 1. 规格覆盖度

| 规格章节 | 任务 |
|---|---|
| 5.1–5.2 resolve | 任务 3 |
| 5.3 normalized | 任务 2 |
| 5.4 前端互斥/禁全关/Claude | 任务 7 |
| 6 Vision 引擎 | 任务 4+5 |
| 7 编排接线 | 任务 6 |
| 8 错误 | 任务 1 + ocr_popup |
| 9 兼容/README | 任务 2+8 |
| 10 测试表 | 各任务 TDD + 任务 9 |
| 不回退 Windows | 任务 6 无 fallback 分支 |
| 不污染 TranslationProvider | 全任务无改 llm provider |

### 2. 占位符扫描

无 TODO/待定/「适当处理」；纯函数与签名均给出。

### 3. 类型一致性

- `ResolvedOcrEngine` / `VisionOcrConfig`：任务 3 定义，任务 5–6 使用同名
- `OcrError` 新变体：任务 1 定义，后续映射一致
- `DEFAULT_OCR_PROMPT` 与 tokens 文案一致
- `service_type` 字符串与 tokens `BuiltinOcrServiceId` 对齐

### 4. 风险备忘

- `image`/`base64` 新依赖：仅 OCR 路径
- 后端 seed Windows 的固定 id 与前端随机 id：仅空配置路径；merge 以 id 为准
- Claude 脏配置 enabled：resolve 报错；UI+normalize 尽量不让其成为唯一 enabled

---

## 执行交接说明

计划完成后由用户选择：

1. **子代理驱动（推荐）** — `subagent-driven-development`，每任务一子代理 + 审查  
2. **内联执行** — `executing-plans`，批量 + 检查点  

编码阶段开始前必须用 `AskUserQuestion` 确认执行方式；未答复不得写代码。
