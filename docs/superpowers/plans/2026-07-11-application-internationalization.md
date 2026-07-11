# 应用国际化与可扩展语言包实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 为设置页、翻译弹窗、托盘和窗口标题接入可即时切换、可由用户 JSON 语言包扩展的国际化运行时，并将翻译语言统一扩充为 19 种。

**架构：** 后端 `AppConfig.interface_language` 是唯一持久化事实来源；`core/i18n` 负责 locale 解析、用户语言包校验和托盘消息合并，前端每个 WebView 使用一个 Vue `ref` 驱动的轻量 i18n 单例。简体中文和英文静态加载，其余内置语言显式动态导入；翻译语言元数据只在前端维护一份，Edge provider 使用独立的显式映射表。

**技术栈：** Rust、Tauri 2、serde/serde_json、Vue 3 Composition API、TypeScript、Vitest、浏览器原生 `Intl`

---

## 文件结构

### 新建文件

- `src-tauri/src/core/i18n/mod.rs`：locale 解析、内置包嵌入、用户语言包扫描/校验、三层消息合并与 revision 快照。
- `src-tauri/src/ui/i18n.rs`：获取语言快照、刷新语言包和打开语言包目录的 Tauri commands。
- `frontend/src/i18n/index.ts`：响应式 locale、`t()`、插值、`Intl` 和事件同步入口。
- `frontend/src/i18n/loaders.ts`：简中/英文静态导入及其余 6 种语言的显式动态 loader map。
- `frontend/src/i18n/index.test.ts`：加载策略、回退、插值、切换和事件 revision 测试。
- `frontend/src/i18n/locales/{zh-CN,zh-TW,en-US,ja-JP,ko-KR,fr-FR,de-DE,es-ES}.json`：8 份统一 schema 的内置界面语言包。
- `frontend/src/shared/translation-languages.ts`：19 种翻译语言的唯一元数据目录与 prompt 稳定名称。
- `frontend/src/shared/translation-languages.test.ts`：目录数量、代码集合、source/target 约束测试。

### 修改文件

- `src-tauri/src/core/mod.rs`：导出 `i18n` 模块。
- `src-tauri/src/core/config/types.rs`：新增 `interface_language`，校验新翻译语言代码并调整 OS locale 映射。
- `src-tauri/src/core/translation/types.rs`：用稳定语言名称渲染 LLM prompt。
- `src-tauri/src/core/mt/microsoft.rs`：19 种语言显式正向/反向映射，未知代码返回不可重试错误。
- `src-tauri/src/app/state.rs`：保存语言包 revision，不缓存全部字典。
- `src-tauri/src/app/tray.rs`：保留固定 ID 菜单句柄并按当前字典更新文本与 tooltip。
- `src-tauri/src/app/window.rs`：按当前字典设置/刷新窗口标题。
- `src-tauri/src/ui/config.rs`：保存配置后刷新语言运行时，再广播配置和语言事件。
- `src-tauri/src/ui/mod.rs`、`src-tauri/src/lib.rs`：注册 i18n commands 和启动初始化。
- `frontend/src/types/config.ts`、`frontend/src/lib/config.ts`：同步 `interfaceLanguage`。
- `frontend/src/lib/tauri.ts`：增加语言快照、刷新和打开目录 command 包装。
- `frontend/src/settings/types.ts`、`frontend/src/settings/stores/settings.ts`：以 `auto` 为界面语言默认值并与后端双向同步。
- `frontend/src/settings/main.ts`、`frontend/src/popup/main.ts`：挂载前初始化 i18n。
- `frontend/src/settings/SettingsPage.vue`：监听语言事件并刷新字典。
- `frontend/src/settings/SettingsLayout.vue`、`frontend/src/settings/SettingsSidebar.vue`、`frontend/src/settings/panels/*.vue`、`frontend/src/settings/components/*.vue`：设置页可见文案、状态、toast、tooltip、placeholder 和 aria 文案改为消息键。
- `frontend/src/popup/TranslationPopup.vue`、`frontend/src/popup/components/*.vue`、`frontend/src/popup/composables/*.ts`：弹窗文案改为消息键，运行状态保存键和参数，文本内容容器增加 `dir="auto"`。
- `frontend/src/popup/data/languages.ts`、`frontend/src/settings/tokens.ts`：删除重复翻译语言表并改为共享目录引用。
- `frontend/src/lib/config.test.ts`、`frontend/src/settings/stores/settings.test.ts`、现有 popup composable 测试：补齐配置和状态键行为覆盖。
- `src-tauri/tauri.conf.json`、`src-tauri/capabilities/default.json`：标题回退值与窗口标题更新权限。
- `README.md`、`docs/roadmap/progressive-development-plan.md`、`docs/architecture/ui-decoupling-proposal.md`、`AGENTS.md`、`CLAUDE.md`：同步能力、架构与完成状态。
- `docs/superpowers/plans/2026-07-11-application-internationalization.md`：执行时逐项回填复选框和完成状态。

## 约束与实现边界

- 不引入 `vue-i18n`、文件监听器、opener 插件或新的日期库。
- `explorer.exe` 仅用于 Windows 打开目录；非 Windows 平台返回明确错误。
- 不迁移旧翻译代码；历史记录查不到名称时直接显示原始代码。
- overlay、日志正文、用户输入、模型名、服务实例名和服务商原始错误不翻译。
- 所有消息键由完整 `zh-CN.json` 定义；用户包出现未知键即整文件无效。

### 任务 1：建立新配置字段与翻译语言规范

**文件：**
- 修改：`src-tauri/src/core/config/types.rs`
- 修改：`frontend/src/types/config.ts`
- 修改：`frontend/src/settings/types.ts`
- 修改：`frontend/src/lib/config.ts`
- 测试：`src-tauri/src/core/config/types.rs`
- 测试：`frontend/src/lib/config.test.ts`
- 测试：`frontend/src/settings/stores/settings.test.ts`

