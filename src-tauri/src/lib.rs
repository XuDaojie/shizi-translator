mod app;
mod core;
mod platform;
mod ui;

use app::{
    logging, popup_window::ensure_popup_window,
    shortcuts::{handle_global_shortcut, register_global_shortcuts_at_startup},
    state::AppState,
    tray::setup_tray,
    window::{ensure_settings_window, setup_close_to_hide},
};
use core::{
    config::ConfigStore,
    history::{store::HistoryError, HistoryStore},
};
use tauri::Manager;
use ui::{
    config::{get_app_config, get_shortcut_conflicts, open_settings, save_app_config},
    history::{clear_translation_history, list_translation_history},
    logging::{export_logs, write_frontend_log},
    ocr_popup::trigger_ocr_translation,
    overlay::{
        cancel_capture, ensure_overlay, get_capture_frame_bytes, get_capture_frame_meta,
        show_overlay, submit_capture_region,
    },
    service_probe::{list_service_models, validate_service_credential},
    web_popup::{
        cancel_translation, get_session_languages, retry_translation, save_edge_translate_env,
        set_session_languages, start_translation, take_pending_source_text,
    },
};

use crate::core::config::AppConfig;

fn load_history_store_or_fallback(
    result: Result<HistoryStore, HistoryError>,
) -> Result<HistoryStore, tauri::Error> {
    match result {
        Ok(store) => Ok(store),
        Err(error) => {
            let message = format!("历史库加载失败，将使用内存历史库: {}", error);
            log::error!("{}", message);
            eprintln!("{}", message);
            HistoryStore::in_memory().map_err(|fallback_error| tauri::Error::Anyhow(fallback_error.into()))
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    handle_global_shortcut(app, shortcut, event);
                })
                .build(),
        )
        .plugin(tauri_plugin_dialog::init());

    // MCP Bridge 插件：仅 debug 构建注册，绑定 127.0.0.1 仅供本机 MCP server 连接，
    // release 包不带此插件、不开放端口。
    #[cfg(debug_assertions)]
    {
        builder = builder.plugin(
            tauri_plugin_mcp_bridge::Builder::new()
                .bind_address("127.0.0.1")
                .build(),
        );
    }

    builder
        .invoke_handler(tauri::generate_handler![
            start_translation,
            trigger_ocr_translation,
            cancel_translation,
            retry_translation,
            get_session_languages,
            set_session_languages,
            save_edge_translate_env,
            take_pending_source_text,
            get_app_config,
            save_app_config,
            get_shortcut_conflicts,
            list_translation_history,
            clear_translation_history,
            open_settings,
            list_service_models,
            validate_service_credential,
            get_capture_frame_meta,
            get_capture_frame_bytes,
            submit_capture_region,
            cancel_capture,
            show_overlay,
            write_frontend_log,
            export_logs,
        ])
        .setup(|app| {
            let config_store = ConfigStore::load(app.handle())
                .map_err(|error| tauri::Error::Anyhow(error.into()))?;
            let log_level = config_store
                .get()
                .map(|c| c.log_level)
                .unwrap_or_else(|_| "info".to_string());
            logging::init_logging(app.handle(), &log_level);
            if let Some(dir) = logging::logs_dir(app.handle()) {
                logging::cleanup_old_logs(&dir, 7);
            }
            log::info!("应用启动，日志等级: {}", log_level);
            let history_store = load_history_store_or_fallback(HistoryStore::load(app.handle()))?;
            app.manage(AppState::new(config_store, history_store));

            setup_tray(app)?;
            setup_close_to_hide(app);

            let config = app
                .state::<AppState>()
                .config_store
                .get()
                .unwrap_or_else(|_| AppConfig::from_env());
            let shortcut_conflicts = register_global_shortcuts_at_startup(app.handle(), &config);
            let _ = app
                .state::<AppState>()
                .set_shortcut_conflicts(shortcut_conflicts);

            // 按窗口策略预创建弹窗与 overlay
            let _ = ensure_popup_window(app.handle(), &config);
            let _ = ensure_settings_window(app.handle());
            let _ = ensure_overlay(app.handle());

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("启动应用失败");
}

#[cfg(test)]
mod tests {
    use super::load_history_store_or_fallback;
    use crate::core::history::{store::HistoryError, HistoryStore};
    use std::path::Path;

    #[test]
    fn history_store_load_failure_falls_back_to_memory_store() {
        let store = load_history_store_or_fallback(Err(HistoryError::Lock)).expect("应降级到内存历史库");
        assert_eq!(store.path(), Path::new(":memory:"));
        assert!(store.list_recent(1).expect("内存历史库应可读").is_empty());
    }

    #[test]
    fn history_store_load_success_keeps_original_store() {
        let store = HistoryStore::in_memory().expect("创建内存历史库");
        let path = store.path().to_path_buf();
        let selected = load_history_store_or_fallback(Ok(store)).expect("成功路径应直接返回");
        assert_eq!(selected.path(), path.as_path());
    }
}
