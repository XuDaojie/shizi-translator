//! 前端日志 command：直接 std::fs append 写 frontend.log，不走 log facade，
//! 与后端 Shizi.log 物理隔离。

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::app::logging::logs_dir;
use crate::app::state::AppState;
use crate::core::config::AppConfig;
use crate::core::logging::redact_api_key;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendLogEntry {
    pub level: String,
    pub message: String,
    pub timestamp: String,
    pub source: String,
    #[serde(default)]
    pub meta: Option<serde_json::Value>,
}

const FRONTEND_LOG_MAX_SIZE: u64 = 5_000_000;

/// 追加一行到 frontend.log，超 5MB 时轮转。best-effort：IO 失败返回 Err。
pub fn append_frontend_log(path: &Path, line: &str, max_size: u64) -> std::io::Result<()> {
    if path.exists() {
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.len() + line.len() as u64 + 1 > max_size {
                rotate_logs(path);
            }
        }
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

fn rotate_logs(path: &Path) {
    // 找当前最大编号
    let mut max_n = 0;
    let mut n = 1;
    loop {
        if rotated_path(path, n).exists() {
            max_n = n;
            n += 1;
        } else {
            break;
        }
    }
    // 从高到低重命名：.max_n → .max_n+1 ... .1 → .2
    for i in (1..=max_n).rev() {
        let from = rotated_path(path, i);
        let to = rotated_path(path, i + 1);
        let _ = std::fs::rename(&from, &to);
    }
    // 当前文件 → .1
    let _ = std::fs::rename(path, rotated_path(path, 1));
}

fn rotated_path(path: &Path, n: u32) -> PathBuf {
    PathBuf::from(format!("{}.{}", path.display(), n))
}

/// 按 config 当前 level 过滤前端 entry（双保险，前端已过滤）。
pub fn should_log(entry_level: &str, filter: log::LevelFilter) -> bool {
    let level = match entry_level {
        "error" => log::Level::Error,
        "warn" => log::Level::Warn,
        "info" => log::Level::Info,
        "debug" => log::Level::Debug,
        _ => return false,
    };
    level <= filter
}

#[tauri::command]
pub async fn write_frontend_log(
    entries: Vec<FrontendLogEntry>,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let config = state.config_store.get().map_err(|e| e.to_string())?;
    let filter = crate::app::logging::parse_level_filter(&config.log_level);
    let dir = logs_dir(&app).ok_or_else(|| "无法解析日志目录".to_string())?;
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("frontend.log");
    for entry in entries {
        if !should_log(&entry.level, filter) {
            continue;
        }
        let line = format!(
            "{} [{}] {} {}",
            entry.timestamp,
            entry.level.to_uppercase(),
            entry.source,
            entry.message
        );
        // best-effort：单条失败跳过，不崩。
        let _ = append_frontend_log(&path, &line, FRONTEND_LOG_MAX_SIZE);
    }
    Ok(())
}

/// 把 AppConfig 序列化为 JSON，每个 service 的 apiKey 用 redact_api_key 脱敏。
pub fn config_snapshot_json(config: &AppConfig) -> String {
    let mut value = match serde_json::to_value(config) {
        Ok(v) => v,
        Err(_) => return "{}".to_string(),
    };
    if let Some(services) = value.get_mut("services").and_then(|s| s.as_array_mut()) {
        for svc in services {
            if let Some(key) = svc.get("apiKey").and_then(|k| k.as_str()) {
                svc["apiKey"] = serde_json::Value::String(redact_api_key(key));
            }
        }
    }
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
}

/// 生成 system-info.txt 内容。
pub fn system_info(config: &AppConfig) -> String {
    let now = chrono::Local::now().to_rfc3339();
    format!(
        "Shizi log export\nVersion: {}\nOS: {}\nExport time: {}\nLog level: {}\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        now,
        config.log_level
    )
}

/// 把 `log_dir` 下所有 `*.log*` + config snapshot + system info 打包到 `zip_path`。
pub fn write_export_zip(
    zip_path: &Path,
    log_dir: &Path,
    config: &AppConfig,
) -> std::io::Result<()> {
    let file = std::fs::File::create(zip_path)?;
    let mut zip = ZipWriter::new(file);
    let opts = SimpleFileOptions::default();

    if let Ok(entries) = std::fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if !(name.ends_with(".log") || name.contains(".log.")) {
                continue;
            }
            zip.start_file(&name, opts)?;
            let bytes = std::fs::read(&path)?;
            zip.write_all(&bytes)?;
        }
    }

    zip.start_file("config-snapshot.json", opts)?;
    zip.write_all(config_snapshot_json(config).as_bytes())?;

    zip.start_file("system-info.txt", opts)?;
    zip.write_all(system_info(config).as_bytes())?;

    zip.finish()?;
    Ok(())
}

