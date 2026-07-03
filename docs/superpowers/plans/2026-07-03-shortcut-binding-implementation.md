# 快捷键绑定实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 让设置页 6 条快捷键绑定保存到后端，其中划词翻译、截图翻译、剪贴板翻译、显示主窗口、打开设置可立即重注册生效，取词翻译只保存不触发。

**架构：** `AppConfig.shortcuts` 作为唯一持久化来源；`src-tauri/src/app/shortcuts.rs` 负责解析、去重、注册、反查和分发。保存配置时先用新配置替换注册，成功后写盘，失败则不保存并尽量恢复旧注册；前端只做投影、重复校验和行级错误展示。

**技术栈：** Tauri 2 + `tauri-plugin-global-shortcut 2` + Rust `BTreeMap`/serde + Vue 3 设置页 + Vitest。

**规格文档：** [docs/superpowers/specs/2026-07-03-shortcut-binding-design.md](../specs/2026-07-03-shortcut-binding-design.md)

---

## 关键上下文

- 现状：`src-tauri/src/app/shortcuts.rs` 硬编码注册 `Alt+T` / `Alt+O`，`classify_shortcut` 只区分 OCR 与划词。
- 现状：`frontend/src/settings/stores/settings.ts` 已有 6 条默认快捷键；`ShortcutRecorder` 已支持录入、清空、错误展示。
- 现状：`frontend/src/settings/panels/ShortcutPanel.vue` 把 `translate-selection` / `translate-screenshot` 设为只读，并给所有行显示 `wip`。
- 不使用 `pot-desktop/`；CodeGraph 查询结果中出现的 Pot 文件只作为索引噪音忽略。
- Ponytail 取舍：不新增快捷键 profile、冲突扫描服务、导入导出、取词触发链路；只做当前 spec 要求的最小闭环。

## 文件结构

### 后端

- 修改：`src-tauri/src/core/config/types.rs` — `AppConfig` 增加 `shortcuts` 字段、默认值和归一化测试。
- 修改：`src-tauri/src/core/selection/mod.rs` — 暴露剪贴板文本读取入口和空剪贴板错误。
- 修改：`src-tauri/src/app/shortcuts.rs` — 统一快捷键解析、去重、注册、回滚、反查和动作分发。
- 修改：`src-tauri/src/ui/config.rs` — `save_app_config` 保存前重注册快捷键，返回行级错误。
- 修改：`src-tauri/src/lib.rs` — 启动时按配置注册全局快捷键。

### 前端

- 修改：`frontend/src/types/config.ts` — 同步 `AppConfig.shortcuts` 类型。
- 修改：`frontend/src/lib/config.ts` — 投影 `state.shortcut.bindings`，导出重复绑定校验函数。
- 修改：`frontend/src/lib/config.test.ts` — 覆盖快捷键投影和重复绑定校验。
- 修改：`frontend/src/settings/stores/settings.ts` — 保存前前端去重，后端错误回填到对应行。
- 修改：`frontend/src/settings/panels/ShortcutPanel.vue` — 移除只读限制，仅 `word-lookup` 显示“规划中”。

### 文档

- 修改：`README.md`
- 修改：`AGENTS.md`
- 修改：`CLAUDE.md`
- 修改：`docs/architecture/screenshot-ocr-architecture.md`
- 修改：`docs/roadmap/progressive-development-plan.md`

---

## 任务 1：后端配置模型接入 shortcuts

**文件：**
- 修改：`src-tauri/src/core/config/types.rs`

- [ ] **步骤 1：编写失败的 Rust 单元测试**

在 `src-tauri/src/core/config/types.rs` 的 `#[cfg(test)] mod tests` 中追加：

```rust
#[test]
fn app_config_defaults_shortcuts() {
    let config = AppConfig::from_env();

    assert_eq!(
        config.shortcuts.get("translate-selection").map(String::as_str),
        Some("Alt+T")
    );
    assert_eq!(
        config.shortcuts.get("translate-clipboard").map(String::as_str),
        Some("Ctrl+Shift+C")
    );
    assert_eq!(
        config.shortcuts.get("translate-screenshot").map(String::as_str),
        Some("Alt+O")
    );
    assert_eq!(
        config.shortcuts.get("word-lookup").map(String::as_str),
        Some("")
    );
    assert_eq!(
        config.shortcuts.get("show-window").map(String::as_str),
        Some("Ctrl+Shift+Space")
    );
    assert_eq!(
        config.shortcuts.get("open-settings").map(String::as_str),
        Some("Ctrl+,")
    );
}

#[test]
fn app_config_normalized_backfills_missing_shortcuts_and_preserves_empty_disable() {
    let mut config = AppConfig::from_env();
    config.shortcuts = [("translate-selection".to_string(), "  ".to_string())]
        .into_iter()
        .collect();

    let config = config.normalized();

    assert_eq!(
        config.shortcuts.get("translate-selection").map(String::as_str),
        Some("")
    );
    assert_eq!(
        config.shortcuts.get("translate-screenshot").map(String::as_str),
        Some("Alt+O")
    );
    assert_eq!(
        config.shortcuts.get("open-settings").map(String::as_str),
        Some("Ctrl+,")
    );
}

#[test]
fn app_config_deserializes_shortcuts_defaults_when_missing() {
    let json = r#"{
        "provider": "openai-compatible",
        "targetLang": "中文",
        "openaiCompatible": {
            "apiKey": "sk-x",
            "baseUrl": "https://api.openai.com/v1",
            "model": "gpt-4o-mini",
            "timeoutSeconds": 60
        }
    }"#;

    let config = serde_json::from_str::<AppConfig>(json)
        .expect("缺少 shortcuts 字段应可反序列化")
        .normalized();

    assert_eq!(
        config.shortcuts.get("translate-selection").map(String::as_str),
        Some("Alt+T")
    );
    assert_eq!(
        config.shortcuts.get("word-lookup").map(String::as_str),
        Some("")
    );
}
```

