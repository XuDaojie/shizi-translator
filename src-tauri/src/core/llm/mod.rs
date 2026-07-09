pub mod mock;
pub mod openai_compatible;
pub mod claude;
pub mod protocol;

pub use claude::{ClaudeConfig, ClaudeProvider};
pub use mock::MockLlmProvider;
pub use openai_compatible::{OpenAiCompatibleConfig, OpenAiCompatibleProvider};
// 通用 provider 抽象已迁至 core::translation::provider，旧 Llm* 名移除。
// provider_for_service 已迁至 core::translation::protocol（任务 3 完成后），
// 当前仍由本模块 protocol 提供：
pub use protocol::provider_for_service;
