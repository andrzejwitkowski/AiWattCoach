use serde::{Deserialize, Serialize};

use crate::domain::llm_tools::LlmToolLoopOutput;

use super::{
    merge_provider_transcript_entries, LlmCacheUsage, LlmChatMessage, LlmChatResponse, LlmError,
    LlmFinishReason, LlmProvider, LlmTokenUsage,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LlmReplyOperationFailureKind {
    CredentialsNotConfigured,
    ProviderNotConfigured,
    ModelNotConfigured,
    ContextTooLarge,
    UnsupportedProvider,
    Transport,
    ProviderRejected,
    RateLimited,
    InvalidResponse,
    Checkpoint,
    Internal,
}

impl LlmReplyOperationFailureKind {
    pub fn from_llm_error(error: &LlmError) -> Self {
        match error {
            LlmError::CredentialsNotConfigured => Self::CredentialsNotConfigured,
            LlmError::ProviderNotConfigured => Self::ProviderNotConfigured,
            LlmError::ModelNotConfigured => Self::ModelNotConfigured,
            LlmError::ContextTooLarge(_) => Self::ContextTooLarge,
            LlmError::UnsupportedProvider(_) => Self::UnsupportedProvider,
            LlmError::Transport(_) => Self::Transport,
            LlmError::ProviderRejected(_) => Self::ProviderRejected,
            LlmError::RateLimited(_) => Self::RateLimited,
            LlmError::InvalidResponse(_) => Self::InvalidResponse,
            LlmError::Checkpoint(_) => Self::Checkpoint,
            LlmError::Internal(_) => Self::Internal,
        }
    }

    pub fn to_llm_error(&self, message: Option<String>) -> LlmError {
        match self {
            Self::CredentialsNotConfigured => LlmError::CredentialsNotConfigured,
            Self::ProviderNotConfigured => LlmError::ProviderNotConfigured,
            Self::ModelNotConfigured => LlmError::ModelNotConfigured,
            Self::ContextTooLarge => LlmError::ContextTooLarge(
                message
                    .unwrap_or_else(|| "packed training context exceeds model limits".to_string()),
            ),
            Self::UnsupportedProvider => LlmError::UnsupportedProvider(
                message.unwrap_or_else(|| "unknown provider".to_string()),
            ),
            Self::Transport => {
                LlmError::Transport(message.unwrap_or_else(|| "transport error".to_string()))
            }
            Self::ProviderRejected => LlmError::ProviderRejected(
                message.unwrap_or_else(|| "provider rejected request".to_string()),
            ),
            Self::RateLimited => LlmError::RateLimited(
                message.unwrap_or_else(|| "provider rate limited request".to_string()),
            ),
            Self::InvalidResponse => LlmError::InvalidResponse(
                message.unwrap_or_else(|| "invalid provider response".to_string()),
            ),
            Self::Checkpoint => {
                LlmError::Checkpoint(message.unwrap_or_else(|| "checkpoint error".to_string()))
            }
            Self::Internal => {
                LlmError::Internal(message.unwrap_or_else(|| "internal llm error".to_string()))
            }
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Transport
                | Self::RateLimited
                | Self::InvalidResponse
                | Self::Checkpoint
                | Self::Internal
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LlmReplyOperationStatus {
    Pending,
    Completed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmReplyOperation {
    pub user_id: String,
    pub scope_id: String,
    pub user_message_id: String,
    pub status: LlmReplyOperationStatus,
    pub failure_kind: Option<LlmReplyOperationFailureKind>,
    pub provider: Option<LlmProvider>,
    pub model: Option<String>,
    pub provider_request_id: Option<String>,
    pub reply_message_id: Option<String>,
    pub cache_scope_key: Option<String>,
    pub provider_cache_id: Option<String>,
    pub token_usage: Option<LlmTokenUsage>,
    pub cache_usage: Option<LlmCacheUsage>,
    pub provider_transcript: Vec<LlmChatMessage>,
    pub finish_reason: Option<LlmFinishReason>,
    pub public_tool_call_ids: Vec<String>,
    pub error_message: Option<String>,
    pub started_at_epoch_seconds: i64,
    pub last_attempt_at_epoch_seconds: i64,
    pub attempt_count: u32,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LlmReplyClaimResult {
    Claimed(LlmReplyOperation),
    Existing(LlmReplyOperation),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletedLlmReply {
    pub provider: LlmProvider,
    pub model: String,
    pub provider_request_id: Option<String>,
    pub reply_message_id: String,
    pub provider_cache_id: Option<String>,
    pub token_usage: LlmTokenUsage,
    pub cache_usage: LlmCacheUsage,
    pub updated_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingLlmReplyCheckpoint {
    pub provider: LlmProvider,
    pub model: String,
    pub provider_request_id: Option<String>,
    pub provider_cache_id: Option<String>,
    pub token_usage: LlmTokenUsage,
    pub cache_usage: LlmCacheUsage,
    pub provider_transcript: Vec<LlmChatMessage>,
    pub finish_reason: Option<LlmFinishReason>,
    pub updated_at_epoch_seconds: i64,
}

impl LlmReplyOperation {
    pub fn pending_checkpoint_from_tool_loop(
        llm_output: &LlmToolLoopOutput,
        updated_at_epoch_seconds: i64,
    ) -> PendingLlmReplyCheckpoint {
        PendingLlmReplyCheckpoint {
            provider: llm_output.response.provider.clone(),
            model: llm_output.response.model.clone(),
            provider_request_id: llm_output.response.provider_request_id.clone(),
            provider_cache_id: llm_output.response.cache.provider_cache_id.clone(),
            token_usage: llm_output.response.usage.clone(),
            cache_usage: llm_output.response.cache.clone(),
            provider_transcript: llm_output.state.provider_transcript.clone(),
            finish_reason: llm_output.state.finish_reason.clone(),
            updated_at_epoch_seconds,
        }
    }

    pub fn completed_reply_from_response(
        llm_response: &LlmChatResponse,
        reply_message_id: String,
        updated_at_epoch_seconds: i64,
    ) -> CompletedLlmReply {
        CompletedLlmReply {
            provider: llm_response.provider.clone(),
            model: llm_response.model.clone(),
            provider_request_id: llm_response.provider_request_id.clone(),
            reply_message_id,
            provider_cache_id: llm_response.cache.provider_cache_id.clone(),
            token_usage: llm_response.usage.clone(),
            cache_usage: llm_response.cache.clone(),
            updated_at_epoch_seconds,
        }
    }

    pub fn pending(
        user_id: String,
        scope_id: String,
        user_message_id: String,
        cache_scope_key: Option<String>,
        reply_message_id: String,
        created_at_epoch_seconds: i64,
    ) -> Self {
        Self {
            user_id,
            scope_id,
            user_message_id,
            status: LlmReplyOperationStatus::Pending,
            failure_kind: None,
            provider: None,
            model: None,
            provider_request_id: None,
            reply_message_id: Some(reply_message_id),
            cache_scope_key,
            provider_cache_id: None,
            token_usage: None,
            cache_usage: None,
            provider_transcript: Vec::new(),
            finish_reason: None,
            public_tool_call_ids: Vec::new(),
            error_message: None,
            started_at_epoch_seconds: created_at_epoch_seconds,
            last_attempt_at_epoch_seconds: created_at_epoch_seconds,
            attempt_count: 1,
            created_at_epoch_seconds,
            updated_at_epoch_seconds: created_at_epoch_seconds,
        }
    }

    pub fn is_stale(&self, stale_before_epoch_seconds: i64) -> bool {
        self.status == LlmReplyOperationStatus::Pending
            && self.last_attempt_at_epoch_seconds <= stale_before_epoch_seconds
    }

    pub fn reclaim(&self, fallback_reply_message_id: String, now_epoch_seconds: i64) -> Self {
        Self {
            user_id: self.user_id.clone(),
            scope_id: self.scope_id.clone(),
            user_message_id: self.user_message_id.clone(),
            status: LlmReplyOperationStatus::Pending,
            failure_kind: None,
            provider: self.provider.clone(),
            model: self.model.clone(),
            provider_request_id: self.provider_request_id.clone(),
            reply_message_id: self
                .reply_message_id
                .clone()
                .or(Some(fallback_reply_message_id)),
            cache_scope_key: self.cache_scope_key.clone(),
            provider_cache_id: self.provider_cache_id.clone(),
            token_usage: self.token_usage.clone(),
            cache_usage: self.cache_usage.clone(),
            provider_transcript: self.provider_transcript.clone(),
            finish_reason: self.finish_reason.clone(),
            public_tool_call_ids: self.public_tool_call_ids.clone(),
            error_message: None,
            started_at_epoch_seconds: self.started_at_epoch_seconds,
            last_attempt_at_epoch_seconds: now_epoch_seconds,
            attempt_count: self.attempt_count.saturating_add(1),
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
        }
    }

    pub fn mark_completed(&self, reply: CompletedLlmReply) -> Self {
        Self {
            user_id: self.user_id.clone(),
            scope_id: self.scope_id.clone(),
            user_message_id: self.user_message_id.clone(),
            status: LlmReplyOperationStatus::Completed,
            failure_kind: None,
            provider: Some(reply.provider),
            model: Some(reply.model),
            provider_request_id: reply.provider_request_id,
            reply_message_id: Some(reply.reply_message_id),
            cache_scope_key: self.cache_scope_key.clone(),
            provider_cache_id: reply.provider_cache_id,
            token_usage: Some(reply.token_usage),
            cache_usage: Some(reply.cache_usage),
            provider_transcript: self.provider_transcript.clone(),
            finish_reason: self.finish_reason.clone(),
            public_tool_call_ids: self.public_tool_call_ids.clone(),
            error_message: None,
            started_at_epoch_seconds: self.started_at_epoch_seconds,
            last_attempt_at_epoch_seconds: self.last_attempt_at_epoch_seconds,
            attempt_count: self.attempt_count,
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds: reply.updated_at_epoch_seconds,
        }
    }

    pub fn record_provider_response(&self, checkpoint: PendingLlmReplyCheckpoint) -> Self {
        let provider_transcript = merge_provider_transcript_entries(
            self.provider_transcript.clone(),
            &checkpoint.provider_transcript,
        );

        Self {
            user_id: self.user_id.clone(),
            scope_id: self.scope_id.clone(),
            user_message_id: self.user_message_id.clone(),
            status: LlmReplyOperationStatus::Pending,
            failure_kind: None,
            provider: Some(checkpoint.provider),
            model: Some(checkpoint.model),
            provider_request_id: checkpoint.provider_request_id,
            reply_message_id: self.reply_message_id.clone(),
            cache_scope_key: self.cache_scope_key.clone(),
            provider_cache_id: checkpoint.provider_cache_id,
            token_usage: Some(checkpoint.token_usage),
            cache_usage: Some(checkpoint.cache_usage),
            provider_transcript,
            finish_reason: checkpoint.finish_reason,
            public_tool_call_ids: self.public_tool_call_ids.clone(),
            error_message: None,
            started_at_epoch_seconds: self.started_at_epoch_seconds,
            last_attempt_at_epoch_seconds: self.last_attempt_at_epoch_seconds,
            attempt_count: self.attempt_count,
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds: checkpoint.updated_at_epoch_seconds,
        }
    }

    pub fn mark_completed_from_existing_message(
        &self,
        reply_message_id: String,
        updated_at_epoch_seconds: i64,
    ) -> Self {
        Self {
            user_id: self.user_id.clone(),
            scope_id: self.scope_id.clone(),
            user_message_id: self.user_message_id.clone(),
            status: LlmReplyOperationStatus::Completed,
            failure_kind: None,
            provider: self.provider.clone(),
            model: self.model.clone(),
            provider_request_id: self.provider_request_id.clone(),
            reply_message_id: Some(reply_message_id),
            cache_scope_key: self.cache_scope_key.clone(),
            provider_cache_id: self.provider_cache_id.clone(),
            token_usage: self.token_usage.clone(),
            cache_usage: self.cache_usage.clone(),
            provider_transcript: self.provider_transcript.clone(),
            finish_reason: self.finish_reason.clone(),
            public_tool_call_ids: self.public_tool_call_ids.clone(),
            error_message: None,
            started_at_epoch_seconds: self.started_at_epoch_seconds,
            last_attempt_at_epoch_seconds: self.last_attempt_at_epoch_seconds,
            attempt_count: self.attempt_count,
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds,
        }
    }

    pub fn mark_failed(&self, error: &LlmError, updated_at_epoch_seconds: i64) -> Self {
        Self {
            user_id: self.user_id.clone(),
            scope_id: self.scope_id.clone(),
            user_message_id: self.user_message_id.clone(),
            status: LlmReplyOperationStatus::Failed,
            failure_kind: Some(LlmReplyOperationFailureKind::from_llm_error(error)),
            provider: self.provider.clone(),
            model: self.model.clone(),
            provider_request_id: self.provider_request_id.clone(),
            reply_message_id: self.reply_message_id.clone(),
            cache_scope_key: self.cache_scope_key.clone(),
            provider_cache_id: self.provider_cache_id.clone(),
            token_usage: self.token_usage.clone(),
            cache_usage: self.cache_usage.clone(),
            provider_transcript: self.provider_transcript.clone(),
            finish_reason: self.finish_reason.clone(),
            public_tool_call_ids: self.public_tool_call_ids.clone(),
            error_message: Some(error.to_string()),
            started_at_epoch_seconds: self.started_at_epoch_seconds,
            last_attempt_at_epoch_seconds: self.last_attempt_at_epoch_seconds,
            attempt_count: self.attempt_count,
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds,
        }
    }
}
