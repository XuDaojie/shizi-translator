//! 源文卡：卡片表面 + 只读可选中正文。

#![cfg(all(windows, feature = "popup-winui"))]

use windows_reactor::{
    border, caption, text_block, vstack, Element, ElementExt, Thickness,
};

use super::tokens::{
    BG_CARD, BORDER_SUBTLE, FG_PRIMARY, FG_TERTIARY, FONT_BODY, FONT_CAPTION, RADIUS_CARD,
};
use crate::app::popup_backend::types::PopupViewModel;

pub fn source_card(vm: &PopupViewModel) -> Element {
    let body = if vm.source_text.trim().is_empty() {
        text_block("（等待源文）")
            .font_size(FONT_BODY)
            .foreground(FG_TERTIARY)
            .wrap()
            .selectable()
    } else {
        text_block(vm.source_text.clone())
            .font_size(FONT_BODY)
            .foreground(FG_PRIMARY)
            .wrap()
            .selectable()
    };

    border(
        vstack((
            caption("源文".to_string()).foreground(FG_TERTIARY).font_size(FONT_CAPTION),
            body,
        ))
        .spacing(4.0)
        .padding(Thickness::uniform(12.0)),
    )
    .corner_radius(RADIUS_CARD)
    .background(BG_CARD)
    .border_brush(BORDER_SUBTLE)
    .border_thickness(Thickness::uniform(1.0))
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_card_empty_and_filled() {
        let empty = PopupViewModel::default();
        let _ = source_card(&empty);
        let filled = PopupViewModel {
            source_text: "hello".into(),
            ..Default::default()
        };
        let _ = source_card(&filled);
    }
}
