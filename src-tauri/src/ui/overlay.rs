use tauri::{Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::{
    app::state::AppState,
    core::config::AppConfig,
    core::ocr::OcrHints,
    platform::recognize_region,
    ui::web_popup::{show_translation_error, start_translation_from_input},
};

pub const OVERLAY_LABEL: &str = "screenshot-overlay";

fn build_overlay(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    let window = WebviewWindowBuilder::new(app, OVERLAY_LABEL, WebviewUrl::App("overlay.html".into()))
        .title("Shizi 截图")
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .fullscreen(true)
        // 创建时不可见：WebView2 加载 HTML + canvas putImageData 期间会显示默认白底，
        // 由前端在内容就绪后 invoke('show_overlay') 让后端显示，消除占位闪烁。
        .visible(false)
        .build()
        .map_err(|e| e.to_string())?;
    // 兜底：overlay 被外部关闭或异常销毁时（非 submit/cancel 正常路径），
    // 释放 pending_capture 帧与 capture 锁，避免锁永久占用导致后续 Alt+O 被拒。
    let app_handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Destroyed = event {
            let state: tauri::State<'_, AppState> = app_handle.state();
            let _ = state.take_pending_capture();
            let _ = state.finish_capture();
        }
    });
    Ok(window)
}

/// 预创建模式下启动时调用：创建并隐藏 overlay。运行时模式无操作。
pub fn ensure_overlay(app: &tauri::AppHandle) -> Result<(), String> {
    let config = app
        .state::<AppState>()
        .config_store
        .get()
        .map_err(|e| format!("读取配置失败: {e}"))?;
    if !config.overlay_precreate {
        return Ok(());
    }
    if app.get_webview_window(OVERLAY_LABEL).is_some() {
        return Ok(());
    }
    build_overlay(app)?;
    Ok(())
}

/// 在光标所在显示器上铺满建 overlay 窗口。整屏帧须已存入 AppState。
/// 按配置分预创建模式（复用持久窗口 + 重载帧）与运行时模式（按需创建）。
pub fn open_overlay(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    let window = if config.overlay_precreate {
        // 预创建模式：获取已有窗口，重载帧以读取新的 pending_capture
        app.get_webview_window(OVERLAY_LABEL)
            .ok_or_else(|| "截图 overlay 未预创建".to_string())?
    } else {
        // 运行时模式：先关闭已有窗口（如果有）
        if let Some(existing) = app.get_webview_window(OVERLAY_LABEL) {
            let _ = existing.close();
        }
        build_overlay(app)?
    };

    // 预创建模式需重载前端以读取新的 pending_capture 帧
    if config.overlay_precreate {
        let _ = window.eval("location.reload()");
    }

    Ok(())
}

// 仅 hide 不 close：submit/cancel 由 overlay 自身的 invoke 触发，命令响应经 wry
// custom-protocol（http://ipc.localhost/<cmd>）由 wry::dispatch_handler 以 PostMessage
// 投递回 overlay 宿主 hwnd。若在此处 close() 销毁 overlay，hwnd 会先于响应投递失效，
// PostMessageW 返回 ERROR_INVALID_WINDOW_HANDLE(0x80070578)（wry 在 debug 下 eprintln
// “PostMessage failed ; is the messages queue full?”）。hide() 令窗口不可见（体验同 close）
// 但保留有效 hwnd，响应可正常投递；实际销毁交由下次 open_overlay 的 existing.close()
// 回收——此时该 hwnd 已无在途响应指向它，close 不再触发该错误。
fn hide_overlay(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = window.hide();
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
    // 释放 capture 锁。幂等：若 submit 已 take 走帧并释放过，此处再清无害。
    // 若 cancel 自己 take 走帧，则此处负责释放 start_translation_from_ocr 占的锁。
    let _ = state.finish_capture();
    hide_overlay(&app);
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

    hide_overlay(&app);

    let Some((frame, scale)) = state.take_pending_capture()? else {
        // 帧已被取消/消费（cancel 或前一次 submit 已 take 并释放 capture 锁），静默。
        return Ok(());
    };
    let region = css_rect_to_physical(x, y, w, h, scale);
    if region.2 == 0 || region.3 == 0 {
        // 选区过小：take 已成功，须释放 start_translation_from_ocr 占的 capture 锁。
        let _ = state.finish_capture();
        return Ok(());
    }

    // recognize 期间持锁，挡住二次 Alt+O 覆盖新帧。
    let result = recognize_region(&frame, region, OcrHints::default()).await;
    // recognize 完成，释放 capture 锁；后续 start_translation_from_input 由 translation_busy 接管。
    let _ = state.finish_capture();

    let app_state = state.inner();
    match result {
        // recognize_cropped_for_translation 永不返回 Ok(None)（空文本走 Err(EmptyResult)）；
        // 此分支若被触达即契约违反，报错而非静默吞掉。
        Ok(None) => show_translation_error(&app, "未识别到文本"),
        Ok(Some(input)) => {
            if let Err(error) = start_translation_from_input(input, app.clone(), app_state) {
                show_translation_error(&app, error);
            }
        }
        Err(error) => show_translation_error(&app, crate::ui::ocr_popup::friendly_ocr_error(error)),
    }
    Ok(())
}

/// 前端 canvas 渲染完成后调用，让 overlay 窗口可见。
/// 后端 Rust 调 window.show() 不走 IPC 权限层，无需 capability 授权 core:window:allow-show。
#[tauri::command]
pub async fn show_overlay(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        window.show().map_err(|e| e.to_string())?;
        let _ = window.set_focus();
    }
    Ok(())
}