- [ ] **步骤 2：运行测试确认失败**

运行：`cd src-tauri && cargo test core::config::types::tests::app_config_defaults_shortcuts core::config::types::tests::app_config_normalized_backfills_missing_shortcuts_and_preserves_empty_disable core::config::types::tests::app_config_deserializes_shortcuts_defaults_when_missing`

预期：FAIL，报错包含 `no field shortcuts on type AppConfig`。

- [ ] **步骤 3：实现最少配置模型代码**

在 `src-tauri/src/core/config/types.rs` 顶部把导入改为：

```rust
use std::{collections::BTreeMap, env};
```

在默认常量后加入：

```rust
pub type ShortcutConfig = BTreeMap<String, String>;

fn default_shortcuts() -> ShortcutConfig {
    [
        ("translate-selection", "Alt+T"),
        ("translate-clipboard", "Ctrl+Shift+C"),
        ("translate-screenshot", "Alt+O"),
        ("word-lookup", ""),
        ("show-window", "Ctrl+Shift+Space"),
        ("open-settings", "Ctrl+,"),
    ]
    .into_iter()
    .map(|(id, keys)| (id.to_string(), keys.to_string()))
    .collect()
}

fn normalize_shortcuts(shortcuts: ShortcutConfig) -> ShortcutConfig {
    let mut normalized = default_shortcuts();
    for (id, keys) in shortcuts {
        if normalized.contains_key(&id) {
            normalized.insert(id, keys.trim().to_string());
        }
    }
    normalized
}
```

在 `AppConfig` 结构体中追加字段：

```rust
    #[serde(default = "default_shortcuts")]
    pub shortcuts: ShortcutConfig,
```

在 `AppConfig::from_env()` 初始化中追加：

```rust
            shortcuts: default_shortcuts(),
```

在 `AppConfig::normalized()` 中追加：

```rust
        self.shortcuts = normalize_shortcuts(self.shortcuts);
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test core::config::types::tests::app_config_defaults_shortcuts core::config::types::tests::app_config_normalized_backfills_missing_shortcuts_and_preserves_empty_disable core::config::types::tests::app_config_deserializes_shortcuts_defaults_when_missing`

