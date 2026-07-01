use std::env;

use serde::{Deserialize, Serialize};

use crate::core::llm::{ClaudeConfig, OpenAiCompatibleConfig};

const DEFAULT_PROVIDER: &str = "openai-compatible";
const DEFAULT_TARGET_LANG: &str = "中文";
const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "gpt-4o-mini";
const DEFAULT_TIMEOUT_SECONDS: u64 = 60;
const DEFAULT_CLAUDE_BASE_URL: &str = "https://api.anthropic.com";
const DEFAULT_CLAUDE_MODEL: &str = "claude-haiku-4-5";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub provider: String,
    pub target_lang: String,
    pub openai_compatible: OpenAiCompatibleAppConfig,
    #[serde(default)]
    pub claude: ClaudeAppConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAiCompatibleAppConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeAppConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub timeout_seconds: u64,
    pub enable_thinking: bool,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            provider: env::var("SHIZI_LLM_PROVIDER")
                .unwrap_or_else(|_| DEFAULT_PROVIDER.to_string()),
            target_lang: env::var("SHIZI_TARGET_LANG")
                .unwrap_or_else(|_| DEFAULT_TARGET_LANG.to_string()),
            openai_compatible: OpenAiCompatibleAppConfig::from_env(),
            claude: ClaudeAppConfig::from_env(),
        }
        .normalized()
    }

    pub fn normalized(mut self) -> Self {
        self.provider = normalize_string(self.provider, DEFAULT_PROVIDER);
        self.target_lang = normalize_string(self.target_lang, DEFAULT_TARGET_LANG);
        self.openai_compatible = self.openai_compatible.normalized();
        self.claude = self.claude.normalized();
        self
    }
}