#[tauri::command]
pub async fn export_logs(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;

    let save_path = app
        .dialog()
        .file()
        .add_filter("ZIP", &["zip"])
        .set_file_name(format!("shizi-logs-{}.zip", chrono::Local::now().format("%Y%m%d-%H%M%S")))
        .blocking_save_file()
        .ok_or_else(|| "用户取消导出".to_string())?
        .into_path()
        .map_err(|_| "无效的保存路径".to_string())?;

    let config = state.config_store.get().map_err(|e| e.to_string())?;
    let dir = logs_dir(&app).ok_or_else(|| "无法解析日志目录".to_string())?;

    write_export_zip(&save_path, &dir, &config).map_err(|e| e.to_string())?;

    Ok(save_path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn append_frontend_log_writes_lines() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("frontend.log");
        append_frontend_log(&path, "hello", 5_000_000).unwrap();
        append_frontend_log(&path, "world", 5_000_000).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "hello\nworld\n");
    }

    #[test]
    fn append_rotates_when_exceeding_max_size() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("frontend.log");
        // 用小 max_size 快速验证：95 字节 + 换行 = 96，再写 "overflow"(8+1=9) → 96+9=105 > 100 触发轮转
        let big = "x".repeat(95);
        append_frontend_log(&path, &big, 100).unwrap();
        append_frontend_log(&path, "overflow", 100).unwrap();
        assert!(rotated_path(&path, 1).exists(), "应产生 frontend.log.1");
        let current = fs::read_to_string(&path).unwrap();
        assert_eq!(current, "overflow\n");
        let rotated = fs::read_to_string(rotated_path(&path, 1)).unwrap();
        assert!(rotated.contains(&big));
    }

    #[test]
    fn append_rotation_shifts_existing_backups() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("frontend.log");
        // 预置 .1
        fs::write(rotated_path(&path, 1), "old1").unwrap();
        let big = "x".repeat(95);
        append_frontend_log(&path, &big, 100).unwrap();
        append_frontend_log(&path, "overflow", 100).unwrap();
        // 原 .1 应被推到 .2
        assert!(rotated_path(&path, 2).exists());
        assert_eq!(fs::read_to_string(rotated_path(&path, 2)).unwrap(), "old1");
    }

    #[test]
    fn should_log_filters_by_level() {
        assert!(should_log("error", log::LevelFilter::Error));
        assert!(should_log("error", log::LevelFilter::Info));
        assert!(!should_log("debug", log::LevelFilter::Info));
        assert!(!should_log("info", log::LevelFilter::Warn));
        assert!(!should_log("unknown", log::LevelFilter::Debug));
    }

    #[test]
    fn config_snapshot_redacts_api_keys() {
        use crate::core::config::AppConfig;
        let mut config = AppConfig::default();
        config.services[0].api_key = Some("sk-abcdef12345678".to_string());
        let json = config_snapshot_json(&config);
        assert!(json.contains("sk-a...5678"), "apiKey 应脱敏: {json}");
        assert!(!json.contains("abcdef12345678"), "原始 key 不应出现: {json}");
    }

    #[test]
    fn config_snapshot_preserves_other_fields() {
        use crate::core::config::AppConfig;
        let config = AppConfig::default();
        let json = config_snapshot_json(&config);
        assert!(json.contains("\"targetLang\""));
        assert!(json.contains("\"logLevel\""));
    }

    #[test]
    fn system_info_includes_version_os_and_level() {
        use crate::core::config::AppConfig;
        let config = AppConfig::default();
        let info = system_info(&config);
        assert!(info.contains("Version:"));
        assert!(info.contains("OS:"));
        assert!(info.contains("Log level:"));
        assert!(info.contains(&config.log_level));
    }

    #[test]
    fn export_zip_bundles_log_files_and_snapshot() {
        use crate::core::config::AppConfig;
        use std::io::{Read, Seek, SeekFrom};
        use zip::ZipArchive;

        let dir = tempdir().unwrap();
        // 造两个日志文件
        fs::write(dir.path().join("Shizi.log"), "backend line\n").unwrap();
        fs::write(dir.path().join("frontend.log"), "frontend line\n").unwrap();
        fs::write(dir.path().join("config.json"), "{}").unwrap(); // 非日志，应被忽略

        let zip_path = dir.path().join("export.zip");
        let config = AppConfig::default();
        write_export_zip(&zip_path, dir.path(), &config).unwrap();

        let mut archive = ZipArchive::new(fs::File::open(&zip_path).unwrap()).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.iter().any(|n| n == "Shizi.log"), "{names:?}");
        assert!(names.iter().any(|n| n == "frontend.log"), "{names:?}");
        assert!(names.iter().any(|n| n == "config-snapshot.json"), "{names:?}");
        assert!(names.iter().any(|n| n == "system-info.txt"), "{names:?}");
        assert!(!names.iter().any(|n| n == "config.json"), "非日志文件不应入包");
    }
}