预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/core/config/types.rs
git commit -m "feat(config): 增加快捷键配置默认值"
```

---

## 任务 2：后端快捷键解析、去重与反查

**文件：**
- 修改：`src-tauri/src/app/shortcuts.rs`

- [ ] **步骤 1：编写失败的 Rust 单元测试**

替换 `src-tauri/src/app/shortcuts.rs` 中现有 `#[cfg(test)] mod tests` 为：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::AppConfig;

    fn config_with(bindings: &[(&str, &str)]) -> AppConfig {
        let mut config = AppConfig::from_env();
        for (id, keys) in bindings {
            config.shortcuts.insert((*id).to_string(), (*keys).to_string());
        }
        config.normalized()
    }

    #[test]
    fn classifies_configured_selection_shortcut() {
        let config = config_with(&[("translate-selection", "Ctrl+Alt+T")]);
        let shortcut = "Ctrl+Alt+T"
            .parse::<Shortcut>()
            .expect("快捷键应可解析");

        assert_eq!(
            classify_shortcut(&shortcut, &config),
            Some(ShortcutAction::SelectionTranslate)
        );
    }

    #[test]
    fn classifies_configured_ocr_shortcut() {
        let config = config_with(&[("translate-screenshot", "Ctrl+Alt+O")]);
        let shortcut = "Ctrl+Alt+O"
            .parse::<Shortcut>()
            .expect("快捷键应可解析");

        assert_eq!(
            classify_shortcut(&shortcut, &config),
            Some(ShortcutAction::OcrTranslate)
        );
    }

    #[test]
    fn classifies_unregistered_empty_binding_as_none() {
        let config = config_with(&[("translate-selection", "")]);
        let shortcut = "Alt+T".parse::<Shortcut>().expect("Alt+T 应可解析");

        assert_eq!(classify_shortcut(&shortcut, &config), None);
    }

    #[test]
    fn validates_duplicate_shortcuts_across_all_bindings() {
        let config = config_with(&[
            ("translate-selection", "Alt+T"),
            ("word-lookup", "Alt+T"),
        ]);

        let error = configured_shortcuts(&config).expect_err("重复快捷键应失败");

        assert_eq!(error.id, "word-lookup");
        assert!(error.message.contains("划词翻译"));
    }

    #[test]
    fn keeps_word_lookup_unimplemented_after_validation() {
        let config = config_with(&[("word-lookup", "Ctrl+Alt+W")]);
        let entries = configured_shortcuts(&config).expect("配置应可解析");

        let word_lookup = entries
            .iter()
            .find(|entry| entry.id == "word-lookup")
            .expect("应保留取词绑定用于保存和去重");

        assert_eq!(word_lookup.action, None);
    }
}
```

- [ ] **步骤 2：运行测试确认失败**

运行：`cd src-tauri && cargo test app::shortcuts::tests`

预期：FAIL，报错包含 `this function takes 1 argument but 2 arguments were supplied` 或 `cannot find function configured_shortcuts`。

- [ ] **步骤 3：实现纯解析、去重、反查代码**

在 `src-tauri/src/app/shortcuts.rs` 顶部导入调整为：

```rust
use std::{thread, time::Duration};

