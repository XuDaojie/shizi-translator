use crate::{
    app::state::AppState,
    core::update::{
        evaluate_check, fetch_releases, CheckUpdateResult, UpdateChannel, RELEASES_PAGE_FALLBACK,
    },
};
use reqwest::Client;
use tauri::Manager;

#[tauri::command]
pub async fn check_for_update(
    channel: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<CheckUpdateResult, String> {
    let config = state.config_store.get().map_err(|e| e.to_string())?;
    let channel = UpdateChannel::parse(
        channel
            .as_deref()
            .unwrap_or(config.update_channel.as_str()),
    );
    let current = env!("CARGO_PKG_VERSION").to_string();

    log::info!(
        "检查更新开始 channel={} current={}",
        channel.as_str(),
        current
    );

    let client = Client::new();
    let result = match fetch_releases(&client).await {
        Ok(releases) => {
            log::debug!("GitHub 返回 release 数: {}", releases.len());
            evaluate_check(&current, &releases, channel)
        }
        Err(message) => {
            log::warn!("检查更新失败: {}", message);
            CheckUpdateResult {
                status: "error".into(),
                current_version: current,
                latest_version: None,
                release_name: None,
                release_url: None,
                is_prerelease: None,
                message: Some(message),
            }
        }
    };

    log::info!(
        "检查更新结束 status={} latest={:?}",
        result.status,
        result.latest_version
    );
    Ok(result)
}

/// setup 完成后调用：best-effort，不阻塞。
pub fn spawn_startup_update_check(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let config = match state.config_store.get() {
            Ok(c) => c,
            Err(e) => {
                log::warn!("启动检查更新：读取配置失败: {e}");
                return;
            }
        };
        if !config.auto_check_update {
            log::info!("启动检查更新：已关闭，跳过");
            return;
        }

        let channel = UpdateChannel::parse(&config.update_channel);
        let current = env!("CARGO_PKG_VERSION").to_string();
        let client = Client::new();
        let result = match fetch_releases(&client).await {
            Ok(releases) => evaluate_check(&current, &releases, channel),
            Err(message) => {
                log::warn!("启动检查更新失败（不打扰用户）: {message}");
                return;
            }
        };

        if result.status != "update_available" {
            log::info!("启动检查更新: {}", result.status);
            return;
        }

        let latest = result.latest_version.clone().unwrap_or_default();
        let url = result
            .release_url
            .clone()
            .unwrap_or_else(|| RELEASES_PAGE_FALLBACK.to_string());
        let pre = result.is_prerelease.unwrap_or(false);
        let body = if pre {
            format!(
                "发现新版本 {latest}（预发布）。\n当前版本 {}。\n是否前往下载页？",
                result.current_version
            )
        } else {
            format!(
                "发现新版本 {latest}。\n当前版本 {}。\n是否前往下载页？",
                result.current_version
            )
        };

        // 系统对话框必须在可与 UI 交互的上下文；blocking_show 在 async 任务中：
        // 用 spawn_blocking 避免卡住 runtime（Windows 消息框）。
        let app2 = app.clone();
        let url2 = url.clone();
        let go = tauri::async_runtime::spawn_blocking(move || {
            use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
            app2.dialog()
                .message(body)
                .title("发现新版本")
                .kind(MessageDialogKind::Info)
                .buttons(MessageDialogButtons::OkCancelCustom(
                    "前往下载".to_string(),
                    "稍后".to_string(),
                ))
                .blocking_show()
        })
        .await
        .unwrap_or(false);

        if go {
            if let Err(e) = crate::ui::config::open_url(url2) {
                log::warn!("启动检查更新：打开下载页失败: {e}");
            }
        }
    });
}
