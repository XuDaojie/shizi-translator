use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::{
    app::{
        popup_window,
        state::{AppState, CapturePurpose},
        window::{hide_ocr_window, show_ocr_window},
    },
    core::{
        capture::{CapturedImage, CapturedImageFormat},
        ocr::{meta::RecognizeImageResponse, OcrError, OcrHints},
        ocr_translation::OcrTranslationError,
        selection::read_clipboard_image,
    },
    platform::{capture_screen, recognize_image_full},
    ui::{ocr_popup::friendly_ocr_error, overlay},
};

/// 无缓存图时重新识别的固定错误文案（前端可依赖此字面量）。
pub(crate) const RERECOGNIZE_NO_IMAGE_MSG: &str =
    "没有可重新识别的图像，请先截图、打开文件或从剪贴板识别。";

/// 前端 invoke：打开文字识别窗口。
#[tauri::command]
pub fn open_ocr_window(app: AppHandle) -> Result<(), String> {
    show_ocr_window(&app)
}

/// 将内存中的图片字节解码为 RGBA8 `CapturedImage`。
pub fn load_image_file_bytes(bytes: &[u8]) -> Result<CapturedImage, OcrError> {
    let dyn_img = image::load_from_memory(bytes)
        .map_err(|e| OcrError::ImageConversionFailed(e.to_string()))?;
    let rgba = dyn_img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Ok(CapturedImage {
        bytes: rgba.into_raw(),
        width: w,
        height: h,
        format: CapturedImageFormat::Rgba8,
    })
}

fn emit_ocr_recognize_failed(app: &AppHandle, message: String) {
    log::warn!("OCR 纯识别失败: {message}");
    if let Err(e) = show_ocr_window(app) {
        log::warn!("打开文字识别窗口失败: {e}");
    }
    if let Err(e) = app.emit("ocr:recognize-failed", message) {
        log::warn!("emit ocr:recognize-failed 失败: {e}");
    }
}

/// 截图纯识别核心流程：不检查 translation_busy，purpose=RecognizeOnly。
/// 失败时 emit `ocr:recognize-failed` 并尽量 show OCR 窗，不用翻译错误 toast。
pub async fn start_ocr_capture_flow(app: AppHandle, state: AppState) {
    if let Err(message) = state.try_begin_capture() {
        emit_ocr_recognize_failed(&app, message);
        return;
    }
    let _ = state.set_capture_purpose(CapturePurpose::RecognizeOnly);

    // 截图前隐藏翻译弹窗与文字识别窗，避免窗口内容进帧（Alt+O / 识别窗「截图」共用此路径）。
    // 框选结束 submit / 失败路径会再 show_ocr_window。
    popup_window::hide_popup(&app);
    hide_ocr_window(&app);
    // 给 DWM 提交 hide 的时间，降低把窗口残影打进下一帧的概率。
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let frame = match capture_screen().await {
        Ok(frame) => frame,
        Err(error) => {
            let _ = state.finish_capture();
            emit_ocr_recognize_failed(
                &app,
                friendly_ocr_error(OcrTranslationError::Capture(error)),
            );
            return;
        }
    };

    let scale = app
        .get_webview_window("main")
        .and_then(|w| w.scale_factor().ok())
        .unwrap_or(1.0);

    if let Err(error) = state.set_pending_capture(frame, scale) {
        let _ = state.finish_capture();
        emit_ocr_recognize_failed(&app, error);
        return;
    }

    let config = match state.config_store.get() {
        Ok(c) => c,
        Err(e) => {
            let _ = state.take_pending_capture();
            let _ = state.finish_capture();
            emit_ocr_recognize_failed(&app, format!("读取配置失败: {e}"));
            return;
        }
    };

    if let Err(error) = overlay::open_overlay(&app, &config) {
        let _ = state.take_pending_capture();
        let _ = state.finish_capture();
        emit_ocr_recognize_failed(&app, format!("无法打开截图窗口：{error}"));
    }
}

/// 前端 invoke：启动截图框选纯识别。
/// `service_id` 写入会话槽，供 overlay RecognizeOnly 提交时 `take` 使用。
#[tauri::command]
pub async fn start_ocr_capture(
    app: AppHandle,
    state: State<'_, AppState>,
    service_id: Option<String>,
) -> Result<(), String> {
    state.set_ocr_session_service_id(service_id)?;
    start_ocr_capture_flow(app, state.inner().clone()).await;
    Ok(())
}

/// 前端 invoke：识别剪贴板中的图片。
#[tauri::command]
pub async fn recognize_clipboard_image(
    state: State<'_, AppState>,
    service_id: Option<String>,
) -> Result<RecognizeImageResponse, String> {
    let image = read_clipboard_image()?
        .ok_or_else(|| "剪贴板中没有图片".to_string())?;
    log::info!(
        "OCR 剪贴板读图: {}x{}",
        image.width,
        image.height
    );
    let config = state.config_store.get().map_err(|e| e.to_string())?;
    let full = recognize_image_full(
        image,
        OcrHints::default(),
        &config.ocr_services,
        service_id,
    )
    .await
    .map_err(|e| friendly_ocr_error(OcrTranslationError::from(e)))?;
    if let Err(e) = state.set_last_ocr_image(full.source_image) {
        log::warn!("写入 last_ocr_image 失败: {e}");
    }
    Ok(full.response)
}