use serde::Serialize;
use tauri::{Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::{
    app::{state::AppState, window::show_window},
    core::{
        config::AppConfig,
        selection::{copy_selected_text, read_clipboard_text},
        translation::TranslationInput,
    },
    ui::{
        ocr_popup::start_translation_from_ocr,
        web_popup::{show_translation_error, show_translation_popup, start_translation_from_input},
    },
};
```

把 `ShortcutAction` 和 `classify_shortcut` 改为：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShortcutAction {
    ClipboardTranslate,
    OcrTranslate,
    OpenSettings,
    SelectionTranslate,
    ShowWindow,
}

#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[error("{message}")]
#[serde(rename_all = "camelCase")]
pub struct ShortcutBindingError {
    pub id: String,
    pub message: String,
}

impl ShortcutBindingError {
    fn new(id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            message: message.into(),
        }
    }

    fn global(message: impl Into<String>) -> Self {
        Self::new("", message)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfiguredShortcut {
    id: String,
    keys: String,
    shortcut: Shortcut,
    action: Option<ShortcutAction>,
}

fn action_for_id(id: &str) -> Option<ShortcutAction> {
    match id {
        "translate-selection" => Some(ShortcutAction::SelectionTranslate),
        "translate-clipboard" => Some(ShortcutAction::ClipboardTranslate),
        "translate-screenshot" => Some(ShortcutAction::OcrTranslate),
        "show-window" => Some(ShortcutAction::ShowWindow),
        "open-settings" => Some(ShortcutAction::OpenSettings),
        "word-lookup" => None,
        _ => None,
    }
}

fn label_for_id(id: &str) -> &'static str {
    match id {
        "translate-selection" => "划词翻译",
        "translate-clipboard" => "剪贴板翻译",
        "translate-screenshot" => "截图翻译",
        "word-lookup" => "取词翻译",
        "show-window" => "显示主窗口",
        "open-settings" => "打开设置",
        _ => "未知动作",
    }
}

fn configured_shortcuts(config: &AppConfig) -> Result<Vec<ConfiguredShortcut>, ShortcutBindingError> {
    let mut entries: Vec<ConfiguredShortcut> = Vec::new();

    for (id, keys) in &config.shortcuts {
        let keys = keys.trim();
        if keys.is_empty() {
            continue;
        }

        let shortcut = keys.parse::<Shortcut>().map_err(|error| {
            ShortcutBindingError::new(id, format!("无法解析快捷键「{keys}」: {error}"))
        })?;

        if let Some(existing) = entries
            .iter()
            .find(|entry| entry.shortcut == shortcut)
        {
            return Err(ShortcutBindingError::new(
                id,
                format!("与「{}」重复", label_for_id(&existing.id)),
            ));
        }

        entries.push(ConfiguredShortcut {
            id: id.clone(),
            keys: keys.to_string(),
            shortcut,
            action: action_for_id(id),
        });
    }

    Ok(entries)
}

fn classify_shortcut(shortcut: &Shortcut, config: &AppConfig) -> Option<ShortcutAction> {
    configured_shortcuts(config)
        .ok()?
        .into_iter()
        .find(|entry| entry.shortcut == *shortcut)
        .and_then(|entry| entry.action)
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test app::shortcuts::tests`

预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/app/shortcuts.rs
git commit -m "feat(shortcuts): 按配置解析并反查快捷键动作"
```

---

## 任务 3：注册入口、动作分发与剪贴板翻译

**文件：**
- 修改：`src-tauri/src/core/selection/mod.rs`
- 修改：`src-tauri/src/app/shortcuts.rs`

- [ ] **步骤 1：编写失败的剪贴板文本归一化测试**

在 `src-tauri/src/core/selection/mod.rs` 末尾追加测试模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_text_trims_non_empty_value() {
        assert_eq!(
            normalize_clipboard_text(Some("  hello  ".to_string())).expect("应读取到文本"),
            "hello"
        );
    }

    #[test]
    fn clipboard_text_rejects_empty_value() {
        let error = normalize_clipboard_text(Some("   ".to_string()))
            .expect_err("空文本应失败");

        assert!(matches!(error, SelectionError::EmptyClipboard));
    }

    #[test]
    fn clipboard_text_rejects_missing_value() {
        let error = normalize_clipboard_text(None).expect_err("无文本应失败");

        assert!(matches!(error, SelectionError::EmptyClipboard));
    }
}
```

- [ ] **步骤 2：运行测试确认失败**

运行：`cd src-tauri && cargo test core::selection::tests`

预期：FAIL，报错包含 `cannot find function normalize_clipboard_text` 和 `no variant or associated item named EmptyClipboard`。

- [ ] **步骤 3：实现剪贴板读取入口**

在 `SelectionError` 中追加：

```rust
    #[error("剪贴板中没有可翻译的文本")]
    EmptyClipboard,
```

在 `copy_selected_text()` 前加入：

```rust
pub fn read_clipboard_text() -> Result<String, SelectionError> {
    normalize_clipboard_text(clipboard::read_text()?)
}

fn normalize_clipboard_text(text: Option<String>) -> Result<String, SelectionError> {
    let text = text.unwrap_or_default().trim().to_string();
    if text.is_empty() {
        Err(SelectionError::EmptyClipboard)
    } else {
        Ok(text)
    }
}
```

- [ ] **步骤 4：运行剪贴板测试验证通过**

运行：`cd src-tauri && cargo test core::selection::tests`

预期：PASS。

- [ ] **步骤 5：实现注册替换和快捷键动作分发**

在 `src-tauri/src/app/shortcuts.rs` 中把旧 `register_global_shortcuts`、`handle_global_shortcut`、`handle_selection_translate` 替换为：

```rust
pub fn register_global_shortcuts(
    app: &tauri::AppHandle,
    config: &AppConfig,
) -> Result<(), ShortcutBindingError> {
    let entries = configured_shortcuts(config)?;

    app.global_shortcut()
        .unregister_all()
        .map_err(|error| ShortcutBindingError::global(format!("无法清理旧快捷键: {error}")))?;

    for entry in entries.into_iter().filter(|entry| entry.action.is_some()) {
        app.global_shortcut()
            .register(entry.keys.as_str())
            .map_err(|error| {
                ShortcutBindingError::new(
                    entry.id,
                    format!("注册快捷键「{}」失败: {error}", entry.keys),
                )
            })?;
    }

    Ok(())
}

pub fn replace_global_shortcuts(
    app: &tauri::AppHandle,
    old_config: &AppConfig,
    new_config: &AppConfig,
) -> Result<(), ShortcutBindingError> {
    if let Err(error) = register_global_shortcuts(app, new_config) {
        let _ = register_global_shortcuts(app, old_config);
        return Err(error);
    }
    Ok(())
}

pub fn handle_global_shortcut(
    app: &tauri::AppHandle,
    shortcut: &tauri_plugin_global_shortcut::Shortcut,
    event: tauri_plugin_global_shortcut::ShortcutEvent,
) {
    if event.state != ShortcutState::Released {
        return;
    }

    let state: State<'_, AppState> = app.state();
    let config = match state.config_store.get() {
        Ok(config) => config,
        Err(error) => {
            show_translation_error(app, error.to_string());
            return;
        }
    };

    match classify_shortcut(shortcut, &config) {
        Some(ShortcutAction::SelectionTranslate) => handle_selection_translate(app),
        Some(ShortcutAction::ClipboardTranslate) => handle_clipboard_translate(app),
        Some(ShortcutAction::OcrTranslate) => {
            let app_handle = app.clone();
            let state = state.inner().clone();
            tauri::async_runtime::spawn(async move {
                start_translation_from_ocr(app_handle, state).await;
            });
        }
        Some(ShortcutAction::ShowWindow | ShortcutAction::OpenSettings) => show_window(app),
        None => {}
    }
}

fn handle_selection_translate(app: &tauri::AppHandle) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        thread::sleep(Duration::from_millis(40));

        let selected_text = match copy_selected_text() {
            Ok(text) => text,
            Err(error) => {
                show_translation_error(&app_handle, error.to_string());
                return;
            }
        };

        start_popup_translation(app_handle, TranslationInput::SelectedText(selected_text));
    });
}

fn handle_clipboard_translate(app: &tauri::AppHandle) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let text = match read_clipboard_text() {
            Ok(text) => text,
            Err(error) => {
                show_translation_error(&app_handle, error.to_string());
                return;
            }
        };

        start_popup_translation(app_handle, TranslationInput::ManualText(text));
    });
}

fn start_popup_translation(app_handle: tauri::AppHandle, input: TranslationInput) {
    let source_text = input.text().to_string();
    let state: State<'_, AppState> = app_handle.state();

    if let Err(error) = state.set_pending_source_text(source_text) {
        show_translation_error(&app_handle, error);
        return;
    }

    let config = state.config_store.get();
    if let Ok(config) = &config {
        if let Err(error) = show_translation_popup(&app_handle, config) {
            show_translation_error(&app_handle, error);
            return;
        }
    }

    if let Err(error) = start_translation_from_input(input, app_handle.clone(), state.inner()) {
        show_translation_error(&app_handle, error);
    }
}
```

- [ ] **步骤 6：运行后端测试和编译验证**

运行：`cd src-tauri && cargo test app::shortcuts::tests core::selection::tests`

预期：PASS。

运行：`cd src-tauri && cargo build`

预期：FAIL，报错来自 `src-tauri/src/lib.rs` 仍按旧签名调用 `register_global_shortcuts(app)`；下一任务修正启动路径。

- [ ] **步骤 7：Commit**

```bash
git add src-tauri/src/core/selection/mod.rs src-tauri/src/app/shortcuts.rs
git commit -m "feat(shortcuts): 注册配置快捷键并接入剪贴板翻译"
```

---

## 任务 4：启动注册与保存前重注册

**文件：**
- 修改：`src-tauri/src/lib.rs`
- 修改：`src-tauri/src/ui/config.rs`

- [ ] **步骤 1：修改启动时注册逻辑**

在 `src-tauri/src/lib.rs` 的 `.setup(|app| { ... })` 中，把当前 `register_global_shortcuts(app)` 调用移动到读取配置之后，形成：

```rust
            let config = app
                .state::<AppState>()
                .config_store
                .get()
                .unwrap_or_else(|_| AppConfig::from_env());

            register_global_shortcuts(app.handle(), &config)
                .map_err(|error| tauri::Error::Anyhow(error.into()))?;

            let _ = ensure_popup_window(app.handle(), &config);
            let _ = ensure_overlay(app.handle());
```

保留后面的主窗口显隐逻辑继续使用同一个 `config`。

- [ ] **步骤 2：修改保存 command 的签名和保存顺序**

把 `src-tauri/src/ui/config.rs` 改为：

```rust
use tauri::Manager;

use crate::{
    app::{
        shortcuts::{replace_global_shortcuts, ShortcutBindingError},
        state::AppState,
        window::show_window,
    },
    core::config::AppConfig,
};

#[tauri::command]
pub fn open_settings(app: tauri::AppHandle) -> Result<(), String> {
    show_window(&app);
    Ok(())
}

#[tauri::command]
pub async fn get_app_config(state: tauri::State<'_, AppState>) -> Result<AppConfig, String> {
    state.config_store.get().map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn save_app_config(
    config: AppConfig,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<AppConfig, ShortcutBindingError> {
    let old_config = state
        .config_store
        .get()
        .map_err(|error| ShortcutBindingError::global(format!("无法读取旧配置: {error}")))?;
    let config = config.normalized();

    replace_global_shortcuts(&app, &old_config, &config)?;

    state
        .config_store
        .save(config)
        .map_err(|error| ShortcutBindingError::global(format!("无法保存配置: {error}")))
}
```

- [ ] **步骤 3：运行后端验证**

运行：`cd src-tauri && cargo test`

预期：PASS。

运行：`cd src-tauri && cargo build`

预期：PASS。

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/ui/config.rs
git commit -m "feat(config): 保存配置时即时重注册快捷键"
```

---

## 任务 5：前端 AppConfig 类型、投影和重复校验

**文件：**
- 修改：`frontend/src/types/config.ts`
- 修改：`frontend/src/lib/config.ts`
- 修改：`frontend/src/lib/config.test.ts`

- [ ] **步骤 1：编写失败的前端单元测试**

在 `frontend/src/lib/config.test.ts` 顶部导入改为：

```typescript
import { projectToAppConfig, validateConfig, validateShortcutBindings } from './config';
```

给 `base` 追加：

```typescript
  shortcuts: {
    'translate-selection': 'Alt+T',
    'translate-screenshot': 'Alt+O',
  },
```

在 `describe('projectToAppConfig', () => { ... })` 内追加：

```typescript
  it('投影快捷键绑定到后端 shortcuts 字段', () => {
    const s = makeState(
      [makeInstance({ id: 'i1', type: 'openai' })],
      'i1',
    );
    s.shortcut.bindings = [
      { id: 'translate-selection', label: '划词翻译', description: '', keys: 'Ctrl+Alt+T' },
      { id: 'translate-screenshot', label: '截图翻译', description: '', keys: '' },
    ];

    const { config } = projectToAppConfig(s, 'openai-compatible');

    expect(config.shortcuts).toEqual({
      'translate-selection': 'Ctrl+Alt+T',
      'translate-screenshot': '',
    });
  });
```

在文件末尾追加：

```typescript
describe('validateShortcutBindings', () => {
  it('空快捷键不参与重复校验', () => {
    expect(validateShortcutBindings([
      { id: 'a', label: 'A', keys: '' },
      { id: 'b', label: 'B', keys: '' },
    ])).toEqual({});
  });

  it('重复快捷键返回两行错误', () => {
    expect(validateShortcutBindings([
      { id: 'selection', label: '划词翻译', keys: 'Alt+T' },
      { id: 'lookup', label: '取词翻译', keys: 'alt+t' },
    ])).toEqual({
      selection: '与「取词翻译」重复',
      lookup: '与「划词翻译」重复',
    });
  });
});
```

- [ ] **步骤 2：运行测试确认失败**

运行：`npm run test -- --run frontend/src/lib/config.test.ts`

预期：FAIL，报错包含 `validateShortcutBindings is not a function` 或 `Property 'shortcuts' is missing`。

- [ ] **步骤 3：同步前端 AppConfig 类型**

在 `frontend/src/types/config.ts` 中加入：

```typescript
export type ShortcutConfig = Record<string, string>;
```

并在 `AppConfig` 中追加：

```typescript
  shortcuts: ShortcutConfig;
```

- [ ] **步骤 4：实现投影和重复校验**

在 `frontend/src/lib/config.ts` 的类型导入改为：

```typescript
import type { AppSettings, ServiceInstance, ShortcutBinding } from '@/settings/types';
```

在默认常量后加入：

```typescript
type ShortcutLike = Pick<ShortcutBinding, 'id' | 'label' | 'keys'>;

const projectShortcuts = (state: AppSettings): Record<string, string> =>
  Object.fromEntries(state.shortcut.bindings.map((binding) => [binding.id, binding.keys.trim()]));

export function validateShortcutBindings(bindings: ShortcutLike[]): Record<string, string> {
  const errors: Record<string, string> = {};
  const seen = new Map<string, ShortcutLike>();

  for (const binding of bindings) {
    const keys = binding.keys.trim();
    if (!keys) continue;

    const normalized = keys.toLowerCase();
    const existing = seen.get(normalized);
    if (existing) {
      errors[binding.id] = `与「${existing.label}」重复`;
      errors[existing.id] ??= `与「${binding.label}」重复`;
    } else {
      seen.set(normalized, binding);
    }
  }

  return errors;
}
```

在 `makeOpenAiConfig`、`makeClaudeConfig`、`makeDefaultConfig` 返回对象中都追加：

```typescript
    shortcuts: projectShortcuts(state),
```

- [ ] **步骤 5：运行前端测试验证通过**

运行：`npm run test -- --run frontend/src/lib/config.test.ts`

预期：PASS。

- [ ] **步骤 6：Commit**

```bash
git add frontend/src/types/config.ts frontend/src/lib/config.ts frontend/src/lib/config.test.ts
git commit -m "feat(settings): 投影快捷键配置并校验重复绑定"
```

---

## 任务 6：前端保存错误回填与快捷键面板解锁

**文件：**
- 修改：`frontend/src/settings/stores/settings.ts`
- 修改：`frontend/src/settings/panels/ShortcutPanel.vue`

- [ ] **步骤 1：修改设置 store 的导入**

把 `frontend/src/settings/stores/settings.ts` 中导入改为：

```typescript
import { projectToAppConfig, validateConfig, validateShortcutBindings } from '@/lib/config'
```

- [ ] **步骤 2：新增快捷键错误辅助函数**

在 `markDirty` 函数后加入：

```typescript
const clearShortcutErrors = (): void => {
  for (const binding of state.shortcut.bindings) {
    binding.error = undefined
  }
}

const applyShortcutErrors = (errors: Record<string, string>): void => {
  for (const binding of state.shortcut.bindings) {
    binding.error = errors[binding.id]
  }
}

const applyBackendShortcutError = (error: unknown): string | null => {
  if (!error || typeof error !== 'object') return null
  const payload = error as { id?: unknown; message?: unknown }
  if (typeof payload.message !== 'string') return null

  if (typeof payload.id === 'string' && payload.id) {
    const binding = state.shortcut.bindings.find((item) => item.id === payload.id)
    if (binding) binding.error = payload.message
  }

  return payload.message
}
```

- [ ] **步骤 3：保存前执行前端去重，后端错误回填**

把 `useSettings().save()` 开头改为：

```typescript
  async save(): Promise<void> {
    clearShortcutErrors()

    const shortcutErrors = validateShortcutBindings(state.shortcut.bindings)
    if (Object.keys(shortcutErrors).length > 0) {
      applyShortcutErrors(shortcutErrors)
      toast.error('保存失败', '请先解决重复快捷键')
      return
    }

    const { config, unsupported, unsupportedName } = projectToAppConfig(state, lastSavedProvider)
```

把 Tauri 保存 catch 分支改为：

```typescript
      } catch (e) {
        const shortcutMessage = applyBackendShortcutError(e)
        toast.error('保存失败', shortcutMessage ?? String(e))
      }
```

- [ ] **步骤 4：解锁快捷键面板**

把 `frontend/src/settings/panels/ShortcutPanel.vue` 的 `<script setup>` 改为：

```vue
<script setup lang="ts">
import { SettingGroup, SettingRow, ShortcutRecorder } from '../components'
import type { AppSettings } from '../types'

defineProps<{
  state: AppSettings
}>()
</script>
```

把 `SettingRow` 状态绑定改为：

```vue
      :status="binding.id === 'word-lookup' ? 'planned' : undefined"
```

把 `ShortcutRecorder` 的 `disabled` 属性删除，保留：

```vue
      <ShortcutRecorder
        :model-value="binding.keys"
        :error="binding.error"
        @update:model-value="(v) => {
          binding.keys = v
          binding.error = undefined
        }"
      />
