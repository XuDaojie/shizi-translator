//! 路径 R 弹窗 UI 总装：五区布局。
//!
//! 状态由 host 的 `use_async_state` 驱动；本模块只渲染。
//! 动作经 `dispatch` 静态 handler（由 `actions::install_action_handler` 注册）。

#![cfg(all(windows, feature = "popup-winui"))]

use windows_reactor::{vstack, Element, ElementExt, Thickness};

use super::language_bar::language_bar;
use super::result_cards::results_list;
use super::source_card::source_card;
use super::status_bar::status_bar;
use super::title_bar::title_bar;
use super::tokens::{BODY_GAP, BODY_PADDING};
use crate::app::popup_backend::types::PopupViewModel;

// 兼容既有 re-export / actions 注册路径
pub use super::dispatch::set_user_action_handler;
pub use super::tokens::POPUP_VIEW_WIDTH;

/// 渲染翻译弹窗（标题栏 + 源文卡 + 语言栏 + 结果卡 + 状态栏）。
pub fn render_popup(vm: &PopupViewModel) -> Element {
    vstack((
        title_bar(),
        source_card(vm),
        language_bar(vm),
        results_list(vm),
        status_bar(vm),
    ))
    .spacing(BODY_GAP)
    .width(POPUP_VIEW_WIDTH)
    .padding(Thickness::uniform(BODY_PADDING))
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::popup_backend::types::{PopupCardStatus, PopupCardVm};
    use super::super::result_cards::{card_body_text, card_status_label, card_tokens_label};
    use super::super::status_bar::{footer_status_label, source_char_count};
    use super::super::meta::display_model_name;
    use super::super::tokens::{ACCENT_ON_PERSIMMON, ACCENT_PERSIMMON, RESULTS_SCROLL_MAX_HEIGHT};
    use windows_reactor::Color;

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
    fn view_render_popup_multi_cards_returns_element() {
        let mut c1 = card(PopupCardStatus::Finished, "one", "");
        c1.service_instance_id = "s1".into();
        c1.service_name = "A".into();
        c1.usage_input = Some(10);
        c1.usage_output = Some(20);
        let mut c2 = card(PopupCardStatus::Failed, "", "timeout");
        c2.service_instance_id = "s2".into();
        c2.service_name = "B".into();
        c2.protocol = "microsoft_edge".into();
        let mut c3 = card(PopupCardStatus::Translating, "", "");
        c3.service_instance_id = "s3".into();
        c3.service_name = "C".into();
        let vm = PopupViewModel {
            source_text: "hello".into(),
            is_translating: true,
            cards: vec![c1, c2, c3],
            source_lang: "en".into(),
            target_lang: "zh-CN".into(),
            ..Default::default()
        };
        let _el = render_popup(&vm);
    }

    #[test]
    fn view_card_status_and_tokens_helpers() {
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
        assert_eq!(display_model_name("openai_chat", "gpt-4o"), "gpt-4o");
        assert_eq!(display_model_name("microsoft_edge", "x"), "");
    }

    #[test]
    fn view_popup_width_is_468() {
        assert!((POPUP_VIEW_WIDTH - 468.0).abs() < f64::EPSILON);
    }

    #[test]
    fn view_accent_is_persimmon_orange() {
        assert_eq!(ACCENT_PERSIMMON, Color::rgb(0xD5, 0x5A, 0x1F));
        assert_eq!(ACCENT_ON_PERSIMMON, Color::rgb(0xFF, 0xFF, 0xFF));
        assert!((RESULTS_SCROLL_MAX_HEIGHT - 360.0).abs() < f64::EPSILON);
    }
}
