//! 路径 R 弹窗 UI：标题栏 + 源文 + 单卡正文 + 复制 + 状态栏。
//!
//! 状态由 host 的 `use_async_state` 驱动；本模块只渲染。
//! 动作经本模块静态 handler 分发（由 `actions::install_action_handler` 注册为
//! `handle_user_action`），避免 `view → actions → host → view` 编译期环导致
//! 测试二进制加载失败（STATUS_ENTRYPOINT_NOT_FOUND）。

#![cfg(all(windows, feature = "popup-winui"))]

use std::sync::Mutex;

use windows_reactor::{button, caption, hstack, text_block, vstack, Element, ElementExt};

use crate::app::popup_backend::types::{
    PopupCardStatus, PopupCardVm, PopupUserAction, PopupViewModel,
};

/// 与 GDI / 原型对齐的逻辑宽度。
pub const POPUP_VIEW_WIDTH: f64 = 468.0;

type UserActionHandler = fn(PopupUserAction);

static USER_ACTION_HANDLER: Mutex<Option<UserActionHandler>> = Mutex::new(None);

/// 由 `actions::install_action_handler` 注册（典型为 `handle_user_action`）。
pub fn set_user_action_handler(handler: UserActionHandler) {
    if let Ok(mut g) = USER_ACTION_HANDLER.lock() {
        *g = Some(handler);
    }
}

fn dispatch_user_action(action: PopupUserAction) {
    let handler = USER_ACTION_HANDLER.lock().ok().and_then(|g| *g);
    if let Some(h) = handler {
        h(action);
    } else {
        log::warn!("Reactor 弹窗未注册动作处理器，忽略: {action:?}");
    }
}

/// 底部状态栏文案：`is_translating` → 翻译中；源文空 → 就绪；否则完成。
fn footer_status_label(is_translating: bool, source_text: &str) -> &'static str {
    if is_translating {
        "翻译中…"
    } else if source_text.trim().is_empty() {
        "就绪"
    } else {
        "完成"
    }
}

/// 源文字数（Unicode 标量，`chars().count()`）。
fn source_char_count(source_text: &str) -> usize {
    source_text.chars().count()
}

fn title_bar() -> Element {
    hstack((
        text_block("柿子翻译").font_size(14.0).semibold(),
        button("钉").on_click(|| {
            log::debug!("Reactor 标题栏：钉（stub）");
        }),
        button("收藏").on_click(|| {
            log::debug!("Reactor 标题栏：收藏（stub）");
        }),
        button("截图").on_click(|| {
            log::debug!("Reactor 标题栏：截图（stub）");
        }),
        button("书签").on_click(|| {
            log::debug!("Reactor 标题栏：书签（stub）");
        }),
        button("设置").on_click(|| {
            dispatch_user_action(PopupUserAction::OpenSettings);
        }),
        // 最小化：路径 R 无独立 hide 动作，与关闭同为 Close（宿主 hide 弹窗）
        button("最小化").on_click(|| {
            dispatch_user_action(PopupUserAction::Close);
        }),
        button("关闭").on_click(|| {
            dispatch_user_action(PopupUserAction::Close);
        }),
    ))
    .spacing(6.0)
    .into()
}

fn status_bar(vm: &PopupViewModel) -> Element {
    let status = footer_status_label(vm.is_translating, &vm.source_text);
    let count = source_char_count(&vm.source_text);
    hstack((
        caption(status.to_string()),
        caption(format!("{count} 字")),
    ))
    .spacing(8.0)
    .into()
}

/// 渲染翻译弹窗（标题栏 + 源文 + 首卡 + 状态栏；多卡见任务 9）。
pub fn render_popup(vm: &PopupViewModel) -> Element {
    let source = if vm.source_text.is_empty() {
        String::from("（无源文）")
    } else {
        vm.source_text.clone()
    };

    let card = vm.cards.first();
    let body = card
        .map(card_body_text)
        .unwrap_or_else(|| String::from("（等待结果）"));
    let service_line = card
        .map(|c| {
            if c.service_name.is_empty() {
                c.model_name.clone()
            } else if c.model_name.is_empty() {
                c.service_name.clone()
            } else {
                format!("{} · {}", c.service_name, c.model_name)
            }
        })
        .unwrap_or_default();
    let sid = card
        .map(|c| c.service_instance_id.clone())
        .unwrap_or_default();

    vstack((
        title_bar(),
        // 源文：只读展示 + 可选中复制（selectable text_block）
        text_block(source).font_size(16.0).wrap().selectable(),
        text_block(service_line).font_size(12.0),
        text_block(body).font_size(14.0).wrap().selectable(),
        button("复制").on_click(move || {
            if sid.is_empty() {
                log::debug!("复制：无服务实例 id，忽略");
                return;
            }
            dispatch_user_action(PopupUserAction::CopyResult {
                service_instance_id: sid.clone(),
            });
        }),
        status_bar(vm),
    ))
    .spacing(12.0)
    .width(POPUP_VIEW_WIDTH)
    .padding(12.0)
    .into()
}

fn card_body_text(card: &PopupCardVm) -> String {
    let t = card.text.trim();
    if !t.is_empty() {
        return t.to_string();
    }
    let e = card.error_message.trim();
    if !e.is_empty() {
        return e.to_string();
    }
    match card.status {
        PopupCardStatus::Pending => "等待…".into(),
        PopupCardStatus::Translating => "翻译中…".into(),
        PopupCardStatus::Finished => String::new(),
        PopupCardStatus::Failed => "失败".into(),
        PopupCardStatus::Cancelled => "已取消".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(status: PopupCardStatus, text: &str, err: &str) -> PopupCardVm {
        PopupCardVm {
            service_instance_id: "s1".into(),
            service_name: "Mock".into(),
            service_type: "llm".into(),
            protocol: "mock".into(),
            model_name: "m".into(),
            status,
            text: text.into(),
            error_message: err.into(),
            usage_input: None,
            usage_output: None,
            detected_source_lang: None,
        }
    }

    #[test]
    fn view_card_body_prefers_text_then_error() {
        assert_eq!(
            card_body_text(&card(PopupCardStatus::Finished, "hello", "")),
            "hello"
        );
        assert_eq!(
            card_body_text(&card(PopupCardStatus::Failed, "", "timeout")),
            "timeout"
        );
        assert_eq!(
            card_body_text(&card(PopupCardStatus::Pending, "", "")),
            "等待…"
        );
    }

    #[test]
    fn view_footer_status_label_by_state() {
        assert_eq!(footer_status_label(true, ""), "翻译中…");
        assert_eq!(footer_status_label(true, "hello"), "翻译中…");
        assert_eq!(footer_status_label(false, ""), "就绪");
        assert_eq!(footer_status_label(false, "   "), "就绪");
        assert_eq!(footer_status_label(false, "hello"), "完成");
    }

    #[test]
    fn view_source_char_count_uses_unicode_scalars() {
        assert_eq!(source_char_count(""), 0);
        assert_eq!(source_char_count("hello"), 5);
        assert_eq!(source_char_count("柿子"), 2);
        assert_eq!(source_char_count("a😀b"), 3);
    }

    #[test]
    fn view_render_popup_returns_element() {
        let vm = PopupViewModel {
            source_text: "hello".into(),
            cards: vec![card(PopupCardStatus::Finished, "world", "")],
            ..Default::default()
        };
        let _el = render_popup(&vm);
    }

    #[test]
    fn view_popup_width_is_468() {
        assert!((POPUP_VIEW_WIDTH - 468.0).abs() < f64::EPSILON);
    }
}