```

- [ ] **步骤 5：运行前端验证**

运行：`npm run test`

预期：PASS。

运行：`npm run typecheck`

预期：PASS。

- [ ] **步骤 6：Commit**

```bash
git add frontend/src/settings/stores/settings.ts frontend/src/settings/panels/ShortcutPanel.vue
git commit -m "feat(settings): 解锁快捷键编辑并回填保存错误"
```

---

## 任务 7：文档同步与最终验证

**文件：**
- 修改：`README.md`
- 修改：`AGENTS.md`
- 修改：`CLAUDE.md`
- 修改：`docs/architecture/screenshot-ocr-architecture.md`
- 修改：`docs/roadmap/progressive-development-plan.md`

- [ ] **步骤 1：更新 README 当前能力与限制**

把 `README.md` 顶部快捷键能力改为：

```markdown
- 可配置全局快捷键：设置页「全局快捷键」可修改、清空并保存划词翻译、截图 OCR 翻译、剪贴板翻译、显示主窗口和打开设置快捷键，保存成功后无需重启即可生效；取词翻译本轮仅保存绑定，不注册触发。
```

把 `README.md` 中“后端硬编码不可配”相关限制改为：

```markdown
- 取词翻译、快捷键分组 / profile、导入导出仍未实现；`word-lookup` 绑定当前只保存不触发。
```

- [ ] **步骤 2：同步 AGENTS.md 与 CLAUDE.md 全局快捷键说明**

把两份文件的「全局快捷键」架构关键点改为同一段：

```markdown
- **全局快捷键**：默认 `Alt+T` 划词复制并自动翻译，`Alt+O` 触发截图 OCR 翻译；设置页会把 6 条快捷键保存到 `AppConfig.shortcuts`，保存成功后后端通过统一入口 `unregister_all()` + `register()` 立即重注册。已触发动作包括划词翻译、剪贴板翻译、截图 OCR 翻译、显示主窗口、打开设置；`word-lookup` 当前只保存绑定，不注册触发。由 `tauri-plugin-global-shortcut` 注册，逻辑集中在 `src-tauri/src/app/shortcuts.rs`。
```

- [ ] **步骤 3：更新截图 OCR 架构文档**

把 `docs/architecture/screenshot-ocr-architecture.md` 中“## 快捷键与 Tauri 权限”下现状段落改为：

```markdown
现有全局快捷键统一由 `src-tauri/src/app/shortcuts.rs` 注册、解析和分发。启动时从 `AppConfig.shortcuts` 读取配置；设置页保存配置时先重注册快捷键，成功后再写入 `ConfigStore`，因此划词翻译、截图 OCR 翻译、剪贴板翻译、显示主窗口和打开设置无需重启即可生效。`word-lookup` 绑定会保存，但本阶段不注册触发。

