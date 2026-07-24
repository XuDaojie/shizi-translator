//! 路径 R 最小弹窗 UI：标题/关闭 + 源文 + 单卡正文 + 复制。
//!
//! 状态由 host 的 `use_async_state` 驱动；本模块只渲染。
//! 动作经本模块静态 handler 分发（由 `actions::install_action_handler` 注册为
//! `handle_user_action`），避免 `view → actions → host → view` 编译期环导致
//! 测试二进制加载失败（STATUS_ENTRYPOINT_NOT_FOUND）。

#![cfg(all(windows, feature = "popup-winui"))]

use std::sync::Mutex;

use windows_reactor::{button, hstack, text_block, vstack, Element, ElementExt};

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

/// 渲染最小翻译弹窗（M1 契约：源文 + 首卡 + 关闭/复制）。
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
    let status_line = card
        .map(|c| status_label(&c.status, vm.is_translating))
        .unwrap_or_else(|| {
            if vm.is_translating {
                "翻译中…".to_string()
            } else {
                String::new()
            }
        });
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
        hstack((
            text_block("柿子翻译").font_size(14.0).semibold(),
            button("关闭").on_click(|| {
                dispatch_user_action(PopupUserAction::Close);
            }),
        ))
        .spacing(8.0),
        text_block(source).font_size(16.0).wrap().selectable(),
        text_block(service_line).font_size(12.0),
        text_block(status_line).font_size(12.0),
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

fn status_label(status: &PopupCardStatus, is_translating: bool) -> String {
    let base = match status {
        PopupCardStatus::Pending => "等待",
        PopupCardStatus::Translating => "翻译中",
        PopupCardStatus::Finished => "完成",
        PopupCardStatus::Failed => "失败",
        PopupCardStatus::Cancelled => "已取消",
    };
    if is_translating && !matches!(status, PopupCardStatus::Translating) {
        format!("{base} · 批次进行中")
    } else {
        base.to_string()
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
    fn view_status_label_reflects_card() {
        assert_eq!(status_label(&PopupCardStatus::Finished, false), "完成");
        assert!(status_label(&PopupCardStatus::Finished, true).contains("批次"));
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
}
