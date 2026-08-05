//! WebDAV 备份与本机导入/导出 commands。

use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::{
    app::state::AppState,
    core::{
        backup::{
            build_backup_file_name, build_backup_zip, build_local_export_json, list_directory,
            parse_backup_zip, parse_local_export_json, put_bytes, test_connection, WebDavClient,
        },
        config::{normalize_webdav_remote_path, AppConfig},
        history::HistorySessionDto,
    },
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavConnectionArgs {
    pub url: String,
    pub username: String,
    pub password: String,
    pub remote_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavTestResult {
    pub last_tested_at: String,
    pub remote_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavBackupResult {
    pub last_backup_at: String,
    pub remote_path: String,
    pub file_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBackupItemDto {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at: String,
    pub size_label: String,
    pub include_history: bool,
    pub include_api_keys: bool,
}

fn client_from_args(args: &WebDavConnectionArgs) -> Result<WebDavClient, String> {
    WebDavClient::new(&args.url, &args.username, &args.password).map_err(|e| e.to_string())
}

fn merge_connection_into_config(config: &mut AppConfig, args: &WebDavConnectionArgs) {
    config.backup.webdav.url = args.url.trim().to_string();
    config.backup.webdav.username = args.username.trim().to_string();
    config.backup.webdav.password = args.password.clone();
    config.backup.webdav.remote_path = normalize_webdav_remote_path(&args.remote_path);
}

/// 从文件名 `shizi-backup-YYYYMMDD-HHMMSS.zip` 解析本地时间近似 ISO。
fn created_at_from_name(name: &str) -> String {
    let stem = name
        .trim_end_matches(".zip")
        .trim_end_matches(".ZIP")
        .strip_prefix("shizi-backup-")
        .unwrap_or("");
    // 20260805-120000
    if stem.len() >= 15 {
        let date = &stem[0..8];
        let time = &stem[9..15];
        if date.chars().all(|c| c.is_ascii_digit()) && time.chars().all(|c| c.is_ascii_digit()) {
            return format!(
                "{}-{}-{}T{}:{}:{}+00:00",
                &date[0..4],
                &date[4..6],
                &date[6..8],
                &time[0..2],
                &time[2..4],
                &time[4..6],
            );
        }
    }
    String::new()
}

#[tauri::command]
pub async fn test_webdav_connection(
    args: WebDavConnectionArgs,
    state: tauri::State<'_, AppState>,
) -> Result<WebDavTestResult, String> {
    let remote_path = normalize_webdav_remote_path(&args.remote_path);
    let client = client_from_args(&args)?;
    test_connection(&client, &remote_path)
        .await
        .map_err(|e| e.to_string())?;
    let last_tested_at = chrono::Utc::now().to_rfc3339();

    let mut config = state.config_store.get().map_err(|e| e.to_string())?;
    merge_connection_into_config(&mut config, &args);
    config.backup.webdav.remote_path = remote_path.clone();
    config.backup.webdav.last_tested_at = last_tested_at.clone();
    state
        .config_store
        .save(config.normalized())
        .map_err(|e| e.to_string())?;

    Ok(WebDavTestResult {
        last_tested_at,
        remote_path,
    })
}

#[tauri::command]
pub async fn backup_to_webdav(
    args: WebDavConnectionArgs,
    state: tauri::State<'_, AppState>,
) -> Result<WebDavBackupResult, String> {
    let mut config = state.config_store.get().map_err(|e| e.to_string())?;
    merge_connection_into_config(&mut config, &args);
    let remote_dir = normalize_webdav_remote_path(&config.backup.webdav.remote_path);
    config.backup.webdav.remote_path = remote_dir.clone();

    let include_history = config.backup.include_history;
    let include_api_keys = config.backup.include_api_keys;

    let history = if include_history {
        let limit = config.history_limit.max(1);
        Some(
            state
                .history_store
                .list_recent(limit)
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };

    let zip_bytes = build_backup_zip(
        &config,
        history.as_deref(),
        include_api_keys,
    )?;
    let file_name = build_backup_file_name(Local::now());
    let remote_file = format!("{remote_dir}{file_name}");

    let client = client_from_args(&args)?;
    put_bytes(&client, &remote_file, zip_bytes)
        .await
        .map_err(|e| e.to_string())?;

    let last_backup_at = chrono::Utc::now().to_rfc3339();
    config.backup.webdav.last_backup_at = last_backup_at.clone();
    state
        .config_store
        .save(config.normalized())
        .map_err(|e| e.to_string())?;

    Ok(WebDavBackupResult {
        last_backup_at,
        remote_path: remote_file,
        file_name,
    })
}

#[tauri::command]
pub async fn list_webdav_backups(
    args: WebDavConnectionArgs,
) -> Result<Vec<RemoteBackupItemDto>, String> {
    let remote_dir = normalize_webdav_remote_path(&args.remote_path);
    let client = client_from_args(&args)?;
    let items = list_directory(&client, &remote_dir)
        .await
        .map_err(|e| e.to_string())?;
    Ok(items
        .into_iter()
        .map(|item| RemoteBackupItemDto {
            id: item.name.clone(),
            name: item.name.clone(),
            path: item.path,
            created_at: created_at_from_name(&item.name),
            size_label: item.size_label,
            // 列表阶段不下载 manifest；前端可按需省略角标
            include_history: false,
            include_api_keys: false,
        })
        .collect())
}

#[tauri::command]
pub async fn restore_from_webdav(
    args: WebDavConnectionArgs,
    remote_file_path: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<AppConfig, String> {
    let client = client_from_args(&args)?;
    let bytes = client
        .get(&remote_file_path)
        .await
        .map_err(|e| e.to_string())?;
    let parsed = parse_backup_zip(&bytes)?;

    // 恢复前保留当前 WebDAV 凭证（避免远端剥离密钥后丢连接）
    let current = state.config_store.get().map_err(|e| e.to_string())?;
    let mut next = parsed.settings;
    // 若备份未含密钥，保留本机服务 key / webdav password
    if !parsed.manifest.include_api_keys {
        let local_keys: std::collections::HashMap<_, _> = current
            .services
            .iter()
            .filter_map(|s| s.api_key.clone().map(|k| (s.id.clone(), k)))
            .collect();
        for svc in &mut next.services {
            if svc.api_key.is_none() {
                if let Some(k) = local_keys.get(&svc.id) {
                    svc.api_key = Some(k.clone());
                }
            }
        }
        let ocr_keys: std::collections::HashMap<_, _> = current
            .ocr_services
            .iter()
            .filter_map(|s| s.api_key.clone().map(|k| (s.id.clone(), k)))
            .collect();
        for svc in &mut next.ocr_services {
            if svc.api_key.is_none() {
                if let Some(k) = ocr_keys.get(&svc.id) {
                    svc.api_key = Some(k.clone());
                }
            }
        }
        if next.backup.webdav.password.is_empty() {
            next.backup.webdav.password = current.backup.webdav.password.clone();
        }
    }
    // 连接表单当前值优先
    merge_connection_into_config(&mut next, &args);
    next.backup.webdav.last_backup_at = current.backup.webdav.last_backup_at.clone();
    next.backup.webdav.last_tested_at = current.backup.webdav.last_tested_at.clone();

    let saved = state
        .config_store
        .save(next.normalized())
        .map_err(|e| e.to_string())?;

    if let Some(history) = parsed.history {
        state
            .history_store
            .replace_all(&history)
            .map_err(|e| e.to_string())?;
    }

    app.emit("app-config:changed", &saved)
        .map_err(|e| format!("无法广播配置变更: {e}"))?;

    Ok(saved)
}

#[tauri::command]
pub async fn export_settings_snapshot(
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let config = state.config_store.get().map_err(|e| e.to_string())?;
    let history = if config.backup.include_history {
        let limit = config.history_limit.max(1);
        Some(
            state
                .history_store
                .list_recent(limit)
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    build_local_export_json(&config, history, config.backup.include_api_keys)
}

#[tauri::command]
pub async fn import_settings_snapshot(
    json: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<AppConfig, String> {
    let (mut next, history) = parse_local_export_json(&json)?;
    let current = state.config_store.get().map_err(|e| e.to_string())?;

    // 导入包若剥密钥：保留本机 key
    let local_keys: std::collections::HashMap<_, _> = current
        .services
        .iter()
        .filter_map(|s| s.api_key.clone().map(|k| (s.id.clone(), k)))
        .collect();
    for svc in &mut next.services {
        if svc.api_key.is_none() {
            if let Some(k) = local_keys.get(&svc.id) {
                svc.api_key = Some(k.clone());
            }
        }
    }
    if next.backup.webdav.password.is_empty() && !current.backup.webdav.password.is_empty() {
        next.backup.webdav.password = current.backup.webdav.password.clone();
    }

    let saved = state
        .config_store
        .save(next.normalized())
        .map_err(|e| e.to_string())?;

    if let Some(hist) = history {
        state
            .history_store
            .replace_all(&hist)
            .map_err(|e| e.to_string())?;
    }

    app.emit("app-config:changed", &saved)
        .map_err(|e| format!("无法广播配置变更: {e}"))?;

    Ok(saved)
}

// 供测试引用
#[allow(dead_code)]
fn _history_type_check(_: HistorySessionDto) {}
