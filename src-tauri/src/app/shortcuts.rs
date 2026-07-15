use std::{thread, time::Duration};

use serde::Serialize;
use tauri::{Manager, State, WebviewWindow, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::{
    app::{
        state::AppState,
        window::{show_settings_window, OCR_LABEL, SETTINGS_LABEL},
    },
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

/// 仅全局作用域的快捷键参与 all-or-nothing 注册；程序快捷键在窗口聚焦时另行挂载。
pub fn register_global_shortcuts(
    app: &tauri::AppHandle,
    config: &AppConfig,
) -> Result<(), ShortcutBindingError> {
    let entries = configured_shortcuts(config)?;

    app.global_shortcut()
        .unregister_all()
        .map_err(|error| ShortcutBindingError::global(format!("无法清理旧快捷键: {error}")))?;

    for entry in entries
        .into_iter()
        .filter(|entry| entry.kind == ShortcutKind::Global && entry.action.is_some())
    {
        app.global_shortcut()
            .register(entry.keys.as_str())
            .map_err(|error| ShortcutBindingError::new(entry.id, friendly_register_error(error)))?;
    }

    // unregister_all 清掉了程序快捷键，若当前有窗口聚焦则重新挂上
    sync_app_local_shortcuts(app, config);

    Ok(())
}

/// 启动时尽力注册全局快捷键：逐条注册，单条失败（如被其他应用占用）
/// 只记录到返回的冲突列表，不阻止应用启动。`register_global_shortcuts`
/// 仍是 all-or-nothing，用于保存路径以便回滚。
pub fn register_global_shortcuts_at_startup(
    app: &tauri::AppHandle,
    config: &AppConfig,
) -> Vec<ShortcutBindingError> {
    let mut conflicts = Vec::new();

    let entries = match configured_shortcuts(config) {
        Ok(entries) => entries,
        Err(error) => {
            conflicts.push(error);
            return conflicts;
        }
    };

    if let Err(error) = app.global_shortcut().unregister_all() {
        conflicts.push(ShortcutBindingError::global(format!(
            "无法清理旧快捷键: {error}"
        )));
        return conflicts;
    }

    for entry in entries
        .into_iter()
        .filter(|entry| entry.kind == ShortcutKind::Global && entry.action.is_some())
    {
        if let Err(error) = app.global_shortcut().register(entry.keys.as_str()) {
            conflicts.push(ShortcutBindingError::new(
                entry.id,
                friendly_register_error(error),
            ));
        }
    }

    // 启动时主窗口可能尚未聚焦；聚焦后由 focus listener 再挂程序快捷键
    sync_app_local_shortcuts(app, config);

    conflicts
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

/// 主窗 / 设置窗获得或失去焦点时调用：有任一窗口聚焦则注册程序快捷键，否则卸下。
/// 使用 OS 级热键（聚焦期间有效），避免 WebView 吞掉 `Ctrl+,` 等组合键。
pub fn sync_app_local_shortcuts(app: &tauri::AppHandle, config: &AppConfig) {
    let Ok(entries) = configured_shortcuts(config) else {
        return;
    };
    let app_locals: Vec<_> = entries
        .into_iter()
        .filter(|entry| entry.kind == ShortcutKind::AppLocal && entry.action.is_some())
        .collect();

    for entry in &app_locals {
        let _ = app.global_shortcut().unregister(entry.keys.as_str());
    }

    if !any_app_window_focused(app) {
        return;
    }

    for entry in &app_locals {
        if let Err(error) = app.global_shortcut().register(entry.keys.as_str()) {
            log::warn!(
                "程序快捷键「{}」注册失败: {}",
                entry.id,
                friendly_register_error(error)
            );
        }
    }
}

pub fn sync_app_local_shortcuts_from_state(app: &tauri::AppHandle) {
    let state: State<'_, AppState> = app.state();
    let Ok(config) = state.config_store.get() else {
        return;
    };
    sync_app_local_shortcuts(app, &config);
}

/// 监听窗口聚焦变化，延迟一拍再同步，避免主窗↔设置窗切换时短暂「双 blur」卸键。
pub fn attach_app_shortcut_focus_listener(window: &WebviewWindow, app: &tauri::AppHandle) {
    let app_handle = app.clone();
    window.on_window_event(move |event| {
        if !matches!(event, WindowEvent::Focused(_)) {
            return;
        }
        let app2 = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(40)).await;
            sync_app_local_shortcuts_from_state(&app2);
        });
    });
}

