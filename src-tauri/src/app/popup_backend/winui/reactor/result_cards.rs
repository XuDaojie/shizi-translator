//! 多服务结果卡列表。

#![cfg(all(windows, feature = "popup-winui"))]

use windows_reactor::{
    border, button, caption, hstack, scroll_viewer, text_block, vstack, Element, ElementExt,
    Thickness,
};

use super::dispatch::dispatch_user_action;
use super::meta::{display_model_name, should_show_tokens};
use super::tokens::{
    BG_CARD, BORDER_SUBTLE, COLOR_DANGER, COLOR_WARNING, FG_PRIMARY, FG_SECONDARY, FG_TERTIARY,
    FONT_BODY, FONT_META, FONT_TITLE, RADIUS_CARD, RESULTS_SCROLL_MAX_HEIGHT,
};
use crate::app::popup_backend::types::{
    PopupCardStatus, PopupCardVm, PopupUserAction, PopupViewModel,
};

pub fn card_status_label(status: &PopupCardStatus) -> &'static str {
    match status {
        PopupCardStatus::Pending => "等待中",
        PopupCardStatus::Translating => "翻译中",
        PopupCardStatus::Finished => "",
        PopupCardStatus::Failed => "失败",
        PopupCardStatus::Cancelled => "已取消",
    }
}

fn status_color(status: &PopupCardStatus) -> windows_reactor::Color {
    match status {
        PopupCardStatus::Failed => COLOR_DANGER,
        PopupCardStatus::Cancelled | PopupCardStatus::Translating => COLOR_WARNING,
        PopupCardStatus::Pending => FG_TERTIARY,
        PopupCardStatus::Finished => FG_SECONDARY,
    }
}

pub fn card_tokens_label(card: &PopupCardVm) -> String {
    let has_usage = card.usage_input.is_some() || card.usage_output.is_some();
    if !should_show_tokens(&card.protocol, has_usage) {
        return String::new();
    }
    match (card.usage_input, card.usage_output) {
        (Some(i), Some(o)) => format!("↑{i} ↓{o}"),
        (Some(i), None) => format!("↑{i}"),
        (None, Some(o)) => format!("↓{o}"),
        _ => String::new(),
    }
}

pub fn card_body_text(card: &PopupCardVm) -> String {
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

fn result_card(card: &PopupCardVm) -> Element {
    let status_label = card_status_label(&card.status);
    let model = display_model_name(&card.protocol, &card.model_name);
    let tokens = card_tokens_label(card);
    let body = card_body_text(card);
    let sid = card.service_instance_id.clone();
    let name = if card.service_name.is_empty() {
        "服务".to_string()
    } else {
        card.service_name.clone()
    };

    let header = hstack((
        text_block(name)
            .font_size(FONT_TITLE)
            .semibold()
            .foreground(FG_PRIMARY),
        caption(status_label.to_string())
            .foreground(status_color(&card.status))
            .font_size(FONT_META),
        button("复制")
            .subtle()
            .on_click(move || {
                if sid.is_empty() {
                    log::debug!("复制：无服务实例 id，忽略");
                    return;
                }
                dispatch_user_action(PopupUserAction::CopyResult {
                    service_instance_id: sid.clone(),
                });
            }),
    ))
    .spacing(8.0);

    let meta_parts: Vec<String> = [model, tokens]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    let meta_line = if meta_parts.is_empty() {
        caption(String::new())
    } else {
        caption(meta_parts.join(" · ")).foreground(FG_TERTIARY).font_size(FONT_META)
    };

    border(
        vstack((
            header,
            text_block(body)
                .font_size(FONT_BODY)
                .foreground(FG_PRIMARY)
                .wrap()
                .selectable(),
            meta_line,
        ))
        .spacing(6.0)
        .padding(Thickness::uniform(12.0)),
    )
    .corner_radius(RADIUS_CARD)
    .background(BG_CARD)
    .border_brush(BORDER_SUBTLE)
    .border_thickness(Thickness::uniform(1.0))
    .into()
}

pub fn results_list(vm: &PopupViewModel) -> Element {
    let cards: Vec<Element> = if vm.cards.is_empty() {
        vec![text_block("（等待结果）")
            .font_size(FONT_BODY)
            .foreground(FG_TERTIARY)
            .into()]
    } else {
        vm.cards.iter().map(result_card).collect()
    };
    scroll_viewer(vstack(cards).spacing(BODY_GAP_LOCAL))
        .max_height(RESULTS_SCROLL_MAX_HEIGHT)
        .into()
}

const BODY_GAP_LOCAL: f64 = 10.0;

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
    fn card_body_prefers_text_then_error() {
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
    fn card_status_and_tokens() {
        assert_eq!(card_status_label(&PopupCardStatus::Pending), "等待中");
        assert_eq!(card_status_label(&PopupCardStatus::Finished), "");
        let mut llm = card(PopupCardStatus::Finished, "ok", "");
        llm.usage_input = Some(1);
        llm.usage_output = Some(2);
        assert_eq!(card_tokens_label(&llm), "↑1 ↓2");
        let mut mt = card(PopupCardStatus::Finished, "ok", "");
        mt.protocol = "microsoft_edge".into();
        mt.usage_input = Some(1);
        assert_eq!(card_tokens_label(&mt), "");
    }

    #[test]
    fn results_list_renders() {
        let vm = PopupViewModel {
            cards: vec![card(PopupCardStatus::Finished, "hi", "")],
            ..Default::default()
        };
        let _ = results_list(&vm);
    }
}
