use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// normalize 兜底用：target_lang 非法时回退简体中文（不读 OS，避免每次 save 查系统 locale）。
const FALLBACK_TARGET_LANG: &str = "zh-CN";

const TRANSLATION_LANGS: &[&str] = &[
    "zh-CN", "zh-TW", "en", "ja", "ko", "fr", "de", "es", "pt", "ru", "it", "nl",
    "pl", "tr", "ar", "th", "vi", "id", "hi",
];

/// 首次安装默认目标语言：读 OS locale 并映射到翻译语言 code，不在列表回退 zh-CN。
fn default_target_lang_from_os() -> String {
    target_lang_from_locale(sys_locale::get_locale())
}

fn target_lang_from_locale(locale: Option<String>) -> String {
    locale
        .map(|value| map_os_lang_to_translation(&value))
        .unwrap_or_else(|| FALLBACK_TARGET_LANG.to_string())
}

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "gpt-4o-mini";
const DEFAULT_TIMEOUT_SECONDS: u32 = 60;
const DEFAULT_CLAUDE_BASE_URL: &str = "https://api.anthropic.com";
const DEFAULT_EDGE_TRANSLATE_URL: &str = "https://edge.microsoft.com/translate/translatetext";
const DEFAULT_PROTOCOL: &str = "openai_chat";

fn default_true() -> bool {
    true
}

fn default_source_lang() -> String {
    "auto".to_string()
}

fn default_interface_language() -> String {
    "auto".to_string()
}

fn default_chain_of_thought() -> String {
    "off".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_update_channel() -> String {
    "stable".to_string()
}

fn normalize_update_channel(value: String) -> String {
    match value.trim() {
        "beta" => "beta".to_string(),
        _ => "stable".to_string(),
    }
}

fn default_popup_ui_backend() -> String {
    "webview".to_string()
}

fn normalize_popup_ui_backend(value: String) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "winui" => "winui".to_string(),
        _ => "webview".to_string(),
    }
}

fn default_history_limit() -> usize {
    500
}

/// 把 OS locale（如 `zh-CN`、`zh-Hans`、`en-GB`）映射到翻译语言 code。
/// 中文区分简繁体，其他语言按主语言映射，都不匹配回退 `zh-CN`。
fn map_os_lang_to_translation(os: &str) -> String {
    let normalized = os.to_lowercase().replace('_', "-");
    let mut segments = normalized.split('-');
    let main = segments.next().unwrap_or("");
    let mapped = match main {
        "zh" => {
            if segments.any(|segment| matches!(segment, "hant" | "tw" | "hk" | "mo")) {
                "zh-TW"
            } else {
                "zh-CN"
            }
        }
        "en" | "ja" | "ko" | "fr" | "de" | "es" | "pt" | "ru" | "it" | "nl"
        | "pl" | "tr" | "ar" | "th" | "vi" | "id" | "hi" => main,
        _ => "zh-CN",
    };
    mapped.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInstanceConfig {
    pub id: String,
    pub service_type: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub protocol: String,
    pub api_key: Option<String>,
    pub endpoint: String,
    pub model: String,
    pub timeout_seconds: u32,
    #[serde(default)]
    pub system_prompt: String,
    #[serde(default)]
    pub translation_prompt: String,
    #[serde(default)]
    pub reflection_prompt: String,
    #[serde(default)]
    pub reflection_enabled: bool,
    #[serde(default = "default_chain_of_thought")]
    pub chain_of_thought: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrServiceInstanceConfig {
    pub id: String,
    pub service_type: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub api_key: Option<String>,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub preferred_lang: String,
    #[serde(default)]
    pub ocr_prompt: String,
}

impl OcrServiceInstanceConfig {
    pub fn normalized(mut self) -> Self {
        self.id = self.id.trim().to_string();
        self.service_type = self.service_type.trim().to_string();
        self.name = self.name.trim().to_string();
        self.api_key = self.api_key.and_then(non_empty_string);
        self.endpoint = self.endpoint.trim().to_string();
        self.model = self.model.trim().to_string();
        self.preferred_lang = self.preferred_lang.trim().to_string();
        self.ocr_prompt = self.ocr_prompt.trim().to_string();
        self
    }
}

/// 磁盘为空时 seed 用的默认 Windows OCR 实例（固定 id 便于前端 merge）。
fn default_windows_ocr_service() -> OcrServiceInstanceConfig {
    OcrServiceInstanceConfig {
        id: "windows-media-ocr".into(),
        service_type: "windows-media-ocr".into(),
        name: "Windows 媒体 OCR".into(),
        enabled: true,
        api_key: None,
        endpoint: String::new(),
        model: String::new(),
        preferred_lang: String::new(),
        ocr_prompt: String::new(),
    }
}

/// OCR 服务列表归一化（规格 5.3）：
/// 1. 空 → seed 单条 Windows（enabled）
/// 2. 多个 enabled → 仅保留列表顺序第一个，其余关闭
/// 3. 零 enabled → 打开已有 Windows 行；若无则插入默认 Windows
/// 不删除视觉实例；不改 apiKey/model 等字段。
fn normalize_ocr_services(mut list: Vec<OcrServiceInstanceConfig>) -> Vec<OcrServiceInstanceConfig> {
    if list.is_empty() {
        return vec![default_windows_ocr_service()];
    }

    // 多 enabled → 只留列表顺序第一个
    let mut seen_enabled = false;
    for item in &mut list {
        if item.enabled {
            if seen_enabled {
                item.enabled = false;
            } else {
                seen_enabled = true;
            }
        }
    }

    // 零 enabled → 打开已有 windows-media-ocr，否则插入默认
    if !list.iter().any(|s| s.enabled) {
        if let Some(win) = list
            .iter_mut()
            .find(|s| s.service_type == "windows-media-ocr")
        {
            win.enabled = true;
        } else {
            list.insert(0, default_windows_ocr_service());
        }
    }

    list
}

/// 某一启动路径下：翻译弹窗 / 截图 overlay 是否在启动时预建 WebView。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WindowPrecreatePair {
    pub popup: bool,
    pub overlay: bool,
}

/// 按启动路径区分的窗口预创建策略（不在设置 UI 暴露）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WindowPrecreateConfig {
    pub manual: WindowPrecreatePair,
    pub autostart: WindowPrecreatePair,
}

