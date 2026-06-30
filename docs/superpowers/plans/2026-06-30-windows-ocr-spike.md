# Windows OCR Spike 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 验证 Rust 后端能通过 `windows` crate 调用 `Windows.Media.Ocr`，并把内存图片识别为 `OcrResult` 或明确映射为 `OcrError`。

**架构：** 新增 `src-tauri/src/platform/windows/ocr.rs`，实现 `WindowsOcrEngine` 与 `OcrEngine` trait。该切片只处理 OCR 引擎、语言选择、raw pixel 到 `SoftwareBitmap` 的转换和错误映射；不接真实截图、不新增快捷键、不改 Web UI。

**技术栈：** Rust 2021、Tauri 2、`windows` crate、`async-trait`、`tokio` 测试运行时、现有 `core::capture` / `core::ocr` 抽象。

---

## 文件结构

### 修改文件

- `src-tauri/Cargo.toml`
  - 扩展 Windows 专用 `windows` crate features，加入 OCR、Graphics Imaging、Globalization、Foundation 等 WinRT API。

- `src-tauri/src/platform/windows/mod.rs`
  - 导出 `ocr` 模块。

- `docs/architecture/screenshot-ocr-architecture.md`
  - 在第一切片落地状态后补充 Windows OCR spike 状态。

### 新增文件

- `src-tauri/src/platform/windows/ocr.rs`
  - 定义 `WindowsOcrEngine`。
  - 实现 OCR 引擎可用性检测。
  - 实现 raw pixel 格式校验。
  - 实现语言选择。
  - 实现 `OcrEngine` trait。
  - 添加默认忽略的 Windows OCR 集成测试。

---

## 任务 1：扩展 windows crate features

**文件：**
- 修改：`src-tauri/Cargo.toml`
- 修改：`src-tauri/Cargo.lock`

- [ ] **步骤 1：修改 Windows 专用依赖**

将 `src-tauri/Cargo.toml` 中：

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = ["Graphics_Capture"] }
```

改为：

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
  "Foundation",
  "Globalization",
  "Graphics_Capture",
  "Graphics_Imaging",
  "Media_Ocr",
  "Storage_Streams",
] }
```

- [ ] **步骤 2：运行依赖解析验证**

运行：

```bash
cd src-tauri && cargo check
```

预期：依赖解析成功，`Cargo.lock` 按需更新，当前代码仍能编译。允许出现现有阶段性 `dead_code` warnings。

- [ ] **步骤 3：Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock

git commit -m "$(cat <<'EOF'
chore(ocr): 扩展 Windows OCR 依赖能力

Co-Authored-By: Claude Sonnet 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## 任务 2：新增 Windows OCR 语言选择与格式校验

**文件：**
- 创建：`src-tauri/src/platform/windows/ocr.rs`
- 修改：`src-tauri/src/platform/windows/mod.rs`

- [ ] **步骤 1：编写失败测试**

创建 `src-tauri/src/platform/windows/ocr.rs`，先写测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        capture::{CapturedImage, CapturedImageFormat},
        ocr::OcrError,
    };

    fn image(format: CapturedImageFormat, bytes: Vec<u8>, width: u32, height: u32) -> CapturedImage {
        CapturedImage { bytes, width, height, format }
    }

    #[test]
    fn rejects_png_input() {
        let error = validate_raw_image(&image(CapturedImageFormat::Png, vec![], 1, 1))
            .expect_err("PNG 在本切片中不支持");

        assert!(matches!(error, OcrError::ImageConversionFailed(_)));
    }

    #[test]
    fn rejects_mismatched_rgba_buffer_len() {
        let error = validate_raw_image(&image(CapturedImageFormat::Rgba8, vec![0, 1, 2], 1, 1))
            .expect_err("RGBA 字节长度必须匹配 width * height * 4");

        assert!(matches!(error, OcrError::ImageConversionFailed(_)));
    }

    #[test]
    fn accepts_matching_rgba_buffer_len() {
        validate_raw_image(&image(CapturedImageFormat::Rgba8, vec![0, 1, 2, 3], 1, 1))
            .expect("RGBA 字节长度匹配时应通过校验");
    }
}
```

- [ ] **步骤 2：导出模块以发现测试**

在 `src-tauri/src/platform/windows/mod.rs` 增加：

```rust
pub mod ocr;
```

- [ ] **步骤 3：运行测试验证失败**

运行：

```bash
cd src-tauri && cargo test validate_raw_image
```

预期：编译失败，报错包含 `cannot find function validate_raw_image`。

- [ ] **步骤 4：实现最小格式校验和引擎类型**

在测试前补充：

```rust
use crate::core::{
    capture::{CapturedImage, CapturedImageFormat},
    ocr::OcrError,
};

