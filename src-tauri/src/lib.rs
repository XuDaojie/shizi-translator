mod app;
mod core;
mod platform;
mod ui;

use std::sync::Mutex;

use app::{
    logging,
    popup_backend::{self, PopupHost},
    shortcuts::{handle_global_shortcut, register_global_shortcuts_at_startup},
    state::AppState,
    tray::{setup_tray, TrayI18nHandles},
};
use core::{
    config::ConfigStore,
    history::{store::HistoryError, HistoryStore},
};
use tauri::Manager;
use ui::{
    config::{
        get_app_config, get_shortcut_conflicts, is_autostart_launch, open_settings, open_url,
        save_app_config,
    },
    history::{clear_translation_history, list_translation_history},
    i18n::{
        apply_interface_language, get_interface_language_snapshot, open_language_pack_directory,
        refresh_interface_languages,
    },
    logging::{export_logs, write_frontend_log},
    ocr_popup::trigger_ocr_translation,
    ocr_window::{
        open_ocr_window, pick_and_recognize_image, recognize_clipboard_image,
        rerecognize_last_image, start_ocr_capture,
    },
    overlay::{
        cancel_capture, ensure_overlay, get_capture_frame_bytes, get_capture_frame_meta,
        show_overlay, submit_capture_region,
    },
    service_probe::{list_service_models, validate_service_credential},
    update::check_for_update,
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
            HistoryStore::in_memory()
                .map_err(|fallback_error| tauri::Error::Anyhow(fallback_error.into()))
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
            open_ocr_window,
            start_ocr_capture,
            recognize_clipboard_image,
            pick_and_recognize_image,
            rerecognize_last_image,
            cancel_translation,
            retry_translation,
            get_session_languages,
            set_session_languages,
            save_edge_translate_env,
            take_pending_source_text,
            get_app_config,
            save_app_config,
            is_autostart_launch,
            get_shortcut_conflicts,
            list_translation_history,
            clear_translation_history,
            open_settings,
            open_url,
            list_service_models,
            validate_service_credential,
            get_capture_frame_meta,
            get_capture_frame_bytes,
            submit_capture_region,
            cancel_capture,
            show_overlay,
            write_frontend_log,
            export_logs,
            get_interface_language_snapshot,
            refresh_interface_languages,
            open_language_pack_directory,
            check_for_update,
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

            let tray_i18n_handles: TrayI18nHandles = setup_tray(app)?;
            app.manage(tray_i18n_handles);
            let configured_locale = app
                .state::<AppState>()
                .config_store
                .get()
                .map(|config| config.interface_language)
                .unwrap_or_else(|_| "auto".into());
            apply_interface_language(
                app.handle(),
                &app.state::<AppState>(),
                &configured_locale,
                false,
                false,
            )
            .map_err(|error| tauri::Error::Anyhow(std::io::Error::other(error).into()))?;
            let config = app
                .state::<AppState>()
                .config_store
                .get()
                .unwrap_or_else(|_| AppConfig::default());
            let shortcut_conflicts = register_global_shortcuts_at_startup(app.handle(), &config);
            let _ = app
                .state::<AppState>()
                .set_shortcut_conflicts(shortcut_conflicts);

            // 弹窗后端宿主：配置解析 → create（WinUI ensure 失败则降级 WebView + 提示）→ manage。
            let kind = popup_backend::resolve_popup_backend_kind(
                &config.popup_ui_backend,
                popup_backend::POPUP_WINUI_FEATURE,
                cfg!(windows),
            );
            let host = popup_backend::create_host_with_winui_fallback(app.handle(), kind);
            app.manage(Mutex::new(host));

            // 按 windowPrecreate（手动 / 自启）决定是否预建 main 与 overlay。
            // 设置页 / 文字识别不在启动时创建。预建经 ensure_popup_window → host.ensure_created。
            let _ = crate::app::popup_window::ensure_popup_window(app.handle(), &config);
            let _ = ensure_overlay(app.handle());

            // 用当前 exe 路径刷新 Run 项（升级后路径变化时仍能自启）；失败不挡启动。
            if let Err(error) = crate::app::autostart::apply(config.launch_at_login) {
                log::warn!("同步开机启动失败: {error}");
            }

            crate::ui::update::spawn_startup_update_check(app.handle().clone());

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("构建应用失败")
        .run(|app_handle, event| match event {
            // 托盘驻留：无窗 / 关最后一窗不退出；托盘「退出」走 app.exit 带 code，不拦截。
            tauri::RunEvent::ExitRequested { api, code, .. } => {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
            // 进程退出时 best-effort 销毁弹窗后端资源。
            tauri::RunEvent::Exit => {
                if let Some(state) = app_handle.try_state::<Mutex<PopupHost>>() {
                    if let Ok(mut host) = state.lock() {
                        host.destroy();
                    }
                }
            }
            _ => {}
        });
}

#[cfg(test)]
mod tests {
    use super::load_history_store_or_fallback;
    use crate::core::history::{store::HistoryError, HistoryStore};
    use std::path::Path;

    #[test]
    fn history_store_load_failure_falls_back_to_memory_store() {
        let store =
            load_history_store_or_fallback(Err(HistoryError::Lock)).expect("应降级到内存历史库");
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
