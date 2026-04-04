use crate::domain::llm::{LlmCacheUsage, LlmError, LlmProvider, LlmTokenUsage};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoachReplyOperationFailureKind {
    CredentialsNotConfigured,
    ProviderNotConfigured,
    ModelNotConfigured,
    ContextTooLarge,
    UnsupportedProvider,
    Transport,
    ProviderRejected,
    RateLimited,
    InvalidResponse,
    Internal,
}

impl CoachReplyOperationFailureKind {
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
            Self::Internal => {
                LlmError::Internal(message.unwrap_or_else(|| "internal llm error".to_string()))
            }
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Transport | Self::RateLimited | Self::InvalidResponse | Self::Internal
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkoutSummaryError {
    AlreadyExists,
    Locked,
    NotFound,
    ReplyAlreadyPending,
    Repository(String),
    Llm(LlmError),
    Validation(String),
}

impl std::fmt::Display for WorkoutSummaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyExists => write!(f, "workout summary already exists"),
            Self::Locked => write!(f, "workout summary is saved and cannot be edited"),
            Self::NotFound => write!(f, "workout summary not found"),
            Self::ReplyAlreadyPending => {
                write!(
                    f,
                    "coach reply generation is already pending for this message"
                )
            }
            Self::Repository(message) => write!(f, "{message}"),
            Self::Llm(error) => write!(f, "{error}"),
            Self::Validation(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for WorkoutSummaryError {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoachReplyOperationStatus {
    Pending,
    Completed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoachReplyOperation {
    pub user_id: String,
    pub workout_id: String,
    pub user_message_id: String,
    pub status: CoachReplyOperationStatus,
    pub failure_kind: Option<CoachReplyOperationFailureKind>,
    pub provider: Option<LlmProvider>,
    pub model: Option<String>,
    pub provider_request_id: Option<String>,
    pub coach_message_id: Option<String>,
    pub cache_scope_key: Option<String>,
    pub provider_cache_id: Option<String>,
    pub token_usage: Option<LlmTokenUsage>,
    pub cache_usage: Option<LlmCacheUsage>,
    pub response_message: Option<String>,
    pub error_message: Option<String>,
    pub started_at_epoch_seconds: i64,
    pub last_attempt_at_epoch_seconds: i64,
    pub attempt_count: u32,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoachReplyClaimResult {
    Claimed(CoachReplyOperation),
    Existing(CoachReplyOperation),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletedCoachReply {
    pub provider: LlmProvider,
    pub model: String,
    pub provider_request_id: Option<String>,
    pub coach_message_id: String,
    pub provider_cache_id: Option<String>,
    pub token_usage: LlmTokenUsage,
    pub cache_usage: LlmCacheUsage,
    pub updated_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingCoachReplyCheckpoint {
    pub provider: LlmProvider,
    pub model: String,
    pub provider_request_id: Option<String>,
    pub provider_cache_id: Option<String>,
    pub token_usage: LlmTokenUsage,
    pub cache_usage: LlmCacheUsage,
    pub response_message: String,
    pub updated_at_epoch_seconds: i64,
}

impl CoachReplyOperation {
    pub fn pending(
        user_id: String,
        workout_id: String,
        user_message_id: String,
        cache_scope_key: Option<String>,
        coach_message_id: String,
        created_at_epoch_seconds: i64,
    ) -> Self {
        Self {
            user_id,
            workout_id,
            user_message_id,
            status: CoachReplyOperationStatus::Pending,
            failure_kind: None,
            provider: None,
            model: None,
            provider_request_id: None,
            coach_message_id: Some(coach_message_id),
            cache_scope_key,
            provider_cache_id: None,
            token_usage: None,
            cache_usage: None,
            response_message: None,
            error_message: None,
            started_at_epoch_seconds: created_at_epoch_seconds,
            last_attempt_at_epoch_seconds: created_at_epoch_seconds,
            attempt_count: 1,
            created_at_epoch_seconds,
            updated_at_epoch_seconds: created_at_epoch_seconds,
        }
    }

    pub fn is_stale(&self, stale_before_epoch_seconds: i64) -> bool {
        self.status == CoachReplyOperationStatus::Pending
            && self.last_attempt_at_epoch_seconds <= stale_before_epoch_seconds
    }

    pub fn reclaim(&self, fallback_coach_message_id: String, now_epoch_seconds: i64) -> Self {
        Self {
            user_id: self.user_id.clone(),
            workout_id: self.workout_id.clone(),
            user_message_id: self.user_message_id.clone(),
            status: CoachReplyOperationStatus::Pending,
            failure_kind: None,
            provider: self.provider.clone(),
            model: self.model.clone(),
            provider_request_id: self.provider_request_id.clone(),
            coach_message_id: self
                .coach_message_id
                .clone()
                .or(Some(fallback_coach_message_id)),
            cache_scope_key: self.cache_scope_key.clone(),
            provider_cache_id: self.provider_cache_id.clone(),
            token_usage: self.token_usage.clone(),
            cache_usage: self.cache_usage.clone(),
            response_message: self.response_message.clone(),
            error_message: None,
            started_at_epoch_seconds: self.started_at_epoch_seconds,
            last_attempt_at_epoch_seconds: now_epoch_seconds,
            attempt_count: self.attempt_count.saturating_add(1),
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
        }
    }

    pub fn mark_completed(&self, reply: CompletedCoachReply) -> Self {
        Self {
            user_id: self.user_id.clone(),
            workout_id: self.workout_id.clone(),
            user_message_id: self.user_message_id.clone(),
            status: CoachReplyOperationStatus::Completed,
            failure_kind: None,
            provider: Some(reply.provider),
            model: Some(reply.model),
            provider_request_id: reply.provider_request_id,
            coach_message_id: Some(reply.coach_message_id),
            cache_scope_key: self.cache_scope_key.clone(),
            provider_cache_id: reply.provider_cache_id,
            token_usage: Some(reply.token_usage),
            cache_usage: Some(reply.cache_usage),
            response_message: None,
            error_message: None,
            started_at_epoch_seconds: self.started_at_epoch_seconds,
            last_attempt_at_epoch_seconds: self.last_attempt_at_epoch_seconds,
            attempt_count: self.attempt_count,
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds: reply.updated_at_epoch_seconds,
        }
    }

    pub fn record_provider_response(&self, checkpoint: PendingCoachReplyCheckpoint) -> Self {
        Self {
            user_id: self.user_id.clone(),
            workout_id: self.workout_id.clone(),
            user_message_id: self.user_message_id.clone(),
            status: CoachReplyOperationStatus::Pending,
            failure_kind: None,
            provider: Some(checkpoint.provider),
            model: Some(checkpoint.model),
            provider_request_id: checkpoint.provider_request_id,
            coach_message_id: self.coach_message_id.clone(),
            cache_scope_key: self.cache_scope_key.clone(),
            provider_cache_id: checkpoint.provider_cache_id,
            token_usage: Some(checkpoint.token_usage),
            cache_usage: Some(checkpoint.cache_usage),
            response_message: Some(checkpoint.response_message),
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
        coach_message_id: String,
        updated_at_epoch_seconds: i64,
    ) -> Self {
        Self {
            user_id: self.user_id.clone(),
            workout_id: self.workout_id.clone(),
            user_message_id: self.user_message_id.clone(),
            status: CoachReplyOperationStatus::Completed,
            failure_kind: None,
            provider: self.provider.clone(),
            model: self.model.clone(),
            provider_request_id: self.provider_request_id.clone(),
            coach_message_id: Some(coach_message_id),
            cache_scope_key: self.cache_scope_key.clone(),
            provider_cache_id: self.provider_cache_id.clone(),
            token_usage: self.token_usage.clone(),
            cache_usage: self.cache_usage.clone(),
            response_message: None,
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
            workout_id: self.workout_id.clone(),
            user_message_id: self.user_message_id.clone(),
            status: CoachReplyOperationStatus::Failed,
            failure_kind: Some(CoachReplyOperationFailureKind::from_llm_error(error)),
            provider: self.provider.clone(),
            model: self.model.clone(),
            provider_request_id: self.provider_request_id.clone(),
            coach_message_id: self.coach_message_id.clone(),
            cache_scope_key: self.cache_scope_key.clone(),
            provider_cache_id: self.provider_cache_id.clone(),
            token_usage: self.token_usage.clone(),
            cache_usage: self.cache_usage.clone(),
            response_message: self.response_message.clone(),
            error_message: Some(error.to_string()),
            started_at_epoch_seconds: self.started_at_epoch_seconds,
            last_attempt_at_epoch_seconds: self.last_attempt_at_epoch_seconds,
            attempt_count: self.attempt_count,
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkoutSummary {
    pub id: String,
    pub user_id: String,
    pub workout_id: String,
    pub rpe: Option<u8>,
    pub messages: Vec<ConversationMessage>,
    pub saved_at_epoch_seconds: Option<i64>,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Coach,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversationMessage {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub created_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersistedUserMessage {
    pub summary: WorkoutSummary,
    pub user_message: ConversationMessage,
    pub athlete_summary_may_regenerate_before_reply: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoachReply {
    pub summary: WorkoutSummary,
    pub coach_message: ConversationMessage,
    pub athlete_summary_was_regenerated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SendMessageResult {
    pub summary: WorkoutSummary,
    pub user_message: ConversationMessage,
    pub coach_message: ConversationMessage,
}

impl WorkoutSummary {
    pub fn new(id: String, user_id: String, workout_id: String, now_epoch_seconds: i64) -> Self {
        Self {
            id,
            user_id,
            workout_id,
            rpe: None,
            messages: Vec::new(),
            saved_at_epoch_seconds: None,
            created_at_epoch_seconds: now_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
        }
    }
}

pub fn validate_rpe(rpe: u8) -> Result<u8, WorkoutSummaryError> {
    if (1..=10).contains(&rpe) {
        Ok(rpe)
    } else {
        Err(WorkoutSummaryError::Validation(
            "rpe must be between 1 and 10".to_string(),
        ))
    }
}

pub fn validate_message_content(content: &str) -> Result<String, WorkoutSummaryError> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err(WorkoutSummaryError::Validation(
            "message content must not be empty".to_string(),
        ));
    }
    if trimmed.chars().count() > 2000 {
        return Err(WorkoutSummaryError::Validation(
            "message must be 2000 characters or fewer".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}
