mod coach_planning_literature;
mod context_prelude;
mod error;
mod logging;
mod model;
mod operation;
mod orchestrator;
pub(crate) mod persistence;
mod ports;
mod preview_messages;
mod request_builder;
mod transcript;

pub use coach_planning_literature::coach_planning_literature_guidance;
pub use context_prelude::{
    packed_training_context_legend_with_guidance, ATHLETE_SUMMARY_CALENDAR_GUARD,
    PACKED_CALENDAR_AUTHORITY_GUIDANCE, PACKED_TRAINING_CONTEXT_LEGEND,
};
pub(crate) use transcript::{
    final_assistant_text, last_nonempty_assistant_content, merge_provider_transcript_entries,
    next_provider_transcript_updated_at_epoch_seconds, provider_transcript_from_legacy_response,
    rebuild_conversation_with_provider_transcript, timestamped_message_content,
};

pub use error::LlmError;
pub use logging::{llm_full_debug_logging_enabled, serialize_logged_body, truncate_logged_body};
pub use model::{
    hash_text, llm_request_timeout, LlmCacheUsage, LlmChatMessage, LlmChatRequest, LlmChatResponse,
    LlmContextCache, LlmFinishReason, LlmMessageRole, LlmProvider, LlmProviderConfig,
    LlmTokenUsage, LlmToolCall, LlmToolChoice, LlmToolDefinition, LLM_REQUEST_TIMEOUT_SECONDS,
};
pub use operation::{
    deserialize_llm_error, serialize_llm_error, CompletedLlmReply, LlmReplyClaimResult,
    LlmReplyOperation, LlmReplyOperationFailureKind, LlmReplyOperationStatus,
    PendingLlmReplyCheckpoint, SerializedLlmError,
};
pub(crate) use orchestrator::{
    resolve_llm_reply_operation, LlmReplyResolutionWorkflow, ResolvedLlmReplyOperation,
};
pub use ports::{BoxFuture, LlmChatPort, LlmContextCacheRepository, UserLlmConfigProvider};
pub use preview_messages::{preview_provider_messages, PreviewProviderMessage};
pub use request_builder::{
    build_chat_request, conversation_timing_volatile_context, current_date_string,
    current_datetime_rfc3339, epoch_seconds_to_rfc3339, find_reusable_context_cache,
    persist_reusable_context_cache, reusable_context_cache_key, LlmChatRequestInput,
    ReusableContextCacheLookup, ReusableContextCacheUpsert,
};
