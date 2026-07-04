use std::sync::Arc;

use crate::core::{
    config::ServiceInstanceConfig,
    llm::{
        ClaudeConfig, ClaudeProvider, LlmProvider, MockLlmProvider, OpenAiCompatibleConfig,
        OpenAiCompatibleProvider,
    },
};

/// 根据 `ServiceInstanceConfig` 的 `protocol` 字段创建对应的 LLM provider。
///
/// 当前支持的协议：
/// - `"mock"` — 返回 MockLlmProvider
/// - `"claude"` — 返回 ClaudeProvider
/// - 其他协议（包括默认的 `"openai-compatible"`）— 返回 OpenAiCompatibleProvider
///
/// 返回 `Result` 是为了后续 provider 构造可能失败时保持扩展性。
pub fn provider_for_service(config: &ServiceInstanceConfig) -> Result<Arc<dyn LlmProvider>, String> {
    match config.protocol.as_str() {
        "mock" => Ok(Arc::new(MockLlmProvider)),
        "claude" => Ok(Arc::new(ClaudeProvider::new(ClaudeConfig {
            api_key: config.api_key.clone(),
            base_url: config.endpoint.clone(),
            model: config.model.clone(),
            timeout_seconds: config.timeout_seconds as u64,
            enable_thinking: false, // ponytail: 默认关闭，用户可在配置扩展时打开
        }))),
        _ => Ok(Arc::new(OpenAiCompatibleProvider::new(OpenAiCompatibleConfig {
            api_key: config.api_key.clone(),
            base_url: config.endpoint.clone(),
            model: config.model.clone(),
            timeout_seconds: config.timeout_seconds as u64,
        }))),
    }
}