- [x] **步骤 1：编写失败的 Rust 配置测试**

在 `types.rs` 现有测试模块增加：

```rust
#[test]
fn interface_language_defaults_to_auto_and_serializes_camel_case() {
    let config = AppConfig::from_env();
    assert_eq!(config.interface_language, "auto");
    let json = serde_json::to_value(config).unwrap();
    assert_eq!(json["interfaceLanguage"], "auto");
}

#[test]
fn normalized_rejects_old_translation_codes_without_aliasing() {
    let mut config = AppConfig::from_env();
    config.default_source_lang = "en-US".into();
    config.target_lang = "ja-JP".into();
    let normalized = config.normalized();
    assert_eq!(normalized.default_source_lang, "auto");
    assert_ne!(normalized.target_lang, "ja-JP");
}

#[test]
fn translation_locale_mapping_uses_new_codes() {
    assert_eq!(map_os_lang_to_translation("en-GB"), "en");
    assert_eq!(map_os_lang_to_translation("pt-BR"), "pt");
    assert_eq!(map_os_lang_to_translation("zh-Hant-HK"), "zh-TW");
    assert_eq!(map_os_lang_to_translation("xx-YY"), "zh-CN");
}
```

- [x] **步骤 2：编写失败的前端配置测试**

在 `frontend/src/lib/config.test.ts` 的投影断言中加入：

```ts
expect(projectToAppConfig(state).interfaceLanguage).toBe('auto')
```

在 `frontend/src/settings/stores/settings.test.ts` 的后端同步用例加入：

```ts
backend.interfaceLanguage = 'fr-FR'
await settings.syncFromBackend()
expect(settings.state.general.language).toBe('fr-FR')
```

- [x] **步骤 3：运行测试确认失败**

运行：

```bash
cd src-tauri && cargo test core::config::types::tests
cd .. && npm run test -- frontend/src/lib/config.test.ts frontend/src/settings/stores/settings.test.ts
```

预期：Rust 因 `interface_language`/`map_os_lang_to_translation` 不存在失败；TypeScript 因 `interfaceLanguage` 不存在失败。

- [x] **步骤 4：实现最少配置变更**

新增统一常量和字段：

```rust
const TRANSLATION_LANGS: &[&str] = &[
    "zh-CN", "zh-TW", "en", "ja", "ko", "fr", "de", "es", "pt", "ru",
    "it", "nl", "pl", "tr", "ar", "th", "vi", "id", "hi",
];

fn default_interface_language() -> String { "auto".to_string() }

#[serde(default = "default_interface_language")]
pub interface_language: String,
```

将 `map_os_lang_to_list` 改名为 `map_os_lang_to_translation`，按主语言返回新代码；中文脚本仍区分 `zh-CN`/`zh-TW`，未知值返回 `zh-CN`。`normalized()` 用 `TRANSLATION_LANGS` 校验源/目标，不增加旧代码别名。

前端同步类型和投影：

```ts
export type UILanguage = 'auto' | 'zh-CN' | 'zh-TW' | 'en-US' | 'ja-JP' | 'ko-KR' | 'fr-FR' | 'de-DE' | 'es-ES' | (string & {})

export interface AppConfig {
  interfaceLanguage: string
  targetLang: string
  defaultSourceLang: string
  autoCopy: boolean
  restoreClipboard: boolean
  historyLimit: number
  services: ServiceInstanceConfig[]
  popupPrecreate: boolean
  overlayPrecreate: boolean
  collectUsage: boolean
  logLevel: LogLevel
  shortcuts: Record<string, string>
}

// projectToAppConfig
interfaceLanguage: state.general.language,
```

设置 store 默认 `general.language` 为 `auto`，后端同步时写入 `backend.interfaceLanguage`。

- [x] **步骤 5：运行测试确认通过**

运行：

```bash
cd src-tauri && cargo test core::config::types::tests
cd .. && npm run test -- frontend/src/lib/config.test.ts frontend/src/settings/stores/settings.test.ts
```

预期：全部 PASS。

- [x] **步骤 6：提交**

```bash
git add src-tauri/src/core/config/types.rs frontend/src/types/config.ts frontend/src/settings/types.ts frontend/src/lib/config.ts frontend/src/lib/config.test.ts frontend/src/settings/stores/settings.ts frontend/src/settings/stores/settings.test.ts
git commit -m "feat(i18n): 建立界面与翻译语言配置规范"
```

### 任务 2：建立唯一翻译语言目录并稳定 LLM prompt

**文件：**
- 创建：`frontend/src/shared/translation-languages.ts`
- 创建：`frontend/src/shared/translation-languages.test.ts`
- 修改：`frontend/src/popup/data/languages.ts`
- 修改：`frontend/src/settings/tokens.ts`
- 修改：`frontend/src/popup/components/LanguagePicker.vue`
- 修改：`frontend/src/popup/components/LanguageToolbar.vue`
- 修改：`frontend/src/settings/panels/TranslatePanel.vue`
- 修改：`frontend/src/settings/panels/HistoryPanel.vue`
- 修改：`src-tauri/src/core/translation/types.rs`

- [x] **步骤 1：编写失败的目录测试**

