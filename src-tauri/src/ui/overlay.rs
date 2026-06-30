use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

use crate::{
    app::state::AppState,
    core::ocr::OcrHints,
    platform::recognize_region,
    ui::web_popup::{show_translation_error, start_translation_from_input},
};

pub const OVERLAY_LABEL: &str = "screenshot-overlay";

/// 在光标所在显示器上铺满建 overlay 窗口。整屏帧须已存入 AppState。
pub fn open_overlay(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = existing.close();
    }
    let window = WebviewWindowBuilder::new(app, OVERLAY_LABEL, WebviewUrl::App("overlay.html".into()))
        .title("Shizi 截图")
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .fullscreen(true)
        .build()
        .map_err(|e| e.to_string())?;
    let _ = window.set_focus();
    Ok(())
}

fn close_overlay(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = window.close();
    }
}

#[tauri::command]
pub async fn get_capture_frame_meta(
    state: tauri::State<'_, AppState>,
) -> Result<Option<(u32, u32, f64)>, String> {
    state.pending_capture_meta()
}

#[tauri::command]
pub async fn get_capture_frame_bytes(
    state: tauri::State<'_, AppState>,
) -> Result<tauri::ipc::Response, String> {
    let bytes = state.pending_capture_bytes()?.unwrap_or_default();
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
pub async fn cancel_capture(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let _ = state.take_pending_capture();
    close_overlay(&app);
    Ok(())
}

/// 前端回传 CSS 逻辑像素矩形（相对 overlay 左上）。
#[tauri::command]
pub async fn submit_capture_region(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    use crate::core::capture::css_rect_to_physical;

    close_overlay(&app);

    let Some((frame, scale)) = state.take_pending_capture()? else {
        return Ok(()); // 帧已被取消/消费，静默
    };
    let region = css_rect_to_physical(x, y, w, h, scale);
    if region.2 == 0 || region.3 == 0 {
        return Ok(()); // 选区过小，静默
    }

    let app_state = state.inner().clone();
    match recognize_region(&frame, region, OcrHints::default()).await {
        Ok(None) => {}
        Ok(Some(input)) => {
            if let Err(error) = start_translation_from_input(input, app.clone(), &app_state) {
                show_translation_error(&app, error);
            }
        }
        Err(error) => show_translation_error(&app, crate::ui::ocr_popup::friendly_ocr_error(error)),
    }
    Ok(())
}
