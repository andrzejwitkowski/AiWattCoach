mod context_prelude;
mod error;
mod model;
mod ports;
mod transcript;

pub(crate) use context_prelude::PACKED_TRAINING_CONTEXT_LEGEND;
pub(crate) use transcript::{
    final_assistant_text, last_nonempty_assistant_content, merge_provider_transcript_entries,
    next_provider_transcript_updated_at_epoch_seconds, provider_transcript_from_legacy_response,
    rebuild_conversation_with_provider_transcript,
};

pub use error::LlmError;
pub use model::{
    approximate_token_budget_for_model, hash_text, llm_request_timeout, LlmCacheUsage,
    LlmChatMessage, LlmChatRequest, LlmChatResponse, LlmContextCache, LlmFinishReason,
    LlmMessageRole, LlmProvider, LlmProviderConfig, LlmTokenUsage, LlmToolCall, LlmToolChoice,
    LlmToolDefinition, LLM_REQUEST_TIMEOUT_SECONDS,
};
pub use ports::{BoxFuture, LlmChatPort, LlmContextCacheRepository, UserLlmConfigProvider};