pub struct WindowsOcrEngine;

impl WindowsOcrEngine {
    pub fn is_available() -> bool {
        windows::Media::Ocr::OcrEngine::TryCreateFromUserProfileLanguages().is_ok()
    }
}

fn validate_raw_image(image: &CapturedImage) -> Result<(), OcrError> {
    match image.format {
        CapturedImageFormat::Png => Err(OcrError::ImageConversionFailed(
            "暂不支持 PNG OCR 输入".to_string(),
        )),
        CapturedImageFormat::Rgba8 | CapturedImageFormat::Bgra8 => {
            let expected_len = image
                .width
                .checked_mul(image.height)
                .and_then(|pixels| pixels.checked_mul(4))
                .map(|bytes| bytes as usize)
                .ok_or_else(|| OcrError::ImageConversionFailed("图片尺寸溢出".to_string()))?;

            if image.bytes.len() != expected_len {
                return Err(OcrError::ImageConversionFailed(format!(
                    "图片字节长度不匹配：期望 {expected_len}，实际 {}",
                    image.bytes.len()
                )));
            }

            Ok(())
        }
    }
}
```

- [ ] **步骤 5：运行测试验证通过**

运行：

```bash
cd src-tauri && cargo test validate_raw_image
```

预期：3 个格式校验测试通过。

- [ ] **步骤 6：运行完整测试**

运行：

```bash
cd src-tauri && cargo test
```

预期：所有测试通过。允许出现阶段性 `dead_code` warnings。

- [ ] **步骤 7：Commit**

```bash
git add src-tauri/src/platform/windows/mod.rs src-tauri/src/platform/windows/ocr.rs

git commit -m "$(cat <<'EOF'
feat(ocr): 添加 Windows OCR 输入校验

Co-Authored-By: Claude Sonnet 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## 任务 3：实现 SoftwareBitmap 转换与 OCR 调用

**文件：**
- 修改：`src-tauri/src/platform/windows/ocr.rs`

- [ ] **步骤 1：编写失败测试**

在 `src-tauri/src/platform/windows/ocr.rs` 的 tests 模块中增加：

```rust
#[test]
fn rejects_image_larger_than_max_dimension() {
    let max = 10;
    let error = validate_image_dimensions(11, 1, max).expect_err("超过最大边长应失败");

    assert_eq!(error, OcrError::ImageTooLarge);
}

#[test]
fn accepts_image_within_max_dimension() {
    validate_image_dimensions(10, 10, 10).expect("边长不超过限制应通过");
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
cd src-tauri && cargo test image_ -- --nocapture
```

预期：编译失败，报错包含 `cannot find function validate_image_dimensions`。

- [ ] **步骤 3：实现尺寸校验、bitmap 转换和 OCR trait**

在 `src-tauri/src/platform/windows/ocr.rs` 中补充必要 imports 和实现：

```rust
use crate::core::{
    capture::{CapturedImage, CapturedImageFormat},
    ocr::{OcrEngine, OcrError, OcrHints, OcrResult},
};

fn validate_image_dimensions(width: u32, height: u32, max_dimension: u32) -> Result<(), OcrError> {
    if width > max_dimension || height > max_dimension {
        return Err(OcrError::ImageTooLarge);
    }
    Ok(())
}

#[async_trait::async_trait]
impl OcrEngine for WindowsOcrEngine {
    async fn recognize(&self, image: CapturedImage, hints: OcrHints) -> Result<OcrResult, OcrError> {
        validate_raw_image(&image)?;
        validate_image_dimensions(
            image.width,
            image.height,
            windows::Media::Ocr::OcrEngine::MaxImageDimension()
                .map_err(|_| OcrError::EngineUnavailable)?,
        )?;

        let engine = create_engine(hints)?;
        let bitmap = captured_image_to_software_bitmap(image)?;
        let result = engine
            .RecognizeAsync(&bitmap)
            .map_err(|error| OcrError::ImageConversionFailed(error.to_string()))?
            .await
            .map_err(|error| OcrError::ImageConversionFailed(error.to_string()))?;

        convert_result(result)
    }
}
```

再实现私有函数：

- `create_engine(hints: OcrHints) -> Result<windows::Media::Ocr::OcrEngine, OcrError>`
- `captured_image_to_software_bitmap(image: CapturedImage) -> Result<windows::Graphics::Imaging::SoftwareBitmap, OcrError>`
- `convert_result(result: windows::Media::Ocr::OcrResult) -> Result<OcrResult, OcrError>`

实现约束：

