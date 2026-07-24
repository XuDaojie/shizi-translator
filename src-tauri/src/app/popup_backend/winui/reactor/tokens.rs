//! 路径 R 浅色 Fluent token（对齐 Open Design winui3.css / 打磨 spec）。

#![cfg(all(windows, feature = "popup-winui"))]

use windows_reactor::Color;

/// 与 GDI / 原型对齐的逻辑宽度。
pub const POPUP_VIEW_WIDTH: f64 = 468.0;

/// 结果区 `scroll_viewer` 最大高度（逻辑 px）。
pub const RESULTS_SCROLL_MAX_HEIGHT: f64 = 360.0;

/// body 内边距 / 区间距。
pub const BODY_PADDING: f64 = 14.0;
pub const BODY_GAP: f64 = 10.0;

/// 卡片圆角。
pub const RADIUS_CARD: f64 = 8.0;

/// 品牌 accent：柿子橙 `#D55A1F`。
pub const ACCENT_PERSIMMON: Color = Color::rgb(0xD5, 0x5A, 0x1F);
pub const ACCENT_ON_PERSIMMON: Color = Color::rgb(0xFF, 0xFF, 0xFF);

/// 前景层级。
pub const FG_PRIMARY: Color = Color::rgb(0x1A, 0x1A, 0x1A);
pub const FG_SECONDARY: Color = Color::rgb(0x5D, 0x5D, 0x5D);
pub const FG_TERTIARY: Color = Color::rgb(0x8A, 0x8A, 0x8A);

/// 卡片表面（实色近似半透白）。
pub const BG_CARD: Color = Color::rgb(0xFF, 0xFF, 0xFF);
/// 边框 ≈ rgba(0,0,0,0.06) 的实色近似。
pub const BORDER_SUBTLE: Color = Color::rgb(0xF0, 0xF0, 0xF0);

/// 状态色。
pub const COLOR_SUCCESS: Color = Color::rgb(0x10, 0x7C, 0x10);
pub const COLOR_WARNING: Color = Color::rgb(0xCA, 0x50, 0x10);
pub const COLOR_DANGER: Color = Color::rgb(0xC4, 0x2B, 0x1C);

/// 字号。
pub const FONT_TITLE: f64 = 13.0;
pub const FONT_BODY: f64 = 14.0;
pub const FONT_CAPTION: f64 = 11.0;
pub const FONT_META: f64 = 12.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_match_open_design_persimmon_and_width() {
        assert!((POPUP_VIEW_WIDTH - 468.0).abs() < f64::EPSILON);
        assert_eq!(ACCENT_PERSIMMON, Color::rgb(0xD5, 0x5A, 0x1F));
        assert_eq!(FG_PRIMARY, Color::rgb(0x1A, 0x1A, 0x1A));
        assert!((RADIUS_CARD - 8.0).abs() < f64::EPSILON);
        assert!((RESULTS_SCROLL_MAX_HEIGHT - 360.0).abs() < f64::EPSILON);
    }
}
