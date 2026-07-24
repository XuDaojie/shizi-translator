//! 状态栏：状态点文案 + 条件取消/重试 + 字数。

#![cfg(all(windows, feature = "popup-winui"))]

use windows_reactor::{
    button, caption, hstack, text_block, Element, ElementExt, Thickness,
};

use super::dispatch::dispatch_user_action;
use super::tokens::{
    ACCENT_ON_PERSIMMON, ACCENT_PERSIMMON, COLOR_SUCCESS, FG_SECONDARY, FONT_CAPTION,
};
use crate::app::popup_backend::types::{PopupCardStatus, PopupUserAction, PopupViewModel};

/// 底部状态栏文案。
pub fn footer_status_label(is_translating: bool, source_text: &str) -> &'static str {
    if is_translating {
        "翻译中…"
    } else if source_text.trim().is_empty() {
        "就绪"
    } else {
        "完成"
    }
}

pub fn source_char_count(source_text: &str) -> usize {
    source_text.chars().count()
}

/// 是否显示重试：非翻译中且存在失败/取消卡。
pub fn should_show_retry(vm: &PopupViewModel) -> bool {
    if vm.is_translating {
        return false;
    }
    vm.cards.iter().any(|c| {
        matches!(
            c.status,
            PopupCardStatus::Failed | PopupCardStatus::Cancelled
        )
    })
}

pub fn status_bar(vm: &PopupViewModel) -> Element {
    let status = footer_status_label(vm.is_translating, &vm.source_text);
    let count = source_char_count(&vm.source_text);

    let status_dot = text_block("●")
        .font_size(8.0)
        .foreground(if vm.is_translating {
            ACCENT_PERSIMMON
        } else if status == "完成" {
            COLOR_SUCCESS
        } else {
            FG_SECONDARY
        });

    let mut row: Vec<Element> = vec![
        status_dot.into(),
        caption(status.to_string())
            .foreground(FG_SECONDARY)
            .font_size(FONT_CAPTION)
            .into(),
    ];

    if vm.is_translating {
        row.push(
            button("取消")
                .background(ACCENT_PERSIMMON)
                .foreground(ACCENT_ON_PERSIMMON)
                .on_click(|| {
                    dispatch_user_action(PopupUserAction::CancelTranslation);
                })
                .into(),
        );
    } else if should_show_retry(vm) {
        row.push(
            button("重试")
                .background(ACCENT_PERSIMMON)
                .foreground(ACCENT_ON_PERSIMMON)
                .on_click(|| {
                    dispatch_user_action(PopupUserAction::Retry {
                        service_instance_id: None,
                    });
                })
                .into(),
        );
    }

    row.push(
        caption(format!("{count} 字"))
            .foreground(FG_SECONDARY)
            .font_size(FONT_CAPTION)
            .into(),
    );

    hstack(row)
        .spacing(8.0)
        .padding(Thickness::xy(4.0, 4.0))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::popup_backend::types::PopupCardVm;

    #[test]
    fn footer_status_by_state() {
        assert_eq!(footer_status_label(true, ""), "翻译中…");
        assert_eq!(footer_status_label(true, "hello"), "翻译中…");
        assert_eq!(footer_status_label(false, ""), "就绪");
        assert_eq!(footer_status_label(false, "   "), "就绪");
        assert_eq!(footer_status_label(false, "hello"), "完成");
    }

    #[test]
    fn char_count_unicode() {
        assert_eq!(source_char_count(""), 0);
        assert_eq!(source_char_count("hello"), 5);
        assert_eq!(source_char_count("柿子"), 2);
        assert_eq!(source_char_count("a😀b"), 3);
    }

    #[test]
    fn retry_only_when_failed_or_cancelled() {
        let mut vm = PopupViewModel {
            is_translating: false,
            cards: vec![],
            ..Default::default()
        };
        assert!(!should_show_retry(&vm));
        vm.cards.push(PopupCardVm {
            service_instance_id: "s".into(),
            service_name: "A".into(),
            service_type: "llm".into(),
            protocol: "mock".into(),
            model_name: "m".into(),
            status: PopupCardStatus::Failed,
            text: String::new(),
            error_message: "e".into(),
            usage_input: None,
            usage_output: None,
            detected_source_lang: None,
        });
        assert!(should_show_retry(&vm));
        vm.is_translating = true;
        assert!(!should_show_retry(&vm));
    }
}
