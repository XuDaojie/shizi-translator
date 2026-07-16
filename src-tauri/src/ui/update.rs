use crate::{
    app::state::AppState,
    core::update::{evaluate_check, fetch_releases, CheckUpdateResult, UpdateChannel},
};
use reqwest::Client;

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
