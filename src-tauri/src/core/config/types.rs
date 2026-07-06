use std::collections::HashMap;
use std::env;

use serde::{Deserialize, Serialize};

const DEFAULT_TARGET_LANG: &str = "中文";
const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "gpt-4o-mini";
const DEFAULT_TIMEOUT_SECONDS: u32 = 60;
const DEFAULT_CLAUDE_BASE_URL: &str = "https://api.anthropic.com";
const DEFAULT_CLAUDE_MODEL: &str = "claude-haiku-4-5";
const DEFAULT_PROTOCOL: &str = "openai_chat";

fn default_true() -> bool {
    true
}

fn default_source_lang() -> String {
    "auto".to_string()
}

fn default_chain_of_thought() -> String {
    "off".to_string()
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
    #[serde(default = "default_source_lang")]
    pub default_source_lang: String,
    #[serde(default = "default_true")]
    pub auto_copy: bool,
    #[serde(default = "default_true")]
    pub restore_clipboard: bool,
    #[serde(default = "default_true")]
    pub popup_precreate: bool,
    #[serde(default = "default_true")]
    pub overlay_precreate: bool,
    #[serde(default = "default_true")]
    pub collect_usage: bool,
}

impl ServiceInstanceConfig {
    pub fn normalized(mut self) -> Self {
        self.api_key = self.api_key.and_then(non_empty_string);
        self.model = normalize_string(self.model, DEFAULT_MODEL);
        if self.endpoint.trim().is_empty() {
            self.endpoint = match self.protocol.as_str() {
                "claude_messages" => DEFAULT_CLAUDE_BASE_URL.to_string(),
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
                .unwrap_or_else(|_| "Alt+E".to_string()),
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
            "show-window".to_string(),
            env::var("SHIZI_SHORTCUT_SHOW_WINDOW")
                .unwrap_or_else(|_| "Ctrl+Shift+Space".to_string()),
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
            ("translate-screenshot", "Alt+O") => "Alt+E".to_string(),
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
                .unwrap_or_else(|_| DEFAULT_TARGET_LANG.to_string()),
            default_source_lang: default_source_lang(),
            auto_copy: true,
            restore_clipboard: true,
            popup_precreate: true,
            overlay_precreate: true,
            collect_usage: env::var("SHIZI_COLLECT_USAGE")
                .map(|v| v.eq_ignore_ascii_case("true"))
                .unwrap_or(true),
        }
        .normalized()
    }

    pub fn normalized(mut self) -> Self {
        self.shortcuts = normalize_shortcuts(self.shortcuts);
        self.services = self.services.into_iter().map(|s| s.normalized()).collect();
        self.target_lang = normalize_string(self.target_lang, DEFAULT_TARGET_LANG);
        self.default_source_lang = normalize_string(self.default_source_lang, "auto");
        self
    }

    #[cfg(test)]
    pub fn is_configured(&self) -> bool {
        self.services.iter().any(|s| {
            if !s.enabled {
                return false;
            }
            match s.protocol.as_str() {
                "mock" => true,
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

#[cfg(test)]
mod tests {
    use super::*;

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
            "targetLang": "中文"
        }"#;
        let config: AppConfig = serde_json::from_str::<AppConfig>(json)
            .expect("缺少字段应可反序列化")
            .normalized();
        assert_eq!(config.target_lang, "中文");
        assert!(config.popup_precreate);
        assert!(config.overlay_precreate);
        assert!(config.collect_usage);
        assert!(config.services.is_empty());
        assert!(!config.is_configured());
    }

    #[test]
    fn deserializes_services_array() {
        let json = r#"{
            "targetLang": "中文",
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
    fn defaults_shortcuts_use_bob_style_keys() {
        let config = AppConfig::from_env();

        assert_eq!(config.shortcuts.get("translate-selection").map(String::as_str), Some("Alt+D"));
        assert_eq!(config.shortcuts.get("translate-screenshot").map(String::as_str), Some("Alt+E"));
        assert_eq!(
            config.shortcuts.get("translate-clipboard").map(String::as_str),
            Some("Ctrl+Shift+C")
        );
        assert_eq!(
            config.shortcuts.get("word-lookup").map(String::as_str), Some("")
        );
        assert_eq!(
            config.shortcuts.get("show-window").map(String::as_str),
            Some("Ctrl+Shift+Space")
        );
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
        assert_eq!(config.shortcuts.get("translate-screenshot").map(String::as_str), Some("Alt+E"));
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
}
