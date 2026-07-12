use std::collections::HashMap;
use std::env;

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
const DEFAULT_CLAUDE_MODEL: &str = "claude-haiku-4-5";
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
pub struct AppConfig {
    #[serde(default)]
    pub shortcuts: HashMap<String, String>,
    #[serde(default)]
    pub services: Vec<ServiceInstanceConfig>,
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
    #[serde(default = "default_true")]
    pub popup_precreate: bool,
    #[serde(default = "default_true")]
    pub overlay_precreate: bool,
    #[serde(default = "default_true")]
    pub collect_usage: bool,
    #[serde(default = "default_log_level")]
    pub log_level: String,
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
        (
            "translate-selection".to_string(),
            env::var("SHIZI_SHORTCUT_TRANSLATE_SELECTION")
                .unwrap_or_else(|_| "Alt+D".to_string()),
        ),
        (
            "translate-screenshot".to_string(),
            env::var("SHIZI_SHORTCUT_TRANSLATE_SCREENSHOT")
                .unwrap_or_else(|_| "Alt+S".to_string()),
        ),
        (
            "translate-clipboard".to_string(),
            env::var("SHIZI_SHORTCUT_TRANSLATE_CLIPBOARD")
                .unwrap_or_else(|_| "Ctrl+Shift+C".to_string()),
        ),
        (
            "word-lookup".to_string(),
            env::var("SHIZI_SHORTCUT_WORD_LOOKUP").unwrap_or_else(|_| "".to_string()),
        ),
        (
            "open-settings".to_string(),
            env::var("SHIZI_SHORTCUT_OPEN_SETTINGS").unwrap_or_else(|_| "Ctrl+,".to_string()),
        ),
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
            _ => keys,
        };
        normalized.insert(id, keys);
    }

    normalized
}

impl AppConfig {
    pub fn from_env() -> Self {
        let protocol = env::var("SHIZI_LLM_PROVIDER")
            .unwrap_or_else(|_| DEFAULT_PROTOCOL.to_string());

        let (api_key, endpoint, model) = match protocol.as_str() {
            "claude_messages" => (
                env::var("SHIZI_CLAUDE_API_KEY").ok(),
                env::var("SHIZI_CLAUDE_BASE_URL")
                    .unwrap_or_else(|_| DEFAULT_CLAUDE_BASE_URL.to_string()),
                env::var("SHIZI_CLAUDE_MODEL")
                    .unwrap_or_else(|_| DEFAULT_CLAUDE_MODEL.to_string()),
            ),
            _ => (
                env::var("SHIZI_OPENAI_API_KEY").ok(),
                env::var("SHIZI_OPENAI_BASE_URL")
                    .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string()),
                env::var("SHIZI_OPENAI_MODEL")
                    .unwrap_or_else(|_| DEFAULT_MODEL.to_string()),
            ),
        };

        let name = match protocol.as_str() {
            "claude_messages" => "默认 Claude 服务".to_string(),
            "mock" => "Mock 服务".to_string(),
            _ => "默认服务".to_string(),
        };

