//! 前端日志 command：直接 std::fs append 写 frontend.log，不走 log facade，
//! 与后端 Shizi.log 物理隔离。

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::app::logging::logs_dir;
use crate::app::state::AppState;

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
}
