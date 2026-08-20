use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::{
    app::state::{AppState, CapturePurpose},
    core::config::AppConfig,
    core::ocr::OcrHints,
    core::translation::TranslationEvent,
    platform::{
        cursor_monitor_physical_bounds, cursor_monitor_scale_factor, recognize_cropped_full,
        recognize_region,
    },
    ui::web_popup::{
        emit_translation_event, show_translation_error, show_translation_popup,
        start_translation_from_input,
    },
};

pub const OVERLAY_LABEL: &str = "screenshot-overlay";

/// 截图 scale 唯一事实来源：overlay 自身 scale_factor（铺满被抓屏时即该屏 DPI）。
/// **不可取 main WebView 的 scale**——main 可能不存在（托盘/纯识别）或位于其它 DPI 屏幕，
/// 落到 1.0 会在高 DPI 上只裁到左上角（历史反复回归的根因）。
fn overlay_scale_factor(app: &tauri::AppHandle) -> f64 {
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        if let Ok(scale) = window.scale_factor() {
            if scale.is_finite() && scale > 0.0 {
                return scale;
            }
        }
    }
    cursor_monitor_scale_factor()
}

fn build_overlay(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    let (phys_x, phys_y, phys_w, phys_h) =
        cursor_monitor_physical_bounds().unwrap_or((0, 0, 1920, 1080));
    let scale = cursor_monitor_scale_factor().max(0.5);

    let logical_x = phys_x as f64 / scale;
    let logical_y = phys_y as f64 / scale;
    let logical_w = phys_w as f64 / scale;
    let logical_h = phys_h as f64 / scale;

    let window = WebviewWindowBuilder::new(app, OVERLAY_LABEL, WebviewUrl::App("overlay.html".into()))
        .title("Shizi 截图")
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .position(logical_x, logical_y)
        .inner_size(logical_w, logical_h)
        .fullscreen(true)
        // 创建时不可见：WebView2 加载 HTML + canvas putImageData 期间会显示默认透明，
        // 由前端在双 rAF 提交 GPU 帧后 invoke('show_overlay') 显示，消除白底与帧闪烁。
        .visible(false)
        .build()
        .map_err(|e| e.to_string())?;

    // 兜底：overlay 被外部关闭或异常销毁时，释放 pending_capture 帧与 capture 锁
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

/// 纯冷启动策略：按需创建，用完即销毁，不在启动时预建以节省内存。
pub fn ensure_overlay(_app: &tauri::AppHandle) -> Result<(), String> {
    Ok(())
}

/// 打开 overlay：每次均冷启动构建全屏截图窗口。
pub fn open_overlay(app: &tauri::AppHandle, _config: &AppConfig) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = window.destroy();
    }
    build_overlay(app)?;
    Ok(())
}

/// 用完销毁 overlay 窗口，释放 WebView2 进程与全部内存资源。
fn destroy_overlay(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = window.hide();
        let window_to_destroy = window.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let _ = window_to_destroy.destroy();
        });
    }
}