fn any_app_window_focused(app: &tauri::AppHandle) -> bool {
    for label in ["main", SETTINGS_LABEL, OCR_LABEL] {
        if let Some(window) = app.get_webview_window(label) {
            if window.is_focused().unwrap_or(false) {
                return true;
            }
        }
    }
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShortcutKind {
    /// 系统级全局，应用未聚焦也生效
    Global,
    /// 仅本应用任一窗口聚焦时挂载
    AppLocal,
    /// 仅保存配置、不注册
    Unimplemented,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShortcutAction {
    ClipboardTranslate,
    OcrTranslate,
    OcrRecognize,
    SelectionTranslate,
    OpenSettings,
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

    pub(crate) fn global(message: impl Into<String>) -> Self {
        Self::new("", message)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfiguredShortcut {
    id: String,
    keys: String,
    shortcut: Shortcut,
    kind: ShortcutKind,
    action: Option<ShortcutAction>,
}

fn kind_for_id(id: &str) -> ShortcutKind {
    match id {
        "translate-selection"
        | "translate-clipboard"
        | "translate-screenshot"
        | "ocr-recognize" => ShortcutKind::Global,
        "open-settings" => ShortcutKind::AppLocal,
        _ => ShortcutKind::Unimplemented,
    }
}

fn action_for_id(id: &str) -> Option<ShortcutAction> {
    match id {
        "translate-selection" => Some(ShortcutAction::SelectionTranslate),
        "translate-clipboard" => Some(ShortcutAction::ClipboardTranslate),
        "translate-screenshot" => Some(ShortcutAction::OcrTranslate),
        "ocr-recognize" => Some(ShortcutAction::OcrRecognize),
        "open-settings" => Some(ShortcutAction::OpenSettings),
        // word-lookup：保留配置用于去重与 UI，本阶段不触发
        _ => None,
    }
}

fn label_for_id(id: &str) -> &'static str {
    match id {
        "translate-selection" => "划词翻译",
        "translate-clipboard" => "剪贴板翻译",
        "translate-screenshot" => "截图翻译",
        "ocr-recognize" => "文字识别",
        "word-lookup" => "取词翻译",
        "open-settings" => "打开设置",
        _ => "未知动作",
    }
}

fn configured_shortcuts(
    config: &AppConfig,
) -> Result<Vec<ConfiguredShortcut>, ShortcutBindingError> {
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
            kind: kind_for_id(id),
            action: action_for_id(id),
        });
    }

    Ok(entries)
}

/// 把全局快捷键注册错误转成简洁可读的冲突原因。
/// `tauri-plugin-global-shortcut` 的原始错误含 `HotKey { ... }` 结构体调试输出，
/// 直接展示会撑爆设置页布局，这里只保留人类可读的结论。
fn friendly_register_error(error: impl std::fmt::Display) -> String {
    if error.to_string().contains("already registered") {
        "快捷键已被占用".to_string()
    } else {
        "快捷键注册失败".to_string()
    }
}

fn classify_shortcut(shortcut: &Shortcut, config: &AppConfig) -> Option<ShortcutAction> {
    configured_shortcuts(config)
        .ok()?
        .into_iter()
        .find(|entry| entry.shortcut == *shortcut)
        .and_then(|entry| entry.action)
}

/// 划词翻译等快捷键在**松开时**触发，而非按下时。
///
/// 曾误改为 Pressed 触发（5f83c56），导致浏览器页面内容划词复制大面积失效：
/// Pressed 时物理 Alt 仍按着，物理 Alt keydown 激活了 Chrome 菜单栏，而 enigo
/// 合成的 Alt keyup 无法取消菜单栏，Ctrl+C 落在菜单栏上不复制页面 selection
/// （系统 Edit 控件不受影响，故输入框划词仍正常）。Released 触发时物理 Alt 已
/// 松开，菜单栏已被物理 keyup 取消，问题消失。不要为追求「按下即响应」改回 Pressed。
fn should_handle_shortcut_state(state: ShortcutState) -> bool {
    state == ShortcutState::Released
}

