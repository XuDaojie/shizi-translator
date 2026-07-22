//! 按显示器 DPI 选择专用位图，避免 Windows 用大图缩小导致标题栏/托盘图标发糊。

use tauri::{image::Image, AppHandle};

/// 标题栏 / 托盘小图标的逻辑基准边长（Windows `SM_CXSMICON` 习惯为 16）。
const SMALL_ICON_LOGICAL_PX: f64 = 16.0;

/// 将 `logical * scale` 映射到已预渲染的物理像素边长。
pub fn small_icon_physical_size(scale_factor: f64) -> u32 {
    match (SMALL_ICON_LOGICAL_PX * scale_factor).round() as u32 {
        0..=16 => 16,
        17..=20 => 20,
        21..=24 => 24,
        25..=28 => 28,
        29..=32 => 32,
        33..=36 => 36,
        37..=40 => 40,
        _ => 48,
    }
}

/// 按物理边长取预渲染 PNG（与托盘同源，保证标题栏与托盘清晰度一致）。
pub fn app_icon_image_for_size(size: u32) -> tauri::Result<Image<'static>> {
    let bytes: &[u8] = match size {
        16 => include_bytes!("../../icons/tray-icon-16.png"),
        20 => include_bytes!("../../icons/tray-icon-20.png"),
        24 => include_bytes!("../../icons/tray-icon-24.png"),
        28 => include_bytes!("../../icons/tray-icon-28.png"),
        32 => include_bytes!("../../icons/tray-icon-32.png"),
        36 => include_bytes!("../../icons/tray-icon-36.png"),
        40 => include_bytes!("../../icons/tray-icon-40.png"),
        _ => include_bytes!("../../icons/tray-icon-48.png"),
    };
    Image::from_bytes(bytes)
}

pub fn app_icon_image_for_scale(scale_factor: f64) -> tauri::Result<Image<'static>> {
    app_icon_image_for_size(small_icon_physical_size(scale_factor))
}

/// 用主显示器 DPI 选图；无显示器时回退 1.0。
pub fn app_icon_image(app: &AppHandle) -> tauri::Result<Image<'static>> {
    let scale_factor = app
        .primary_monitor()?
        .map(|monitor| monitor.scale_factor())
        .unwrap_or(1.0);
    app_icon_image_for_scale(scale_factor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_icon_size_matches_common_windows_scale_factors() {
        let cases = [
            (1.0, 16),
            (1.25, 20),
            (1.5, 24),
            (1.75, 28),
            (2.0, 32),
            (2.25, 36),
            (2.5, 40),
            (3.0, 48),
        ];
        for (scale_factor, expected_size) in cases {
            assert_eq!(small_icon_physical_size(scale_factor), expected_size);
        }
    }

    #[test]
    fn selected_icons_match_their_physical_size() {
        for scale_factor in [1.0, 1.25, 1.5, 2.0, 3.0] {
            let expected_size = small_icon_physical_size(scale_factor);
            let icon =
                app_icon_image_for_scale(scale_factor).expect("对应 DPI 的专用图标应可解码");
            assert_eq!(
                (icon.width(), icon.height()),
                (expected_size, expected_size),
                "scale_factor={scale_factor}"
            );
        }
    }
}
