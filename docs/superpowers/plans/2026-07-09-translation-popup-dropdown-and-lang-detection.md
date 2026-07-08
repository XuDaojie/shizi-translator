# 翻译弹窗语言下拉对齐原型 + 语言联动增强 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** ①翻译弹窗语言下拉改为 inline 搜索式 combobox（对齐高保真原型，规避浮层被 overflow 裁剪）；②source=auto 时模型回传检测到的原文语言并动态显示在 `.lang-badge`；③首次安装默认目标语言读 OS 语言，不在列表则回退英语。

**架构：** 下拉框纯前端改造（`translate.html/css/js`，inline picker 替换浮层 dropdown）；源语言检测在后端 `TranslationService::translate_with` 共用层加流式首行解析状态机（prompt 约定模型首行输出 `【源语言：xxx】`，状态机吞掉标记行、解析语言名、剩余作 Delta 透传），`TranslationEvent::Finished` 新增 `detectedSourceLang` 字段；OS 默认语言在 `AppConfig::from_env` 注入（`sys-locale` + `map_os_lang_to_list` 纯函数映射）。

**技术栈：** Rust（edition 2021，`sys-locale 0.3`、`tokio`、`async-trait`）、Tauri 2、原生静态前端（HTML/CSS/JS，无构建步骤，Tauri 直接加载）。

**spec 文档：** [docs/superpowers/specs/2026-07-08-translation-popup-dropdown-and-lang-detection-design.md](../specs/2026-07-08-translation-popup-dropdown-and-lang-detection-design.md)

---

## 对 spec 的实现细化（执行前必读）

以下 4 点是对 spec 的工程细化，已与 spec 意图一致，执行时按本计划实现：

1. **`setSourceBadge` 命名冲突修正**：spec 5.4 新增的 `setSourceBadge(text)` 操作 `.lang-badge`，但 `translate.js` 已存在 `setSourceBadge(sourceType)` 函数（操作 `#sourceBadge`/`.source-badge`，显示「来自划词」「来自 OCR」）。为避免同名冲突，**新增函数命名为 `setLangBadge(text)`**，操作 `.lang-badge`；现有 `setSourceBadge(sourceType)` 保持不变。

2. **`SHIZI_TARGET_LANG` 环境变量覆盖保留**：spec 6.2 字面写 `target_lang: default_target_lang_from_os()`，但现有 `from_env` 用 `env::var("SHIZI_TARGET_LANG").unwrap_or_else(|_| DEFAULT_TARGET_LANG.to_string())`。删除环境变量覆盖属未声明的行为回归（破坏开发/测试逃生口）。**保留 `SHIZI_TARGET_LANG` 覆盖**：环境变量非空时直接用其值（已是 code 形式），否则用 `default_target_lang_from_os()`。

3. **首行标记解析用字符串查找而非正则**：spec 5.2 写"正则 `/【源语言：(.+?)】/`"，但 `Cargo.toml` 无 `regex` 依赖。为避免引入新依赖（YAGNI），**用 `str::find` 字符串查找** `【源语言：` 前缀与 `】` 后缀提取语言名，语义等价。

4. **多服务 `detectedSourceLang` 统一处理**：spec 5.4"取首个非 null"。实现上单服务 `finished` 时若 `detectedSourceLang` 非空立即显示（即时反馈），`updateBatchStatus` 的 `allFinished` 分支统一取所有卡片中首个非 null 值，全 null 则隐藏（降级，不留「检测中…」占位）。

---

## 文件结构

### 后端（src-tauri/）

| 文件 | 职责 | 改动 |
|---|---|---|
| `Cargo.toml` | 依赖清单 | 加 `sys-locale = "0.3"` |
| `src/core/config/types.rs` | 配置模型与默认值 | `DEFAULT_TARGET_LANG` 拆为 `FALLBACK_TARGET_LANG` + `default_target_lang_from_os()` + `map_os_lang_to_list()`；`from_env`/`normalized` 改用 |
| `src/core/translation/types.rs` | 翻译请求/事件类型 | `user_prompt` auto 时追加检测指令；`TranslationEvent::Finished` 加 `detected_source_lang` 字段 |
| `src/core/translation/service.rs` | 翻译服务（流式编排） | `translate_with` 加首行解析状态机；新增 `HeaderParseState`/`parse_detected_lang`/`process_auto_delta` |
| `src/core/llm/mock.rs` | MockLlmProvider | auto 时首行输出 `【源语言：英语】\n` 标记，支持状态机手动验证 |

### 前端（frontend/public/）

| 文件 | 职责 | 改动 |
|---|---|---|
| `translate.html` | 翻译弹窗结构 | `.lang-toolbar` 后插 `.lang-picker`；`.lang-side` 改 `<button>`；`.lang-badge` 去静态文案 |
| `translate.css` | 弹窗样式 | 删 `.lang-dropdown*`；新增 `.lang-picker*`/`.lang-option*`/`@keyframes langPickerIn`；`.lang-badge:empty` 隐藏 |
| `translate.js` | 弹窗逻辑 | `LANGUAGES` 补 `english`；删浮层逻辑；新增 inline picker；新增 `setLangBadge` 并集成到 `renderLangLabels`/`renderTranslationEvent` |

### 文档（收尾硬门禁）

| 文件 | 改动 |
|---|---|
| `README.md` | 下拉 inline 搜索式行为、模型检测源语言、OS 默认目标语言 |
| `docs/roadmap/progressive-development-plan.md` | 标注相关项完成 |
| `CLAUDE.md` / `AGENTS.md` | 架构关键点补 inline combobox / Finished.detectedSourceLang / from_env 读 OS；前后端通信补 Finished 字段 |

---

## 任务 1：加 sys-locale 依赖

**文件：**
- 修改：`src-tauri/Cargo.toml`

- [ ] **步骤 1：在 `[dependencies]` 末尾加 `sys-locale`**

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 段（`chrono = ...` 行之后）追加：

```toml
sys-locale = "0.3"
```

- [ ] **步骤 2：验证依赖编译**

运行：`cd src-tauri && cargo build`
预期：编译成功，`sys-locale` 被拉取（首次会下载 crate）。