impl Default for WindowPrecreateConfig {
    fn default() -> Self {
        Self {
            // 手动启动：翻译窗预建并展示；overlay 按需
            manual: WindowPrecreatePair {
                popup: true,
                overlay: false,
            },
            // 开机自启：默认都不预建
            autostart: WindowPrecreatePair {
                popup: false,
                overlay: false,
            },
        }
    }
}

impl WindowPrecreateConfig {
    pub fn for_launch(&self, autostart: bool) -> &WindowPrecreatePair {
        if autostart {
            &self.autostart
        } else {
            &self.manual
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(default)]
    pub shortcuts: HashMap<String, String>,
    #[serde(default)]
    pub services: Vec<ServiceInstanceConfig>,
    #[serde(default)]
    pub ocr_services: Vec<OcrServiceInstanceConfig>,
    pub target_lang: String,
    #[serde(default = "default_interface_language")]
    pub interface_language: String,
    #[serde(default = "default_source_lang")]
    pub default_source_lang: String,
    #[serde(default = "default_true")]
    pub auto_copy: bool,
    #[serde(default = "default_true")]
    pub restore_clipboard: bool,
    #[serde(default = "default_history_limit")]
    pub history_limit: usize,
    #[serde(default)]
    pub window_precreate: WindowPrecreateConfig,
    #[serde(default = "default_true")]
    pub collect_usage: bool,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default = "default_update_channel")]
    pub update_channel: String,
    #[serde(default = "default_true")]
    pub auto_check_update: bool,
    /// 登录 Windows 后自动启动（HKCU Run；默认关闭，用户显式开启）。
    #[serde(default)]
    pub launch_at_login: bool,
    /// 翻译弹窗 UI 后端：`webview`（默认）或 `winui`。
    #[serde(default = "default_popup_ui_backend")]
    pub popup_ui_backend: String,
}

impl ServiceInstanceConfig {
    pub fn normalized(mut self) -> Self {
        self.api_key = self.api_key.and_then(non_empty_string);
        // 机器翻译无模型概念；勿用 LLM 默认模型（gpt-4o-mini）回填，否则结果卡右下角会误显
        self.model = match self.protocol.as_str() {
            "microsoft_edge" => String::new(),
            _ => normalize_string(self.model, DEFAULT_MODEL),
        };
        if self.endpoint.trim().is_empty() {
            self.endpoint = match self.protocol.as_str() {
                "claude_messages" => DEFAULT_CLAUDE_BASE_URL.to_string(),
                "microsoft_edge" => DEFAULT_EDGE_TRANSLATE_URL.to_string(),
                _ => DEFAULT_BASE_URL.to_string(),
            };
        }
        if self.timeout_seconds == 0 {
            self.timeout_seconds = DEFAULT_TIMEOUT_SECONDS;
        }
        self.system_prompt = self.system_prompt.trim().to_string();
        self.translation_prompt = self.translation_prompt.trim().to_string();
        self.reflection_prompt = self.reflection_prompt.trim().to_string();
        self.chain_of_thought = normalize_chain_of_thought(self.chain_of_thought);
        self
    }
}

fn default_shortcuts() -> HashMap<String, String> {
    HashMap::from([
        ("translate-selection".to_string(), "Alt+D".to_string()),
        ("translate-screenshot".to_string(), "Alt+S".to_string()),
        ("translate-clipboard".to_string(), "Ctrl+Shift+C".to_string()),
        ("word-lookup".to_string(), String::new()),
        ("open-settings".to_string(), "Ctrl+,".to_string()),
        // 文字识别默认不注册全局快捷键；用户可在设置页自行绑定
        ("ocr-recognize".to_string(), String::new()),
    ])
}