Tauri 官方文档说明 global shortcut 插件默认不启用危险能力，需要通过 capabilities 显式授权。当前项目已有 `src-tauri/capabilities/default.json` 的 `global-shortcut:default`。
```

把端到端落地状态中 “已注册 `Alt+O` 全局快捷键” 改为：

```markdown
- 默认注册 `Alt+O` 作为截图 OCR 快捷键；用户可在设置页改绑或清空，保存后立即生效。
```

- [ ] **步骤 4：更新 roadmap 进度**

在 `docs/roadmap/progressive-development-plan.md` 的“快捷键、OCR、截图、弹窗体验打磨”附近加入：

```markdown
- **快捷键绑定配置**（已完成，2026-07）：设置页 6 条快捷键接入后端 `AppConfig.shortcuts`；划词翻译、截图 OCR 翻译、剪贴板翻译、显示主窗口、打开设置支持改绑 / 清空 / 保存后即时生效；重复或系统占用时阻止保存并回填对应行错误；`word-lookup` 仅保存绑定。
```

- [ ] **步骤 5：运行完整验证**

运行：`cd src-tauri && cargo test`

预期：PASS。

运行：`cd src-tauri && cargo build`

预期：PASS。

运行：`npm run test`

预期：PASS。

运行：`npm run typecheck`

预期：PASS。

- [ ] **步骤 6：手动验收**

运行：`SHIZI_LLM_PROVIDER=mock npm run tauri dev`

逐项验证：

1. 把划词翻译从 `Alt+T` 改为 `Ctrl+Alt+T` 并保存，`Alt+T` 不再触发，`Ctrl+Alt+T` 触发划词翻译。
2. 把截图翻译从 `Alt+O` 改为 `Ctrl+Alt+O` 并保存，旧快捷键不触发，新快捷键打开 overlay。
3. 清空划词翻译快捷键并保存，对应动作不触发。
4. 剪贴板中有文本时，剪贴板翻译快捷键打开翻译弹窗并翻译剪贴板文本。
5. 剪贴板为空时，翻译弹窗展示“剪贴板中没有可翻译的文本”且不可重试。
6. 显示主窗口 / 打开设置快捷键都能唤起并聚焦主窗口。
7. 设置两行相同快捷键，前端阻止保存并在两行显示重复错误。
8. 设置系统占用快捷键，后端拒绝保存并在对应行显示注册失败原因。
9. 重启应用后，保存过的快捷键仍生效。

- [ ] **步骤 7：Commit**

```bash
git add README.md AGENTS.md CLAUDE.md docs/architecture/screenshot-ocr-architecture.md docs/roadmap/progressive-development-plan.md
git commit -m "docs(shortcuts): 同步快捷键绑定配置状态"
```

---

## 自检

### 1. 规格覆盖度

- `translate-selection` 可改绑、清空、保存、立即生效：任务 1-4 后端闭环，任务 5-6 前端保存闭环，任务 7 验收 1/3。
- `translate-screenshot` 可改绑、清空、保存、立即生效：任务 1-4 后端闭环，任务 7 验收 2。
- 保存后无需重启：任务 4 `save_app_config` 先 `replace_global_shortcuts` 再写盘。
- 快捷键为空表示禁用：任务 1 保留空字符串，任务 2/3 注册时跳过空绑定。
- 注册失败不保存并行级展示：任务 3 `register_global_shortcuts` 返回 `ShortcutBindingError`，任务 4 保存前注册，任务 6 回填错误。
- `translate-clipboard`：任务 3 暴露 `read_clipboard_text` 并分发 `ClipboardTranslate`。
- `show-window` / `open-settings`：任务 3 分发到 `show_window`，任务 4 让 command 也复用该函数。
- `word-lookup` 只保存不注册：任务 2 `action_for_id("word-lookup") -> None`，任务 3 注册过滤 `action.is_some()`，任务 6 显示“规划中”。
- 重复快捷键前后端都拒绝：任务 2 后端 `configured_shortcuts`，任务 5/6 前端 `validateShortcutBindings`。
- 文档同步：任务 7 覆盖 spec 要求的 5 个文档。

遗漏：无。

### 2. 占位符扫描

- 无禁用占位词。
- 每个涉及代码的步骤都提供了目标代码块。
- 每个验证步骤都有命令和预期结果。
- 所有 commit message 均符合 `<type>(<scope>): <中文描述>`。

### 3. 类型一致性

- 后端 `AppConfig.shortcuts` 使用 `BTreeMap<String, String>`，前端 `ShortcutConfig` 使用 `Record<string, string>`；JSON 字段名同为 `shortcuts`。
- `ShortcutBindingError` 字段为 `id` / `message`，serde camelCase 后仍是 `id` / `message`；前端 catch 按同名字段读取。
- `ShortcutAction` 只在后端内部使用，不进入 JSON。
- `translate-clipboard` 使用 `TranslationInput::ManualText`，复用现有翻译事件和弹窗渲染。
- `show-window` 与 `open-settings` 都复用 `app::window::show_window`，行为符合 spec。