- 如果 `hints.preferred_languages` 为空，使用 `TryCreateFromUserProfileLanguages()`。
- 如果 hints 非空，逐个尝试 `windows::Globalization::Language::CreateLanguage(language)` 与 `IsLanguageSupported`，第一个可用语言用于 `TryCreateFromLanguage`。
- 如果所有 hints 都不可用，返回最后一个语言的 `OcrError::LanguageUnavailable(language)`。
- `CapturedImageFormat::Rgba8` 和 `Bgra8` 必须转换成 `SoftwareBitmap` 可接受的 BGRA8 premultiplied 或 straight alpha 格式。
- 如果 `convert_result` 得到的全文 trim 后为空，返回 `OcrError::EmptyResult`。
- `OcrLine`、`OcrWord`、`OcrBoundingBox` 按 WinRT 结果映射；如果 word 边界读取失败，返回 `ImageConversionFailed`。

- [ ] **步骤 4：运行测试验证通过**

运行：

```bash
cd src-tauri && cargo test image_ -- --nocapture
```

预期：尺寸校验测试通过。

- [ ] **步骤 5：运行完整测试和构建**

运行：

```bash
cd src-tauri && cargo test
cd src-tauri && cargo build
```

预期：测试和构建通过。允许阶段性 `dead_code` warnings。

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/platform/windows/ocr.rs

git commit -m "$(cat <<'EOF'
feat(ocr): 接入 Windows Media OCR 引擎

Co-Authored-By: Claude Sonnet 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## 任务 4：添加默认忽略的 Windows OCR 集成测试

**文件：**
- 修改：`src-tauri/src/platform/windows/ocr.rs`

- [ ] **步骤 1：编写忽略测试**

在 tests 模块中增加：

```rust
#[tokio::test]
#[ignore]
async fn windows_ocr_engine_can_be_called_with_generated_bitmap() {
    if !WindowsOcrEngine::is_available() {
        return;
    }

    let image = image(CapturedImageFormat::Bgra8, vec![255; 32 * 32 * 4], 32, 32);
    let result = WindowsOcrEngine
        .recognize(image, OcrHints::default())
        .await;

    assert!(matches!(result, Ok(_) | Err(OcrError::EmptyResult)));
}
```

- [ ] **步骤 2：运行忽略测试验证可执行**

运行：

```bash
cd src-tauri && cargo test windows_ocr -- --ignored
```

预期：测试执行并通过；如果系统 OCR 不可用，测试应提前 return 并通过。

- [ ] **步骤 3：运行默认测试验证忽略测试不影响常规测试**

运行：

```bash
cd src-tauri && cargo test
```

预期：默认测试通过，忽略测试不执行。

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/platform/windows/ocr.rs

git commit -m "$(cat <<'EOF'
test(ocr): 添加 Windows OCR 集成验证

Co-Authored-By: Claude Sonnet 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## 任务 5：同步文档并最终验证

**文件：**
- 修改：`docs/architecture/screenshot-ocr-architecture.md`

- [ ] **步骤 1：同步架构文档状态**

在 `docs/architecture/screenshot-ocr-architecture.md` 的「第一切片落地状态」后增加：

```markdown
## Windows OCR Spike 落地状态

Windows OCR spike 已验证 `Windows.Media.Ocr` 接入路径：

- 已新增 `WindowsOcrEngine`，实现 `OcrEngine` trait。
- 已支持 `CapturedImageFormat::Rgba8` / `Bgra8` 到 Windows OCR 输入的转换。
- 已明确映射语言不可用、图片过大、格式不支持和 OCR 空文本错误。
- 已添加默认忽略的 Windows OCR 集成测试，用于人工验证真实 WinRT OCR 调用路径。

真实截图获取、OCR 快捷键和端到端截图翻译仍留给后续切片。
```

- [ ] **步骤 2：运行最终验证**

运行：

```bash
cd src-tauri && cargo test
cd src-tauri && cargo build
node --check frontend/main.js
git status --short
```

预期：

- `cargo test` 通过。
- `cargo build` 通过。
- `node --check frontend/main.js` 无输出且退出码为 0。
- `git status --short` 只显示文档变更。

- [ ] **步骤 3：Commit**

```bash
git add docs/architecture/screenshot-ocr-architecture.md

git commit -m "$(cat <<'EOF'
docs(architecture): 同步 Windows OCR spike 状态

Co-Authored-By: Claude Sonnet 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## 自检清单

- [ ] 规格覆盖度：计划覆盖 Windows OCR 可用性、语言选择、图片格式校验、`SoftwareBitmap` 转换、OCR 结果转换、错误映射、忽略集成测试和文档同步。
- [ ] 范围控制：计划不实现真实截图、不新增 OCR 快捷键、不改 Web UI。
- [ ] 类型一致性：`WindowsOcrEngine`、`CapturedImage`、`OcrHints`、`OcrResult`、`OcrError` 命名与现有代码一致。
- [ ] 验证闭环：每个代码任务都有失败测试、通过测试和 commit 步骤。