fn normalize_shortcuts(mut shortcuts: HashMap<String, String>) -> HashMap<String, String> {
    let defaults = default_shortcuts();
    let mut normalized = HashMap::new();

    for (id, default_keys) in defaults {
        let keys = shortcuts
            .remove(&id)
            .map(|value| value.trim().to_string())
            .unwrap_or(default_keys);
        let keys = match (id.as_str(), keys.as_str()) {
            ("translate-selection", "Alt+T") => "Alt+D".to_string(),
            // 历史默认：Alt+O → Alt+E → Alt+S
            ("translate-screenshot", "Alt+O" | "Alt+E") => "Alt+S".to_string(),
            // 历史默认 Alt+O → 空（默认不设置）
            ("ocr-recognize", "Alt+O") => String::new(),
            _ => keys,
        };
        normalized.insert(id, keys);
    }

    normalized
}

impl AppConfig {
    /// 首次安装 / 配置缺失时的默认值。Key、endpoint、模型等均由设置页写入 config.json。
    pub fn default() -> Self {
        Self {
            shortcuts: default_shortcuts(),
            services: vec![ServiceInstanceConfig {
                id: "default".to_string(),
                service_type: "openai".to_string(),
                name: "默认服务".to_string(),
                enabled: true,
                protocol: DEFAULT_PROTOCOL.to_string(),
                api_key: None,
                endpoint: DEFAULT_BASE_URL.to_string(),
                model: DEFAULT_MODEL.to_string(),
                timeout_seconds: DEFAULT_TIMEOUT_SECONDS,
                system_prompt: String::new(),
                translation_prompt: String::new(),
                reflection_prompt: String::new(),
                reflection_enabled: false,
                chain_of_thought: default_chain_of_thought(),
            }],
            ocr_services: vec![],
            target_lang: default_target_lang_from_os(),
            interface_language: default_interface_language(),
            default_source_lang: default_source_lang(),
            auto_copy: true,
            restore_clipboard: true,
            history_limit: default_history_limit(),
            window_precreate: WindowPrecreateConfig::default(),
            collect_usage: true,
            log_level: default_log_level(),
            update_channel: default_update_channel(),
            auto_check_update: true,
            launch_at_login: false,
            popup_ui_backend: default_popup_ui_backend(),
        }
        .normalized()
    }

    pub fn normalized(mut self) -> Self {
        self.shortcuts = normalize_shortcuts(self.shortcuts);
        self.services = self.services.into_iter().map(|s| s.normalized()).collect();
        self.ocr_services = self
            .ocr_services
            .into_iter()
            .map(|s| s.normalized())
            .collect();
        self.ocr_services = normalize_ocr_services(self.ocr_services);
        if !TRANSLATION_LANGS.contains(&self.target_lang.as_str()) {
            self.target_lang = FALLBACK_TARGET_LANG.to_string();
        }
        if self.default_source_lang != "auto"
            && !TRANSLATION_LANGS.contains(&self.default_source_lang.as_str())
        {
            self.default_source_lang = default_source_lang();
        }
        if self.history_limit == 0 {
            self.history_limit = default_history_limit();
        }
        self.log_level = normalize_log_level(self.log_level);
        self.update_channel = normalize_update_channel(self.update_channel);
        self.popup_ui_backend = normalize_popup_ui_backend(self.popup_ui_backend);
        self
    }

    #[cfg(test)]
    pub fn is_configured(&self) -> bool {
        self.services.iter().any(|s| {
            if !s.enabled {
                return false;
            }
            match s.protocol.as_str() {
                "mock" | "microsoft_edge" => true,
                _ => s.api_key.is_some(),
            }
        })
    }
}

fn normalize_string(value: String, default_value: &str) -> String {
    non_empty_string(value).unwrap_or_else(|| default_value.to_string())
}

fn non_empty_string(value: String) -> Option<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn normalize_chain_of_thought(value: String) -> String {
    match value.trim() {
        "short" | "medium" | "long" => value.trim().to_string(),
        _ => "off".to_string(),
    }
}