- [ ] **步骤 3：Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore(deps): 加 sys-locale 依赖用于读取 OS 语言"
```

---

## 任务 2：map_os_lang_to_list 纯函数（TDD）

**文件：**
- 修改：`src-tauri/src/core/config/types.rs`（实现 + 测试模块）

- [ ] **步骤 1：编写失败的测试**

在 `src-tauri/src/core/config/types.rs` 测试模块（`from_env_default_target_lang_is_zh_cn` 测试附近）追加：

```rust
    #[test]
    fn map_os_lang_exact_match() {
        assert_eq!(map_os_lang_to_list("zh-CN"), "zh-CN");
        assert_eq!(map_os_lang_to_list("en-US"), "en-US");
        assert_eq!(map_os_lang_to_list("fr-FR"), "fr-FR");
    }

    #[test]
    fn map_os_lang_zh_variants() {
        assert_eq!(map_os_lang_to_list("zh-Hans"), "zh-CN");
        assert_eq!(map_os_lang_to_list("zh-SG"), "zh-CN");
        assert_eq!(map_os_lang_to_list("zh-Hant"), "zh-TW");
        assert_eq!(map_os_lang_to_list("zh-HK"), "zh-TW");
        assert_eq!(map_os_lang_to_list("zh-TW"), "zh-TW");
    }

    #[test]
    fn map_os_lang_main_prefix() {
        assert_eq!(map_os_lang_to_list("en-GB"), "en-US");
        assert_eq!(map_os_lang_to_list("ja-JP"), "ja-JP");
        assert_eq!(map_os_lang_to_list("ko-KR"), "ko-KR");
        assert_eq!(map_os_lang_to_list("de-DE"), "de-DE");
        assert_eq!(map_os_lang_to_list("es-ES"), "es-ES");
        assert_eq!(map_os_lang_to_list("ru-RU"), "ru-RU");
    }

    #[test]
    fn map_os_lang_unmapped_falls_back_to_en_us() {
        assert_eq!(map_os_lang_to_list("th-TH"), "en-US");
        assert_eq!(map_os_lang_to_list("xx-YY"), "en-US");
        assert_eq!(map_os_lang_to_list(""), "en-US");
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib map_os_lang`
预期：FAIL，报错 `cannot find function 'map_os_lang_to_list'`。

- [ ] **步骤 3：实现 map_os_lang_to_list**

在 `src-tauri/src/core/config/types.rs` 顶层（`DEFAULT_TARGET_LANG` 常量附近，`default_source_lang` 等辅助函数同一区域）新增：

```rust
/// 把 OS locale（如 `zh-CN`、`zh-Hans`、`en-GB`）映射到语言下拉列表中的 code。
/// 精确匹配优先；否则按主语言前缀映射；都不匹配回退 `en-US`。
fn map_os_lang_to_list(os: &str) -> String {
    let lower = os.to_lowercase();
    let codes = [
        "zh-CN", "zh-TW", "en-US", "ja-JP", "ko-KR", "fr-FR", "de-DE", "es-ES", "ru-RU",
    ];
    if codes.contains(&lower.as_str()) {
        return lower;
    }
    let main = lower.split('-').next().unwrap_or("");
    let mapped = match main {
        "zh" => {
            if lower.contains("hant") || lower.contains("tw") || lower.contains("hk") {
                "zh-TW"
            } else {
                "zh-CN"
            }
        }
        "en" => "en-US",
        "ja" => "ja-JP",
        "ko" => "ko-KR",
        "fr" => "fr-FR",
        "de" => "de-DE",
        "es" => "es-ES",
        "ru" => "ru-RU",
        _ => "en-US",
    };
    mapped.to_string()
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib map_os_lang`
预期：4 个测试 PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/core/config/types.rs
git commit -m "feat(config): map_os_lang_to_list OS 语言映射纯函数"
```

---

## 任务 3：from_env 默认目标语言读 OS 语言

**文件：**
- 修改：`src-tauri/src/core/config/types.rs`

- [ ] **步骤 1：编写失败的测试**

将 `src-tauri/src/core/config/types.rs` 中现有测试：

```rust
    #[test]
    fn from_env_default_target_lang_is_zh_cn() {
        let config = AppConfig::from_env();
        assert_eq!(config.target_lang, "zh-CN");
    }
```

替换为：

```rust
    #[test]
    fn from_env_target_lang_uses_os_or_fallback() {
        let config = AppConfig::from_env();
        let valid = [
            "zh-CN", "zh-TW", "en-US", "ja-JP", "ko-KR", "fr-FR", "de-DE", "es-ES", "ru-RU",
        ];
        assert!(
            valid.contains(&config.target_lang.as_str()),
            "from_env target_lang 应是 OS 映射结果（列表 code 之一），实际: {}",
            config.target_lang
        );
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib from_env_target_lang_uses_os_or_fallback`
预期：可能 PASS（取决于测试机 locale），但 `from_env` 仍用旧 `DEFAULT_TARGET_LANG`。先继续实现，最后统一验证。

- [ ] **步骤 3：替换 DEFAULT_TARGET_LANG 常量**

在 `src-tauri/src/core/config/types.rs` 顶部，将：

```rust
const DEFAULT_TARGET_LANG: &str = "zh-CN";
```

替换为：

```rust
/// normalize 兜底用：target_lang 为空时回退英语（不读 OS，避免每次 save 查系统 locale）。
const FALLBACK_TARGET_LANG: &str = "en-US";

/// 首次安装默认目标语言：读 OS locale 并映射到语言列表 code，不在列表回退 en-US。
fn default_target_lang_from_os() -> String {
    let os = sys_locale::get_locale().unwrap_or_else(|| "en-US".to_string());
    map_os_lang_to_list(&os)
}
```

- [ ] **步骤 4：from_env 改用 OS 语言（保留环境变量覆盖）**

在 `AppConfig::from_env()` 中，将：

```rust
            target_lang: env::var("SHIZI_TARGET_LANG")
                .unwrap_or_else(|_| DEFAULT_TARGET_LANG.to_string()),
```

替换为：

```rust
            target_lang: env::var("SHIZI_TARGET_LANG")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(default_target_lang_from_os),
```

- [ ] **步骤 5：normalized 改用 FALLBACK_TARGET_LANG**

在 `AppConfig::normalized()` 中，将：

```rust
        self.target_lang = normalize_string(self.target_lang, DEFAULT_TARGET_LANG);
```

替换为：

```rust
        self.target_lang = normalize_string(self.target_lang, FALLBACK_TARGET_LANG);
```

- [ ] **步骤 6：运行全部 config 测试验证通过**

运行：`cd src-tauri && cargo test --lib config::types`
预期：所有测试 PASS（含 `from_env_target_lang_uses_os_or_fallback`、`serializes_camel_case`、`deserializes_with_defaults`）。

> 说明：`deserializes_with_defaults` 输入 `"targetLang": "zh-CN"`（非空），`normalize_string` 非空直接返回，不触发 `FALLBACK_TARGET_LANG`，断言 `"zh-CN"` 仍通过。

- [ ] **步骤 7：Commit**

```bash
git add src-tauri/src/core/config/types.rs
git commit -m "feat(config): from_env 默认目标语言读 OS 语言"
```

---

## 任务 4：user_prompt auto 时追加检测指令（TDD）

**文件：**
- 修改：`src-tauri/src/core/translation/types.rs`（实现 + 测试模块）

- [ ] **步骤 1：编写失败的测试**

在 `src-tauri/src/core/translation/types.rs` 测试模块追加（需在测试模块加 `use serde_json;`，若已有则跳过）：

```rust
    fn request_with_source_lang(source_lang: &str) -> TranslationRequest {
        TranslationRequest {
            session_id: TranslationSessionId("test".to_string()),
            input: TranslationInput::ManualText("hello".to_string()),
            target_lang: "中文".to_string(),
            service: fake_service(),
            prompts: TranslationPromptConfig {
                source_lang: source_lang.to_string(),
                ..Default::default()
            },
        }
    }

    #[test]
    fn user_prompt_appends_detection_instruction_when_auto() {
        let request = request_with_source_lang("auto");
        let prompt = request.user_prompt();
        assert!(
            prompt.contains("【源语言：语言名称】"),
            "auto 时 user_prompt 应含检测指令: {}",
            prompt
        );
        assert!(prompt.contains("hello"), "应含原文");
    }

    #[test]
    fn user_prompt_no_append_when_specific_source() {
        let request = request_with_source_lang("en-US");
        let prompt = request.user_prompt();
        assert!(
            !prompt.contains("【源语言："),
            "具体源语言时不应追加检测指令: {}",
            prompt
        );
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib user_prompt_appends_detection_instruction_when_auto`
预期：FAIL（当前 `user_prompt` 不追加指令，`user_prompt_no_append_when_specific_source` 可能 PASS）。

- [ ] **步骤 3：改造 user_prompt**

在 `src-tauri/src/core/translation/types.rs` 的 `impl TranslationRequest` 中，将 `user_prompt` 方法：

```rust
    pub fn user_prompt(&self) -> String {
        let template = self.prompts.translation_prompt.trim();
        if template.is_empty() {
            return format!(
                "请将以下文本翻译为{}：\n\n{}",
                self.target_lang,
                self.source_text()
            );
        }

        let rendered = template
            .replace("{source_lang}", &self.prompts.source_lang)
            .replace("{target_lang}", &self.target_lang)
            .replace("{text}", self.source_text());
        if template.contains("{text}") {
            rendered
        } else {
            format!("{rendered}\n\n{}", self.source_text())
        }
    }
```

替换为：

```rust
    pub fn user_prompt(&self) -> String {
        let template = self.prompts.translation_prompt.trim();
        let base = if template.is_empty() {
            format!(
                "请将以下文本翻译为{}：\n\n{}",
                self.target_lang,
                self.source_text()
            )
        } else {
            let rendered = template
                .replace("{source_lang}", &self.prompts.source_lang)
                .replace("{target_lang}", &self.target_lang)
                .replace("{text}", self.source_text());
            if template.contains("{text}") {
                rendered
            } else {
                format!("{rendered}\n\n{}", self.source_text())
            }
        };

        if self.prompts.source_lang == "auto" {
            format!(
                "{base}\n\n请先在第一行用【源语言：语言名称】输出你检测到的原文语言（如：英语、日语、中文），换行后再输出译文。"
            )
        } else {
            base
        }
    }
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib user_prompt`
预期：2 个测试 PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/core/translation/types.rs
git commit -m "feat(translation): user_prompt auto 时追加源语言检测指令"
```

---

## 任务 5：TranslationEvent::Finished 加 detectedSourceLang（TDD）

**文件：**
- 修改：`src-tauri/src/core/translation/types.rs`（实现 + 测试）
- 修改：`src-tauri/src/core/translation/service.rs`（构造 Finished 处暂传 None，任务 6 再接状态机）

- [ ] **步骤 1：编写失败的测试**

在 `src-tauri/src/core/translation/types.rs` 测试模块追加（需 `use serde_json;`）：

```rust
    fn finished_event(detected: Option<&str>) -> TranslationEvent {
        TranslationEvent::Finished {
            session_id: TranslationSessionId("s1".to_string()),
            service: fake_service(),
            full_text: "译文".to_string(),
            usage: None,
            detected_source_lang: detected.map(|s| s.to_string()),
        }
    }

    #[test]
    fn finished_event_serializes_with_detected_source_lang() {
        let json = serde_json::to_string(&finished_event(Some("英语"))).expect("序列化");
        assert!(
            json.contains("\"detectedSourceLang\":\"英语\""),
            "应输出 camelCase detectedSourceLang: {}",
            json
        );
    }

    #[test]
    fn finished_event_detected_source_lang_null_when_none() {
        let json = serde_json::to_string(&finished_event(None)).expect("序列化");
        assert!(
            json.contains("\"detectedSourceLang\":null"),
            "None 时应为 null: {}",
            json
        );
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib finished_event`
预期：FAIL，编译错误 `no field 'detected_source_lang'`。

- [ ] **步骤 3：Finished 加字段**

在 `src-tauri/src/core/translation/types.rs` 的 `TranslationEvent::Finished` 变体：

```rust
    Finished {
        session_id: TranslationSessionId,
        #[serde(flatten)]
        service: TranslationServiceMeta,
        full_text: String,
        usage: Option<TokenUsage>,
    },
```

替换为：

```rust
    Finished {
        session_id: TranslationSessionId,
        #[serde(flatten)]
        service: TranslationServiceMeta,
        full_text: String,
        usage: Option<TokenUsage>,
        detected_source_lang: Option<String>,
    },
```

> 说明：enum 级已有 `#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "type")]`，`detected_source_lang` 自动序列化为 `detectedSourceLang`。

- [ ] **步骤 4：service.rs 构造 Finished 暂传 None**

在 `src-tauri/src/core/translation/service.rs` 的 `translate_with` 中，将 `Finished` 构造：

```rust
            emit(TranslationEvent::Finished {
                session_id: request.session_id,
                service: request.service,
                full_text,
                usage,
            });
```

替换为：

```rust
            emit(TranslationEvent::Finished {
                session_id: request.session_id,
                service: request.service,
                full_text,
                usage,
                detected_source_lang: None,
            });
```

- [ ] **步骤 5：运行测试与编译验证**

运行：`cd src-tauri && cargo test --lib finished_event`
预期：2 个测试 PASS。

运行：`cd src-tauri && cargo build`
预期：编译成功（确认所有 `Finished` 构造点已更新；现有测试用 `TranslationEvent::Finished { usage, .. }`/`Finished { .. }` 的 `..` 模式不受影响）。

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/core/translation/types.rs src-tauri/src/core/translation/service.rs
git commit -m "feat(translation): TranslationEvent::Finished 加 detectedSourceLang"
```

---

## 任务 6：source=auto 流式首行解析状态机（TDD，核心）

**文件：**
- 修改：`src-tauri/src/core/translation/service.rs`（状态机 + translate_with 改造 + 测试）

- [ ] **步骤 1：编写失败的测试**

在 `src-tauri/src/core/translation/service.rs` 测试模块，先在 import 区（现有 `use crate::core::translation::{TokenUsage, TranslationInput, TranslationRequest, TranslationServiceMeta, TranslationSessionId};`）追加 `TranslationPromptConfig`：

```rust
    use crate::core::translation::{
        TokenUsage, TranslationInput, TranslationPromptConfig, TranslationRequest,
        TranslationServiceMeta, TranslationSessionId,
    };
```

在测试模块内追加 fake provider、辅助函数与 4 个测试：

```rust
    /// 可按预设 chunks 输出 Delta 的 fake provider，用于状态机测试。
    struct DetectFakeProvider {
        chunks: Vec<String>,
    }

    #[async_trait::async_trait]
    impl LlmProvider for DetectFakeProvider {
        async fn stream_translate(
            &self,
            _request: &TranslationRequest,
            on_event: &mut (dyn FnMut(LlmStreamEvent) + Send),
            _cancel: &CancellationToken,
        ) -> Result<(), LlmError> {
            for chunk in &self.chunks {
                on_event(LlmStreamEvent::Delta(chunk.clone()));
            }
            Ok(())
        }
    }

    fn request_with_source(source_lang: &str) -> TranslationRequest {
        TranslationRequest {
            session_id: TranslationSessionId("test-session".to_string()),
            input: TranslationInput::ManualText("hello".to_string()),
            target_lang: "中文".to_string(),
            service: fake_service(),
            prompts: TranslationPromptConfig {
                source_lang: source_lang.to_string(),
                ..Default::default()
            },
        }
    }

    fn collect_deltas(events: &[TranslationEvent]) -> String {
        events
            .iter()
            .filter_map(|e| match e {
                TranslationEvent::Delta { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    fn collect_detected(events: &[TranslationEvent]) -> Option<String> {
        events.iter().find_map(|e| match e {
            TranslationEvent::Finished {
                detected_source_lang, ..
            } => detected_source_lang.clone(),
            _ => None,
        })
    }

    async fn run_translate(provider: DetectFakeProvider, source_lang: &str) -> Vec<TranslationEvent> {
        let service = TranslationService::new(Arc::new(provider));
        let cancel = CancellationToken::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_for_task = events.clone();
        service
            .translate_with(request_with_source(source_lang), true, cancel, |event| {
                events_for_task.lock().unwrap().push(event);
            })
            .await
            .expect("应返回 Ok");
        let events = events.lock().unwrap();
        events.clone()
    }

    #[tokio::test]
    async fn translate_detects_source_lang_from_header() {
        let events = run_translate(
            DetectFakeProvider {
                chunks: vec!["【源语言：英语】\n译文内容".to_string()],
            },
            "auto",
        )
        .await;
        assert_eq!(collect_deltas(&events), "译文内容");
        assert_eq!(collect_detected(&events), Some("英语".to_string()));
    }

    #[tokio::test]
    async fn translate_fallbacks_when_no_header_marker() {
        let events = run_translate(
            DetectFakeProvider {
                chunks: vec!["译文无标记".to_string()],
            },
            "auto",
        )
        .await;
        assert_eq!(collect_deltas(&events), "译文无标记");
        assert_eq!(collect_detected(&events), None);
    }

    #[tokio::test]
    async fn translate_handles_marker_across_chunks() {
        let events = run_translate(
            DetectFakeProvider {
                chunks: vec!["【源语言：英".to_string(), "语】\n译文".to_string()],
            },
            "auto",
        )
        .await;
        assert_eq!(collect_deltas(&events), "译文");
        assert_eq!(collect_detected(&events), Some("英语".to_string()));
    }

    #[tokio::test]
    async fn translate_does_not_parse_when_source_specific() {
        let events = run_translate(
            DetectFakeProvider {
                chunks: vec!["【源语言：英语】\n译文".to_string()],
            },
            "en-US",
        )
        .await;
        assert_eq!(collect_deltas(&events), "【源语言：英语】\n译文");
        assert_eq!(collect_detected(&events), None);
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib translate_detects_source_lang_from_header`
预期：FAIL（当前无状态机，Delta 含标记、detected 为 None）。

- [ ] **步骤 3：新增状态机辅助类型与函数**

在 `src-tauri/src/core/translation/service.rs` 的 `impl TranslationService { ... }` 之后、`#[cfg(test)]` 之前新增：

```rust
/// source=auto 时的首行解析状态。
struct HeaderParseState {
    /// 累积首行字符，直到遇到首个 `\n`。
    pending: String,
    /// 首行是否已解析完毕。
    parsed: bool,
    /// 解析到的语言名（匹配 `【源语言：xxx】`）；未匹配为 None。
    detected: Option<String>,
}

/// 从首行 `【源语言：xxx】` 提取语言名；不匹配返回 None。
/// 用字符串查找而非正则，避免引入 regex 依赖。
fn parse_detected_lang(first_line: &str) -> Option<String> {
    const PREFIX: &str = "【源语言：";
    let start = first_line.find(PREFIX)?;
    let after = &first_line[start + PREFIX.len()..];
    let end = after.find('】')?;
    let name = after[..end].trim();
    if name.is_empty() {
        return None;
    }
    Some(name.to_string())
}

/// source=auto 时的流式首行解析：累积到首个 `\n` 后解析标记行，
/// 返回应作为 Delta 发出的纯译文片段（标记行被吞掉；标记不匹配则首行作 Delta 补发，不吞译文）。
fn process_auto_delta(state: &Mutex<HeaderParseState>, text: &str) -> Vec<String> {
    let mut st = state.lock().unwrap();
    if st.parsed {
        return vec![text.to_string()];
    }
    st.pending.push_str(text);
    let Some(pos) = st.pending.find('\n') else {
        return Vec::new();
    };
    let first_line = st.pending[..pos].to_string();
    let rest = st.pending[pos + 1..].to_string();
    st.parsed = true;
    st.detected = parse_detected_lang(&first_line);
    st.pending.clear();
    let mut out = Vec::new();
    if st.detected.is_none() {
        out.push(first_line);
    }
    if !rest.is_empty() {
        out.push(rest);
    }
    out
}
```

- [ ] **步骤 4：translate_with 接入状态机**

在 `src-tauri/src/core/translation/service.rs` 的 `translate_with` 中，将 `stream_translate` 调用块及其后的 Finished 构造（从 `let full_text = Arc::new(...)` 到 `emit(TranslationEvent::Finished {...})`）：

```rust
        let full_text = Arc::new(Mutex::new(String::new()));
        let usage: Arc<Mutex<Option<TokenUsage>>> = Arc::new(Mutex::new(None));
        let delta_text = full_text.clone();
        let usage_slot = usage.clone();
        let delta_session_id = request.session_id.clone();
        let delta_service = request.service.clone();

        self.provider
            .stream_translate(&request, &mut |ev| {
                match ev {
                    LlmStreamEvent::Delta(text) => {
                        if let Ok(mut t) = delta_text.lock() {
                            t.push_str(&text);
                        }
                        emit(TranslationEvent::Delta {
                            session_id: delta_session_id.clone(),
                            service: delta_service.clone(),
                            text,
                        });
                    }
                    LlmStreamEvent::Usage(u) => {
                        if collect_usage {
                            if let Ok(mut slot) = usage_slot.lock() {
                                *slot = Some(u);
                            }
                        }
                    }
                }
            }, &cancel)
            .await?;

        let full_text = full_text
            .lock()
            .map(|text| text.clone())
            .unwrap_or_default();

        if cancel.is_cancelled() {
            log::warn!(
                "翻译取消: service={} session={}",
                request.service.service_name,
                request.session_id.0
            );
            emit(TranslationEvent::Cancelled {
                session_id: request.session_id,
                service: request.service,
            });
        } else {
            let usage = usage
                .lock()
                .map(|slot| slot.clone())
                .unwrap_or(None);
            log::info!(
                "翻译完成: service={} session={} len={}",
                request.service.service_name,
                request.session_id.0,
                full_text.chars().count()
            );
            emit(TranslationEvent::Finished {
                session_id: request.session_id,
                service: request.service,
                full_text,
                usage,
                detected_source_lang: None,
            });
        }

        Ok(())
```

替换为：

```rust
        let is_auto = request.prompts.source_lang == "auto";
        let full_text = Arc::new(Mutex::new(String::new()));
        let usage: Arc<Mutex<Option<TokenUsage>>> = Arc::new(Mutex::new(None));
        let header_state = Arc::new(Mutex::new(HeaderParseState {
            pending: String::new(),
            parsed: false,
            detected: None,
        }));
        let delta_text = full_text.clone();
        let usage_slot = usage.clone();
        let header_slot = header_state.clone();
        let delta_session_id = request.session_id.clone();
        let delta_service = request.service.clone();

        self.provider
            .stream_translate(&request, &mut |ev| {
                match ev {
                    LlmStreamEvent::Delta(text) => {
                        let pieces = if is_auto {
                            process_auto_delta(&header_slot, &text)
                        } else {
                            vec![text]
                        };
                        for piece in pieces {
                            if let Ok(mut t) = delta_text.lock() {
                                t.push_str(&piece);
                            }
                            emit(TranslationEvent::Delta {
                                session_id: delta_session_id.clone(),
                                service: delta_service.clone(),
                                text: piece,
                            });
                        }
                    }
                    LlmStreamEvent::Usage(u) => {
                        if collect_usage {
                            if let Ok(mut slot) = usage_slot.lock() {
                                *slot = Some(u);
                            }
                        }
                    }
                }
            }, &cancel)
            .await?;

        // 译文极短无 `\n`：首行未解析，pending 累积的内容补作 Delta（不丢译文），detected 为 None。
        let detected = if is_auto {
            let mut st = header_state.lock().unwrap();
            if !st.parsed && !st.pending.is_empty() {
                let pending = std::mem::take(&mut st.pending);
                if let Ok(mut t) = delta_text.lock() {
                    t.push_str(&pending);
                }
                emit(TranslationEvent::Delta {
                    session_id: delta_session_id.clone(),
                    service: delta_service.clone(),
                    text: pending,
                });
            }
            st.detected.clone()
        } else {
            None
        };

        let full_text = full_text
            .lock()
            .map(|text| text.clone())
            .unwrap_or_default();

        if cancel.is_cancelled() {
            log::warn!(
                "翻译取消: service={} session={}",
                request.service.service_name,
                request.session_id.0
            );
            emit(TranslationEvent::Cancelled {
                session_id: request.session_id,
                service: request.service,
            });
        } else {
            let usage = usage
                .lock()
                .map(|slot| slot.clone())
                .unwrap_or(None);
            log::info!(
                "翻译完成: service={} session={} len={}",
                request.service.service_name,
                request.session_id.0,
                full_text.chars().count()
            );
            emit(TranslationEvent::Finished {
                session_id: request.session_id,
                service: request.service,
                full_text,
                usage,
                detected_source_lang: detected,
            });
        }

        Ok(())
```

- [ ] **步骤 5：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib translate_`
预期：4 个状态机测试 PASS，且现有 `emits_cancelled_when_cancelled_before_completion`/`emits_finished_when_not_cancelled`/`finished_carries_usage_when_collect_enabled`/`finished_usage_none_when_collect_disabled` 不回归。

- [ ] **步骤 6：运行全量后端测试**

运行：`cd src-tauri && cargo test`
预期：全部 PASS。

- [ ] **步骤 7：Commit**

```bash
git add src-tauri/src/core/translation/service.rs
git commit -m "feat(translation): source=auto 流式首行解析检测源语言"
```

---

## 任务 7：MockLlmProvider auto 时输出源语言标记

**文件：**
- 修改：`src-tauri/src/core/llm/mock.rs`

- [ ] **步骤 1：编写失败的测试**

在 `src-tauri/src/core/llm/mock.rs` 测试模块追加：

```rust
    #[tokio::test]
    async fn mock_emits_detection_header_when_auto() {
        let provider = MockLlmProvider;
        let cancel = CancellationToken::new();
        let mut events = Vec::new();
        let mut req = request();
        req.prompts.source_lang = "auto".to_string();
        provider
            .stream_translate(&req, &mut |ev: LlmStreamEvent| events.push(ev), &cancel)
            .await
            .expect("mock 应成功");
        let text: String = events
            .iter()
            .filter_map(|ev| match ev {
                LlmStreamEvent::Delta(t) => Some(t.clone()),
                _ => None,
            })
            .collect();
        assert!(
            text.starts_with("【源语言：英语】\n"),
            "auto 时应首行输出标记: {}",
            text
        );
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib mock_emits_detection_header_when_auto`
预期：FAIL（当前 mock 不输出标记）。

- [ ] **步骤 3：改造 MockLlmProvider::stream_translate**

在 `src-tauri/src/core/llm/mock.rs` 的 `stream_translate` 中，将 chunks 构造：

```rust
        let chunks = [
            "[Mock 翻译] ".to_string(),
            request.source_text().to_string(),
            " -> ".to_string(),
            request.target_lang.clone(),
        ];
```

替换为：

```rust
        let is_auto = request.prompts.source_lang == "auto";
        let mut chunks: Vec<String> = Vec::new();
        if is_auto {
            chunks.push("【源语言：英语】\n".to_string());
        }
        chunks.push("[Mock 翻译] ".to_string());
        chunks.push(request.source_text().to_string());
        chunks.push(" -> ".to_string());
        chunks.push(request.target_lang.clone());
```

- [ ] **步骤 4：运行 mock 测试验证通过且不回归**

运行：`cd src-tauri && cargo test --lib mock_`
预期：3 个 mock 测试（含新增）PASS。现有 `mock_emits_usage_at_end`/`mock_emits_delta_before_usage` 用 `request()`（`prompts: Default::default()`，source_lang 空，非 auto），走原逻辑，不回归。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/core/llm/mock.rs
git commit -m "feat(llm): MockLlmProvider auto 时输出源语言标记"
```

---

## 任务 8：前端 .lang-badge 动态显示检测语言

**文件：**
- 修改：`frontend/public/translate.html`
- 修改：`frontend/public/translate.css`
- 修改：`frontend/public/translate.js`

- [ ] **步骤 1：translate.html .lang-badge 去静态文案**

在 `frontend/public/translate.html` 中，将：

```html
            <span class="lang-badge">自动检测</span>
```

替换为：

```html
            <span class="lang-badge" id="langBadge"></span>
```

- [ ] **步骤 2：translate.css .lang-badge 空时隐藏**

在 `frontend/public/translate.css` 的 `.lang-badge { ... }` 规则之后追加：

```css
.lang-badge:empty { display: none; }
```

- [ ] **步骤 3：translate.js 新增 langBadge 引用与 setLangBadge**

在 `frontend/public/translate.js` 的元素引用区（`const sourceBadge = ...` 附近）追加：

```js
const langBadge = document.getElementById('langBadge');
```

在 `/* === 来源徽章 === */` 区块的 `setSourceBadge` 函数之后新增：

```js
/* === 检测语言徽章（.lang-badge） === */
function setLangBadge(text) {
  langBadge.textContent = text || '';
}
```

- [ ] **步骤 4：renderLangLabels 末尾联动 setLangBadge**

将 `renderLangLabels`：

```js
function renderLangLabels() {
  langSource.querySelector('.lang-label').textContent = LANG_LABEL(sessionSourceLang);
  langTarget.querySelector('.lang-label').textContent = LANG_LABEL(sessionTargetLang);
}
```

替换为：

```js
function renderLangLabels() {
  langSource.querySelector('.lang-label').textContent = LANG_LABEL(sessionSourceLang);
  langTarget.querySelector('.lang-label').textContent = LANG_LABEL(sessionTargetLang);
  setLangBadge(sessionSourceLang === 'auto' ? '检测中…' : '');
}
```

- [ ] **步骤 5：getCard 返回的 ref 加 detectedSourceLang 字段**

在 `frontend/public/translate.js` 的 `getCard` 函数末尾，将：

```js
  const ref = { el: card, text, actions, tokens, inputTokens, outputTokens, status: 'pending' };
```

替换为：

```js
  const ref = {
    el: card,
    text,
    actions,
    tokens,
    inputTokens,
    outputTokens,
    status: 'pending',
    detectedSourceLang: null,
  };
```

- [ ] **步骤 6：renderTranslationEvent started 设置「检测中…」**

在 `renderTranslationEvent` 的 `case 'started':` 的 `isNewBatch` 块内，将：

```js
        setSourceBadge(payload.sourceType);
        isTranslating = true;
```

替换为：

```js
        setSourceBadge(payload.sourceType);
        setLangBadge(sessionSourceLang === 'auto' ? '检测中…' : '');
        isTranslating = true;
```

- [ ] **步骤 7：renderTranslationEvent finished 记录并显示 detectedSourceLang**

在 `renderTranslationEvent` 的 `case 'finished':` 内，将：

```js
      card.text.textContent = payload.fullText ?? card.text.textContent;
      card.text.style.color = '';
      setStreamCursor(card, false);
      setHeaderDot(card, false);
```

替换为：

```js
      card.text.textContent = payload.fullText ?? card.text.textContent;
      card.text.style.color = '';
      setStreamCursor(card, false);
      setHeaderDot(card, false);
      card.detectedSourceLang = payload.detectedSourceLang ?? null;
      if (sessionSourceLang === 'auto' && payload.detectedSourceLang) {
        setLangBadge(payload.detectedSourceLang);
      }
```

- [ ] **步骤 8：updateBatchStatus allFinished 统一取首个非 null**

在 `updateBatchStatus` 的 `if (allFinished) {` 块内，将：

```js
  if (allFinished) {
    isTranslating = false;
    currentBatchId = null;
    setSourceBadge(null);
    setStatus({ text: '翻译完成', loading: false, action: { label: '重试', onClick: retryTranslation } });
    applyPendingConfigRefresh();
  }
```

替换为：

```js
  if (allFinished) {
    isTranslating = false;
    currentBatchId = null;
    setSourceBadge(null);
    if (sessionSourceLang === 'auto') {
      const detected = cards.find((c) => c.detectedSourceLang)?.detectedSourceLang;
      setLangBadge(detected || '');
    }
    setStatus({ text: '翻译完成', loading: false, action: { label: '重试', onClick: retryTranslation } });
    applyPendingConfigRefresh();
  }
```

- [ ] **步骤 9：验证前端构建**

运行：`npm run build`
预期：构建成功。

运行：`npm run typecheck`
预期：通过（translate.js/html/css 不在 typecheck 范围，settings 无改动不应破坏）。

- [ ] **步骤 10：Commit**

```bash
git add frontend/public/translate.html frontend/public/translate.css frontend/public/translate.js
git commit -m "feat(popup): 检测源语言动态显示在 lang-badge"
```

---

## 任务 9：语言下拉改 inline 搜索式 combobox

**文件：**
- 修改：`frontend/public/translate.html`
- 修改：`frontend/public/translate.css`
- 修改：`frontend/public/translate.js`

- [ ] **步骤 1：translate.html .lang-side 改 button + 插入 .lang-picker**

在 `frontend/public/translate.html` 中，将 `.lang-toolbar` 块：

```html
      <div class="lang-toolbar">
        <div class="lang-side" id="langSource">
          <span class="lang-label">自动检测</span>
          <svg class="lang-chevron" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>
        </div>
        <div class="lang-swap" id="langSwap" title="交换语言">
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M7 16l-4-4 4-4"/><path d="M17 8l4 4-4 4"/><line x1="3" y1="12" x2="21" y2="12"/></svg>
        </div>
        <div class="lang-side" id="langTarget">
          <span class="lang-label">简体中文</span>
          <svg class="lang-chevron" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>
        </div>
      </div>
```

替换为（`.lang-side` 改 `<button type="button">`，`.lang-toolbar` 后插入 `.lang-picker`）：

```html
      <div class="lang-toolbar">
        <button type="button" class="lang-side" id="langSource">
          <span class="lang-label">自动检测</span>
          <svg class="lang-chevron" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>
        </button>
        <div class="lang-swap" id="langSwap" title="交换语言">
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M7 16l-4-4 4-4"/><path d="M17 8l4 4-4 4"/><line x1="3" y1="12" x2="21" y2="12"/></svg>
        </div>
        <button type="button" class="lang-side" id="langTarget">
          <span class="lang-label">简体中文</span>
          <svg class="lang-chevron" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>
        </button>
      </div>

      <div class="lang-picker" id="langPicker" hidden>
        <div class="lang-picker-search">
          <svg class="lang-picker-search-icon" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="7"/><line x1="20" y1="20" x2="16.65" y2="16.65"/></svg>
          <input type="text" class="lang-picker-input" id="langPickerInput" placeholder="搜索语言…" autocomplete="off" spellcheck="false" />
        </div>
        <ul class="lang-picker-list" id="langPickerList"></ul>
      </div>
```

- [ ] **步骤 2：translate.css 删 .lang-dropdown* 并新增 .lang-picker***

在 `frontend/public/translate.css` 中，将整个 `/* === 语言下拉 === */` 区块：

```css
/* === 语言下拉 === */
.lang-dropdown {
  position: absolute;
  z-index: 50;
  background: var(--bg-card);
  border: 0.5px solid var(--border);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-popup);
  padding: 4px;
  max-height: 240px;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--border-2) transparent;
}
.lang-dropdown-item {
  display: block;
  width: 100%;
  text-align: left;
  border: none;
  background: transparent;
  font-family: var(--font-family);
  font-size: 0.75rem;
  color: var(--fg);
  padding: 6px 10px;
  border-radius: 5px;
  cursor: pointer;
  transition: background .12s, color .12s;
}
.lang-dropdown-item:hover { background: var(--bg-soft); color: var(--accent); }
.lang-dropdown-item.selected { color: var(--accent); font-weight: 600; }
.lang-dropdown-item:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
```

替换为：

```css
/* === 语言选择器（inline 搜索式 combobox，不依赖浮层，避免被弹窗 overflow 截断） === */
.lang-picker {
  background: var(--bg-card);
  border-radius: var(--radius-md);
  border: 0.5px solid var(--border);
  box-shadow: var(--shadow-card);
  overflow: hidden;
  animation: langPickerIn .15s ease;
}
.lang-picker[hidden] { display: none; }
.lang-picker-search {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 10px;
  border-bottom: 0.5px solid var(--border);
  background: var(--bg-soft);
}
.lang-picker-search-icon { width: 13px; height: 13px; color: var(--fg-3); flex-shrink: 0; }
.lang-picker-input {
  flex: 1;
  min-width: 0;
  border: none;
  background: transparent;
  font-family: var(--font-family);
  font-size: 0.75rem;
  color: var(--fg);
  outline: none;
}
.lang-picker-input::placeholder { color: var(--fg-3); }
.lang-picker-list {
  list-style: none;
  max-height: 220px;
  overflow-y: auto;
  padding: 4px 0;
  scrollbar-width: thin;
  scrollbar-color: var(--border-2) transparent;
}
.lang-picker-list::-webkit-scrollbar { width: 4px; }
.lang-picker-list::-webkit-scrollbar-thumb { background: var(--border-2); border-radius: 999px; }
.lang-picker-list::-webkit-scrollbar-track { background: transparent; }
.lang-option {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  padding: 6px 12px;
  font-size: 0.75rem;
  color: var(--fg);
  cursor: pointer;
  transition: background .08s;
}
.lang-option:hover,
.lang-option.is-active { background: var(--bg-soft); }
.lang-option.is-selected { color: var(--accent); font-weight: 600; }
.lang-option.is-selected .lang-option-english { color: var(--accent); opacity: .7; }
.lang-option-native { flex-shrink: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.lang-option-english { color: var(--fg-3); font-size: 0.6875rem; flex-shrink: 0; }
@keyframes langPickerIn {
  from { opacity: 0; transform: translateY(-4px); }
  to   { opacity: 1; transform: translateY(0); }
}
```

- [ ] **步骤 3：translate.js LANGUAGES 补 english**

在 `frontend/public/translate.js` 中，将 `LANGUAGES`：

```js
const LANGUAGES = [
  { value: 'auto', label: '自动检测' },
  { value: 'zh-CN', label: '简体中文' },
  { value: 'zh-TW', label: '繁體中文' },
  { value: 'en-US', label: 'English' },
  { value: 'ja-JP', label: '日本語' },
  { value: 'ko-KR', label: '한국어' },
  { value: 'fr-FR', label: 'Français' },
  { value: 'de-DE', label: 'Deutsch' },
  { value: 'es-ES', label: 'Español' },
  { value: 'ru-RU', label: 'Русский' },
];
```

替换为：

```js
const LANGUAGES = [
  { value: 'auto',  label: '自动检测', english: 'Auto Detect' },
  { value: 'zh-CN', label: '简体中文', english: 'Chinese (Simplified)' },
  { value: 'zh-TW', label: '繁體中文', english: 'Chinese (Traditional)' },
  { value: 'en-US', label: 'English', english: 'English' },
  { value: 'ja-JP', label: '日本語',   english: 'Japanese' },
  { value: 'ko-KR', label: '한국어',   english: 'Korean' },
  { value: 'fr-FR', label: 'Français', english: 'French' },
  { value: 'de-DE', label: 'Deutsch',  english: 'German' },
  { value: 'es-ES', label: 'Español',  english: 'Spanish' },
  { value: 'ru-RU', label: 'Русский',  english: 'Russian' },
];
```

- [ ] **步骤 4：translate.js 新增 picker 元素引用**

在 `frontend/public/translate.js` 的元素引用区（`const langTarget = ...` 附近）追加：

```js
const langPicker = document.getElementById('langPicker');
const langPickerInput = document.getElementById('langPickerInput');
const langPickerList = document.getElementById('langPickerList');
```

- [ ] **步骤 5：translate.js 删浮层逻辑、新增 inline picker**

在 `frontend/public/translate.js` 中，将整个 `/* === 语言下拉 === */` 区块：

```js
/* === 语言下拉 === */
let activeDropdown = null;

function closeDropdown() {
  if (activeDropdown) {
    activeDropdown.remove();
    activeDropdown = null;
    document.removeEventListener('mousedown', onDropdownOutsideClick, true);
    document.removeEventListener('keydown', onDropdownEsc, true);
  }
}

function onDropdownOutsideClick(e) {
  if (activeDropdown && !activeDropdown.contains(e.target) && !e.target.closest('.lang-side')) {
    closeDropdown();
  }
}

function onDropdownEsc(e) {
  if (e.key === 'Escape') closeDropdown();
}

function openDropdown(side) {
  closeDropdown();
  const options = side === 'source'
    ? LANGUAGES
    : LANGUAGES.filter((l) => l.value !== 'auto');
  const current = side === 'source' ? sessionSourceLang : sessionTargetLang;
  const dd = document.createElement('div');
  dd.className = 'lang-dropdown';
  options.forEach((opt) => {
    const item = document.createElement('button');
    item.type = 'button';
    item.className = 'lang-dropdown-item' + (opt.value === current ? ' selected' : '');
    item.textContent = opt.label;
    item.addEventListener('click', () => {
      selectLang(side, opt.value);
      closeDropdown();
    });
    dd.appendChild(item);
  });
  const anchor = side === 'source' ? langSource : langTarget;
  anchor.parentElement.appendChild(dd);
  const rect = anchor.getBoundingClientRect();
  const parentRect = anchor.parentElement.getBoundingClientRect();
  dd.style.left = (rect.left - parentRect.left) + 'px';
  dd.style.top = (rect.bottom - parentRect.top) + 'px';
  dd.style.minWidth = rect.width + 'px';
  activeDropdown = dd;
  document.addEventListener('mousedown', onDropdownOutsideClick, true);
  document.addEventListener('keydown', onDropdownEsc, true);
}

async function selectLang(side, code) {
  if (side === 'source') sessionSourceLang = code;
  else sessionTargetLang = code;
  renderLangLabels();
  try {
    await invoke('set_session_languages', { sourceLang: sessionSourceLang, targetLang: sessionTargetLang });
  } catch (e) {
    showToast(String(e));
  }
}

async function swapLangs() {
  if (sessionSourceLang === 'auto' || sessionTargetLang === 'auto') {
    showToast('自动检测不支持交换');
    return;
  }
  [sessionSourceLang, sessionTargetLang] = [sessionTargetLang, sessionSourceLang];
  renderLangLabels();
  try {
    await invoke('set_session_languages', { sourceLang: sessionSourceLang, targetLang: sessionTargetLang });
  } catch (e) {
    showToast(String(e));
  }
}
```

替换为：

```js
/* === 语言选择器（inline 搜索式 combobox） === */
let activeLangType = null; // null | 'source' | 'target'

function openLangPicker(side) {
  if (activeLangType === side) {
    closeLangPicker();
    return;
  }
  activeLangType = side;
  langPickerInput.placeholder = side === 'source' ? '搜索源语言…' : '搜索目标语言…';
  langPickerInput.value = '';
  renderLangList('');
  langPicker.hidden = false;
  adjustHeight();
  requestAnimationFrame(() => langPickerInput.focus());
}

function closeLangPicker() {
  langPicker.hidden = true;
  activeLangType = null;
  adjustHeight();
}

function renderLangList(query) {
  const q = (query || '').trim().toLowerCase();
  const list = activeLangType === 'source'
    ? LANGUAGES
    : LANGUAGES.filter((l) => l.value !== 'auto');
  const filtered = q
    ? list.filter((l) => l.label.toLowerCase().includes(q) || l.english.toLowerCase().includes(q))
    : list;
  const current = activeLangType === 'source' ? sessionSourceLang : sessionTargetLang;
  langPickerList.innerHTML = '';
  filtered.forEach((opt) => {
    const li = document.createElement('li');
    li.className = 'lang-option' + (opt.value === current ? ' is-selected' : '');
    li.dataset.value = opt.value;
    li.innerHTML =
      '<span class="lang-option-native">' + opt.label + '</span>' +
      '<span class="lang-option-english">' + opt.english + '</span>';
    li.addEventListener('click', () => selectLang(activeLangType, opt.value));
    langPickerList.appendChild(li);
  });
  const active = langPickerList.querySelector('.is-selected') || langPickerList.firstElementChild;
  if (active) {
    active.classList.add('is-active');
    active.scrollIntoView({ block: 'nearest' });
  }
}

async function selectLang(side, code) {
  if (side === 'source') sessionSourceLang = code;
  else sessionTargetLang = code;
  renderLangLabels();
  closeLangPicker();
  try {
    await invoke('set_session_languages', { sourceLang: sessionSourceLang, targetLang: sessionTargetLang });
  } catch (e) {
    showToast(String(e));
  }
}

async function swapLangs() {
  if (activeLangType) closeLangPicker();
  if (sessionSourceLang === 'auto' || sessionTargetLang === 'auto') {
    showToast('自动检测不支持交换');
    return;
  }
  [sessionSourceLang, sessionTargetLang] = [sessionTargetLang, sessionSourceLang];
  renderLangLabels();
  try {
    await invoke('set_session_languages', { sourceLang: sessionSourceLang, targetLang: sessionTargetLang });
  } catch (e) {
    showToast(String(e));
  }
}
```

- [ ] **步骤 6：translate.js 绑定 picker 事件、改 langSource/langTarget 点击**

在 `frontend/public/translate.js` 中，将工具栏按钮绑定区：

```js
langSource.addEventListener('click', () => openDropdown('source'));
langTarget.addEventListener('click', () => openDropdown('target'));
langSwap.addEventListener('click', swapLangs);
```

替换为：

```js
langSource.addEventListener('click', () => openLangPicker('source'));
langTarget.addEventListener('click', () => openLangPicker('target'));
langSwap.addEventListener('click', swapLangs);

langPickerInput.addEventListener('input', () => renderLangList(langPickerInput.value));
langPickerInput.addEventListener('keydown', (e) => {
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    e.preventDefault();
    const items = Array.from(langPickerList.querySelectorAll('.lang-option'));
    if (items.length === 0) return;
    let idx = items.findIndex((el) => el.classList.contains('is-active'));
    if (idx === -1) idx = 0;
    idx = e.key === 'ArrowDown' ? (idx + 1) % items.length : (idx - 1 + items.length) % items.length;
    items.forEach((el) => el.classList.remove('is-active'));
    items[idx].classList.add('is-active');
    items[idx].scrollIntoView({ block: 'nearest' });
  } else if (e.key === 'Enter') {
    e.preventDefault();
    const active = langPickerList.querySelector('.is-active');
    if (active && active.dataset.value) {
      selectLang(activeLangType, active.dataset.value);
    }
  } else if (e.key === 'Escape') {
    e.preventDefault();
    closeLangPicker();
  }
});

document.addEventListener('mousedown', (e) => {
  if (!activeLangType) return;
  if (langPicker.contains(e.target)) return;
  if (e.target.closest('.lang-side')) return;
  closeLangPicker();
});
```

- [ ] **步骤 7：验证前端构建**

运行：`npm run build`
预期：构建成功。

运行：`npm run typecheck`
预期：通过。

- [ ] **步骤 8：Commit**

```bash
git add frontend/public/translate.html frontend/public/translate.css frontend/public/translate.js
git commit -m "feat(popup): 语言下拉改 inline 搜索式 combobox"
```

---

## 任务 10：文档同步（收尾硬门禁）

**文件：**
- 修改：`README.md`
- 修改：`docs/roadmap/progressive-development-plan.md`
- 修改：`CLAUDE.md`、`AGENTS.md`

- [ ] **步骤 1：README.md 补充当前能力**

在 `README.md` 的能力/特性章节，补充三点（措辞与现有风格一致）：
- 翻译弹窗语言下拉为 inline 搜索式 combobox（带搜索框、英文名双列、键盘 ↑↓/Enter/Esc 导航），不被弹窗 overflow 裁剪。
- 源语言选「自动检测」时，模型回传检测到的原文语言并显示在译文区右下角标签；翻译中显示「检测中…」。
- 首次安装默认目标语言读操作系统语言，不在支持列表则回退英语；存量用户已选目标语言不受影响。

- [ ] **步骤 2：roadmap 标注完成**

在 `docs/roadmap/progressive-development-plan.md` 中，将本次需求对应条目标注为完成（勾选复选框或更新状态列）。

- [ ] **步骤 3：CLAUDE.md / AGENTS.md 同步架构关键点**

在 `CLAUDE.md` 与 `AGENTS.md`（两者须同步，见开发说明第 1 条）的「架构关键点」章节，补充：
- 翻译弹窗语言下拉为 inline 搜索式 combobox（`.lang-picker`，非浮层，规避 `.content` overflow 裁剪）。
- `TranslationEvent::Finished` 含 `detectedSourceLang: Option<String>`（source=auto 时由 `TranslationService::translate_with` 流式首行解析状态机填充，序列化为 camelCase `detectedSourceLang`）。
- `AppConfig::from_env` 默认 `target_lang` 读 OS 语言（`sys-locale` + `map_os_lang_to_list`），`normalized` 兜底用常量 `FALLBACK_TARGET_LANG = "en-US"`（不读 OS）。

在「前后端通信」章节，`translation:event` 的 `Finished` 补注 `detectedSourceLang` 字段。

- [ ] **步骤 4：Commit**

```bash
git add README.md docs/roadmap/progressive-development-plan.md CLAUDE.md AGENTS.md
git commit -m "docs: 同步下拉 inline/源语言检测/OS 默认语言文档"
```

---

## 手动验证清单（npm run tauri dev）

全部任务落地、`cargo test` + `npm run build` 通过后执行：

1. **下拉 inline**：点源语言 -> picker 出现在工具栏下方（非浮层），10 项含「自动检测」，搜索框自动获焦；点目标 -> 9 项无「自动检测」。
2. **搜索**：输「英」出 English；输「japanese」出日本語；输「chinese」出简体/繁体中文。
3. **键盘导航**：↑↓ 移动 `is-active` 焦点（滚动跟随），Enter 选中，Esc 关闭。
4. **toggle/外部点击/swap 关闭**：同 side 再点关闭；点 picker 外区域关闭；点 swap 关闭。
5. **不裁剪**：翻译中打开 picker -> `.content` 撑高，下拉项完整可见（ResizeObserver 调高弹窗）。
6. **开关动画**：picker 显示有淡入下移（`langPickerIn`）。
7. **OS 默认目标**：删除 app config dir 下 `config.json` 后启动 -> 弹窗目标 = OS 语言（中文 OS -> 简体中文；英文 OS -> English）。
8. **OS 不在列表**：OS 设为泰语 -> 默认目标 English。
9. **检测源语言**：source=auto，输入英语原文翻译 -> 译文区右下角 `.lang-badge` 显示「英语」；翻译中显示「检测中…」。
10. **具体源语言**：source 选 English -> `.lang-badge` 隐藏。
11. **mock 验证检测**：启用 mock 服务，source=auto 翻译 -> 译文为 `[Mock 翻译] ... -> 中文`（无标记行），`.lang-badge` 显示「英语」。

---

## 风险与回退

- **模型不遵守首行标记格式**：状态机降级，首行作 Delta 补发，`detected=None`，`.lang-badge` 翻译完成后隐藏。单测覆盖（`translate_fallbacks_when_no_header_marker`）。
- **标记跨 chunk**：状态机累积到首个 `\n` 才解析，正确拼接。单测覆盖（`translate_handles_marker_across_chunks`）。
- **译文极短无 `\n`**：Finished 前 `pending` 补作 Delta，`detected=None`。单测覆盖（`translate_fallbacks_when_no_header_marker` 的 "译文无标记" 无 `\n` 场景）。
- **`Finished` schema 变更**：新增 `Option<String>` 字段，旧前端忽略未知字段；本需求前后端同步改，无版本错位。
- **`sys-locale` 依赖**：轻量纯 Rust crate，无平台特定编译要求。
- **存量用户 config 不变**：`config.json` 已存在的 `target_lang` 保留原值，`from_env` 只在首次安装（config.json 不存在）触发。`normalized` 的 `FALLBACK_TARGET_LANG` 仅在 `target_lang` 为空时触发，极少。
- **`SHIZI_TARGET_LANG` 环境变量**：保留覆盖（开发逃生口），未设置时才读 OS。

---

## 自检结果

**1. 规格覆盖度：**
- 需求 1（下拉 inline combobox）：任务 9 全覆盖（HTML/CSS/JS + LANGUAGES 补 english + 键盘导航 + 搜索 + toggle/外部点击/swap 关闭）。
- 需求 2-A（模型回传检测语言）：任务 4（prompt 指令）+ 任务 5（Finished 字段）+ 任务 6（状态机）+ 任务 7（mock 改造）+ 任务 8（前端 badge 动态化）全覆盖。
- 需求 2-B（OS 默认目标语言）：任务 1（依赖）+ 任务 2（映射函数）+ 任务 3（from_env/normalized 改造）全覆盖。
- spec 第 10 节文档同步：任务 10 全覆盖。
- spec 第 9 节后端单测：map_os_lang_to_list（任务 2）、from_env_target_lang（任务 3）、finished_event 序列化（任务 5）、user_prompt（任务 4）、translate 状态机 4 场景（任务 6）、mock auto（任务 7）全覆盖；上一版 `from_env_default_target_lang_is_zh_cn` 调整（任务 3）。

**2. 占位符扫描：** 无 TODO/待定/「类似任务 N」；每个代码步骤含完整代码块；命令与预期输出齐全。

**3. 类型一致性：**
- `detected_source_lang: Option<String>` 在 types.rs（任务 5 定义）、service.rs（任务 5 暂传 None / 任务 6 接状态机）一致；前端 `detectedSourceLang`（任务 8 读取）与序列化 `detectedSourceLang` 一致。
- `map_os_lang_to_list`、`default_target_lang_from_os`、`FALLBACK_TARGET_LANG` 在任务 2/3 定义并一致使用；`DEFAULT_TARGET_LANG` 全仓库源码引用仅 3 处（已确认），任务 3 全部替换。
- `setLangBadge`（任务 8 新增）与现有 `setSourceBadge`（操作 `.source-badge`）不冲突。
- `HeaderParseState`/`parse_detected_lang`/`process_auto_delta`（任务 6）在 `translate_with` 中一致调用；`request_with_source` 辅助函数在 types.rs（任务 4）与 service.rs（任务 6）各自定义，签名一致。
- 前端 `LANGUAGES` 补 `english`（任务 9）后，`renderLangList` 使用 `l.english` 一致。