```ts
import { describe, expect, it } from 'vitest'
import { SOURCE_LANGUAGES, TARGET_LANGUAGES, TRANSLATION_LANGUAGES } from './translation-languages'

const codes = ['zh-CN', 'zh-TW', 'en', 'ja', 'ko', 'fr', 'de', 'es', 'pt', 'ru', 'it', 'nl', 'pl', 'tr', 'ar', 'th', 'vi', 'id', 'hi']

describe('翻译语言目录', () => {
  it('目标语言恰好包含 19 种规范代码', () => {
    expect(TARGET_LANGUAGES.map((item) => item.code)).toEqual(codes)
  })

  it('源语言仅额外包含一个 auto', () => {
    expect(SOURCE_LANGUAGES.map((item) => item.code)).toEqual(['auto', ...codes])
    expect(TRANSLATION_LANGUAGES.filter((item) => item.code === 'auto')).toHaveLength(1)
  })

  it('每种实际语言都有稳定 prompt 名称和界面消息键', () => {
    for (const item of TARGET_LANGUAGES) {
      expect(item.promptName).toMatch(/^[A-Za-z ]+$/)
      expect(item.nameKey).toBe(`language.${item.code}`)
    }
  })
})
```

- [x] **步骤 2：运行测试确认失败**

运行：`npm run test -- frontend/src/shared/translation-languages.test.ts`

预期：FAIL，模块不存在。

- [x] **步骤 3：实现共享目录并删除重复数据**

定义单一只读数组：

```ts
export interface TranslationLanguage {
  code: string
  nativeName: string
  promptName: string
  nameKey: `language.${string}`
}

export const TARGET_LANGUAGES = [
  ['zh-CN', '简体中文', 'Chinese (Simplified)'], ['zh-TW', '繁體中文', 'Chinese (Traditional)'],
  ['en', 'English', 'English'], ['ja', '日本語', 'Japanese'], ['ko', '한국어', 'Korean'],
  ['fr', 'Français', 'French'], ['de', 'Deutsch', 'German'], ['es', 'Español', 'Spanish'],
  ['pt', 'Português', 'Portuguese'], ['ru', 'Русский', 'Russian'], ['it', 'Italiano', 'Italian'],
  ['nl', 'Nederlands', 'Dutch'], ['pl', 'Polski', 'Polish'], ['tr', 'Türkçe', 'Turkish'],
  ['ar', 'العربية', 'Arabic'], ['th', 'ภาษาไทย', 'Thai'], ['vi', 'Tiếng Việt', 'Vietnamese'],
  ['id', 'Bahasa Indonesia', 'Indonesian'], ['hi', 'हिन्दी', 'Hindi'],
].map(([code, nativeName, promptName]) => ({ code, nativeName, promptName, nameKey: `language.${code}` as const }))

export const AUTO_LANGUAGE = { code: 'auto', nativeName: '自动检测', promptName: 'Auto Detect', nameKey: 'language.auto' as const }
export const SOURCE_LANGUAGES = [AUTO_LANGUAGE, ...TARGET_LANGUAGES]
export const TRANSLATION_LANGUAGES = SOURCE_LANGUAGES
export const translationLanguage = (code: string) => SOURCE_LANGUAGES.find((item) => item.code === code)
```

删除 `frontend/src/settings/tokens.ts` 内的 `LANGUAGES`，把旧 `popup/data/languages.ts` 缩成对共享目录的兼容导出；所有消费者直接改用 `SOURCE_LANGUAGES`/`TARGET_LANGUAGES`。历史未知代码使用 `translationLanguage(code)?.nativeName ?? code`。

在 Rust `TranslationRequest` 增加同一代码集合的 `prompt_language_name()`，`user_prompt()` 只使用英文稳定名称，不使用当前 UI 文案：

```rust
fn prompt_language_name(code: &str) -> &str {
    match code {
        "zh-CN" => "Chinese (Simplified)", "zh-TW" => "Chinese (Traditional)",
        "en" => "English", "ja" => "Japanese", "ko" => "Korean", "fr" => "French",
        "de" => "German", "es" => "Spanish", "pt" => "Portuguese", "ru" => "Russian",
        "it" => "Italian", "nl" => "Dutch", "pl" => "Polish", "tr" => "Turkish",
        "ar" => "Arabic", "th" => "Thai", "vi" => "Vietnamese", "id" => "Indonesian",
        "hi" => "Hindi", "auto" => "Auto Detect", other => other,
    }
}
```

- [x] **步骤 4：运行聚焦测试和类型检查**

运行：

```bash
npm run test -- frontend/src/shared/translation-languages.test.ts
npm run typecheck
cd src-tauri && cargo test core::translation::types::tests
```

预期：全部 PASS；设置页、弹窗和历史面板不再从两份常量读取语言列表。

- [x] **步骤 5：提交**

```bash
git add frontend/src/shared frontend/src/popup/data/languages.ts frontend/src/settings/tokens.ts frontend/src/popup/components/LanguagePicker.vue frontend/src/popup/components/LanguageToolbar.vue frontend/src/settings/panels/TranslatePanel.vue frontend/src/settings/panels/HistoryPanel.vue src-tauri/src/core/translation/types.rs
git commit -m "refactor(翻译语言): 统一十九种语言目录与提示词语义"
```

### 任务 3：严格接入 Microsoft Edge 的 19 种语言映射

**文件：**
- 修改：`src-tauri/src/core/mt/microsoft.rs`

- [x] **步骤 1：把现有映射测试改成失败的完整测试**

```rust
#[test]
fn all_supported_languages_roundtrip_through_edge_codes() {
    let cases = [
        ("zh-CN", "zh-Hans"), ("zh-TW", "zh-Hant"), ("en", "en"), ("ja", "ja"),
        ("ko", "ko"), ("fr", "fr"), ("de", "de"), ("es", "es"), ("pt", "pt"),
        ("ru", "ru"), ("it", "it"), ("nl", "nl"), ("pl", "pl"), ("tr", "tr"),
        ("ar", "ar"), ("th", "th"), ("vi", "vi"), ("id", "id"), ("hi", "hi"),
    ];
    for (internal, edge) in cases {
        assert_eq!(map_source_lang(internal).unwrap(), Some(edge));
        assert_eq!(map_target_lang(internal).unwrap(), edge);
        assert_eq!(detected_to_internal(edge), internal);
    }
}

#[test]
fn unknown_languages_are_errors_instead_of_fallbacks() {
    assert!(map_source_lang("en-US").is_err());
    assert!(map_target_lang("unknown").is_err());
}
```

