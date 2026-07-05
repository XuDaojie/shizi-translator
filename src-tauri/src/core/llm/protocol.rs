use std::sync::Arc;

use crate::core::{
    config::ServiceInstanceConfig,
    llm::{
        ClaudeConfig, ClaudeProvider, LlmProvider, MockLlmProvider, OpenAiCompatibleConfig,
        OpenAiCompatibleProvider,
    },
};

/// 协议 id 映射到的 provider 类型，供 `provider_for_service` 分发与单测断言。
#[derive(Debug)]
pub enum ProviderKind {
    OpenAiCompatible,
    Claude,
    Mock,
}

/// 把协议 id 字符串映射到 `ProviderKind`。
///
/// 与前端 `frontend/src/types/config.ts` 的 `ServiceProtocolId` 保持一致：
/// - `"openai_chat"` → `OpenAiCompatible`
/// - `"claude_messages"` → `Claude`
/// - `"mock"` → `Mock`
/// - 其他 → 返回错误，不再静默走 OpenAI 兼容（修复 Claude 渠道被误当 OpenAI 的 bug）。
pub fn protocol_to_kind(protocol: &str) -> Result<ProviderKind, String> {
    match protocol {
        "openai_chat" => Ok(ProviderKind::OpenAiCompatible),
        "claude_messages" => Ok(ProviderKind::Claude),
        "mock" => Ok(ProviderKind::Mock),
        other => Err(format!("未支持的协议：{other}")),
    }
}

/// 根据 `ServiceInstanceConfig` 的 `protocol` 字段创建对应的 LLM provider。
///
/// 协议 id 由 [`protocol_to_kind`] 解析，未知协议返回 `Err`，避免静默误匹配。
pub fn provider_for_service(
    config: &ServiceInstanceConfig,
) -> Result<Arc<dyn LlmProvider>, String> {
    match protocol_to_kind(&config.protocol)? {
        ProviderKind::Mock => Ok(Arc::new(MockLlmProvider)),
        ProviderKind::Claude => Ok(Arc::new(ClaudeProvider::new(ClaudeConfig {
            api_key: config.api_key.clone(),
            base_url: config.endpoint.clone(),
            model: config.model.clone(),
            timeout_seconds: config.timeout_seconds as u64,
            enable_thinking: false, // ponytail: 默认关闭，用户可在配置扩展时打开
        }))),
        ProviderKind::OpenAiCompatible => Ok(Arc::new(OpenAiCompatibleProvider::new(
            OpenAiCompatibleConfig {
                api_key: config.api_key.clone(),
                base_url: config.endpoint.clone(),
                model: config.model.clone(),
                timeout_seconds: config.timeout_seconds as u64,
            },
        ))),
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
        assert!(matches!(
            protocol_to_kind("openai_chat"),
            Ok(ProviderKind::OpenAiCompatible)
        ));
    }

    #[test]
    fn protocol_to_kind_claude_messages() {
        assert!(matches!(
            protocol_to_kind("claude_messages"),
            Ok(ProviderKind::Claude)
        ));
    }

    #[test]
    fn protocol_to_kind_mock() {
        assert!(matches!(
            protocol_to_kind("mock"),
            Ok(ProviderKind::Mock)
        ));
    }

    #[test]
    fn protocol_to_kind_unknown_returns_err() {
        let err = protocol_to_kind("openai-compatible").unwrap_err();
        assert!(err.contains("openai-compatible"), "错误信息应包含协议名: {err}");
    }

    #[test]
    fn provider_for_service_claude_messages_ok() {
        let config = svc("claude_messages");
        assert!(provider_for_service(&config).is_ok());
    }

    #[test]
    fn provider_for_service_unknown_returns_err() {
        let config = svc("openai-compatible");
        assert!(provider_for_service(&config).is_err());
    }
}
