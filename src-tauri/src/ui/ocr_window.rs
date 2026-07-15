use tauri::AppHandle;

use crate::app::state::AppState;
use crate::app::window::show_ocr_window;

pub fn open_ocr_window(app: &AppHandle) -> Result<(), String> {
    show_ocr_window(app)
}

/// 任务 9 完整实现：截图 + RecognizeOnly purpose + overlay
pub async fn start_ocr_capture(app: AppHandle, state: AppState) {
    // 本任务 stub：后续任务 9 填充。可 log::debug 占位。
    log::debug!("start_ocr_capture stub（任务 9 实现）");
    let _ = (app, state);
}