#[tauri::command]
pub async fn get_capture_frame_meta(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<Option<(u32, u32, f64)>, String> {
    let Some((w, h)) = state.pending_capture_meta()? else {
        return Ok(None);
    };
    Ok(Some((w, h, overlay_scale_factor(&app))))
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
    // 在 finish 前读 purpose：取消纯识别框选时需恢复 OCR 窗（截图前被 hide）。
    let purpose = state.capture_purpose();
    let _ = state.take_pending_capture();
    // 释放 capture 锁。幂等：若 submit 已 take 走帧并释放过，此处再清无害。
    // 若 cancel 自己 take 走帧，则此处负责释放 start_translation_from_ocr 占的锁。
    let _ = state.finish_capture();
    destroy_overlay(&app);
    // 仅 RecognizeOnly 清会话槽并恢复 OCR 窗；Translate / Alt+S 路径不碰。
    if purpose == CapturePurpose::RecognizeOnly {
        let _ = state.clear_ocr_session_service_id();
        if let Err(e) = crate::app::window::show_ocr_window(&app) {
            log::warn!("取消截图后恢复文字识别窗口失败: {e}");
        }
    }
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

    let scale = overlay_scale_factor(&app);
    destroy_overlay(&app);

    let Some(frame) = state.take_pending_capture()? else {
        // 帧已被取消/消费（cancel 或前一次 submit 已 take 并释放 capture 锁），静默。
        return Ok(());
    };
    let region = css_rect_to_physical(x, y, w, h, scale);
    if region.2 == 0 || region.3 == 0 {
        // 选区过小：take 已成功，须释放 start_translation_from_ocr 占的 capture 锁。
        let _ = state.finish_capture();
        return Ok(());
    }

    // recognize 前读配置：解析 ocrServices 引擎，并复用于后续 show。
    // take 已成功：读配置失败也须释放 capture 锁。
    let config = match state.config_store.get() {
        Ok(c) => c,
        Err(e) => {
            let _ = state.finish_capture();
            return Err(e.to_string());
        }
    };

    // 按入口设置的用途分叉：Translate → 翻译弹窗；RecognizeOnly → OCR 窗事件。
    // 耗时 OCR 与翻译/识别处理放入后台任务执行，使 submit_capture_region 立即返回，
    // 避免 overlay 窗口在 50ms 销毁后因 IPC 响应回传与广播产生「无效窗口句柄」报错。
    let purpose = state.capture_purpose();
    let app_handle = app.clone();
    let app_state = state.inner().clone();
    tauri::async_runtime::spawn(async move {
        process_captured_region(app_handle, app_state, frame, region, purpose, config).await;
    });

    Ok(())
}

async fn process_captured_region(
    app: tauri::AppHandle,
    state: AppState,
    frame: crate::core::capture::CapturedImage,
    region: (u32, u32, u32, u32),
    purpose: CapturePurpose,
    config: AppConfig,
) {
    match purpose {
        CapturePurpose::Translate => {
            // 尽早释放截图锁：OCR 不再占用 capture，允许识别中再开截图（会 cancel 旧 OCR）。
            let _ = state.finish_capture();

            let cancel = tokio_util::sync::CancellationToken::new();
            let generation = match state.begin_ocr_overriding(cancel.clone()) {
                Ok(g) => g,
                Err(e) => {
                    show_translation_error(&app, e);
                    return;
                }
            };

            // 先开弹窗再 OCR，消除空窗；识别中态由 OcrStarted 驱动。
            let _ = show_translation_popup(&app, &config);
            let _ = emit_translation_event(&app, TranslationEvent::OcrStarted);

            let result = tokio::select! {
                _ = cancel.cancelled() => {
                    let _ = state.finish_ocr_if_current(generation);
                    return;
                }
                r = recognize_region(
                    &frame,
                    region,
                    OcrHints::default(),
                    &config.ocr_services,
                ) => r,
            };

            // 关窗 / 新一轮 OCR 会 cancel 并递增 generation：丢弃结果，禁止进入翻译。
            if cancel.is_cancelled() || !state.is_ocr_generation_current(generation) {
                let _ = state.finish_ocr_if_current(generation);
                return;
            }
            let _ = state.finish_ocr_if_current(generation);
            if !state.is_ocr_generation_current(generation) {
                return;
            }

            match result {
                // recognize_cropped_for_translation 永不返回 Ok(None)（空文本走 Err(EmptyResult)）；
                // 此分支若被触达即契约违反，报错而非静默吞掉。
                Ok(None) => show_translation_error(&app, "未识别到文本"),
                Ok(Some(input)) => {
                    if let Err(error) = start_translation_from_input(input, app.clone(), &state) {
                        show_translation_error(&app, error);
                    }
                }
                Err(error) => {
                    show_translation_error(&app, crate::ui::ocr_popup::friendly_ocr_error(error))
                }
            }
        }
        CapturePurpose::RecognizeOnly => {
            // take：避免泄漏到下次；cancel 路径另行 clear。Translate 分支绝不读写此槽。
            let service_id = state.take_ocr_session_service_id().unwrap_or(None);
            let result = recognize_cropped_full(
                &frame,
                region,
                OcrHints::default(),
                &config.ocr_services,
                service_id,
            )
            .await;
            let _ = state.finish_capture();
            match result {
                Ok(full) => {
                    if let Err(e) = state.set_last_ocr_image(full.source_image) {
                        log::warn!("写入 last_ocr_image 失败: {e}");
                    }
                    let _ = crate::app::window::show_ocr_window(&app);
                    if let Err(e) = app.emit("ocr:recognize-result", &full.response) {
                        log::warn!("emit ocr:recognize-result 失败: {e}");
                    }
                }
                Err(error) => {
                    // 失败不清除 last_ocr_image，保留上次成功源图供重新识别
                    let msg = crate::ui::ocr_popup::friendly_ocr_error(error);
                    let _ = crate::app::window::show_ocr_window(&app);
                    if let Err(e) = app.emit("ocr:recognize-failed", msg) {
                        log::warn!("emit ocr:recognize-failed 失败: {e}");
                    }
                }
            }
        }
    }
}

/// 前端 canvas 渲染完成后调用，让 overlay 窗口可见。
/// 后端 Rust 调 window.show() 不走 IPC 权限层，无需 capability 授权 core:window:allow-show。
#[tauri::command]
pub async fn show_overlay(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        window.show().map_err(|e| e.to_string())?;
    }
    Ok(())
}
