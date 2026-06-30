use std::env;

use serde::{Deserialize, Serialize};

use crate::core::llm::OpenAiCompatibleConfig;

const DEFAULT_PROVIDER: &str = "openai-compatible";
const DEFAULT_TARGET_LANG: &str = "中文";
const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "gpt-4o-mini";
const DEFAULT_TIMEOUT_SECONDS: u64 = 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub provider: String,
    pub target_lang: String,
    pub openai_compatible: OpenAiCompatibleAppConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAiCompatibleAppConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub timeout_seconds: u64,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            provider: env::var("SHIZI_LLM_PROVIDER")
                .unwrap_or_else(|_| DEFAULT_PROVIDER.to_string()),
            target_lang: env::var("SHIZI_TARGET_LANG")
                .unwrap_or_else(|_| DEFAULT_TARGET_LANG.to_string()),
            openai_compatible: OpenAiCompatibleAppConfig::from_env(),
        }
        .normalized()
    }

    pub fn normalized(mut self) -> Self {
        self.provider = normalize_string(self.provider, DEFAULT_PROVIDER);
        self.target_lang = normalize_string(self.target_lang, DEFAULT_TARGET_LANG);
        self.openai_compatible = self.openai_compatible.normalized();
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