        Self {
            shortcuts: default_shortcuts(),
            services: vec![ServiceInstanceConfig {
                id: "default".to_string(),
                service_type: "openai".to_string(),
                name,
                enabled: true,
                protocol,
                api_key,
                endpoint,
                model,
                timeout_seconds: DEFAULT_TIMEOUT_SECONDS,
                system_prompt: String::new(),
                translation_prompt: String::new(),
                reflection_prompt: String::new(),
                reflection_enabled: false,
                chain_of_thought: default_chain_of_thought(),
            }],
            target_lang: env::var("SHIZI_TARGET_LANG")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(default_target_lang_from_os),
            interface_language: default_interface_language(),
            default_source_lang: default_source_lang(),
            auto_copy: true,
            restore_clipboard: true,
            history_limit: default_history_limit(),
            popup_precreate: true,
            overlay_precreate: true,
            collect_usage: env::var("SHIZI_COLLECT_USAGE")
                .map(|v| v.eq_ignore_ascii_case("true"))
                .unwrap_or(true),
            log_level: default_log_level(),
        }
        .normalized()
    }

    pub fn normalized(mut self) -> Self {
        self.shortcuts = normalize_shortcuts(self.shortcuts);
        self.services = self.services.into_iter().map(|s| s.normalized()).collect();
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
        let config = AppConfig::from_env();
        assert_eq!(config.interface_language, "auto");
        let json = serde_json::to_value(config).unwrap();
        assert_eq!(json["interfaceLanguage"], "auto");
    }

    #[test]
    fn normalized_rejects_old_translation_codes_without_aliasing() {
        let mut config = AppConfig::from_env();
        config.default_source_lang = "en-US".into();
        config.target_lang = "ja-JP".into();
        let normalized = config.normalized();
        assert_eq!(normalized.default_source_lang, "auto");
        assert_eq!(normalized.target_lang, "zh-CN");
    }

    #[test]
    fn normalized_keeps_valid_translation_codes() {
        let mut config = AppConfig::from_env();
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
    fn from_env_creates_default_service() {
        let config = AppConfig::from_env();
        assert_eq!(config.services.len(), 1);
        assert_eq!(config.services[0].id, "default");
        assert!(config.services[0].enabled);
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
        let mut config = AppConfig::from_env();
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
        let mut config = AppConfig::from_env();
        config.history_limit = 0;

        let normalized = config.normalized();

        assert_eq!(normalized.history_limit, 500);
    }

    #[test]
    fn is_configured_true_with_enabled_service_and_key() {
        let mut config = AppConfig::from_env();
        config.services[0].api_key = Some("sk-test".to_string());
        assert!(config.is_configured());
    }

    #[test]
    fn is_configured_false_without_key() {
        let config = AppConfig::from_env();
        assert!(!config.is_configured());
    }

    #[test]
    fn is_configured_true_with_mock_protocol() {
        let mut config = AppConfig::from_env();
        config.services[0].protocol = "mock".to_string();
        config.services[0].api_key = None;
        assert!(config.is_configured());
    }

    #[test]
    fn is_configured_true_with_microsoft_edge_no_key() {
        let mut config = AppConfig::from_env();
        config.services[0].protocol = "microsoft_edge".to_string();
        config.services[0].api_key = None;
        config.services[0].model = String::new();
        assert!(config.is_configured());
    }

    #[test]
    fn is_configured_true_with_second_service() {
        let mut config = AppConfig::from_env();
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
        let mut config = AppConfig::from_env();
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
    fn from_env_target_lang_uses_os_or_fallback() {
        let config = AppConfig::from_env();
        assert!(
            TRANSLATION_LANGS.contains(&config.target_lang.as_str()),
            "from_env target_lang 应是 OS 映射结果（列表 code 之一），实际: {}",
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
        let config = AppConfig::from_env();
        let json = serde_json::to_string(&config).expect("序列化");
        assert!(json.contains("\"targetLang\""), "应输出 camelCase: {json}");
        assert!(json.contains("\"popupPrecreate\""), "应输出 camelCase: {json}");
        assert!(json.contains("\"overlayPrecreate\""), "应输出 camelCase: {json}");
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
        assert!(config.popup_precreate);
        assert!(config.overlay_precreate);
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
    fn defaults_precreate_window_strategies() {
        let config = AppConfig::from_env();
        assert!(config.popup_precreate);
        assert!(config.overlay_precreate);
    }

    #[test]
    fn defaults_collect_usage_true() {
        let config = AppConfig::from_env();
        assert!(config.collect_usage);
    }

    #[test]
    fn from_env_default_protocol_is_openai_chat() {
        let config = AppConfig::from_env();
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
        let config = AppConfig::from_env();

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
    fn normalized_migrates_old_default_shortcuts() {
        let mut config = AppConfig::from_env();
        config.shortcuts.insert("translate-selection".to_string(), "Alt+T".to_string());
        config.shortcuts.insert("translate-screenshot".to_string(), "Alt+O".to_string());

        let config = config.normalized();

        assert_eq!(config.shortcuts.get("translate-selection").map(String::as_str), Some("Alt+D"));
        assert_eq!(config.shortcuts.get("translate-screenshot").map(String::as_str), Some("Alt+S"));

        let mut config = AppConfig::from_env();
        config.shortcuts.insert("translate-screenshot".to_string(), "Alt+E".to_string());
        let config = config.normalized();
        assert_eq!(config.shortcuts.get("translate-screenshot").map(String::as_str), Some("Alt+S"));
    }

    #[test]
    fn normalized_keeps_custom_shortcuts_and_empty_disabled_bindings() {
        let mut config = AppConfig::from_env();
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
        let mut config = AppConfig::from_env();
        config.log_level = "trace".to_string();
        let normalized = config.normalized();
        assert_eq!(normalized.log_level, "info");
    }

    #[test]
    fn normalized_log_level_keeps_valid_values() {
        for level in ["error", "warn", "info", "debug"] {
            let mut config = AppConfig::from_env();
            config.log_level = level.to_string();
            assert_eq!(config.normalized().log_level, level);
        }
    }

    #[test]
    fn from_env_default_log_level_is_info() {
        let config = AppConfig::from_env();
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn serializes_log_level_camel_case() {
        let config = AppConfig::from_env();
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
}