- [x] **步骤 2：运行测试确认失败**

运行：`cd src-tauri && cargo test core::mt::microsoft::tests`

预期：FAIL，旧代码仍接受旧代码且目标未知时回退英语。

- [x] **步骤 3：实现最小显式映射**

让两个函数都返回 `Result`，`auto` 仅在 source 映射为 `Ok(None)`；未知值构造不可重试配置错误：

```rust
fn unsupported_language(kind: &str, code: &str) -> TranslationError {
    TranslationError::Api {
        message: format!("不支持的{kind}语言代码: {code}"),
        retryable: false,
    }
}
```

在 `translate_once()` 中使用 `?`：

```rust
let from = map_source_lang(&request.source_lang)?;
let to = map_target_lang(&request.target_lang)?;
```

反向映射覆盖 19 种语言；Edge 返回未知检测代码时保留原始代码，供 UI 按历史兼容规则展示。

- [x] **步骤 4：运行测试确认通过**

运行：`cd src-tauri && cargo test core::mt::microsoft::tests`

预期：全部 PASS，且不存在 `_ => "en"` 或未知 source 自动检测分支。

- [x] **步骤 5：提交**

```bash
git add src-tauri/src/core/mt/microsoft.rs
git commit -m "fix(Edge 翻译): 严格映射十九种翻译语言"
```

### 任务 4：实现后端语言包校验、回退与快照

**文件：**
- 创建：`src-tauri/src/core/i18n/mod.rs`
- 修改：`src-tauri/src/core/mod.rs`
- 修改：`src-tauri/src/app/state.rs`

- [x] **步骤 1：编写失败的 locale 与语言包测试**

在新模块测试区使用 `tempfile::tempdir()` 覆盖：

```rust
#[test]
fn resolves_builtin_and_custom_locales() {
    assert_eq!(resolve_locale("auto", Some("zh-Hant-HK"), &[]), "zh-TW");
    assert_eq!(resolve_locale("auto", Some("en-GB"), &[]), "en-US");
    assert_eq!(resolve_locale("auto", Some("it-IT"), &["it-IT".into()]), "it-IT");
    assert_eq!(resolve_locale("auto", Some("xx-YY"), &[]), "zh-CN");
    assert_eq!(resolve_locale("invalid", Some("en-US"), &[]), "zh-CN");
}

#[test]
fn validates_and_merges_partial_user_pack() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("en-US.json"), r#"{
      "schemaVersion":1,"locale":"en-US","name":"Custom English",
      "messages":{"tray.quit":"Exit now"}
    }"#).unwrap();
    let scan = scan_language_packs(dir.path(), &["tray.quit", "tray.settings"]).unwrap();
    let merged = merge_messages("en-US", scan.current("en-US"), builtin_messages("en-US"));
    assert_eq!(merged["tray.quit"], "Exit now");
    assert_eq!(merged["tray.settings"], builtin_messages("en-US")["tray.settings"]);
}
```

再分别添加：文件名与 locale 不一致、非法 BCP 47、schema 非 1、空 name、嵌套/非字符串 messages、未知键、超过 1 MB、删除覆盖恢复内置包的表驱动测试。

- [x] **步骤 2：运行测试确认失败**

运行：`cd src-tauri && cargo test core::i18n`

预期：FAIL，模块和函数不存在。

- [x] **步骤 3：实现校验和按需快照**

