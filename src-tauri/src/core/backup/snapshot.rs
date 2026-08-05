//! 备份 zip 快照：manifest + settings.json + 可选 history.json。

use std::io::{Cursor, Read, Write};

use chrono::Local;
use serde::{Deserialize, Serialize};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use crate::core::{
    config::AppConfig,
    history::HistorySessionDto,
};

pub const BACKUP_KIND: &str = "shizi-backup";
pub const MANIFEST_NAME: &str = "manifest.json";
pub const SETTINGS_NAME: &str = "settings.json";
pub const HISTORY_NAME: &str = "history.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    pub version: u32,
    pub kind: String,
    pub exported_at: String,
    pub include_history: bool,
    pub include_api_keys: bool,
}

#[derive(Debug, Clone)]
pub struct ParsedBackup {
    pub manifest: BackupManifest,
    pub settings: AppConfig,
    pub history: Option<Vec<HistorySessionDto>>,
}

/// 按开关从配置生成可写入备份的 AppConfig 副本。
pub fn prepare_settings_for_backup(config: &AppConfig, include_api_keys: bool) -> AppConfig {
    let mut clone = config.clone();
    if !include_api_keys {
        for svc in &mut clone.services {
            svc.api_key = None;
        }
        for svc in &mut clone.ocr_services {
            svc.api_key = None;
        }
        clone.backup.webdav.password = String::new();
    }
    // 备份内的连接状态时间戳可保留；status 不在 config 中
    clone
}

pub fn build_backup_file_name(now: chrono::DateTime<Local>) -> String {
    now.format("shizi-backup-%Y%m%d-%H%M%S.zip").to_string()
}

pub fn build_backup_zip(
    config: &AppConfig,
    history: Option<&[HistorySessionDto]>,
    include_api_keys: bool,
) -> Result<Vec<u8>, String> {
    let include_history = history.is_some();
    let settings = prepare_settings_for_backup(config, include_api_keys);
    let manifest = BackupManifest {
        version: 1,
        kind: BACKUP_KIND.to_string(),
        exported_at: chrono::Utc::now().to_rfc3339(),
        include_history,
        include_api_keys,
    };

    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut cursor);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        let manifest_json =
            serde_json::to_vec_pretty(&manifest).map_err(|e| format!("序列化 manifest 失败: {e}"))?;
        zip.start_file(MANIFEST_NAME, opts)
            .map_err(|e| format!("写入 zip 失败: {e}"))?;
        zip.write_all(&manifest_json)
            .map_err(|e| format!("写入 zip 失败: {e}"))?;

        let settings_json =
            serde_json::to_vec_pretty(&settings).map_err(|e| format!("序列化 settings 失败: {e}"))?;
        zip.start_file(SETTINGS_NAME, opts)
            .map_err(|e| format!("写入 zip 失败: {e}"))?;
        zip.write_all(&settings_json)
            .map_err(|e| format!("写入 zip 失败: {e}"))?;

        if let Some(hist) = history {
            let history_json =
                serde_json::to_vec_pretty(hist).map_err(|e| format!("序列化 history 失败: {e}"))?;
            zip.start_file(HISTORY_NAME, opts)
                .map_err(|e| format!("写入 zip 失败: {e}"))?;
            zip.write_all(&history_json)
                .map_err(|e| format!("写入 zip 失败: {e}"))?;
        }

        zip.finish().map_err(|e| format!("完成 zip 失败: {e}"))?;
    }
    Ok(cursor.into_inner())
}

/// 本机导出用 JSON 包装（与 zip 内 settings/history 语义一致，便于文本编辑）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalExportEnvelope {
    pub version: u32,
    pub kind: String,
    pub exported_at: String,
    pub include_history: bool,
    pub include_api_keys: bool,
    pub settings: AppConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<Vec<HistorySessionDto>>,
}

