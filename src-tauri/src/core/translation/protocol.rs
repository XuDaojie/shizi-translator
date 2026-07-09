use std::sync::Arc;

use crate::core::config::ServiceInstanceConfig;
use crate::core::llm::{
    ClaudeConfig, ClaudeProvider, MockLlmProvider, OpenAiCompatibleConfig,
    OpenAiCompatibleProvider,
};
use crate::core::mt::{EdgeTranslateEnv, MicrosoftMtProvider};
use crate::core::translation::provider::{StreamingAdapter, TranslationProvider};

/// 协议 id 映射到的 provider 类型，供 `provider_for_service` 分发与单测断言。
#[derive(Debug)]
pub enum ProviderKind {
    OpenAiCompatible,
    Claude,
    Mock,
    Microsoft,
}

/// 把协议 id 字符串映射到 `ProviderKind`。
///
/// 与前端 `frontend/src/types/config.ts` 的 `ServiceProtocolId` 保持一致：
/// - `"openai_chat"` -> `OpenAiCompatible`
/// - `"claude_messages"` -> `Claude`
/// - `"mock"` -> `Mock`
/// - `"microsoft_edge"` -> `Microsoft`
/// - 其他 -> 返回错误，不静默走 OpenAI 兼容。
pub fn protocol_to_kind(protocol: &str) -> Result<ProviderKind, String> {
    match protocol {
        "openai_chat" => Ok(ProviderKind::OpenAiCompatible),
        "claude_messages" => Ok(ProviderKind::Claude),
        "mock" => Ok(ProviderKind::Mock),
        "microsoft_edge" => Ok(ProviderKind::Microsoft),
        other => Err(format!("未支持的协议：{other}")),
    }
}

/// 根据 `ServiceInstanceConfig` 的 `protocol` 字段创建对应的 provider。
/// `env` 为微软翻译所需的浏览器环境信息（UA/Accept-Language），仅 `microsoft_edge` 使用，
/// 传 `None` 时用编译期默认 UA 兜底。
pub fn provider_for_service(
    config: &ServiceInstanceConfig,
    env: Option<&EdgeTranslateEnv>,
) -> Result<Arc<dyn TranslationProvider>, String> {
    match protocol_to_kind(&config.protocol)? {
        ProviderKind::Mock => Ok(Arc::new(MockLlmProvider)),
        ProviderKind::Claude => Ok(Arc::new(ClaudeProvider::new(ClaudeConfig {
            api_key: config.api_key.clone(),
            base_url: config.endpoint.clone(),
            model: config.model.clone(),
            timeout_seconds: config.timeout_seconds as u64,
        }))),
        ProviderKind::OpenAiCompatible => Ok(Arc::new(OpenAiCompatibleProvider::new(
            OpenAiCompatibleConfig {
                api_key: config.api_key.clone(),
                base_url: config.endpoint.clone(),
                model: config.model.clone(),
                timeout_seconds: config.timeout_seconds as u64,
            },
        ))),
        ProviderKind::Microsoft => Ok(Arc::new(StreamingAdapter(MicrosoftMtProvider::new(
            env.cloned().unwrap_or_default(),
        )))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svc(protocol: &str) -> ServiceInstanceConfig {
        ServiceInstanceConfig {
            id: "test".to_string(),
            service_type: "openai".to_string(),
            name: "测试".to_string(),
            enabled: true,
            protocol: protocol.to_string(),
            api_key: Some("sk-test".to_string()),
            endpoint: "https://api.example.com".to_string(),
            model: "gpt-4o-mini".to_string(),
            timeout_seconds: 60,
            system_prompt: String::new(),
            translation_prompt: String::new(),
            reflection_prompt: String::new(),
            reflection_enabled: false,
            chain_of_thought: "off".to_string(),
        }
    }

    #[test]
    fn protocol_to_kind_openai_chat() {
        assert!(matches!(protocol_to_kind("openai_chat"), Ok(ProviderKind::OpenAiCompatible)));
    }
    #[test]
    fn protocol_to_kind_claude_messages() {
        assert!(matches!(protocol_to_kind("claude_messages"), Ok(ProviderKind::Claude)));
    }
    #[test]
    fn protocol_to_kind_mock() {
        assert!(matches!(protocol_to_kind("mock"), Ok(ProviderKind::Mock)));
    }
    #[test]
    fn protocol_to_kind_microsoft_edge() {
        assert!(matches!(protocol_to_kind("microsoft_edge"), Ok(ProviderKind::Microsoft)));
    }
    #[test]
    fn protocol_to_kind_unknown_returns_err() {
        let err = protocol_to_kind("openai-compatible").unwrap_err();
        assert!(err.contains("openai-compatible"), "错误信息应包含协议名: {err}");
    }
    #[test]
    fn provider_for_service_claude_messages_ok() {
        let config = svc("claude_messages");
        assert!(provider_for_service(&config, None).is_ok());
    }
    #[test]
    fn provider_for_service_microsoft_returns_streaming_adapter() {
        let config = svc("microsoft_edge");
        assert!(provider_for_service(&config, None).is_ok());
    }
    #[test]
    fn provider_for_service_unknown_returns_err() {
        let config = svc("openai-compatible");
        assert!(provider_for_service(&config, None).is_err());
    }
}