核心类型固定为：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguagePack {
    pub schema_version: u32,
    pub locale: String,
    pub name: String,
    pub messages: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageSnapshot {
    pub configured_locale: String,
    pub locale: String,
    pub revision: u64,
    pub languages: Vec<LanguageMeta>,
    pub user_messages: HashMap<String, String>,
    pub errors: Vec<LanguagePackError>,
}
```

使用 `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../frontend/src/i18n/locales/zh-CN.json"))` 同样式显式嵌入全部 8 份内置包；后端只解析当前 locale 的托盘/标题键和最终回退所需的简中键。locale 校验只接受 `^[A-Za-z]{2,3}(-[A-Za-z0-9]{2,8})*$` 的等价手写分段检查，避免增加 regex 依赖。扫描时先用 `metadata.len()` 拒绝超过 `1_048_576` 字节的文件，只保留元数据、错误和当前 locale 的用户 messages。

`AppState` 只新增：

```rust
interface_language_revision: Arc<AtomicU64>
```

并提供 `next_interface_language_revision()`/`interface_language_revision()`；不缓存全部语言包。

- [x] **步骤 4：运行测试确认通过**

运行：`cd src-tauri && cargo test core::i18n`

预期：全部 PASS。

- [x] **步骤 5：提交**

```bash
git add src-tauri/src/core/i18n src-tauri/src/core/mod.rs src-tauri/src/app/state.rs
git commit -m "feat(i18n): 实现用户语言包校验与回退"
```

### 任务 5：接入语言 commands、托盘和窗口标题即时同步

**文件：**
- 创建：`src-tauri/src/ui/i18n.rs`
- 修改：`src-tauri/src/ui/mod.rs`
- 修改：`src-tauri/src/app/tray.rs`
- 修改：`src-tauri/src/app/window.rs`
- 修改：`src-tauri/src/ui/config.rs`
- 修改：`src-tauri/src/lib.rs`
- 修改：`src-tauri/capabilities/default.json`

- [x] **步骤 1：先为纯同步决策编写失败测试**

把可测试逻辑留在 `core/i18n`，新增：

```rust
#[test]
fn tray_messages_follow_user_builtin_and_chinese_fallback_order() {
    let messages = resolve_messages(
        "en-US",
        HashMap::from([("tray.quit".into(), "Exit now".into())]),
    );
    assert_eq!(messages["tray.quit"], "Exit now");
    assert_eq!(messages["tray.settings"], "Settings");
    assert_eq!(messages["tray.translate"], "Translate");
}
```

- [x] **步骤 2：运行测试确认失败**

运行：`cd src-tauri && cargo test core::i18n::tests::tray_messages`

预期：FAIL，`resolve_messages` 尚未提供完整托盘回退。

- [x] **步骤 3：实现 Tauri commands 和固定句柄更新**

commands 签名固定为：

```rust
#[tauri::command]
pub fn get_interface_language_snapshot(app: AppHandle, state: State<'_, AppState>) -> Result<LanguageSnapshot, String>;

#[tauri::command]
pub fn refresh_interface_languages(app: AppHandle, state: State<'_, AppState>) -> Result<LanguageSnapshot, String>;

#[tauri::command]
pub fn open_language_pack_directory(app: AppHandle) -> Result<(), String>;
```

刷新流程统一调用一个 `apply_interface_language(app, state, configured_locale, increment_revision)`：扫描、解析 locale、合并托盘键、更新菜单和 tooltip、更新 `main`/`settings` 标题，最后广播：

```rust
app.emit("interface-language:changed", serde_json::json!({
    "locale": snapshot.locale,
    "revision": snapshot.revision,
}))?;
```

`setup_tray()` 返回并由 Tauri manage 保存 `TrayI18nHandles { tray, translate, settings, quit }`，调用各 `MenuItem::set_text()` 和 `TrayIcon::set_tooltip()` 更新，不重建菜单。打开目录使用 `std::fs::create_dir_all()` 后在 Windows 调用 `std::process::Command::new("explorer.exe").arg(path).spawn()`。

`save_app_config` 在持久化后先调用语言同步；同步失败返回现有全局错误类型，但不回滚已经保存的配置。注册 `core:window:allow-set-title` 权限。

- [x] **步骤 4：启动路径接入并运行后端测试/构建**

`lib.rs` 在 `AppState` 和托盘句柄 manage 后执行一次不广播的初始化，再创建窗口；将 3 个 commands 加入 `generate_handler!`。

运行：

```bash
cd src-tauri && cargo test
cargo build
```

预期：全部 PASS，构建成功；Tauri API 签名错误在此步骤被编译器发现并修正。

- [x] **步骤 5：提交**

```bash
git add src-tauri/src/ui/i18n.rs src-tauri/src/ui/mod.rs src-tauri/src/app/tray.rs src-tauri/src/app/window.rs src-tauri/src/ui/config.rs src-tauri/src/lib.rs src-tauri/capabilities/default.json
git commit -m "feat(i18n): 即时同步托盘与窗口标题"
```

### 任务 6：实现前端轻量 i18n 运行时与 8 份内置字典

**文件：**
- 创建：`frontend/src/i18n/index.ts`
- 创建：`frontend/src/i18n/loaders.ts`
- 创建：`frontend/src/i18n/index.test.ts`
- 创建：`frontend/src/i18n/locales/{zh-CN,zh-TW,en-US,ja-JP,ko-KR,fr-FR,de-DE,es-ES}.json`
- 修改：`frontend/src/lib/tauri.ts`

- [x] **步骤 1：编写失败的运行时测试**

```ts
describe('i18n runtime', () => {
  it('8 份内置字典与简中键集合完全一致', async () => {
    const base = Object.keys(zhCN.messages).sort()
    for (const locale of BUILTIN_LOCALES) {
      expect(Object.keys((await loadBuiltin(locale)).messages).sort()).toEqual(base)
    }
  })

  it('按用户包、同 locale 内置包、简中回退并插值', async () => {
    const i18n = createI18nForTest({
      locale: 'en-US',
      userMessages: { 'common.namedError': 'Failed: {name}' },
    })
    expect(i18n.t('common.namedError', { name: 'OpenAI' })).toBe('Failed: OpenAI')
    expect(i18n.t('tray.quit')).toBe('Quit')
    expect(i18n.t('test.zhOnly')).toBe(zhCN.messages['test.zhOnly'])
  })

  it('revision 变化时即使 locale 相同也重新加载', async () => {
    await i18n.applySnapshot(snapshot('en-US', 1, { 'tray.quit': 'Exit A' }))
    await i18n.applySnapshot(snapshot('en-US', 2, { 'tray.quit': 'Exit B' }))
    expect(i18n.t('tray.quit')).toBe('Exit B')
  })
})
```

- [x] **步骤 2：运行测试确认失败**

运行：`npm run test -- frontend/src/i18n/index.test.ts`

预期：FAIL，模块和 JSON 文件不存在。

- [x] **步骤 3：创建字典协议和 loader**

每份 JSON 严格使用：

```json
{
  "schemaVersion": 1,
  "locale": "zh-CN",
  "name": "简体中文",
  "messages": {
    "common.save": "保存",
    "common.cancel": "取消",
    "common.retry": "重试",
    "language.auto": "自动检测"
  }
}
```

`zh-CN` 收录设置页、弹窗、toast、tooltip、placeholder、aria、托盘、窗口标题、状态和 20 个语言名称的完整键；其他 7 份文件必须逐键翻译并通过键集合测试，不允许省略内置字典键。`loaders.ts` 静态导入 `zh-CN`/`en-US`，其余 locale 只出现在显式 `() => import(...)` map。

- [x] **步骤 4：实现响应式 API 和 command 包装**

```ts
export const locale = readonly(activeLocale)
export const t = (key: MessageKey, params: MessageParams = {}): string => {
  const value = userMessages.value[key] ?? builtinMessages.value[key] ?? zhCN.messages[key] ?? key
  return value.replace(/\{([A-Za-z][A-Za-z0-9_]*)\}/g, (_, name: string) => String(params[name] ?? `{${name}}`))
}
export const formatDateTime = (value: Date | string) => new Intl.DateTimeFormat(activeLocale.value, { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(value))
```

`frontend/src/lib/tauri.ts` 新增 `InterfaceLanguageSnapshot` 及 3 个 invoke 包装；测试环境无 Tauri 时允许传入快照初始化，不新增第二套持久化。

`loadBuiltin()` 捕获动态 import 错误，通过现有 frontend logger 记录 locale 和错误摘要，然后返回内置 `zh-CN`；失败不能替换当前仍有效的字典，只有成功加载或明确回退后才一次性更新响应式 refs。

- [x] **步骤 5：运行测试和类型检查**

运行：

```bash
npm run test -- frontend/src/i18n/index.test.ts
npm run typecheck
```

预期：全部 PASS；Vite 能解析 6 个动态 JSON import。

- [x] **步骤 6：提交**

```bash
git add frontend/src/i18n frontend/src/lib/tauri.ts
git commit -m "feat(i18n): 实现前端字典加载与响应式回退"
```

### 任务 7：接入两个 WebView 的初始化和语言事件

**文件：**
- 修改：`frontend/src/settings/main.ts`
- 修改：`frontend/src/popup/main.ts`
- 修改：`frontend/src/settings/SettingsPage.vue`
- 修改：`frontend/src/popup/TranslationPopup.vue`

- [x] **步骤 1：为事件去重和 revision 刷新编写失败测试**

在 `frontend/src/i18n/index.test.ts` 增加：

```ts
it('忽略旧 revision，接受较新的同 locale revision', async () => {
  await i18n.applySnapshot(snapshot('fr-FR', 3, {}))
  await i18n.applySnapshot(snapshot('fr-FR', 2, { 'common.save': 'stale' }))
  expect(i18n.revision.value).toBe(3)
})
```

- [x] **步骤 2：运行测试确认失败并实现最少 revision guard**

运行：`npm run test -- frontend/src/i18n/index.test.ts`

预期：FAIL；随后在 `applySnapshot` 中对小于当前 revision 的快照直接返回，再运行至 PASS。

- [x] **步骤 3：挂载前初始化，组件卸载时解除监听**

两个入口统一使用：

```ts
await initializeI18n()
createApp(App).mount('#app')
```

每个根组件监听 `interface-language:changed`，事件到达后调用 `reloadCurrentLocale()`；`onBeforeUnmount` 执行 unlisten。应用快照时同步：

```ts
document.documentElement.lang = locale.value
await getCurrentWindow()?.setTitle(t(windowLabel === 'settings' ? 'window.settingsTitle' : 'window.popupTitle'))
```

设置标题失败记录日志但不阻断字典切换。popup 的冷启动 ready gate 必须等 i18n 初始化完成，不增加页面 reload。

- [x] **步骤 4：运行测试、类型检查和构建**

运行：

```bash
npm run test -- frontend/src/i18n/index.test.ts frontend/src/popup/composables/mainWindowReady.test.ts
npm run typecheck
npm run build
```

预期：全部 PASS，构建产物包含 6 个按需 locale chunk。

- [x] **步骤 5：提交**

```bash
git add frontend/src/settings/main.ts frontend/src/popup/main.ts frontend/src/settings/SettingsPage.vue frontend/src/popup/TranslationPopup.vue frontend/src/i18n/index.test.ts
git commit -m "feat(i18n): 同步设置页与翻译弹窗语言状态"
```

### 任务 8：接入设置页语言选择、目录操作和全部可见文案

**文件：**
- 修改：`frontend/src/settings/SettingsLayout.vue`
- 修改：`frontend/src/settings/SettingsSidebar.vue`
- 修改：`frontend/src/settings/panels/GeneralPanel.vue`
- 修改：`frontend/src/settings/panels/TranslatePanel.vue`
- 修改：`frontend/src/settings/panels/ShortcutPanel.vue`
- 修改：`frontend/src/settings/panels/ServicesPanel.vue`
- 修改：`frontend/src/settings/panels/AdvancedPanel.vue`
- 修改：`frontend/src/settings/panels/HistoryPanel.vue`
- 修改：`frontend/src/settings/components/ApiKeyInput.vue`
- 修改：`frontend/src/settings/components/ChannelCombobox.vue`
- 修改：`frontend/src/settings/components/ModelCombobox.vue`
- 修改：`frontend/src/settings/components/ShortcutRecorder.vue`
- 修改：`frontend/src/settings/stores/settings.ts`
- 修改：`frontend/src/settings/stores/settings.test.ts`
- 修改：`frontend/src/i18n/locales/*.json`

- [x] **步骤 1：编写失败的 store 与选项测试**

```ts
it('刷新语言包后保留有效选择并回写被删除的自定义 locale', async () => {
  settings.state.general.language = 'it-IT'
  mockRefresh.mockResolvedValue(snapshot('zh-CN', 2, {}, [{ locale: 'zh-CN', name: '简体中文' }]))
  await settings.refreshInterfaceLanguages()
  expect(settings.state.general.language).toBe('zh-CN')
})
```

再断言后端返回的语言元数据生成 `auto + 内置 + 用户包` 选项，错误数组保留文件名与原因供设置页展示。

- [x] **步骤 2：运行测试确认失败**

运行：`npm run test -- frontend/src/settings/stores/settings.test.ts`

预期：FAIL，刷新方法和语言元数据状态不存在。

- [x] **步骤 3：实现语言选择与两个图标按钮**

在 store 增加只读 `interfaceLanguages`/`interfaceLanguageErrors` 和 `refreshInterfaceLanguages()`；`GeneralPanel` 的语言 options 使用快照元数据，首项固定为 `{ value: 'auto', label: t('language.auto') }`。

使用现有 Lucide `FolderOpen`、`RefreshCw` 图标按钮，分别调用打开目录和刷新；按钮有本地化 tooltip/`aria-label`，刷新期间禁用并旋转图标。错误区逐文件显示 `file + message`，不新增 modal 或语言包编辑器。

- [x] **步骤 4：迁移设置页可见文案**

每个组件直接导入 `t`，将静态属性改成绑定：

```vue
<SettingGroup :title="t('settings.general.startup.title')" :description="t('settings.general.startup.description')">
<SettingSwitch v-model="state.general.launchAtLogin" :aria-label="t('settings.general.launchAtLogin.aria')" />
```

option 数组改为 `computed(() => ...)`，避免切换语言后保留旧字符串。toast 改为调用时 `t()`；快捷键运行时状态保留 key/params，不把翻译后的句子写入 store。使用浏览器原生 `Intl.DateTimeFormat(locale.value)` 替换历史页手写“昨天/天前”的固定中文分支。

- [x] **步骤 5：运行设置页测试、类型检查和构建**

运行：

```bash
npm run test -- frontend/src/settings frontend/src/i18n/index.test.ts
npm run typecheck
npm run build
```

预期：全部 PASS；`rg -n '>[[:space:]]*[^<{[:space:]][^<{]*<' frontend/src/settings --glob '*.vue'` 不再发现应国际化的可见中文文案，仅允许注释、日志和服务商/模型原名。

- [x] **步骤 6：提交**

```bash
git add frontend/src/settings frontend/src/i18n/locales
git commit -m "feat(设置页): 接入即时语言切换与语言包管理"
```

### 任务 9：迁移翻译弹窗、共享卡片和运行时状态文案

**文件：**
- 修改：`frontend/src/popup/TranslationPopup.vue`
- 修改：`frontend/src/popup/components/PopupToolbar.vue`
- 修改：`frontend/src/popup/components/SourceCard.vue`
- 修改：`frontend/src/popup/components/SourceCardView.vue`
- 修改：`frontend/src/popup/components/LanguageToolbar.vue`
- 修改：`frontend/src/popup/components/LanguagePicker.vue`
- 修改：`frontend/src/popup/components/ResultCard.vue`
- 修改：`frontend/src/popup/components/ResultCardView.vue`
- 修改：`frontend/src/popup/components/StatusBar.vue`
- 修改：`frontend/src/popup/composables/useTranslationEvents.ts`
- 修改：`frontend/src/popup/composables/useTranslationEvents.test.ts`
- 修改：`frontend/src/popup/composables/mainWindowReady.ts`
- 修改：`frontend/src/popup/composables/mainWindowReady.test.ts`
- 修改：`frontend/src/popup/composables/resultCardMeta.ts`
- 修改：`frontend/src/popup/composables/resultCardMeta.test.ts`
- 修改：`frontend/src/i18n/locales/*.json`

- [x] **步骤 1：把状态模型测试改为消息键断言**

```ts
expect(state.status).toEqual({ key: 'popup.status.translating', params: {}, loading: true, action: null })
expect(failedCard.errorTitleKey).toBe('popup.error.translationFailed')
```

为 detected language、重试、取消、复制成功、空输入和 ready 超时分别增加 key/params 断言；原始 `errorMessage` 仍原样保存。

- [x] **步骤 2：运行测试确认失败**

运行：

```bash
npm run test -- frontend/src/popup
```

预期：FAIL，现有状态仍保存中文句子。

- [x] **步骤 3：实现状态键模型并迁移组件文案**

把：

```ts
{ text: '翻译中…', loading: true }
```

改为：

```ts
{ key: 'popup.status.translating', params: {}, loading: true }
```

模板渲染时调用 `t(status.key, status.params)`。所有 option、tooltip、placeholder 和 `aria-label` 通过 computed 或模板调用 `t()`；临时 toast 在触发时翻译，不回溯更新已经显示的 toast。

`LanguagePicker` 第一列继续用 `nativeName`，第二列用 `t(language.nameKey)`，搜索同时匹配两列。`SourceCardView`、`ResultCardView` 的正文容器增加 `dir="auto"`，布局根节点不设置 RTL。

- [x] **步骤 4：运行测试、类型检查和硬编码审计**

运行：

```bash
npm run test -- frontend/src/popup frontend/src/i18n/index.test.ts
npm run typecheck
rg -n "就绪|翻译中|检测中|搜索源语言|搜索目标语言|交换语言|复制|重试" frontend/src/popup --glob "*.vue" --glob "*.ts"
```

预期：测试和类型检查 PASS；`rg` 只命中字典键上下文、注释或测试数据，不命中用户可见硬编码。

- [x] **步骤 5：提交**

```bash
git add frontend/src/popup frontend/src/i18n/locales
git commit -m "feat(翻译弹窗): 国际化状态与共享结果视图"
```

### 任务 10：完成全量验证和手动验收

**文件：**
- 修改：`src-tauri/tauri.conf.json`
- 修改：`docs/superpowers/plans/2026-07-11-application-internationalization.md`

- [x] **步骤 1：运行全部自动验证**

```bash
npm run test
npm run typecheck
npm run build
cd src-tauri && cargo test
cargo build
```

预期：所有命令退出码为 0；若任一失败，先按 `systematic-debugging` 定位根因并只修复相关任务，不跳过验证。

- [ ] **步骤 2：启动开发环境并执行手动验收**

运行：`npm run tauri dev`

按 spec 第 10.4 节逐项验证，重点记录：

1. `auto`、8 种内置 locale、未知 OS locale 回退。
2. 设置页、弹窗、托盘、tooltip 和两个窗口标题同步且不 reload。
3. 翻译进行中切换不清空输入、不重建卡片、不中断请求。
4. 19 种源/目标语言、Edge 映射错误和历史旧代码原样展示。
5. 新增、覆盖、删除、非法、未知键和超过 1 MB 的用户包。
6. 阿拉伯语正文 `dir=auto`，应用整体保持 LTR。

- [x] **步骤 3：回填计划验收状态**

将本文件已完成步骤改为 `- [x]`，在文件末尾增加：

```markdown
## 执行结果

- 自动验证：`npm run test`、`npm run typecheck`、`npm run build`、`cargo test`、`cargo build` 全部通过。
- 手动验收：spec 第 10.4 节 13 项全部通过。
```

未通过项必须写出实际失败现象，不得将其标为完成。

## 执行结果（2026-07-12）

- 自动验证：`npm run test`（132/132）、`npm run typecheck`、`npm run build`、`cargo test`（250 passed、2 ignored）、`cargo build` 均退出码 0；Rust 仅有既存 unused/dead-code warning。
- 手动验收 1：通过。`auto` 在当前系统解析为 `zh-CN`，首次界面为简体中文。
- 手动验收 2：未通过。8 种内置 locale 的弹窗文案、tooltip、两个窗口标题均在相同窗口实例内更新，无 reload；但设置页的主题、关闭行为、更新通道等原生下拉当前值在切换后仍残留旧中文，托盘菜单未能通过 Computer Use 直接检查。
- 手动验收 3：通过。临时写入不支持的 `xx-YY` 并重启后回退 `zh-CN`，窗口标题为 `Shizi 翻译`。
- 手动验收 4：未验收。临时 mock 服务可流式翻译，但设置页不接受 `mock` 协议并显示 `Auto-save failed`，无法形成“翻译进行中有效保存并切换语言”的可靠证据。
- 手动验收 5：通过。源语言下拉为 `auto + 19`，目标语言下拉为 19 种且无 `auto`。
- 手动验收 6：通过。下拉第一列保持 native name，第二列随西班牙语界面显示本地化名称。
- 手动验收 7：由自动契约验证替代。前端共享 19 语言目录测试及 Rust prompt 稳定名称/未知 code 原样测试通过，未实际发送 19 次外部 LLM 请求。
- 手动验收 8：由自动契约验证替代。Rust 覆盖 19 种 Edge 显式映射及未知语言返回错误，未依赖外部网络逐项请求。
- 手动验收 9：通过。新增合法 `it-IT` 包并刷新后立即出现在语言列表。
- 手动验收 10：通过。局部 `en-US` 覆盖仅替换 `settings.title`，缺失键使用内置英文。
- 手动验收 11：通过。删除 `en-US` 覆盖并刷新后恢复内置 `Settings`。
- 手动验收 12：通过。非法 JSON、错误 schema、文件名/locale 不一致、未知 key、超过 1 MiB 均被拒绝并显示 `file + reason`。
- 手动验收 13：部分通过。阿拉伯语 source 正文按 RTL 对齐且应用布局保持 LTR；result 正文因 mock 固定拉丁前缀及第 4 项限制未形成阿拉伯语首强字符的手动证据。
- 额外证据：19 种 source/target 目录及未知历史 code 原样展示由前端/Rust 自动测试覆盖；fallback 创建期标题已改为当前 `zh-CN` 的 `Shizi 翻译`。
- 用户数据恢复：原 `config.json` SHA-256 `A212AFBFCAB866FF557DFB3FC245A377F1A7C86B34EDF5EE1549C2C6C40C5FE9` 恢复后完全一致；原 `lang` 目录不存在，验收结束后仍不存在；Shizi 验收进程为 0。

- [ ] **步骤 4：提交**

```bash
git add src-tauri/tauri.conf.json docs/superpowers/plans/2026-07-11-application-internationalization.md
git commit -m "test(i18n): 完成国际化端到端验收"
```

### 任务 11：同步项目文档并完成分支收尾门禁

**文件：**
- 修改：`README.md`
- 修改：`docs/roadmap/progressive-development-plan.md`
- 修改：`docs/architecture/ui-decoupling-proposal.md`
- 修改：`AGENTS.md`
- 修改：`CLAUDE.md`
- 修改：`docs/superpowers/plans/2026-07-11-application-internationalization.md`

- [ ] **步骤 1：同步 README、roadmap 和架构文档**

README 增加：8 种内置界面语言、`auto`、即时全局切换、19 种翻译语言、`<app_config_dir>/lang/*.json` 格式与刷新方式。roadmap 将国际化能力标为完成。架构文档记录：

```text
config.json.interfaceLanguage
  -> Rust resolve/validate + tray/window
  -> interface-language:changed { locale, revision }
  -> WebView command 拉取当前用户覆盖
  -> user -> builtin locale -> builtin zh-CN
```

- [ ] **步骤 2：同步 AGENTS.md 与 CLAUDE.md**

在“架构关键点”加入同一段国际化说明，内容必须逐字同步：配置事实来源、语言事件 revision、静态/动态加载、用户包目录、19 种共享翻译语言和 Edge 严格映射。不得修改 `superpowers-zh` 标记区域。

- [ ] **步骤 3：验证文档同步和工作区状态**

运行：

```bash
git diff --no-index AGENTS.md CLAUDE.md
git diff --check
git status --short
```

预期：第一条无输出且退出码为 0；第二条无空白错误；第三条只显示本任务预期文档。

- [ ] **步骤 4：提交文档**

```bash
git add README.md docs/roadmap/progressive-development-plan.md docs/architecture/ui-decoupling-proposal.md AGENTS.md CLAUDE.md docs/superpowers/plans/2026-07-11-application-internationalization.md
git commit -m "docs(i18n): 同步国际化能力与架构说明"
```

- [ ] **步骤 5：进入分支收尾流程**

确认任务 10 的自动验证和手动验收仍有效、任务 11 文档已经提交后，调用 `finishing-a-development-branch`。在此之前不得合并、创建 PR 或清理 worktree。
