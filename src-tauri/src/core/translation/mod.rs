pub mod auto_lang;
pub mod batch;
pub mod protocol;
pub mod provider;
pub mod service;
pub mod types;

// cdylib crate-type 无 Rust 外部消费者，pub use re-export 易被判死代码；保留供短路径访问
#[allow(unused_imports)]
pub use provider::{
    BatchTranslateProvider, StreamingAdapter, TranslationError, TranslationProvider,
    TranslationResult, TranslationStreamEvent,
};
#[allow(unused_imports)]
pub use protocol::{provider_for_service, ProviderKind};
pub use service::TranslationService;
pub use types::{
    TokenUsage, TranslationEvent, TranslationInput, TranslationPromptConfig, TranslationRequest,
    TranslationServiceMeta, TranslationSessionId,
};