/// 前端 invoke：文件选择器选图/PDF 并识别；用户取消返回 `Ok(None)`。
/// PDF 仅渲染第 1 页，meta 填充 `source_page`/`source_page_count`；渲染失败不写缓存。
#[tauri::command]
pub async fn pick_and_recognize_image(
    app: AppHandle,
    state: State<'_, AppState>,
    service_id: Option<String>,
) -> Result<Option<RecognizeImageResponse>, String> {
    let app2 = app.clone();
    let path = tauri::async_runtime::spawn_blocking(move || {
        app2.dialog()
            .file()
            .add_filter(
                "图片与 PDF",
                &["png", "jpg", "jpeg", "webp", "bmp", "pdf"],
            )
            .add_filter("PDF", &["pdf"])
            .blocking_pick_file()
    })
    .await
    .map_err(|e| e.to_string())?;

    let Some(file_path) = path else {
        return Ok(None);
    };
    let path = file_path.into_path().map_err(|e| e.to_string())?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    // 禁止 log 文件内容 / base64；仅记路径
    log::info!("OCR 文件读入: path={}", path.display());

    let (image, pdf_pages): (CapturedImage, Option<u32>) =
        if crate::core::ocr::pdf_detect::looks_like_pdf(Some(&path), &bytes) {
            let rendered = crate::platform::render_pdf_first_page(&bytes)
                .await
                .map_err(|e| friendly_ocr_error(OcrTranslationError::from(e)))?;
            log::info!(
                "PDF 首页: pages={} {}x{}",
                rendered.page_count,
                rendered.image.width,
                rendered.image.height
            );
            (rendered.image, Some(rendered.page_count))
        } else {
            let image = load_image_file_bytes(&bytes).map_err(|e| e.to_string())?;
            log::info!("OCR 文件尺寸: {}x{}", image.width, image.height);
            (image, None)
        };

    let config = state.config_store.get().map_err(|e| e.to_string())?;
    let full = recognize_image_full(
        image,
        OcrHints::default(),
        &config.ocr_services,
        service_id,
    )
    .await
    .map_err(|e| friendly_ocr_error(OcrTranslationError::from(e)))?;
    // source_image 是栅格图（PDF 已渲染为位图），不是原始 PDF 字节
    if let Err(e) = state.set_last_ocr_image(full.source_image) {
        log::warn!("写入 last_ocr_image 失败: {e}");
    }
    let mut response = full.response;
    if let Some(n) = pdf_pages {
        response.meta.source_page = Some(1);
        response.meta.source_page_count = Some(n);
    }
    Ok(Some(response))
}

/// 对最近一次纯识别成功的源图再跑一遍 OCR。
/// 不占 capture 锁、不查 translation_busy；无缓存时返回固定文案错误。
#[tauri::command]
pub async fn rerecognize_last_image(
    state: State<'_, AppState>,
    service_id: Option<String>,
) -> Result<RecognizeImageResponse, String> {
    let image = state
        .clone_last_ocr_image()?
        .ok_or_else(|| RERECOGNIZE_NO_IMAGE_MSG.to_string())?;
    log::info!("OCR 重新识别: {}x{}", image.width, image.height);
    let config = state.config_store.get().map_err(|e| e.to_string())?;
    let full = recognize_image_full(
        image,
        OcrHints::default(),
        &config.ocr_services,
        service_id,
    )
    .await
    .map_err(|e| friendly_ocr_error(OcrTranslationError::from(e)))?;
    if let Err(e) = state.set_last_ocr_image(full.source_image) {
        log::warn!("写入 last_ocr_image 失败: {e}");
    }
    Ok(full.response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, ImageFormat, Rgba};

    #[test]
    fn rerecognize_no_image_error_message() {
        assert_eq!(
            RERECOGNIZE_NO_IMAGE_MSG,
            "没有可重新识别的图像，请先截图、打开文件或从剪贴板识别。"
        );
    }

    #[test]
    fn load_image_file_bytes_decodes_minimal_png() {
        let mut png = Vec::new();
        {
            let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
                ImageBuffer::from_pixel(2, 3, Rgba([10, 20, 30, 255]));
            img.write_to(&mut std::io::Cursor::new(&mut png), ImageFormat::Png)
                .expect("encode png");
        }
        let captured = load_image_file_bytes(&png).expect("decode png");
        assert_eq!(captured.width, 2);
        assert_eq!(captured.height, 3);
        assert_eq!(captured.format, CapturedImageFormat::Rgba8);
        assert_eq!(captured.bytes.len(), 2 * 3 * 4);
        assert_eq!(&captured.bytes[0..4], &[10, 20, 30, 255]);
    }

    #[test]
    fn load_image_file_bytes_rejects_garbage() {
        let err = load_image_file_bytes(b"not-an-image").expect_err("garbage");
        assert!(matches!(err, OcrError::ImageConversionFailed(_)));
    }
}