fn normalize_log_level(value: String) -> String {
    match value.trim() {
        "error" | "warn" | "info" | "debug" => value.trim().to_string(),
        _ => "info".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interface_language_defaults_to_auto_and_serializes_camel_case() {
        let config = AppConfig::default();
        assert_eq!(config.interface_language, "auto");
        let json = serde_json::to_value(config).unwrap();
        assert_eq!(json["interfaceLanguage"], "auto");
    }

    #[test]
    fn normalized_rejects_old_translation_codes_without_aliasing() {
        let mut config = AppConfig::default();
        config.default_source_lang = "en-US".into();
        config.target_lang = "ja-JP".into();
        let normalized = config.normalized();
        assert_eq!(normalized.default_source_lang, "auto");
        assert_eq!(normalized.target_lang, "zh-CN");
    }

    #[test]
    fn normalized_keeps_valid_translation_codes() {
        let mut config = AppConfig::default();
        config.default_source_lang = "en".into();
        config.target_lang = "ja".into();
        let normalized = config.normalized();
        assert_eq!(normalized.default_source_lang, "en");
        assert_eq!(normalized.target_lang, "ja");
    }

    #[test]
    fn translation_locale_mapping_uses_new_codes() {
        assert_eq!(map_os_lang_to_translation("en-GB"), "en");
        assert_eq!(map_os_lang_to_translation("pt-BR"), "pt");
        assert_eq!(map_os_lang_to_translation("zh-Hant-HK"), "zh-TW");
        assert_eq!(map_os_lang_to_translation("xx-YY"), "zh-CN");
        assert_eq!(map_os_lang_to_translation("ZH_mO"), "zh-TW");
    }

    #[test]
    fn missing_os_locale_uses_target_fallback() {
        assert_eq!(target_lang_from_locale(None), "zh-CN");
    }

    #[test]
    fn default_creates_default_service() {
        let config = AppConfig::default();
        assert_eq!(config.services.len(), 1);
        assert_eq!(config.services[0].id, "default");
        assert!(config.services[0].enabled);
    }

    #[test]
    fn default_launch_at_login_false_and_missing_field_deserializes() {
        let config = AppConfig::default();
        assert!(!config.launch_at_login);
        let json = r#"{"targetLang":"zh-CN"}"#;
        let parsed: AppConfig = serde_json::from_str(json).unwrap();
        assert!(!parsed.launch_at_login);
        let roundtrip = serde_json::to_value(config).unwrap();
        assert_eq!(roundtrip["launchAtLogin"], false);
    }

    #[test]
    fn normalized_fills_empty_service_fields() {
        let svc = ServiceInstanceConfig {
            id: "test".to_string(),
            service_type: "llm".to_string(),
            name: "测试".to_string(),
            enabled: true,
            protocol: "openai_chat".to_string(),
            api_key: Some("   ".to_string()),
            endpoint: "".to_string(),
            model: "".to_string(),
            timeout_seconds: 0,
            system_prompt: String::new(),
            translation_prompt: String::new(),
            reflection_prompt: String::new(),
            reflection_enabled: false,
            chain_of_thought: default_chain_of_thought(),
        }.normalized();
        assert!(svc.api_key.is_none());
        assert_eq!(svc.endpoint, DEFAULT_BASE_URL);
        assert_eq!(svc.model, DEFAULT_MODEL);
        assert_eq!(svc.timeout_seconds, DEFAULT_TIMEOUT_SECONDS);
    }

    #[test]
    fn normalized_fills_ui_runtime_defaults() {
        let mut config = AppConfig::default();
        config.default_source_lang = "".to_string();
        config.auto_copy = false;
        config.restore_clipboard = false;
        config.services[0].system_prompt = "  ".to_string();
        config.services[0].translation_prompt = "  ".to_string();
        config.services[0].chain_of_thought = "bad".to_string();

        let normalized = config.normalized();

        assert_eq!(normalized.default_source_lang, "auto");
        assert!(!normalized.auto_copy);
        assert!(!normalized.restore_clipboard);
        assert_eq!(normalized.services[0].system_prompt, "");
        assert_eq!(normalized.services[0].translation_prompt, "");
        assert_eq!(normalized.services[0].chain_of_thought, "off");
    }

    #[test]
    fn normalized_fills_empty_history_limit() {
        let mut config = AppConfig::default();
        config.history_limit = 0;

        let normalized = config.normalized();

        assert_eq!(normalized.history_limit, 500);
    }

    #[test]
    fn is_configured_true_with_enabled_service_and_key() {
        let mut config = AppConfig::default();
        config.services[0].api_key = Some("sk-test".to_string());
        assert!(config.is_configured());
    }

    #[test]
    fn is_configured_false_without_key() {
        let config = AppConfig::default();
        assert!(!config.is_configured());
    }

    #[test]
    fn is_configured_true_with_mock_protocol() {
        let mut config = AppConfig::default();
        config.services[0].protocol = "mock".to_string();
        config.services[0].api_key = None;
        assert!(config.is_configured());
    }

    #[test]
    fn is_configured_true_with_microsoft_edge_no_key() {
        let mut config = AppConfig::default();
        config.services[0].protocol = "microsoft_edge".to_string();
        config.services[0].api_key = None;
        config.services[0].model = String::new();
        assert!(config.is_configured());
    }

    #[test]
    fn is_configured_true_with_second_service() {
        let mut config = AppConfig::default();
        config.services[0].api_key = None;
        config.services.push(ServiceInstanceConfig {
            id: "svc-2".to_string(),
            service_type: "llm".to_string(),
            name: "副服务".to_string(),
            enabled: true,
            protocol: "openai_chat".to_string(),
            api_key: Some("sk-2".to_string()),
            endpoint: "https://api.example.com".to_string(),
            model: "gpt-4".to_string(),
            timeout_seconds: 30,
            system_prompt: String::new(),
            translation_prompt: String::new(),
            reflection_prompt: String::new(),
            reflection_enabled: false,
            chain_of_thought: default_chain_of_thought(),
        });
        assert!(config.is_configured());
    }

    #[test]
    fn is_configured_false_when_only_disabled_service_has_key() {
        let mut config = AppConfig::default();
        config.services[0].api_key = None;
        config.services.push(ServiceInstanceConfig {
            id: "disabled".to_string(),
            service_type: "llm".to_string(),
            name: "已禁用".to_string(),
            enabled: false,
            protocol: "openai_chat".to_string(),
            api_key: Some("sk-disabled".to_string()),
            endpoint: "https://api.example.com".to_string(),
            model: "gpt-4".to_string(),
            timeout_seconds: 30,
            system_prompt: String::new(),
            translation_prompt: String::new(),
            reflection_prompt: String::new(),
            reflection_enabled: false,
            chain_of_thought: default_chain_of_thought(),
        });
        assert!(!config.is_configured());
    }

    #[test]
    fn default_target_lang_uses_os_or_fallback() {
        let config = AppConfig::default();
        assert!(
            TRANSLATION_LANGS.contains(&config.target_lang.as_str()),
            "default target_lang 应是 OS 映射结果（列表 code 之一），实际: {}",
            config.target_lang
        );
    }

    #[test]
    fn map_os_lang_exact_match() {
        assert_eq!(map_os_lang_to_translation("zh-CN"), "zh-CN");
        assert_eq!(map_os_lang_to_translation("en-US"), "en");
        assert_eq!(map_os_lang_to_translation("fr-FR"), "fr");
    }

    #[test]
    fn map_os_lang_zh_variants() {
        assert_eq!(map_os_lang_to_translation("zh-Hans"), "zh-CN");
        assert_eq!(map_os_lang_to_translation("zh-SG"), "zh-CN");
        assert_eq!(map_os_lang_to_translation("zh-Hant"), "zh-TW");
        assert_eq!(map_os_lang_to_translation("zh-HK"), "zh-TW");
        assert_eq!(map_os_lang_to_translation("zh-TW"), "zh-TW");
    }

    #[test]
    fn map_os_lang_main_prefix() {
        assert_eq!(map_os_lang_to_translation("en-GB"), "en");
        assert_eq!(map_os_lang_to_translation("ja-JP"), "ja");
        assert_eq!(map_os_lang_to_translation("ko-KR"), "ko");
        assert_eq!(map_os_lang_to_translation("de-DE"), "de");
        assert_eq!(map_os_lang_to_translation("es-ES"), "es");
        assert_eq!(map_os_lang_to_translation("ru-RU"), "ru");
    }

    #[test]
    fn map_os_lang_unmapped_falls_back_to_zh_cn() {
        assert_eq!(map_os_lang_to_translation("th-TH"), "th");
        assert_eq!(map_os_lang_to_translation("xx-YY"), "zh-CN");
        assert_eq!(map_os_lang_to_translation(""), "zh-CN");
    }

    #[test]
    fn serializes_camel_case() {
        let config = AppConfig::default();
        let json = serde_json::to_string(&config).expect("序列化");
        assert!(json.contains("\"targetLang\""), "应输出 camelCase: {json}");
        assert!(json.contains("\"windowPrecreate\""), "应输出 camelCase: {json}");
        assert!(json.contains("\"collectUsage\""), "应输出 camelCase: {json}");
        assert!(json.contains("\"serviceType\""), "应输出 camelCase: {json}");
        assert!(json.contains("\"timeoutSeconds\""), "应输出 camelCase: {json}");
        assert!(json.contains("\"apiKey\""), "应输出 camelCase: {json}");
    }

    #[test]
    fn deserializes_with_defaults() {
        let json = r#"{
            "targetLang": "zh-CN"
        }"#;
        let config: AppConfig = serde_json::from_str::<AppConfig>(json)
            .expect("缺少字段应可反序列化")
            .normalized();
        assert_eq!(config.target_lang, "zh-CN");
        assert_eq!(
            config.window_precreate,
            WindowPrecreateConfig::default()
        );
        assert!(config.collect_usage);
        assert!(config.services.is_empty());
        assert!(!config.is_configured());
    }

    #[test]
    fn deserializes_services_array() {
        let json = r#"{
            "targetLang": "zh-CN",
            "services": [
                {
                    "id": "svc-1",
                    "serviceType": "llm",
                    "name": "OpenAI",
                    "enabled": true,
                    "protocol": "openai_chat",
                    "apiKey": "sk-test",
                    "endpoint": "https://api.openai.com/v1",
                    "model": "gpt-4o-mini",
                    "timeoutSeconds": 60
                }
            ]
        }"#;
        let config: AppConfig = serde_json::from_str::<AppConfig>(json)
            .expect("services 数组应可反序列化");
        assert_eq!(config.services.len(), 1);
        assert_eq!(config.services[0].id, "svc-1");
        assert_eq!(config.services[0].api_key.as_deref(), Some("sk-test"));
        assert!(config.is_configured());
    }

    #[test]
    fn service_instance_config_serializes_camel_case() {
        let svc = ServiceInstanceConfig {
            id: "s1".to_string(),
            service_type: "llm".to_string(),
            name: "测试".to_string(),
            enabled: true,
            protocol: "openai_chat".to_string(),
            api_key: Some("sk-xxx".to_string()),
            endpoint: "https://test.example.com".to_string(),
            model: "gpt-4".to_string(),
            timeout_seconds: 30,
            system_prompt: String::new(),
            translation_prompt: String::new(),
            reflection_prompt: String::new(),
            reflection_enabled: false,
            chain_of_thought: default_chain_of_thought(),
        };
        let json = serde_json::to_string(&svc).expect("序列化");
        assert!(json.contains("\"serviceType\""), "应输出 serviceType: {json}");
        assert!(json.contains("\"apiKey\""), "应输出 apiKey: {json}");
        assert!(json.contains("\"timeoutSeconds\""), "应输出 timeoutSeconds: {json}");
    }

    #[test]
    fn defaults_window_precreate_by_launch_mode() {
        let config = AppConfig::default();
        assert!(config.window_precreate.manual.popup);
        assert!(!config.window_precreate.manual.overlay);
        assert!(!config.window_precreate.autostart.popup);
        assert!(!config.window_precreate.autostart.overlay);
        assert!(config.window_precreate.for_launch(false).popup);
        assert!(!config.window_precreate.for_launch(true).popup);
    }

    #[test]
    fn defaults_collect_usage_true() {
        let config = AppConfig::default();
        assert!(config.collect_usage);
    }

    #[test]
    fn default_protocol_is_openai_chat() {
        let config = AppConfig::default();
        assert_eq!(config.services[0].protocol, "openai_chat");
        assert_eq!(config.services[0].service_type, "openai");
    }

    #[test]
    fn normalized_claude_messages_uses_claude_base_url() {
        let svc = ServiceInstanceConfig {
            id: "test".to_string(),
            service_type: "claude".to_string(),
            name: "Claude".to_string(),
            enabled: true,
            protocol: "claude_messages".to_string(),
            api_key: None,
            endpoint: "".to_string(),
            model: "".to_string(),
            timeout_seconds: 0,
            system_prompt: String::new(),
            translation_prompt: String::new(),
            reflection_prompt: String::new(),
            reflection_enabled: false,
            chain_of_thought: default_chain_of_thought(),
        }
        .normalized();
        assert_eq!(svc.endpoint, DEFAULT_CLAUDE_BASE_URL);
    }

    #[test]
    fn normalized_fills_edge_url_for_microsoft_edge() {
        let svc = ServiceInstanceConfig {
            id: "ms".to_string(),
            service_type: "microsoft".to_string(),
            name: "微软翻译".to_string(),
            enabled: true,
            protocol: "microsoft_edge".to_string(),
            api_key: None,
            endpoint: "".to_string(),
            model: String::new(),
            timeout_seconds: 0,
            system_prompt: String::new(),
            translation_prompt: String::new(),
            reflection_prompt: String::new(),
            reflection_enabled: false,
            chain_of_thought: default_chain_of_thought(),
        }
        .normalized();
        assert_eq!(svc.endpoint, "https://edge.microsoft.com/translate/translatetext");
        assert!(svc.api_key.is_none());
        assert_eq!(svc.model, "", "微软翻译不应回填 LLM 默认模型");
    }

    #[test]
    fn normalized_clears_stale_model_for_microsoft_edge() {
        let svc = ServiceInstanceConfig {
            id: "ms".to_string(),
            service_type: "microsoft".to_string(),
            name: "微软翻译".to_string(),
            enabled: true,
            protocol: "microsoft_edge".to_string(),
            api_key: None,
            endpoint: DEFAULT_EDGE_TRANSLATE_URL.to_string(),
            model: DEFAULT_MODEL.to_string(), // 旧配置误写入的 LLM 默认模型
            timeout_seconds: 60,
            system_prompt: String::new(),
            translation_prompt: String::new(),
            reflection_prompt: String::new(),
            reflection_enabled: false,
            chain_of_thought: default_chain_of_thought(),
        }
        .normalized();
        assert_eq!(svc.model, "");
    }

    #[test]
    fn defaults_shortcuts_use_bob_style_keys() {
        let config = AppConfig::default();

        assert_eq!(config.shortcuts.get("translate-selection").map(String::as_str), Some("Alt+D"));
        assert_eq!(config.shortcuts.get("translate-screenshot").map(String::as_str), Some("Alt+S"));
        assert_eq!(
            config.shortcuts.get("translate-clipboard").map(String::as_str),
            Some("Ctrl+Shift+C")
        );
        assert_eq!(
            config.shortcuts.get("word-lookup").map(String::as_str), Some("")
        );
        assert!(!config.shortcuts.contains_key("show-window"));
        assert_eq!(
            config.shortcuts.get("open-settings").map(String::as_str), Some("Ctrl+,")
        );
    }

    #[test]
    fn default_shortcuts_ocr_recognize_is_empty() {
        let config = AppConfig::default();
        assert_eq!(
            config.shortcuts.get("ocr-recognize").map(String::as_str),
            Some("")
        );
    }

    #[test]
    fn normalize_migrates_ocr_recognize_alt_o_to_empty_while_migrating_screenshot() {
        let mut config = AppConfig::default();
        config.shortcuts.insert("translate-screenshot".into(), "Alt+O".into());
        config.shortcuts.insert("ocr-recognize".into(), "Alt+O".into());
        let n = config.normalized();
        assert_eq!(n.shortcuts.get("translate-screenshot").unwrap(), "Alt+S");
        assert_eq!(n.shortcuts.get("ocr-recognize").unwrap(), "");
    }

    #[test]
    fn normalize_keeps_custom_ocr_recognize_shortcut() {
        let mut config = AppConfig::default();
        config.shortcuts.insert("ocr-recognize".into(), "Ctrl+Alt+O".into());
        let n = config.normalized();
        assert_eq!(n.shortcuts.get("ocr-recognize").unwrap(), "Ctrl+Alt+O");
    }

    #[test]
    fn normalized_migrates_old_default_shortcuts() {
        let mut config = AppConfig::default();
        config.shortcuts.insert("translate-selection".to_string(), "Alt+T".to_string());
        config.shortcuts.insert("translate-screenshot".to_string(), "Alt+O".to_string());
        config.shortcuts.insert("ocr-recognize".to_string(), "Alt+O".to_string());

        let config = config.normalized();

        assert_eq!(config.shortcuts.get("translate-selection").map(String::as_str), Some("Alt+D"));
        assert_eq!(config.shortcuts.get("translate-screenshot").map(String::as_str), Some("Alt+S"));
        assert_eq!(config.shortcuts.get("ocr-recognize").map(String::as_str), Some(""));

        let mut config = AppConfig::default();
        config.shortcuts.insert("translate-screenshot".to_string(), "Alt+E".to_string());
        let config = config.normalized();
        assert_eq!(config.shortcuts.get("translate-screenshot").map(String::as_str), Some("Alt+S"));
    }

    #[test]
    fn normalized_keeps_custom_shortcuts_and_empty_disabled_bindings() {
        let mut config = AppConfig::default();
        config.shortcuts.insert("translate-selection".to_string(), "Ctrl+Alt+T".to_string());
        config.shortcuts.insert("translate-screenshot".to_string(), "".to_string());

        let config = config.normalized();

        assert_eq!(
            config.shortcuts.get("translate-selection").map(String::as_str),
            Some("Ctrl+Alt+T")
        );
        assert_eq!(config.shortcuts.get("translate-screenshot").map(String::as_str), Some(""));
    }

    #[test]
    fn normalized_log_level_falls_back_to_info_for_invalid() {
        let mut config = AppConfig::default();
        config.log_level = "trace".to_string();
        let normalized = config.normalized();
        assert_eq!(normalized.log_level, "info");
    }

    #[test]
    fn normalized_log_level_keeps_valid_values() {
        for level in ["error", "warn", "info", "debug"] {
            let mut config = AppConfig::default();
            config.log_level = level.to_string();
            assert_eq!(config.normalized().log_level, level);
        }
    }

    #[test]
    fn default_log_level_is_info() {
        let config = AppConfig::default();
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn serializes_log_level_camel_case() {
        let config = AppConfig::default();
        let json = serde_json::to_string(&config).expect("序列化");
        assert!(json.contains("\"logLevel\""), "应输出 logLevel: {json}");
    }

    #[test]
    fn deserializes_log_level_with_default() {
        let json = r#"{ "targetLang": "zh-CN" }"#;
        let config: AppConfig = serde_json::from_str::<AppConfig>(json)
            .expect("缺少字段应可反序列化")
            .normalized();
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn ocr_services_default_seeds_windows_and_missing_field_deserializes_empty() {
        let config = AppConfig::default();
        assert_eq!(config.ocr_services.len(), 1);
        assert_eq!(config.ocr_services[0].service_type, "windows-media-ocr");

        let json = r#"{"targetLang":"zh-CN","services":[]}"#;
        let parsed: AppConfig = serde_json::from_str(json).expect("parse");
        assert!(parsed.ocr_services.is_empty()); // 未 normalized
        assert_eq!(parsed.normalized().ocr_services.len(), 1);
    }

    #[test]
    fn ocr_services_roundtrip_camel_case() {
        let mut config = AppConfig::default();
        config.ocr_services = vec![OcrServiceInstanceConfig {
            id: "ocr-win".into(),
            service_type: "windows-media-ocr".into(),
            name: "Windows 媒体 OCR".into(),
            enabled: true,
            api_key: None,
            endpoint: String::new(),
            model: String::new(),
            preferred_lang: String::new(),
            ocr_prompt: String::new(),
        }];
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("ocrServices"));
        assert!(json.contains("serviceType"));
        assert!(json.contains("preferredLang"));
        assert!(json.contains("ocrPrompt"));
        let back: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.ocr_services.len(), 1);
        assert_eq!(back.ocr_services[0].service_type, "windows-media-ocr");
        assert!(back.ocr_services[0].enabled);
    }

    #[test]
    fn normalized_trims_ocr_service_fields() {
        let mut config = AppConfig::default();
        config.ocr_services = vec![OcrServiceInstanceConfig {
            id: "  ocr-1  ".into(),
            service_type: "openai-vision".into(),
            name: "  V  ".into(),
            enabled: true,
            api_key: Some("  sk  ".into()),
            endpoint: "  https://api.openai.com/v1  ".into(),
            model: "  gpt-4o  ".into(),
            preferred_lang: "  en  ".into(),
            ocr_prompt: "  hello  ".into(),
        }];
        let n = config.normalized();
        assert_eq!(n.ocr_services[0].id, "ocr-1");
        assert_eq!(n.ocr_services[0].name, "V");
        assert_eq!(n.ocr_services[0].api_key.as_deref(), Some("sk"));
        assert_eq!(n.ocr_services[0].endpoint, "https://api.openai.com/v1");
        assert_eq!(n.ocr_services[0].model, "gpt-4o");
        assert_eq!(n.ocr_services[0].preferred_lang, "en");
        assert_eq!(n.ocr_services[0].ocr_prompt, "hello");
    }

    #[test]
    fn normalized_seeds_windows_ocr_when_empty() {
        let config = AppConfig::default(); // 走 normalized
        assert_eq!(config.ocr_services.len(), 1);
        assert_eq!(config.ocr_services[0].service_type, "windows-media-ocr");
        assert!(config.ocr_services[0].enabled);
    }

    #[test]
    fn normalized_enables_windows_when_all_ocr_disabled() {
        let mut config = AppConfig::default();
        config.ocr_services = vec![
            OcrServiceInstanceConfig {
                id: "win".into(),
                service_type: "windows-media-ocr".into(),
                name: "W".into(),
                enabled: false,
                api_key: None,
                endpoint: String::new(),
                model: String::new(),
                preferred_lang: String::new(),
                ocr_prompt: String::new(),
            },
            OcrServiceInstanceConfig {
                id: "v".into(),
                service_type: "openai-vision".into(),
                name: "V".into(),
                enabled: false,
                api_key: Some("sk".into()),
                endpoint: "https://api.openai.com/v1".into(),
                model: "gpt-4o".into(),
                preferred_lang: String::new(),
                ocr_prompt: String::new(),
            },
        ];
        let n = config.normalized();
        assert!(n.ocr_services.iter().find(|s| s.id == "win").unwrap().enabled);
        assert!(!n.ocr_services.iter().find(|s| s.id == "v").unwrap().enabled);
    }

    #[test]
    fn normalized_keeps_only_first_enabled_ocr() {
        let mut config = AppConfig::default();
        config.ocr_services = vec![
            OcrServiceInstanceConfig {
                id: "v1".into(),
                service_type: "openai-vision".into(),
                name: "V1".into(),
                enabled: true,
                api_key: Some("sk".into()),
                endpoint: "https://a".into(),
                model: "m".into(),
                preferred_lang: String::new(),
                ocr_prompt: String::new(),
            },
            OcrServiceInstanceConfig {
                id: "win".into(),
                service_type: "windows-media-ocr".into(),
                name: "W".into(),
                enabled: true,
                api_key: None,
                endpoint: String::new(),
                model: String::new(),
                preferred_lang: String::new(),
                ocr_prompt: String::new(),
            },
        ];
        let n = config.normalized();
        assert!(n.ocr_services[0].enabled);
        assert!(!n.ocr_services[1].enabled);
    }

    #[test]
    fn normalized_inserts_windows_when_all_disabled_and_no_windows_row() {
        let mut config = AppConfig::default();
        config.ocr_services = vec![OcrServiceInstanceConfig {
            id: "v".into(),
            service_type: "openai-vision".into(),
            name: "V".into(),
            enabled: false,
            api_key: None,
            endpoint: "https://a".into(),
            model: "m".into(),
            preferred_lang: String::new(),
            ocr_prompt: String::new(),
        }];
        let n = config.normalized();
        assert!(n
            .ocr_services
            .iter()
            .any(|s| s.service_type == "windows-media-ocr" && s.enabled));
    }

    #[test]
    fn app_config_defaults_update_fields() {
        let config = AppConfig::default();
        assert_eq!(config.update_channel, "stable");
        assert!(config.auto_check_update);
    }

    #[test]
    fn app_config_missing_update_fields_deserialize_to_defaults() {
        let json = r#"{
            "targetLang": "zh-CN",
            "services": [],
            "ocrServices": []
        }"#;
        let config: AppConfig = serde_json::from_str(json).expect("deserialize");
        let config = config.normalized();
        assert_eq!(config.update_channel, "stable");
        assert!(config.auto_check_update);
    }

    #[test]
    fn app_config_normalized_rejects_invalid_update_channel() {
        let mut config = AppConfig::default();
        config.update_channel = "nightly".into();
        let config = config.normalized();
        assert_eq!(config.update_channel, "stable");
    }

    #[test]
    fn app_config_defaults_popup_ui_backend_webview() {
        let config = AppConfig::default();
        assert_eq!(config.popup_ui_backend, "webview");
    }

    #[test]
    fn app_config_missing_popup_ui_backend_deserializes_to_webview() {
        let json = r#"{"targetLang":"zh-CN","services":[],"ocrServices":[]}"#;
        let config: AppConfig = serde_json::from_str(json).expect("deserialize");
        let config = config.normalized();
        assert_eq!(config.popup_ui_backend, "webview");
    }

    #[test]
    fn app_config_popup_ui_backend_roundtrip_camel_case() {
        let mut config = AppConfig::default();
        config.popup_ui_backend = "winui".into();
        let json = serde_json::to_string(&config).expect("ser");
        assert!(json.contains("\"popupUiBackend\":\"winui\""), "got {json}");
        let back: AppConfig = serde_json::from_str(&json).expect("de");
        assert_eq!(back.popup_ui_backend, "winui");
    }

    #[test]
    fn normalized_rejects_unknown_popup_ui_backend() {
        let mut config = AppConfig::default();
        config.popup_ui_backend = "qt".into();
        let n = config.normalized();
        assert_eq!(n.popup_ui_backend, "webview");
    }
}
