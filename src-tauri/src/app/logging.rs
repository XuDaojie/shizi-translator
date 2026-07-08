//! 日志装配：目录解析、等级解析、插件初始化、旧日志清理。

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use tauri::Manager;

/// 日志目录：`app_config_dir()/logs/`。
pub fn logs_dir(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("logs"))
}

/// 字符串等级 → `log::LevelFilter`，非法值回退 `Info`。
pub fn parse_level_filter(level: &str) -> log::LevelFilter {
    match level.trim() {
        "error" => log::LevelFilter::Error,
        "warn" => log::LevelFilter::Warn,
        "info" => log::LevelFilter::Info,
        "debug" => log::LevelFilter::Debug,
        _ => log::LevelFilter::Info,
    }
}

/// 启动时清理 `dir` 下 mtime 早于 `days` 天的 `*.log*` 文件。best-effort，失败静默。
pub fn cleanup_old_logs(dir: &Path, days: u64) {
    let cutoff = SystemTime::now() - Duration::from_secs(days * 86400);
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !is_log_file(&path) {
            continue;
        }
        let mtime = match entry.metadata().and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(_) => continue,
        };
        if mtime < cutoff {
            let _ = std::fs::remove_file(&path);
        }
    }
}

fn is_log_file(path: &Path) -> bool {
    let name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return false,
    };
    // 匹配 Shizi.log / frontend.log / Shizi.log.1 / frontend.log.2 等
    name.ends_with(".log") || name.contains(".log.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::SystemTime;
    use tempfile::tempdir;

    #[test]
    fn cleanup_removes_files_older_than_threshold() {
        let dir = tempdir().unwrap();
        let old = dir.path().join("Shizi.log.5");
        fs::write(&old, "old").unwrap();
        // 把 mtime 设为 10 天前
        let ten_days_ago = SystemTime::now() - std::time::Duration::from_secs(10 * 86400);
        let _ = filetime::set_file_mtime(&old, filetime::FileTime::from_system_time(ten_days_ago)).ok();

        let recent = dir.path().join("frontend.log");
        fs::write(&recent, "recent").unwrap();

        cleanup_old_logs(dir.path(), 7);

        assert!(!old.exists(), "旧文件应被删除");
        assert!(recent.exists(), "新文件应保留");
    }

    #[test]
    fn cleanup_ignores_non_log_files() {
        let dir = tempdir().unwrap();
        let config = dir.path().join("config.json");
        fs::write(&config, "{}").unwrap();
        let ten_days_ago = SystemTime::now() - std::time::Duration::from_secs(10 * 86400);
        let _ = filetime::set_file_mtime(&config, filetime::FileTime::from_system_time(ten_days_ago)).ok();

        cleanup_old_logs(dir.path(), 7);

        assert!(config.exists(), "非日志文件不应被清理");
    }

    #[test]
    fn cleanup_swallows_errors_on_missing_dir() {
        // 目录不存在不应 panic
        cleanup_old_logs(Path::new("/nonexistent/path/that/does/not/exist"), 7);
    }

    #[test]
    fn parse_level_filter_maps_known_levels() {
        assert_eq!(parse_level_filter("error"), log::LevelFilter::Error);
        assert_eq!(parse_level_filter("warn"), log::LevelFilter::Warn);
        assert_eq!(parse_level_filter("info"), log::LevelFilter::Info);
        assert_eq!(parse_level_filter("debug"), log::LevelFilter::Debug);
    }

    #[test]
    fn parse_level_filter_falls_back_to_info() {
        assert_eq!(parse_level_filter("trace"), log::LevelFilter::Info);
        assert_eq!(parse_level_filter(""), log::LevelFilter::Info);
        assert_eq!(parse_level_filter("garbage"), log::LevelFilter::Info);
    }
}
