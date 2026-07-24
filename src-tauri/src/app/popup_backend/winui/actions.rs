//! 原生弹窗用户动作 → 现有 core / commands 同等路径。
//!
//! **禁止**在此复制翻译协议、批次构建或 provider 调用；只调度 `AppState` 与
//! `start_translation_from_input` / `show_settings` 等已有入口。
//!
//! `AppHandle` 保存在本模块（而非 `ui`），供 `wnd_proc` 经函数指针回调使用。

use std::sync::Mutex;

use tauri::{AppHandle, Manager};

use crate::app::popup_backend::types::PopupUserAction;
use crate::app::popup_backend::with_host;
use crate::app::state::AppState;
use crate::app::window::request_show_settings_window;
use crate::core::selection::write_clipboard_text;
use crate::ui::web_popup::start_translation_from_input;

static BOUND_APP: Mutex<Option<AppHandle>> = Mutex::new(None);

/// 后端 ensure 时绑定。
pub fn bind_app(app: AppHandle) {
    if let Ok(mut g) = BOUND_APP.lock() {
        *g = Some(app);
    }
}

fn try_app() -> Option<AppHandle> {
    BOUND_APP.lock().ok().and_then(|g| g.clone())
}

/// UI 回调入口（无 `AppHandle` 参数；从本模块静态取）。
pub fn handle_user_action(action: PopupUserAction) {
    let Some(app) = try_app() else {
        log::warn!("原生弹窗未绑定 AppHandle，忽略动作: {action:?}");
        return;
    };
    handle_user_action_with(&app, action);
}

/// 处理 [`PopupUserAction`]。best-effort：失败只打日志，不向上抛。
///
/// 重试 / 换语言重译在独立线程执行，避免 UI 线程被
/// `start_translation_from_input` 内 120ms sleep 卡住。
pub fn handle_user_action_with(app: &AppHandle, action: PopupUserAction) {
    match action {
        PopupUserAction::Close => {
            if let Err(e) = with_host(app, |host| host.hide()) {
                log::warn!("原生弹窗关闭(hide)失败: {e}");
            }
        }
        PopupUserAction::CancelTranslation => {
            let state = app.state::<AppState>();
            if let Err(e) = state.cancel_current_translation() {
                log::warn!("取消翻译失败: {e}");
            }
        }
        PopupUserAction::Retry {
            service_instance_id: _,
        } => {
            // 当前 web 路径为整批重试；单服务 id 预留，暂忽略。
            let app = app.clone();
            std::thread::spawn(move || {
                if let Err(e) = retry_translation_sync(&app) {
                    log::warn!("重试翻译失败: {e}");
                }
            });
        }
        PopupUserAction::CopyResult {
            service_instance_id,
        } => {
            if let Err(e) = copy_card_text(&service_instance_id) {
                log::warn!("复制译文失败: {e}");
            }
        }
        PopupUserAction::OpenSettings => {
            // 与托盘/快捷键一致：独立线程建窗，避免 Win32 UI 回调死锁。
            request_show_settings_window(app);
        }
        PopupUserAction::SetSessionLanguages {
            source_lang,
            target_lang,
        } => {
            let state = app.state::<AppState>();
            if let Err(e) = state.set_session_languages(source_lang, target_lang) {
                log::warn!("设置会话语言失败: {e}");
                return;
            }
            // 有可重试输入时自动重译（与弹窗换语言体验一致）。
            let app = app.clone();
            std::thread::spawn(move || {
                if let Err(e) = retry_translation_sync(&app) {
                    // 无可重试输入时属正常（仅改语言、尚无会话）
                    log::debug!("换语言后未重译: {e}");
                }
            });
        }
    }
}

/// 与 `retry_translation` command 同等同步路径。
pub fn retry_translation_sync(app: &AppHandle) -> Result<String, String> {
    let state = app.state::<AppState>();
    let input = state
        .take_last_translation_input()?
        .ok_or_else(|| "没有可重试的翻译".to_string())?;
    start_translation_from_input(input, app.clone(), state.inner())
}

fn copy_card_text(service_instance_id: &str) -> Result<(), String> {
    let snap = super::reactor::state::global_snapshot();
    let text = super::reactor::state::resolve_copy_text(&snap, service_instance_id)
        .ok_or_else(|| "没有可复制的译文".to_string())?;
    write_clipboard_text(&text).map_err(|e| e.to_string())
}

/// 注册 UI 动作分发（函数指针，避免 UI 模块编译期反向依赖本模块业务路径）。
///
/// - GDI `ui.rs`：继续 `set_action_handler`（遗留，任务 11 删除）
/// - 路径 R `reactor::view`：注册同一 [`handle_user_action`]（view 经静态
///   指针分发，避免 `view → actions → host → view` 环）
///
/// 复制统一只读 `reactor::state` 全局快照。
pub fn install_action_handler() {
    super::ui::set_action_handler(handle_user_action);
    super::reactor::view::set_user_action_handler(handle_user_action);
}

#[cfg(test)]
mod tests {
    use crate::app::popup_backend::types::{PopupCardStatus, PopupCardVm, PopupViewModel};
    use crate::app::popup_backend::winui::reactor::state::{
        first_copyable_service_id, resolve_copy_text,
    };

    fn card(
        id: &str,
        name: &str,
        protocol: &str,
        model: &str,
        status: PopupCardStatus,
        text: &str,
        error: &str,
    ) -> PopupCardVm {
        PopupCardVm {
            service_instance_id: id.into(),
            service_name: name.into(),
            service_type: "llm".into(),
            protocol: protocol.into(),
            model_name: model.into(),
            status,
            text: text.into(),
            error_message: error.into(),
            usage_input: None,
            usage_output: None,
            detected_source_lang: None,
        }
    }

    fn sample_vm() -> PopupViewModel {
        PopupViewModel {
            source_text: "hi".into(),
            source_lang: "en".into(),
            target_lang: "zh-CN".into(),
            is_translating: false,
            cards: vec![
                card(
                    "a",
                    "A",
                    "mock",
                    "m",
                    PopupCardStatus::Finished,
                    "",
                    "",
                ),
                card(
                    "b",
                    "B",
                    "openai_chat",
                    "gpt",
                    PopupCardStatus::Finished,
                    "你好",
                    "",
                ),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn resolve_copy_prefers_card_text() {
        let snap = sample_vm();
        assert_eq!(resolve_copy_text(&snap, "b").as_deref(), Some("你好"));
        assert_eq!(resolve_copy_text(&snap, "a"), None);
        assert_eq!(resolve_copy_text(&snap, "missing"), None);
    }

    #[test]
    fn first_copyable_skips_empty() {
        let snap = sample_vm();
        assert_eq!(first_copyable_service_id(&snap).as_deref(), Some("b"));
    }

    #[test]
    fn resolve_copy_falls_back_to_error_message() {
        let snap = PopupViewModel {
            source_lang: "auto".into(),
            target_lang: "zh-CN".into(),
            cards: vec![card(
                "e",
                "E",
                "mock",
                "m",
                PopupCardStatus::Failed,
                "",
                "超时",
            )],
            ..Default::default()
        };
        assert_eq!(resolve_copy_text(&snap, "e").as_deref(), Some("超时"));
    }
}
