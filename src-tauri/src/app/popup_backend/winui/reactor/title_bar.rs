//! 标题栏：品牌 + 钉 / 截图 / 设置 / 最小化 / 关闭（图标按钮）。

#![cfg(all(windows, feature = "popup-winui"))]

use windows_reactor::{
    border, button, hstack, text_block, Element, ElementExt, Symbol, Thickness,
};

use super::dispatch::dispatch_user_action;
use super::state;
use super::tokens::{
    ACCENT_ON_PERSIMMON, ACCENT_PERSIMMON, FG_PRIMARY, FONT_TITLE, RADIUS_CARD,
};
use crate::app::popup_backend::types::PopupUserAction;

fn icon_btn(symbol: Symbol, label: &str) -> windows_reactor::Button {
    button("")
        .icon(symbol)
        .subtle()
        .automation_name(label)
}

/// 标题栏（~44px 视觉高度由内容与 padding 近似）。
pub fn title_bar() -> Element {
    let pin_btn = {
        let mut b = icon_btn(Symbol::Pin, if state::is_pinned() { "取消置顶" } else { "置顶" });
        if state::is_pinned() {
            b = b.accent();
        }
        b.on_click(|| {
            dispatch_user_action(PopupUserAction::TogglePin);
        })
    };

    let brand = hstack((
        border(text_block("文").font_size(12.0).semibold().foreground(ACCENT_ON_PERSIMMON))
            .corner_radius(6.0)
            .background(ACCENT_PERSIMMON)
            .padding(Thickness::xy(6.0, 2.0)),
        text_block("shizi")
            .font_size(FONT_TITLE)
            .semibold()
            .foreground(FG_PRIMARY),
        pin_btn,
    ))
    .spacing(8.0);

    let actions = hstack((
        icon_btn(Symbol::Camera, "截图翻译").on_click(|| {
            dispatch_user_action(PopupUserAction::TriggerOcr);
        }),
        icon_btn(Symbol::Setting, "设置").on_click(|| {
            dispatch_user_action(PopupUserAction::OpenSettings);
        }),
        // 最小化：与关闭同为 Close（宿主 hide）
        icon_btn(Symbol::Remove, "最小化").on_click(|| {
            dispatch_user_action(PopupUserAction::Close);
        }),
        icon_btn(Symbol::Cancel, "关闭").on_click(|| {
            dispatch_user_action(PopupUserAction::Close);
        }),
    ))
    .spacing(2.0);

    // 简易左右分布：左侧品牌+钉，右侧动作（无完美 flex spacer 时靠间距）
    hstack((brand, actions))
        .spacing(12.0)
        .padding(Thickness::xy(10.0, 6.0))
        .into()
}

/// 品牌标圆角常量（测试可见）。
#[cfg(test)]
pub fn brand_corner() -> f64 {
    RADIUS_CARD - 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_bar_renders() {
        let _ = title_bar();
    }
}
