use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{ErrorKind, Read},
    path::Path,
};

const MAX_PACK_SIZE: u64 = 1_048_576;
const BUILTINS: [(&str, &str); 8] = [
    (
        "zh-CN",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../frontend/src/i18n/locales/zh-CN.json"
        )),
    ),
    (
        "zh-TW",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../frontend/src/i18n/locales/zh-TW.json"
        )),
    ),
    (
        "en-US",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../frontend/src/i18n/locales/en-US.json"
        )),
    ),
    (
        "ja-JP",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../frontend/src/i18n/locales/ja-JP.json"
        )),
    ),
    (
        "ko-KR",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../frontend/src/i18n/locales/ko-KR.json"
        )),
    ),
    (
        "fr-FR",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../frontend/src/i18n/locales/fr-FR.json"
        )),
    ),
    (
        "de-DE",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../frontend/src/i18n/locales/de-DE.json"
        )),
    ),
    (
        "es-ES",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../frontend/src/i18n/locales/es-ES.json"
        )),
    ),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguagePack {
    pub schema_version: u32,
    pub locale: String,
    pub name: String,
    pub messages: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageSnapshot {
    pub configured_locale: String,
    pub locale: String,
    pub revision: u64,
    pub languages: Vec<LanguageMeta>,
    pub user_messages: HashMap<String, String>,
    pub errors: Vec<LanguagePackError>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageMeta {
    pub locale: String,
    pub name: String,
    pub builtin: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguagePackError {
    pub file: String,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct LanguagePackScan {
    pub languages: Vec<LanguageMeta>,
    pub user_messages: HashMap<String, String>,
    pub errors: Vec<LanguagePackError>,
}

pub fn scan_language_packs(dir: &Path, current_locale: Option<&str>) -> LanguagePackScan {
    let mut scan = LanguagePackScan {
        languages: builtin_metadata(),
        ..Default::default()
    };
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return scan,
        Err(error) => {
            scan.errors.push(LanguagePackError {
                file: dir.to_string_lossy().into_owned(),
                message: format!("无法读取语言包目录: {error}"),
            });
            return scan;
        }
    };
    let allowed = builtin_pack("zh-CN").messages;
    let mut files = Vec::new();

    for entry in entries {
        match entry {
            Ok(entry) => files.push(entry),
            Err(error) => scan.errors.push(LanguagePackError {
                file: dir.to_string_lossy().into_owned(),
                message: format!("无法枚举语言包目录: {error}"),
            }),
        }
    }
    files.sort_by_key(|entry| entry.file_name());

    for entry in files {
        let path = entry.path();
        if path.extension().and_then(|v| v.to_str()) != Some("json") {
            continue;
        }
        let file = entry.file_name().to_string_lossy().into_owned();
        let result = validate_pack(&path, &allowed);
        match result {
            Ok(pack) => {
                if !scan
                    .languages
                    .iter()
                    .any(|meta| meta.locale.eq_ignore_ascii_case(&pack.locale))
                {
                    scan.languages.push(LanguageMeta {
                        locale: pack.locale.clone(),
                        name: pack.name.clone(),
                        builtin: false,
                    });
                }
                if current_locale.is_some_and(|locale| locale.eq_ignore_ascii_case(&pack.locale)) {
                    scan.user_messages = pack.messages;
                }
            }
            Err(message) => scan.errors.push(LanguagePackError { file, message }),
        }
    }
    scan
}

fn validate_pack(path: &Path, allowed: &HashMap<String, String>) -> Result<LanguagePack, String> {
    let file = File::open(path).map_err(|e| format!("无法打开语言包: {e}"))?;
    let size = file
        .metadata()
        .map_err(|e| format!("无法读取文件信息: {e}"))?
        .len();
    if size > MAX_PACK_SIZE {
        return Err("语言包超过 1 MiB 限制".into());
    }
    let mut bytes = Vec::with_capacity((size + 1) as usize);
    file.take(MAX_PACK_SIZE + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("无法读取语言包: {e}"))?;
    if bytes.len() as u64 > MAX_PACK_SIZE {
        return Err("语言包超过 1 MiB 限制".into());
    }
    let text = String::from_utf8(bytes).map_err(|_| "语言包不是有效 UTF-8".to_string())?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("JSON 格式无效: {e}"))?;
    if value.get("schemaVersion").and_then(|v| v.as_u64()) != Some(1) {
        return Err("schemaVersion 必须为 1".into());
    }
    if value
        .get("name")
        .and_then(|v| v.as_str())
        .is_none_or(|v| v.trim().is_empty())
    {
        return Err("name 不能为空".into());
    }
    let messages = value
        .get("messages")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "messages 必须是对象".to_string())?;
    if messages.values().any(|value| !value.is_string()) {
        return Err("messages 的值必须是字符串".into());
    }
    if let Some(key) = messages.keys().find(|key| !allowed.contains_key(*key)) {
        return Err(format!("messages 包含未知键: {key}"));
    }
    let locale = value
        .get("locale")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if !is_valid_locale(locale) {
        return Err("locale 不是合法 BCP47 标识".into());
    }
    if path.file_stem().and_then(|v| v.to_str()) != Some(locale) {
        return Err("文件名 locale 与包内 locale 不一致".into());
    }
    serde_json::from_value(value).map_err(|e| format!("语言包字段无效: {e}"))
}

pub fn resolve_locale(
    configured: &str,
    os_locale: Option<&str>,
    scan: &LanguagePackScan,
) -> String {
    if configured != "auto" {
        return canonical_available(configured, scan).unwrap_or_else(|| "zh-CN".into());
    }
    let Some(locale) = os_locale.filter(|locale| is_valid_locale(locale)) else {
        return "zh-CN".into();
    };
    if let Some(canonical) = canonical_available(locale, scan) {
        return canonical;
    }
    let parts: Vec<_> = locale.split('-').collect();
    match parts[0].to_ascii_lowercase().as_str() {
        "zh" if parts.iter().skip(1).any(|part| {
            matches!(
                part.to_ascii_lowercase().as_str(),
                "hant" | "tw" | "hk" | "mo"
            )
        }) =>
        {
            "zh-TW"
        }
        "zh" => "zh-CN",
        "en" => "en-US",
        "ja" => "ja-JP",
        "ko" => "ko-KR",
        "fr" => "fr-FR",
        "de" => "de-DE",
        "es" => "es-ES",
        _ => "zh-CN",
    }
    .to_string()
}

pub fn resolve_messages(locale: &str, scan: &LanguagePackScan) -> HashMap<String, String> {
    let fallback = builtin_pack("zh-CN").messages;
    let same_locale = if BUILTINS.iter().any(|(candidate, _)| *candidate == locale) {
        builtin_pack(locale).messages
    } else {
        HashMap::new()
    };
    merge_messages(&scan.user_messages, &same_locale, &fallback)
}

pub fn merge_messages(
    user: &HashMap<String, String>,
    same_locale: &HashMap<String, String>,
    fallback: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut messages = fallback.clone();
    messages.extend(same_locale.clone());
    messages.extend(user.clone());
    messages
}

pub fn builtin_metadata() -> Vec<LanguageMeta> {
    BUILTINS
        .iter()
        .map(|(locale, _)| {
            let pack = builtin_pack(locale);
            LanguageMeta {
                locale: pack.locale,
                name: pack.name,
                builtin: true,
            }
        })
        .collect()
}

fn builtin_pack(locale: &str) -> LanguagePack {
    let json = BUILTINS
        .iter()
        .find(|(candidate, _)| *candidate == locale)
        .map(|(_, json)| *json)
        .expect("内置 locale 必须存在");
    serde_json::from_str(json).expect("内置语言包必须有效")
}

fn canonical_available(locale: &str, scan: &LanguagePackScan) -> Option<String> {
    if !is_valid_locale(locale) {
        return None;
    }
    scan.languages
        .iter()
        .find(|meta| meta.locale.eq_ignore_ascii_case(locale))
        .map(|meta| meta.locale.clone())
}

fn is_valid_locale(locale: &str) -> bool {
    let mut parts = locale.split('-');
    let Some(first) = parts.next() else {
        return false;
    };
    (2..=3).contains(&first.len())
        && first.bytes().all(|byte| byte.is_ascii_alphabetic())
        && parts.all(|part| {
            (2..=8).contains(&part.len()) && part.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_pack(dir: &TempDir, file: &str, json: &str) {
        fs::write(dir.path().join(file), json).expect("写入测试语言包");
    }

    fn valid_pack(locale: &str, messages: &str) -> String {
        format!(r#"{{"schemaVersion":1,"locale":"{locale}","name":"Test","messages":{messages}}}"#)
    }

    #[test]
    fn resolve_locale_maps_os_locale_and_preserves_available_user_locale() {
        let dir = TempDir::new().unwrap();
        write_pack(
            &dir,
            "it-IT.json",
            &valid_pack("it-IT", r#"{"tray.quit":"Esci"}"#),
        );
        let scan = scan_language_packs(dir.path(), None);

        assert_eq!(resolve_locale("auto", Some("zh-Hant-HK"), &scan), "zh-TW");
        assert_eq!(resolve_locale("auto", Some("en-GB"), &scan), "en-US");
        assert_eq!(resolve_locale("auto", Some("it-IT"), &scan), "it-IT");
        assert_eq!(resolve_locale("auto", Some("xx-XX"), &scan), "zh-CN");
        assert_eq!(resolve_locale("not_valid", Some("en-US"), &scan), "zh-CN");
        assert_eq!(resolve_locale("ja-JP", None, &scan), "ja-JP");
        assert_eq!(resolve_locale("it-IT", None, &scan), "it-IT");
    }

    #[test]
    fn partial_user_pack_overrides_then_falls_back_to_same_locale_builtin() {
        let dir = TempDir::new().unwrap();
        write_pack(
            &dir,
            "en-US.json",
            &valid_pack("en-US", r#"{"tray.quit":"Exit now"}"#),
        );
        let scan = scan_language_packs(dir.path(), Some("en-US"));
        let messages = resolve_messages("en-US", &scan);

        assert_eq!(messages["tray.quit"], "Exit now");
        assert_eq!(messages["tray.settings"], "Settings");
    }

    #[test]
    fn invalid_packs_report_precise_errors() {
        let cases = [
            ("wrong.json", valid_pack("fr-FR", r#"{"tray.quit":"x"}"#), "文件名 locale 与包内 locale 不一致"),
            ("bad_locale.json", valid_pack("bad_locale", r#"{"tray.quit":"x"}"#), "locale 不是合法 BCP47 标识"),
            ("schema.json", r#"{"schemaVersion":2,"locale":"fr-FR","name":"x","messages":{}}"#.to_string(), "schemaVersion 必须为 1"),
            ("empty.json", r#"{"schemaVersion":1,"locale":"fr-FR","name":"  ","messages":{}}"#.to_string(), "name 不能为空"),
            ("nested.json", r#"{"schemaVersion":1,"locale":"fr-FR","name":"x","messages":{"tray.quit":{"x":"y"}}}"#.to_string(), "messages 的值必须是字符串"),
            ("number.json", r#"{"schemaVersion":1,"locale":"fr-FR","name":"x","messages":{"tray.quit":1}}"#.to_string(), "messages 的值必须是字符串"),
            ("unknown.json", valid_pack("fr-FR", r#"{"unknown.key":"x"}"#), "messages 包含未知键: unknown.key"),
        ];

        for (file, json, expected) in cases {
            let dir = TempDir::new().unwrap();
            write_pack(&dir, file, &json);
            let scan = scan_language_packs(dir.path(), None);
            assert_eq!(scan.errors.len(), 1, "{file}");
            assert_eq!(scan.errors[0].file, file);
            assert_eq!(scan.errors[0].message, expected, "{file}");
        }
    }

    #[test]
    fn oversized_pack_is_rejected_before_reading() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("large.json"), vec![b'x'; 1_048_577]).unwrap();
        let scan = scan_language_packs(dir.path(), None);
        assert_eq!(scan.errors[0].message, "语言包超过 1 MiB 限制");
    }

    #[test]
    fn rescanning_after_override_deletion_restores_builtin_message() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("en-US.json");
        fs::write(&path, valid_pack("en-US", r#"{"tray.quit":"Exit now"}"#)).unwrap();
        assert_eq!(
            resolve_messages("en-US", &scan_language_packs(dir.path(), Some("en-US")))["tray.quit"],
            "Exit now"
        );

        fs::remove_file(path).unwrap();
        assert_eq!(
            resolve_messages("en-US", &scan_language_packs(dir.path(), Some("en-US")))["tray.quit"],
            "Quit"
        );
    }

    #[test]
    fn snapshot_and_metadata_use_camel_case_contract() {
        let snapshot = LanguageSnapshot {
            configured_locale: "auto".into(),
            locale: "zh-CN".into(),
            revision: 1,
            languages: vec![LanguageMeta {
                locale: "zh-CN".into(),
                name: "简体中文".into(),
                builtin: true,
            }],
            user_messages: Default::default(),
            errors: vec![LanguagePackError {
                file: "x.json".into(),
                message: "x".into(),
            }],
        };
        let value = serde_json::to_value(snapshot).unwrap();
        assert!(value.get("configuredLocale").is_some());
        assert_eq!(value["languages"][0]["builtin"], true);
    }

    #[test]
    fn locale_segments_match_declared_pattern() {
        assert!(is_valid_locale("en-US"));
        assert!(!is_valid_locale("en-x"));
    }

    #[test]
    fn scan_results_follow_file_name_order() {
        let dir = TempDir::new().unwrap();
        write_pack(&dir, "z-bad.json", "not json");
        write_pack(&dir, "a-bad.json", "not json");
        write_pack(
            &dir,
            "it-IT.json",
            &valid_pack("it-IT", r#"{"tray.quit":"Esci"}"#),
        );
        write_pack(
            &dir,
            "nl-NL.json",
            &valid_pack("nl-NL", r#"{"tray.quit":"Afsluiten"}"#),
        );

        let scan = scan_language_packs(dir.path(), None);
        assert_eq!(
            scan.errors
                .iter()
                .map(|error| error.file.as_str())
                .collect::<Vec<_>>(),
            vec!["a-bad.json", "z-bad.json"]
        );
        assert_eq!(
            scan.languages
                .iter()
                .filter(|meta| !meta.builtin)
                .map(|meta| meta.locale.as_str())
                .collect::<Vec<_>>(),
            vec!["it-IT", "nl-NL"]
        );
    }

    #[test]
    fn non_directory_scan_path_reports_error() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("packs");
        fs::write(&file, "x").unwrap();

        let scan = scan_language_packs(&file, None);
        assert_eq!(scan.errors.len(), 1);
        assert!(scan.errors[0].message.starts_with("无法读取语言包目录:"));
    }

    #[test]
    fn resolve_locale_returns_canonical_available_locale() {
        let dir = TempDir::new().unwrap();
        write_pack(
            &dir,
            "it-IT.json",
            &valid_pack("it-IT", r#"{"tray.quit":"Esci"}"#),
        );
        let scan = scan_language_packs(dir.path(), None);

        assert_eq!(resolve_locale("en-us", None, &scan), "en-US");
        assert_eq!(resolve_locale("IT-it", None, &scan), "it-IT");
    }

    #[test]
    fn builtin_user_override_matches_locale_case_insensitively() {
        let dir = TempDir::new().unwrap();
        write_pack(
            &dir,
            "en-us.json",
            &valid_pack("en-us", r#"{"tray.quit":"Exit now"}"#),
        );

        let scan = scan_language_packs(dir.path(), Some("en-US"));
        assert_eq!(
            scan.languages
                .iter()
                .filter(|meta| meta.locale.eq_ignore_ascii_case("en-US"))
                .count(),
            1
        );
        assert_eq!(scan.user_messages["tray.quit"], "Exit now");
        assert_eq!(resolve_locale("en-us", None, &scan), "en-US");
    }
}