impl OpenAiCompatibleAppConfig {
    pub fn from_env() -> Self {
        Self {
            api_key: env::var("SHIZI_OPENAI_API_KEY").ok(),
            base_url: env::var("SHIZI_OPENAI_BASE_URL")
                .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string()),
            model: env::var("SHIZI_OPENAI_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string()),
            timeout_seconds: env::var("SHIZI_OPENAI_TIMEOUT_SECS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(DEFAULT_TIMEOUT_SECONDS),
        }
        .normalized()
    }

    pub fn normalized(mut self) -> Self {
        self.api_key = self.api_key.and_then(non_empty_string);
        self.base_url = normalize_string(self.base_url, DEFAULT_BASE_URL);
        self.model = normalize_string(self.model, DEFAULT_MODEL);
        if self.timeout_seconds == 0 {
            self.timeout_seconds = DEFAULT_TIMEOUT_SECONDS;
        }
        self
    }
}

impl ClaudeAppConfig {
    pub fn from_env() -> Self {
        Self {
            api_key: env::var("SHIZI_CLAUDE_API_KEY").ok(),
            base_url: env::var("SHIZI_CLAUDE_BASE_URL")
                .unwrap_or_else(|_| DEFAULT_CLAUDE_BASE_URL.to_string()),
            model: env::var("SHIZI_CLAUDE_MODEL")
                .unwrap_or_else(|_| DEFAULT_CLAUDE_MODEL.to_string()),
            timeout_seconds: env::var("SHIZI_CLAUDE_TIMEOUT_SECS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(DEFAULT_TIMEOUT_SECONDS),
            enable_thinking: env::var("SHIZI_CLAUDE_ENABLE_THINKING")
                .map(|value| value.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
        }
        .normalized()
    }

    pub fn normalized(mut self) -> Self {
        self.api_key = self.api_key.and_then(non_empty_string);
        self.base_url = normalize_string(self.base_url, DEFAULT_CLAUDE_BASE_URL);
        self.model = normalize_string(self.model, DEFAULT_CLAUDE_MODEL);
        if self.timeout_seconds == 0 {
            self.timeout_seconds = DEFAULT_TIMEOUT_SECONDS;
        }
        self
    }
}

impl From<OpenAiCompatibleAppConfig> for OpenAiCompatibleConfig {
    fn from(config: OpenAiCompatibleAppConfig) -> Self {
        Self {
            api_key: config.api_key,
            base_url: config.base_url,
            model: config.model,
            timeout_seconds: config.timeout_seconds,
        }
    }
}

impl From<ClaudeAppConfig> for ClaudeConfig {
    fn from(config: ClaudeAppConfig) -> Self {
        Self {
            api_key: config.api_key,
            base_url: config.base_url,
            model: config.model,
            timeout_seconds: config.timeout_seconds,
            enable_thinking: config.enable_thinking,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_app_config_default_then_normalized_uses_defaults() {
        let config = ClaudeAppConfig::default().normalized();
        assert_eq!(config.base_url, DEFAULT_CLAUDE_BASE_URL);
        assert_eq!(config.model, DEFAULT_CLAUDE_MODEL);
        assert_eq!(config.timeout_seconds, DEFAULT_TIMEOUT_SECONDS);
        assert!(!config.enable_thinking);
        assert!(config.api_key.is_none());
    }

    #[test]
    fn claude_app_config_normalized_fills_empty_strings() {
        let config = ClaudeAppConfig {
            api_key: Some("   ".to_string()),
            base_url: "".to_string(),
            model: "".to_string(),
            timeout_seconds: 0,
            enable_thinking: false,
        }
        .normalized();
        assert!(config.api_key.is_none());
        assert_eq!(config.base_url, DEFAULT_CLAUDE_BASE_URL);
        assert_eq!(config.model, DEFAULT_CLAUDE_MODEL);
        assert_eq!(config.timeout_seconds, DEFAULT_TIMEOUT_SECONDS);
    }

    #[test]
    fn claude_app_config_from_env_reads_overrides() {
        std::env::set_var("SHIZI_CLAUDE_API_KEY", "sk-claude-test");
        std::env::set_var("SHIZI_CLAUDE_BASE_URL", "https://gateway.example.com");
        std::env::set_var("SHIZI_CLAUDE_MODEL", "claude-haiku-4-5");
        std::env::set_var("SHIZI_CLAUDE_TIMEOUT_SECS", "120");
        std::env::set_var("SHIZI_CLAUDE_ENABLE_THINKING", "true");

        let config = ClaudeAppConfig::from_env();

        std::env::remove_var("SHIZI_CLAUDE_API_KEY");
        std::env::remove_var("SHIZI_CLAUDE_BASE_URL");
        std::env::remove_var("SHIZI_CLAUDE_MODEL");
        std::env::remove_var("SHIZI_CLAUDE_TIMEOUT_SECS");
        std::env::remove_var("SHIZI_CLAUDE_ENABLE_THINKING");

        assert_eq!(config.api_key.as_deref(), Some("sk-claude-test"));
        assert_eq!(config.base_url, "https://gateway.example.com");
        assert_eq!(config.model, "claude-haiku-4-5");
        assert_eq!(config.timeout_seconds, 120);
        assert!(config.enable_thinking);
    }

    #[test]
    fn from_claude_app_config_maps_all_fields() {
        let app_config = ClaudeAppConfig {
            api_key: Some("sk-test".to_string()),
            base_url: "https://api.anthropic.com".to_string(),
            model: "claude-haiku-4-5".to_string(),
            timeout_seconds: 90,
            enable_thinking: true,
        };
        let runtime: ClaudeConfig = app_config.into();
        assert_eq!(runtime.api_key.as_deref(), Some("sk-test"));
        assert_eq!(runtime.base_url, "https://api.anthropic.com");
        assert_eq!(runtime.model, "claude-haiku-4-5");
        assert_eq!(runtime.timeout_seconds, 90);
        assert!(runtime.enable_thinking);
    }

    #[test]
    fn app_config_deserializes_without_claude_field() {
        let json = r#"{
            "provider": "openai-compatible",
            "targetLang": "中文",
            "openaiCompatible": {
                "apiKey": "sk-x",
                "baseUrl": "https://api.openai.com/v1",
                "model": "gpt-4o-mini",
                "timeoutSeconds": 60
            }
        }"#;
        let config: AppConfig = serde_json::from_str::<AppConfig>(json)
            .expect("旧配置缺少 claude 字段应可反序列化")
            .normalized();
        assert_eq!(config.provider, "openai-compatible");
        assert_eq!(config.claude.base_url, DEFAULT_CLAUDE_BASE_URL);
        assert_eq!(config.claude.model, DEFAULT_CLAUDE_MODEL);
        assert_eq!(config.claude.timeout_seconds, DEFAULT_TIMEOUT_SECONDS);
        assert!(!config.claude.enable_thinking);
        assert!(config.claude.api_key.is_none());
    }
}