pub fn handle_global_shortcut(
    app: &tauri::AppHandle,
    shortcut: &tauri_plugin_global_shortcut::Shortcut,
    event: tauri_plugin_global_shortcut::ShortcutEvent,
) {
    if !should_handle_shortcut_state(event.state) {
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
        Some(ShortcutAction::OcrRecognize) => {
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = crate::ui::ocr_window::open_ocr_window(&app_handle) {
                    log::warn!("打开文字识别窗口失败: {e}");
                }
                let state = app_handle.state::<AppState>().inner().clone();
                crate::ui::ocr_window::start_ocr_capture(app_handle, state).await;
            });
        }
        Some(ShortcutAction::OpenSettings) => {
            // 程序快捷键：仅在窗口聚焦期间注册，此处直接打开设置
            if let Err(error) = show_settings_window(app) {
                log::warn!("打开设置失败: {error}");
            }
        }
        None => {}
    }
}

fn handle_selection_translate(app: &tauri::AppHandle) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        thread::sleep(Duration::from_millis(40));

        let restore_clipboard = app_handle.state::<AppState>().config_store.get()
            .ok()
            .map(|config| config.restore_clipboard)
            .unwrap_or(true);

        let selected_text = match copy_selected_text(restore_clipboard) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::AppConfig;

    fn config_with(bindings: &[(&str, &str)]) -> AppConfig {
        let mut config = AppConfig::default();
        for (id, keys) in bindings {
            config
                .shortcuts
                .insert((*id).to_string(), (*keys).to_string());
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
    fn classifies_ocr_recognize_shortcut() {
        let config = config_with(&[("ocr-recognize", "Alt+O")]);
        let shortcut = "Alt+O".parse::<Shortcut>().unwrap();
        assert_eq!(
            classify_shortcut(&shortcut, &config),
            Some(ShortcutAction::OcrRecognize)
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
            ("translate-selection", "Ctrl+Alt+9"),
            ("word-lookup", "Ctrl+Alt+9"),
        ]);

        let error = configured_shortcuts(&config).expect_err("重复快捷键应失败");

        // ponytail: HashMap 迭代顺序不保证，只验证报告冲突
        assert!(["translate-selection", "word-lookup"].contains(&error.id.as_str()));
        assert!(
            error.message.contains("划词翻译") || error.message.contains("取词翻译"),
            "expected mention of conflicting binding, got: {}",
            error.message
        );
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
        assert_eq!(word_lookup.kind, ShortcutKind::Unimplemented);
    }

    #[test]
    fn open_settings_is_app_local_not_global() {
        let config = config_with(&[("open-settings", "Ctrl+,")]);
        let entries = configured_shortcuts(&config).expect("配置应可解析");
        let open = entries
            .iter()
            .find(|entry| entry.id == "open-settings")
            .expect("应包含打开设置");

        assert_eq!(open.kind, ShortcutKind::AppLocal);
        assert_eq!(open.action, Some(ShortcutAction::OpenSettings));
        assert_eq!(
            classify_shortcut(&open.shortcut, &config),
            Some(ShortcutAction::OpenSettings)
        );
    }

    #[test]
    fn handles_released_shortcut_events_only() {
        assert!(should_handle_shortcut_state(ShortcutState::Released));
        assert!(!should_handle_shortcut_state(ShortcutState::Pressed));
    }

    #[test]
    fn friendly_register_error_classifies_already_registered() {
        assert_eq!(
            friendly_register_error("HotKey already registered: HotKey { ... }"),
            "快捷键已被占用"
        );
    }

    #[test]
    fn friendly_register_error_falls_back_for_unknown_errors() {
        assert_eq!(
            friendly_register_error("其他注册错误"),
            "快捷键注册失败"
        );
    }
}