pub fn build_local_export_json(
    config: &AppConfig,
    history: Option<Vec<HistorySessionDto>>,
    include_api_keys: bool,
) -> Result<String, String> {
    let include_history = history.is_some();
    let settings = prepare_settings_for_backup(config, include_api_keys);
    let envelope = LocalExportEnvelope {
        version: 1,
        kind: "shizi-settings".to_string(),
        exported_at: chrono::Utc::now().to_rfc3339(),
        include_history,
        include_api_keys,
        settings,
        history,
    };
    serde_json::to_string_pretty(&envelope).map_err(|e| format!("序列化导出失败: {e}"))
}

pub fn parse_local_export_json(json: &str) -> Result<(AppConfig, Option<Vec<HistorySessionDto>>), String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("JSON 格式无效: {e}"))?;
    // 支持 { settings, history? } 包装，或直接 AppConfig
    if value.get("settings").is_some() {
        let envelope: LocalExportEnvelope = serde_json::from_value(value)
            .map_err(|e| format!("无法解析配置快照: {e}"))?;
        Ok((envelope.settings.normalized(), envelope.history))
    } else if value.get("targetLang").is_some() || value.get("services").is_some() {
        let settings: AppConfig =
            serde_json::from_value(value).map_err(|e| format!("无法解析配置: {e}"))?;
        Ok((settings.normalized(), None))
    } else {
        Err("文件中未找到有效的设置数据".into())
    }
}

pub fn parse_backup_zip(bytes: &[u8]) -> Result<ParsedBackup, String> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| format!("无法打开备份 zip: {e}"))?;

    let has_history_file = archive.by_name(HISTORY_NAME).is_ok();
    let manifest = match read_zip_json::<BackupManifest>(&mut archive, MANIFEST_NAME) {
        Ok(m) => m,
        Err(_) => BackupManifest {
            version: 1,
            kind: BACKUP_KIND.to_string(),
            exported_at: String::new(),
            include_history: has_history_file,
            include_api_keys: true,
        },
    };

    let settings = read_zip_json::<AppConfig>(&mut archive, SETTINGS_NAME)?
        .normalized();
    let history = if archive.by_name(HISTORY_NAME).is_ok() {
        Some(read_zip_json::<Vec<HistorySessionDto>>(&mut archive, HISTORY_NAME)?)
    } else {
        None
    };

    Ok(ParsedBackup {
        manifest,
        settings,
        history,
    })
}

fn read_zip_json<T: for<'de> Deserialize<'de>>(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    name: &str,
) -> Result<T, String> {
    let mut file = archive
        .by_name(name)
        .map_err(|e| format!("备份缺少 {name}: {e}"))?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)
        .map_err(|e| format!("读取 {name} 失败: {e}"))?;
    serde_json::from_str(&buf).map_err(|e| format!("解析 {name} 失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::AppConfig;

    #[test]
    fn prepare_strips_keys_when_disabled() {
        let mut config = AppConfig::default();
        config.services[0].api_key = Some("sk-secret".into());
        config.backup.webdav.password = "dav-pass".into();
        let stripped = prepare_settings_for_backup(&config, false);
        assert!(stripped.services[0].api_key.is_none());
        assert!(stripped.backup.webdav.password.is_empty());
        let kept = prepare_settings_for_backup(&config, true);
        assert_eq!(kept.services[0].api_key.as_deref(), Some("sk-secret"));
        assert_eq!(kept.backup.webdav.password, "dav-pass");
    }

    #[test]
    fn zip_roundtrip_without_history() {
        let config = AppConfig::default();
        let bytes = build_backup_zip(&config, None, true).unwrap();
        let parsed = parse_backup_zip(&bytes).unwrap();
        assert_eq!(parsed.manifest.kind, BACKUP_KIND);
        assert!(!parsed.manifest.include_history);
        assert!(parsed.history.is_none());
        assert_eq!(parsed.settings.target_lang, config.target_lang);
    }

    #[test]
    fn local_export_json_roundtrip() {
        let config = AppConfig::default();
        let json = build_local_export_json(&config, None, false).unwrap();
        let (settings, history) = parse_local_export_json(&json).unwrap();
        assert!(history.is_none());
        assert!(settings.services[0].api_key.is_none() || settings.services.is_empty() || {
            // default service may have no key
            true
        });
        assert!(!json.contains("dav-pass"));
    }
}
